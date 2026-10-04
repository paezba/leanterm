use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Local;
use fuzzy_match::FuzzyMatchResult;
use repo_metadata::RepoMetadataModel;
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::watcher::DirectoryWatcher;
use session_sharing_protocol::common::Role;
use smol_str::SmolStr;
use unindent::Unindent;
use warp_completer::completer::{
    Match, MatchStrategy, MatchedSuggestion, Priority, Suggestion, SuggestionResults,
    SuggestionType,
};
use warp_completer::meta::Span;
use warpui::platform::WindowStyle;
use warpui::{App, UpdateView, WindowId};
use watcher::HomeDirectoryWatcher;
use workflows::workflow::{Argument, ArgumentType, Workflow};

use super::*;
use crate::auth::AuthStateProvider;
use crate::auth::auth_manager::AuthManager;
use crate::changelog_model::ChangelogModel;
use crate::cloud_object::model::persistence::CloudModel;
use crate::context_chips::prompt::Prompt;
use crate::editor::{DisplayPoint, EditorAction, TextStyleOperation};
use crate::input_suggestions::Item;
use crate::network::NetworkStatus;
use crate::persisted_workspace::PersistedWorkspace;
use crate::pricing::PricingInfoModel;
use crate::search::files::model::FileSearchModel;
use crate::server::cloud_objects::listener::Listener;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::server_api::ServerApiProvider;
use crate::server::sync_queue::SyncQueue;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings::import::model::ImportedConfigModel;
use crate::settings::{AliasExpansionSettings, AppEditorSettings, PrivacySettings};
use crate::settings_view::keybindings::KeybindingChangedNotifier;
#[cfg(windows)]
use crate::system::SystemInfo;
use crate::system::SystemStats;
use crate::terminal::TerminalView;
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::terminal::block_list_viewport::ScrollPosition;
use crate::terminal::event::{BlockMetadataReceivedEvent, BootstrappedEvent};
use crate::terminal::general_settings::UserDefaultShellUnsupportedBannerState;
use crate::terminal::keys::TerminalKeybindings;
use crate::terminal::local_shell::LocalShellState;
use crate::terminal::local_tty::shell::ShellStarter;
use crate::terminal::model::ansi::{Handler, PromptMetadata};
use crate::terminal::model::block::{BlockId, SerializedBlock};
use crate::terminal::model::session::command_executor::{CommandExecutor, ExecuteCommandOptions};
use crate::terminal::model::session::{BootstrapSessionType, SessionInfo};
use crate::terminal::model_events::ModelEvent;
use crate::terminal::resizable_data::ResizableData;
use crate::terminal::shared_session::permissions_manager::SessionPermissionsManager;
use crate::terminal::shell::{Shell, ShellType};
use crate::terminal::view::Event as TerminalViewEvent;
use crate::test_util::assert_eventually;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::themes::theme::AnsiColorIdentifier;
use crate::warp_managed_paths_watcher::WarpManagedPathsWatcher;
use crate::workspace::{ActiveSession, OneTimeModalModel, ToastStack, WorkspaceRegistry};
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::update_manager::TeamUpdateManager;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::{
    GlobalResourceHandles, GlobalResourceHandlesProvider, ReferralThemeStatus, experiments,
};

fn pending_ctrl_r_handoff() -> PendingShellWidgetHandoff {
    PendingShellWidgetHandoff {
        session_id: SessionId::from(1),
        original_buffer: "draft".to_string(),
        selection: None,
        block_id: BlockId::new(),
        apply_mode: ShellWidgetApplyMode::Replace,
        cursor_offset: None,
    }
}

fn pending_ctrl_t_handoff() -> PendingShellWidgetHandoff {
    PendingShellWidgetHandoff {
        session_id: SessionId::from(1),
        original_buffer: "echo ".to_string(),
        selection: None,
        block_id: BlockId::new(),
        apply_mode: ShellWidgetApplyMode::Splice,
        cursor_offset: Some(ByteOffset::from(5)),
    }
}

#[test]
fn matching_shell_widget_handoff_selection_is_applied() {
    let mut handoff = pending_ctrl_r_handoff();
    handoff.maybe_apply_selection(SessionId::from(1), "echo selected");
    assert_eq!(handoff.restore_text(), "echo selected");

    let mut handoff = pending_ctrl_t_handoff();
    handoff.maybe_apply_selection(SessionId::from(1), "selected/file.txt");
    assert_eq!(handoff.selection, Some("selected/file.txt".to_string()));
}

#[test]
fn unsolicited_or_stale_shell_widget_handoff_selection_is_ignored() {
    let mut handoff = pending_ctrl_r_handoff();
    handoff.maybe_apply_selection(SessionId::from(2), "echo selected");
    assert_eq!(handoff.restore_text(), "draft");
}

#[test]
fn empty_shell_widget_handoff_selection_keeps_original_buffer() {
    let mut handoff = pending_ctrl_r_handoff();
    handoff.maybe_apply_selection(SessionId::from(1), "");
    assert_eq!(handoff.restore_text(), "draft");

    let mut handoff = pending_ctrl_t_handoff();
    handoff.maybe_apply_selection(SessionId::from(1), "");
    assert_eq!(handoff.selection, None);
}

#[test]
fn renders_git_checkout_prompt_chip_command_as_single_shell_argument() {
    let command = PromptChipShellCommand::GitCheckout {
        branch_name: "poc;id>/tmp/proof $(whoami) `id` | cat 'tail'".to_string(),
    };

    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Bash),
        r#"git checkout 'poc;id>/tmp/proof $(whoami) `id` | cat '"'"'tail'"'"''"#
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Zsh),
        r#"git checkout 'poc;id>/tmp/proof $(whoami) `id` | cat '"'"'tail'"'"''"#
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Fish),
        r"git checkout 'poc;id>/tmp/proof $(whoami) `id` | cat \'tail\''"
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::PowerShell),
        "git checkout 'poc;id>/tmp/proof $(whoami) `id` | cat ''tail'''"
    );
}

#[test]
fn renders_nvm_use_prompt_chip_command_as_single_shell_argument() {
    let command = PromptChipShellCommand::NvmUse {
        version: "v20.0.0;touch /tmp/pwn 'x'".to_string(),
    };

    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Bash),
        r#"nvm use 'v20.0.0;touch /tmp/pwn '"'"'x'"'"''"#
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Fish),
        r"nvm use 'v20.0.0;touch /tmp/pwn \'x\''"
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::PowerShell),
        "nvm use 'v20.0.0;touch /tmp/pwn ''x'''"
    );
}

#[test]
fn renders_change_directory_prompt_chip_command_as_single_shell_argument() {
    let command = PromptChipShellCommand::ChangeDirectory {
        dir_name: "repo dir;rm -rf / 'x'".to_string(),
    };

    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Bash),
        r#"cd 'repo dir;rm -rf / '"'"'x'"'"''"#
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::PowerShell),
        "cd 'repo dir;rm -rf / ''x'''"
    );
}

#[test]
fn renders_echo_prompt_chip_command_as_single_shell_argument() {
    let command = PromptChipShellCommand::Echo {
        message: "a message containing \"double\" and 'single' quotes",
    };

    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::Bash),
        r#"echo 'a message containing "double" and '"'"'single'"'"' quotes'"#
    );
    assert_eq!(
        render_prompt_chip_shell_command(&command, ShellType::PowerShell),
        r#"echo 'a message containing "double" and ''single'' quotes'"#
    );
}

#[test]
fn renders_fixed_prompt_chip_command_without_interpolation() {
    assert_eq!(
        render_prompt_chip_shell_command(
            &PromptChipShellCommand::NvmInstallLatestNode,
            ShellType::Bash,
        ),
        "nvm install node"
    );
}

pub fn initialize_app(app: &mut App) {
    initialize_settings_for_tests(app);

    // NLD is now opt-in by default (`ai_autodetection_enabled_internal` defaults to false).
    // These tests exercise the natural-language-detection-on code paths (buffer-driven slash
    // command detection, auto-detection input mode), so explicitly re-enable it here to preserve
    // the pre-opt-in test behavior. The opt-in default itself is covered by
    // `ai_autodetection_defaults_to_opt_in` in `settings/ai_tests.rs`.

    // Make sure we set up all necessary custom action bindings.
    app.update(init);

    // Initialize any global models required by the Input view.
    app.add_singleton_model(|_| ServerApiProvider::new_for_test());
    app.add_singleton_model(|ctx| ChangelogModel::new(ServerApiProvider::as_ref(ctx).get()));
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(|_| Prompt::mock());
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(ImportedConfigModel::new);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(TeamUpdateManager::mock);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(Listener::mock);
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_ctx| SyncedInputState::mock());
    app.add_singleton_model(|_| ResizableData::default());
    app.add_singleton_model(|_| History::default());
    app.add_singleton_model(LocalWorkflows::new);
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(TerminalKeybindings::new);
    app.add_singleton_model(|_| ActiveSession::default());
    // QueuedQueryModel subscribes to history events; register after the
    // history model is in place.
    // Pill bar model subscribes to history events; register after the
    // history model is in place.
    // The blocklist controller created during terminal bootstrap subscribes to
    // OrchestrationEventService and OrchestrationEventStreamer unconditionally,
    // so both singletons must be registered before bootstrap.
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(SessionPermissionsManager::new);
    app.add_singleton_model(DirectoryWatcher::new);
    app.add_singleton_model(|_| DetectedRepositories::default());
    app.add_singleton_model(crate::remote_server::manager::RemoteServerManager::new);
    app.add_singleton_model(|_| crate::code_review::git_repo_model::GitRepoModels::new());
    app.add_singleton_model(RepoMetadataModel::new);
    app.add_singleton_model(FileSearchModel::new);
    app.add_singleton_model(|_| IgnoredSuggestionsModel::new(vec![]));
    app.add_singleton_model(HomeDirectoryWatcher::new_for_test);
    app.add_singleton_model(WarpManagedPathsWatcher::new_for_testing);

    // Add GlobalResourceHandlesProvider for persistence
    let tips_handle = app.add_model(|_| TipsCompleted::default());
    let referral_theme_status = app.add_model(ReferralThemeStatus::new);
    let user_default_shell_unsupported_banner_model_handle =
        app.add_model(|_| UserDefaultShellUnsupportedBannerState::default_value());
    app.add_singleton_model(move |_ctx| {
        GlobalResourceHandlesProvider::new(GlobalResourceHandles {
            model_event_sender: None, // No persistence in tests
            tips_completed: tips_handle,
            referral_theme_status,
            user_default_shell_unsupported_banner_model_handle,
            settings_file_error: None,
        })
    });

    #[cfg(windows)]
    {
        app.add_singleton_model(SystemInfo::new);
    }

    app.update(experiments::init);
    AltScreenReporting::register(app);
    app.add_singleton_model(OneTimeModalModel::new);
    app.add_singleton_model(|_| WorkspaceRegistry::new());
    app.add_singleton_model(|_| ToastStack);
    app.add_singleton_model(|_| PricingInfoModel::new());
    app.add_singleton_model(PersistedWorkspace::new_for_test);
    // `LocalShellState` captures the user's interactive login-shell PATH (used
    // for MCP/sbx executable resolution). Tests don't exercise that capture, so
    // register the singleton in its `NotLoaded` state to satisfy callers that
    // look it up via `LocalShellState::handle(ctx)`.
    app.add_singleton_model(|_| LocalShellState::NotLoaded);
}

fn bootstrap_terminal(
    terminal: &ViewHandle<TerminalView>,
    bootstrapped_event: BootstrappedEvent,
    app: &mut App,
) {
    let session_id = bootstrapped_event.session_info.session_id;
    terminal.update(app, |terminal, ctx| {
        terminal.model.lock().block_list_mut().set_bootstrapped();

        // Set session_id since precmd is not called in unit tests.
        terminal
            .model
            .lock()
            .block_list_mut()
            .active_block_for_test()
            .set_session_id(session_id);
        let model_event_dispatcher = terminal.model_event_dispatcher().clone();
        model_event_dispatcher.update(ctx, |dispatcher, _| {
            dispatcher.set_active_session_id(session_id);
        });

        terminal.sessions_model().update(ctx, |sessions, ctx| {
            let BootstrappedEvent {
                session_info,
                restored_block_commands,
                rcfiles_duration_seconds,
                spawning_command,
            } = bootstrapped_event;
            sessions.initialize_bootstrapped_session(
                *session_info,
                spawning_command,
                restored_block_commands,
                rcfiles_duration_seconds,
                ctx,
            );
        });
    });
}

pub async fn add_window_with_bootstrapped_terminal(
    app: &mut App,
    history_file_commands: Option<Vec<String>>,
    session_info: Option<SessionInfo>,
) -> ViewHandle<TerminalView> {
    add_window_with_bootstrapped_terminal_and_window_id(app, history_file_commands, session_info)
        .await
        .1
}

pub async fn add_window_with_bootstrapped_terminal_and_window_id(
    app: &mut App,
    history_file_commands: Option<Vec<String>>,
    session_info: Option<SessionInfo>,
) -> (WindowId, ViewHandle<TerminalView>) {
    let tips_model = app.add_model(|_| TipsCompleted::default());

    let shell_starter_source =
        ShellStarter::init(crate::terminal::available_shells::AvailableShell::default())
            .expect("Could not create a shell starter source or wsl name")
            .to_shell_starter_source()
            .await
            .expect("Could not create a shell starter source");
    let shell_type = shell_starter_source.shell_type();

    let session_info = session_info
        .unwrap_or_else(SessionInfo::new_for_test)
        .with_session_type(BootstrapSessionType::Local)
        .with_shell_type(shell_type);
    let history_file_commands = history_file_commands.unwrap_or_default();

    let (window_id, terminal) = app.add_window(WindowStyle::NotStealFocus, move |ctx| {
        TerminalView::new_for_test(tips_model, None, ctx)
    });

    // TODO(vorporeal): There's a lot of fuckiness here.  `TerminalView::new_for_test`
    // calls `TerminalModel::new_for_test`, which fakes the InitShell and Bootstrapped
    // lifecycle events.  We then _also_ bootstrap the terminal here, which can and does
    // lead to inconsistent states.  We ought to only bootstrap the terminal once.
    let session_id = session_info.session_id;
    let bootstrapped_event = BootstrappedEvent {
        session_info: Box::new(session_info),
        restored_block_commands: history_file_commands
            .into_iter()
            .map(|command| HistoryEntry::command_at_time(command, Local::now(), None, true))
            .collect_vec(),
        rcfiles_duration_seconds: None,
        spawning_command: "test command".to_string(),
    };
    bootstrap_terminal(&terminal, bootstrapped_event, app);

    // Wait until history has been initialized for the session.
    let mut history_handle = History::handle(app);
    History::initialized_sessions(&mut history_handle, app, vec![session_id]).await;

    let input = terminal.read(app, |terminal, _| terminal.input().clone());
    // Notify the input that the session has bootstrapped
    input.update(app, |input, ctx| {
        input.set_active_block_metadata(BlockMetadata::new(Some(session_id), None), false, ctx);
    });
    (window_id, terminal)
}

/// Simulates being in a particular directory, for the purposes of completion
/// and syntax highlighting. The current directory is used to resolve
/// paths when parsing commands, and without it, completion/highlighting will
/// not run.
///
/// In particular, this sends precmd data and sets the active block's metadata.
pub fn simulate_directory_for_completion<A, S>(
    session_id: SessionId,
    terminal: &ViewHandle<TerminalView>,
    app: &mut A,
    directory: S,
) where
    A: UpdateView,
    S: Into<String>,
{
    let directory = directory.into();
    terminal.update(app, |terminal, ctx| {
        let block_metadata = BlockMetadata::new(Some(session_id), Some(directory.clone()));
        let block_index = {
            let mut model = terminal.model.lock();
            model.block_list_mut().prompt_only_precmd(PromptMetadata {
                pwd: Some(directory.clone()),
                session_id: Some(session_id.into()),
                ..Default::default()
            });
            model.block_list().active_block_index()
        };

        // Normally, the precmd message should be sufficient to also set this block metadata.
        // However, in unit tests the foreground executor does not relay the event, so notify
        // the dispatcher directly for models that observe active-session metadata.
        terminal
            .model_event_dispatcher()
            .update(ctx, |dispatcher, ctx| {
                dispatcher.set_active_session_id(session_id);
                ctx.emit(ModelEvent::BlockMetadataReceived(
                    BlockMetadataReceivedEvent {
                        block_metadata: block_metadata.clone(),
                        block_index,
                        is_after_in_band_command: false,
                        is_done_bootstrapping: true,
                    },
                ));
            });

        // Keep the input's block metadata in sync with the active-session metadata above.
        terminal.input().update(ctx, |input, ctx| {
            input.set_active_block_metadata(block_metadata, false, ctx);
        });
    });
}

fn argument_suggestion(name: impl Into<SmolStr>) -> MatchedSuggestion {
    let suggestion = Suggestion::with_same_display_and_replacement(
        name,
        None,
        SuggestionType::Argument,
        Priority::default(),
    );
    MatchedSuggestion::new(
        suggestion,
        Match::Prefix {
            is_case_sensitive: true,
        },
    )
}

fn case_insensitive_argument_suggestion(name: impl Into<SmolStr>) -> MatchedSuggestion {
    let suggestion = Suggestion::with_same_display_and_replacement(
        name,
        None,
        SuggestionType::Argument,
        Priority::default(),
    );
    MatchedSuggestion::new(
        suggestion,
        Match::Prefix {
            is_case_sensitive: false,
        },
    )
}

fn case_insensitive_exact_argument_suggestion(name: impl Into<SmolStr>) -> MatchedSuggestion {
    let suggestion = Suggestion::with_same_display_and_replacement(
        name,
        None,
        SuggestionType::Argument,
        Priority::default(),
    );
    MatchedSuggestion::new(
        suggestion,
        Match::Exact {
            is_case_sensitive: false,
        },
    )
}

fn fuzzy_argument_suggestion(
    name: impl Into<SmolStr>,
    matched_indices: Vec<usize>,
) -> MatchedSuggestion {
    let suggestion = Suggestion::with_same_display_and_replacement(
        name,
        None,
        SuggestionType::Argument,
        Priority::default(),
    );
    MatchedSuggestion::new(
        suggestion,
        Match::Fuzzy {
            match_result: FuzzyMatchResult {
                score: 1,
                matched_indices,
            },
        },
    )
}

fn editor_model_snapshot(input: &Input, ctx: &mut ViewContext<Input>) -> EditorSnapshot {
    input
        .editor()
        .read(ctx, |editor, ctx| editor.snapshot_model(ctx))
}

fn set_alias_expansion_setting(new_value: bool, app: &mut App) {
    AliasExpansionSettings::handle(app).update(app, |settings, ctx| {
        if let Err(e) = settings.alias_expansion_enabled.set_value(new_value, ctx) {
            panic!("Unable to set alias expansion setting in test, {e:?}");
        }
    });
}

#[test]
fn test_input_tab() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        // Note: we have similar boilerplate for many tests in this file - it would be nice to refactor this into a common helper!
        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let editor = input.read(&app, |input, _| input.editor().clone());
        // If there is no non-whitespace input, pass the tab to the editor
        input.read(&app, |input, ctx| {
            assert!(input.buffer_text(ctx).is_empty());
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "    ");
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "        ");
        });
        input.update(&mut app, |input, ctx| {
            input.input_shift_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "    ");
        });

        // Test that if there is a single cursor at the end, we do not pass tab to the editor.
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("c", ctx);
            input.user_insert("d", ctx);
            input.user_insert(" ", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd ");
        });

        // Test that we don't pass the tab if the single cursor is in the middle either
        input.update(&mut app, |input, ctx| {
            input.user_insert("s", ctx);
            input.user_insert("o", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_left(/* stop at line start */ false, ctx);
            editor.move_left(/* stop at line start */ false, ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd so");
        });

        // Test that if we select the entire buffer, we pass tab to the editor.
        input.update(&mut app, |input, ctx| {
            input.editor.update(ctx, |editor, ctx| {
                editor.select_all(ctx);
            })
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "    cd so");
        });
    });
}

#[test]
fn test_history_up_for_shared_session_executor() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Initialize as shared session executor
        // such that the history model isn't also initialized during bootstrapping
        // TODO(maggs): Improve testing utils for session sharing
        let tips_model = app.add_model(|_| TipsCompleted::default());
        let (_, terminal) = app.add_window(WindowStyle::NotStealFocus, move |ctx| {
            TerminalView::new_for_test(tips_model, None, ctx)
        });
        terminal.update(&mut app, |view, _| {
            let mut model = view.model.lock();
            model.block_list_mut().set_bootstrapped();
            model
                .block_list_mut()
                .active_block_for_test()
                .set_session_id(SessionId::from(0));
            model.set_shared_session_status(SharedSessionStatus::ActiveViewer {
                role: Role::Executor,
            });
        });

        let (input, suggestions) = terminal.read(&app, |view, _ctx| {
            let input = view.input().clone();
            let input_suggestions = input.read(&app, |input, _ctx| input.input_suggestions.clone());
            (input, input_suggestions)
        });

        input.update(&mut app, |input, ctx| {
            // Initialize shared session history model
            let shared_session_history_model = ctx.add_model(|_| SharedSessionHistoryModel::new());

            // Simulate blocks
            shared_session_history_model.update(ctx, |history_model, _ctx| {
                history_model.push(HistoryEntry::for_completed_block(
                    "echo foo".into(),
                    &SerializedBlock::new_for_test("echo foo".as_bytes().to_vec(), vec![]),
                ));

                history_model.push(HistoryEntry::for_completed_block(
                    "cd ~".into(),
                    &SerializedBlock::new_for_test("cd ~".as_bytes().to_vec(), vec![]),
                ));
            });

            input.shared_session_input_state = Some(SharedSessionInputState {
                history_model: shared_session_history_model,
                pending_command_execution_request: None,
            });
            input.editor_up(ctx);
        });

        // Arrow up displays history in the correct order for an empty buffer
        suggestions.read(&app, |suggestions, _ctx| {
            assert_eq!(suggestions.items().len(), 2);
            assert_eq!(suggestions.item_text(0).as_str(), "echo foo");
            assert_eq!(suggestions.item_text(1).as_str(), "cd ~");
        });

        // The buffer should contain the text of the last item
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd ~");
        });

        // Shared session executor should be able to navigate through history
        input.update(&mut app, |input, ctx| {
            input.editor_up(ctx);
        });

        // The buffer should contain the text of the second last item after another arrow-up
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "echo foo");
        });
    });
}

fn build_suggestion_results<S: Into<Span>>(
    suggestions: Vec<MatchedSuggestion>,
    replacement_span: S,
    matcher: MatchStrategy,
) -> Option<SuggestionResults> {
    Some(SuggestionResults {
        replacement_span: replacement_span.into(),
        suggestions,
        match_strategy: matcher,
    })
}

#[derive(Debug)]
struct CancellationTrackingExecutor(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl CommandExecutor for CancellationTrackingExecutor {
    async fn execute_command(
        &self,
        _command: &str,
        _shell: &Shell,
        _current_directory_path: Option<&str>,
        _environment_variables: Option<HashMap<String, String>>,
        _execute_command_options: ExecuteCommandOptions,
    ) -> anyhow::Result<warp_completer::completer::CommandOutput> {
        anyhow::bail!("no executor command expected")
    }

    fn cancel_active_commands(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn supports_parallel_command_execution(&self) -> bool {
        false
    }
}

fn respond_to_native_shell_completions(
    app: &mut App,
    terminal: &ViewHandle<TerminalView>,
    completions: Vec<ShellCompletion>,
) {
    app.update(|ctx| {
        ctx.subscribe_to_view(terminal, move |_, event: &TerminalViewEvent, _| {
            if let TerminalViewEvent::RunNativeShellCompletions { results_tx, .. } = event {
                results_tx
                    .try_send((completions.clone(), None))
                    .expect("native completion response receiver must remain open");
            }
        });
    });
}

#[test]
fn combined_completions_show_file_paths_after_empty_native_results() {
    let _native_completions_flag = FeatureFlag::NativeShellCompletions.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .warp_completions_enabled
                    .set_value(true, ctx)
                    .expect("Warp completions setting must update");
                settings
                    .native_shell_completions_enabled
                    .set_value(true, ctx)
                    .expect("native completions setting must update");
            });
        });

        let working_directory = tempfile::TempDir::new().expect("completion working directory");
        let source_directory = working_directory.path().join("src");
        std::fs::create_dir(&source_directory).expect("source directory must be created");
        std::fs::write(source_directory.join("alpha.rs"), "")
            .expect("alpha fixture must be created");
        std::fs::write(source_directory.join("beta.rs"), "").expect("beta fixture must be created");

        let session_info = SessionInfo::new_for_test();
        let session_id = session_info.session_id;
        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, None, Some(session_info)).await;
        simulate_directory_for_completion(
            session_id,
            &terminal,
            &mut app,
            working_directory.path().to_string_lossy(),
        );
        respond_to_native_shell_completions(&mut app, &terminal, Vec::new());
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("warptool ./src/", ctx);
            input.input_tab(ctx);
        });

        assert_eventually!(
            600 => input.read(&app, |input, _| {
                input.input_suggestions.read(&app, |suggestions, _| {
                    let items = suggestions.items().iter().map(|item| item.text()).collect_vec();
                    items.iter().any(|item| item.ends_with("alpha.rs"))
                        && items.iter().any(|item| item.ends_with("beta.rs"))
                })
            }),
            "gave up waiting for file paths after empty bundled and native completions"
        );
    });
}

#[test]
fn combined_completions_preserve_nonempty_native_results() {
    let _native_completions_flag = FeatureFlag::NativeShellCompletions.override_enabled(true);

    App::test((), |mut app| async move {
        initialize_app(&mut app);
        app.update(|ctx| {
            InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .warp_completions_enabled
                    .set_value(true, ctx)
                    .expect("Warp completions setting must update");
                settings
                    .native_shell_completions_enabled
                    .set_value(true, ctx)
                    .expect("native completions setting must update");
            });
        });

        let working_directory = tempfile::TempDir::new().expect("completion working directory");
        std::fs::write(working_directory.path().join("native-file"), "")
            .expect("file fallback fixture must be created");

        let session_info = SessionInfo::new_for_test();
        let session_id = session_info.session_id;
        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, None, Some(session_info)).await;
        simulate_directory_for_completion(
            session_id,
            &terminal,
            &mut app,
            working_directory.path().to_string_lossy(),
        );
        respond_to_native_shell_completions(
            &mut app,
            &terminal,
            vec![
                ShellCompletion::new("native-shell-alpha".to_string()),
                ShellCompletion::new("native-shell-beta".to_string()),
            ],
        );
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("warptool n", ctx);
            input.input_tab(ctx);
        });

        assert_eventually!(
            600 => input.read(&app, |input, _| {
                input.input_suggestions.read(&app, |suggestions, _| {
                    let items = suggestions.items().iter().map(|item| item.text()).collect_vec();
                    items.contains(&"native-shell-alpha")
                        && items.contains(&"native-shell-beta")
                        && !items.contains(&"native-file")
                })
            }),
            "gave up waiting for nonempty native suggestions"
        );
    });
}

#[test]
fn native_shell_replacement_span_is_clamped_into_the_buffer_before_the_cursor() {
    let buffer_text = "cd app/D";
    let cursor_position = buffer_text.len();

    let honored = native_shell_suggestion_results(
        Vec::new(),
        Some(Span::new(3, 8)),
        buffer_text,
        cursor_position,
    );
    assert_eq!(
        honored.replacement_span,
        Span::new(3, 8),
        "a span a shell can really report must survive untouched"
    );

    let out_of_range = native_shell_suggestion_results(
        Vec::new(),
        Some(Span::new(1_000_000, 1_000_001)),
        buffer_text,
        cursor_position,
    );
    assert_eq!(out_of_range.replacement_span, Span::new(8, 8));

    let saturated = native_shell_suggestion_results(
        Vec::new(),
        Some(Span::new(usize::MAX, usize::MAX)),
        buffer_text,
        cursor_position,
    );
    assert_eq!(saturated.replacement_span, Span::new(8, 8));
}

#[test]
fn native_shell_replacement_span_is_clamped_to_the_cursor_not_the_whole_buffer() {
    let buffer_text = "cd app/Documents";
    let cursor_position = "cd app/D".len();

    let results = native_shell_suggestion_results(
        Vec::new(),
        Some(Span::new(12, 16)),
        buffer_text,
        cursor_position,
    );

    assert_eq!(results.replacement_span, Span::new(8, 8));
}

#[test]
fn native_shell_replacement_span_falls_back_to_the_whitespace_token_when_none_is_reported() {
    let buffer_text = "cd app/D";

    let results = native_shell_suggestion_results(Vec::new(), None, buffer_text, buffer_text.len());

    assert_eq!(results.replacement_span, Span::new(3, 8));
}

#[test]
fn test_tab_completion_with_multibyte_chars() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |view, _| view.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("➤", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "➤");
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "➤");
        });
    });
}

#[test]
fn test_open_slash_command_requires_path() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text("/open-file ", ctx)
            });
        });

        input.update(&mut app, |input, ctx| {
            input.input_enter(ctx);
        });
    });
}

#[test]
fn test_open_slash_command_opens_files_palette_when_entered_from_slash_menu() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.user_insert("/", ctx);
            input.user_insert("open-file", ctx);
        });

        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "/open-file");
        });

        input.update(&mut app, |input, ctx| {
            input.input_enter(ctx);
        });
    });
}

#[test]
fn test_tab_completion_single_prefix_suggestion_with_fuzzy_suggestions() {
    // If there is a single prefix suggestion with other fuzzy suggestions,
    // we should insert that prefix suggestion directly into the buffer
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("cha.txt"),
                        fuzzy_argument_suggestion("bobcha.txt", (3..=5).collect()),
                    ],
                    (5, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha.txt ");
        });
    });
}

#[test]
fn test_tab_completion_only_fuzzy_suggestions() {
    // If there are only fuzzy suggestions, we don't insert a prefix even if there is a common prefix
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        fuzzy_argument_suggestion("bobcha1.txt", (3..=5).collect()),
                        fuzzy_argument_suggestion("bobcha2.txt", (3..=5).collect()),
                    ],
                    (5, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha");
        });
    });
}

#[test]
fn test_tab_completion_prioritizes_longest_common_prefix_with_fuzzy_suggestions() {
    // If there are multiple prefix suggestions with any number of fuzzy suggestions,
    // the common prefix is inserted.
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let suggestions = input.read(&app, |input, _ctx| input.input_suggestions.clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("charlie1.txt"),
                        argument_suggestion("charlie2.txt"),
                        fuzzy_argument_suggestion("bobcha1.pdf", (3..=5).collect()),
                        fuzzy_argument_suggestion("bobcha11.pdf", (3..=5).collect()),
                    ],
                    (5, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open charlie");
        });

        // We also just check that we don't insert the common prefix when typing
        // to filter if there isn't a common prefix or the replacement
        // does not start the common prefix.
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha1", ctx);
        });
        suggestions.update(&mut app, |suggestions, _| {
            suggestions.set_items(vec![
                Item::from_text("charlie1.txt".to_string()),
                Item::from_text("bobcha1.pdf".to_string()),
                Item::from_text("bobcha11.pdf".to_string()),
            ]);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1");
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("p", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1p");
        });
        suggestions.update(&mut app, |suggestions, _| {
            suggestions.set_items(vec![
                Item::from_text("bobcha1.pdf".to_string()),
                Item::from_text("bobcha11.pdf".to_string()),
            ]);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1p");
        });
    });
}

#[test]
fn test_tab_completion_single_prefix_suggestion_after_fuzzy_suggestions() {
    // If there is a single prefix suggestion ordered after other fuzzy suggestions, we
    // insert that prefix suggestion directly into the buffer.
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("git a", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        fuzzy_argument_suggestion("dab", vec![4]),
                        argument_suggestion("add"),
                    ],
                    (4, 5),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git add ");
        });
    });
}

#[test]
fn test_tab_completion_case_sensitive_single_suggestion() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open ab", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("abc.txt"),
                        case_insensitive_argument_suggestion("Abcd.txt"),
                    ],
                    (5, 6),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            // There is only 1 case-sensitive prefix suggestion, so we insert it
            assert_eq!(input.buffer_text(ctx), "open abc.txt ");
        });

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open ab", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        case_insensitive_argument_suggestion("Abc.txt"),
                        fuzzy_argument_suggestion("bobabc.txt", (3..=4).collect()),
                    ],
                    (5, 6),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            // There are no case-sensitive prefixes, but 1 case-insensitive prefix,
            // suggestion, so we insert it.
            assert_eq!(input.buffer_text(ctx), "open Abc.txt ");
        });
    });
}

#[test]
fn test_tab_completion_case_sensitivity_common_prefix() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open ab", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("abcdef.txt"),
                        argument_suggestion("abcdag.txt"),
                        case_insensitive_argument_suggestion("Abcd.txt"),
                    ],
                    (5, 6),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            // Insert the common prefix for the case-sensitive suggestions.
            assert_eq!(input.buffer_text(ctx), "open abcd");
        });
    });
}

#[test]
fn test_tab_completion_case_insensitive_exact_match() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("abc", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("abcdef"),
                        case_insensitive_exact_argument_suggestion("Abc"),
                    ],
                    (0, 3),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            // Single case-sensitive prefix suggestions are inserted even if there's
            // a case-insensitive exact match.
            assert_eq!(input.buffer_text(ctx), "abcdef ");
        });

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("abc", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("abcdef"),
                        argument_suggestion("abcdeg"),
                        case_insensitive_exact_argument_suggestion("Abc"),
                    ],
                    (0, 3),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            // Case-sensitive common prefixes are inserted even if there's a
            // case-insensitive exact match.
            assert_eq!(input.buffer_text(ctx), "abcde");
        });
    });
}

#[test]
fn test_tab_completion_longest_common_prefix_with_fuzzy_suggestions() {
    // We want to test the following behaviour:
    // 1. If there is a single prefix suggestion with other fuzzy suggestions,
    //    we should insert that prefix suggestion directly into the buffer
    // 2. If there are only fuzzy suggestions, we don't insert a prefix even if there is a common prefix
    // 3. If there is a single prefix suggestion ordered after other fuzzy suggestions, we
    //     insert that prefix suggestion directly into the buffer.
    // We also check that this behaviour works when typing to filter.
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let suggestions = input.read(&app, |input, _ctx| input.input_suggestions.clone());

        // Case 1. If there is a single prefix suggestion with other fuzzy suggestions, we should insert that prefix suggestion
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("cha.txt"),
                        fuzzy_argument_suggestion("bobcha.txt", (3..=5).collect()),
                    ],
                    (5, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha.txt ");
        });

        // Case 2. If there are only fuzzy suggestions, we don't insert a prefix even if there is a common prefix
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("open cha", ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        fuzzy_argument_suggestion("bobcha1.txt", (3..=5).collect()),
                        fuzzy_argument_suggestion("bobcha2.txt", (3..=5).collect()),
                    ],
                    (5, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha");
        });

        // We also just check that we don't insert the common prefix when typing
        // to filter if there isn't a common prefix or the replacement
        // does not start the common prefix.
        input.update(&mut app, |input, ctx| {
            input.user_insert("1", ctx);
        });
        suggestions.update(&mut app, |suggestions, _| {
            suggestions.set_items(vec![
                Item::from_text("charlie1.txt".to_string()),
                Item::from_text("bobcha1.pdf".to_string()),
                Item::from_text("bobcha11.pdf".to_string()),
            ]);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1");
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("p", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1p");
        });
        suggestions.update(&mut app, |suggestions, _| {
            suggestions.set_items(vec![
                Item::from_text("bobcha1.pdf".to_string()),
                Item::from_text("bobcha11.pdf".to_string()),
            ]);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "open cha1p");
        });

        // Case 3: Ensure that the prefix suggestion is inserted, even if it's not the first
        // ordered suggestion.
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("git a", ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        fuzzy_argument_suggestion("dab", vec![4]),
                        argument_suggestion("add"),
                    ],
                    (4, 5),
                    MatchStrategy::Fuzzy,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            )
        });

        input.update(&mut app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git add ");
        });
    });
}

#[test]
fn test_tab_completion_common_prefix_shorter() {
    // We need to check the same two cases as the 'longest_common_prefix' test, however we want
    // to verify that if the longest common prefix is _shorter_ than what the user typed, we
    // don't insert it
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let suggestions = input.read(&app, |input, _| input.input_suggestions.clone());

        // Case 1: When a user triggers a tab completion, ensure longest common prefix is
        // longer than the text
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("cd foo/b", ctx);
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("foo/Bar"),
                        argument_suggestion("foo/bazz"),
                    ],
                    (3, 8),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd foo/b");
        });

        // Case 2: When user types to filter the completion results and then triggers tab
        // completion again, we still want to ensure the longest common prefix is longer
        // than the text
        input.update(&mut app, |input, ctx| {
            input.close_input_suggestions(/*should_focus_input=*/ true, ctx);
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("cd f", ctx);
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("far"),
                        argument_suggestion("foo/Bar"),
                        argument_suggestion("foo/bazz"),
                    ],
                    (3, 4),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
            input.user_insert("oo/b", ctx);
        });
        suggestions.update(&mut app, |suggestions, _| {
            suggestions.set_items(vec![
                Item::from_text("foo/Bar".into()),
                Item::from_text("foo/bazz".into()),
            ]);
        });
        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd foo/b");
        });
    });
}

#[test]
fn test_cursor_movement() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let history_file_commands = vec![
            "cd Documents/zed".to_string(),
            "curl https://app.warp.dev".to_string(),
            "cargo check\ncargo run".to_string(),
        ];
        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, Some(history_file_commands), None)
                .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let editor = input.read(&app, |input, _| input.editor.clone());
        // Test cursor movement
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("c", ctx);
            input.user_insert("d", ctx);
            input.user_insert(" ", ctx);
            input.user_insert("D", ctx);
        });

        // XXX Note that it's necessary to put `input_tab` in a separate call.
        // Otherwise, there's a race where we crash because editor:cursor is not set.
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd D");
        });

        input.update(&mut app, |input, ctx| {
            input.input_tab(ctx);
            input.handle_completion_suggestions_results(
                build_suggestion_results(
                    vec![
                        argument_suggestion("Downloads"),
                        argument_suggestion("Documents"),
                    ],
                    (3, 4),
                    MatchStrategy::CaseInsensitive,
                ),
                CompletionsTrigger::Keybinding,
                editor_model_snapshot(input, ctx),
                ctx,
            );
        });
        let expected_completion = InputSuggestionsMode::CompletionSuggestions {
            replacement_start: 3,
            buffer_text_original: "cd D".to_string(),
            completion_results: SuggestionResults {
                suggestions: vec![
                    argument_suggestion("Downloads"),
                    argument_suggestion("Documents"),
                ],
                replacement_span: Span::new(3, 4),
                match_strategy: MatchStrategy::CaseInsensitive,
            },
            trigger: CompletionsTrigger::Keybinding,
            menu_position: TabCompletionsMenuPosition::AtLastCursor,
        };
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd Do");
            assert_eq!(
                *input.suggestions_mode_model().as_ref(ctx).mode(),
                expected_completion
            );
        });
        // move back 1 character, and we're still showing the completion, except ignoring the
        // characters _after_ the cursor
        editor.update(&mut app, |editor, ctx| {
            editor.move_left(/* stop at line start */ false, ctx)
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "cd Do");
            assert_eq!(
                *input.suggestions_mode_model().as_ref(ctx).mode(),
                expected_completion
            );
        });
        editor.read(&app, |editor, ctx| {
            assert!(editor.is_single_cursor_only(ctx));
            let column = editor.start_byte_index_of_last_selection(ctx).as_usize();
            assert_eq!(column, 4);
        });

        // Put the cursor back at the end
        editor.update(&mut app, |editor, ctx| {
            editor.move_right(/* stop at line end */ false, ctx);
        });

        editor.read(&app, |editor, ctx| {
            assert!(editor.is_single_cursor_only(ctx));
            let column = editor.start_byte_index_of_last_selection(ctx).as_usize();
            assert_eq!(column, 5);
        });
    });
}

#[cfg_attr(windows, ignore = "TODO(CORE-3626)")]
#[test]
fn test_newline_insertion() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let editor = input.read(&app, |input, _| input.editor().clone());

        // Fill in the buffer with `ls \`
        editor.update(&mut app, |editor, ctx| {
            editor.user_insert(r"ls \", ctx);
        });

        // There should only be one line.
        editor.read(&app, |editor, ctx| {
            assert_eq!(editor.max_point(ctx).row(), 0);
        });

        // Move cursor to the end of the first line
        editor.update(&mut app, |input, ctx| {
            let line_0_end = DisplayPoint::new(0, input.line_len(0, ctx).unwrap());
            input
                .select_ranges(Some(line_0_end..line_0_end), ctx)
                .unwrap();
        });

        // Handle a return
        input.update(&mut app, |input, ctx| {
            input.input_enter(ctx);
        });

        // We should have inserted a newline
        editor.read(&app, |editor, ctx| {
            assert_eq!(editor.max_point(ctx).row(), 1);
        });
    })
}

#[test]
fn test_should_not_insert_newline_on_enter_in_empty_buffer() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        input.read(&app, |input, ctx| {
            assert!(input.buffer_text(ctx).is_empty());
            assert!(!input.should_insert_newline_on_enter(ctx));
        });
    })
}

#[cfg_attr(windows, ignore = "TODO(CORE-3626)")]
#[test]
fn test_should_insert_newline_on_enter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let base_text = r"
            1 slash \
            2 slashes \\
            3 slashes \\\
            4 slashes \\\\
            no slashes
        "
        .unindent();

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        input.update(&mut app, |input, ctx| {
            input.replace_buffer_content(base_text.as_str(), ctx);
            input.editor.update(ctx, |editor, ctx| {
                editor
                    .select_ranges(vec![DisplayPoint::new(0, 0)..DisplayPoint::new(0, 0)], ctx)
                    .unwrap();
            })
        });

        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), base_text);
            assert!(input.editor.as_ref(ctx).single_cursor_on_first_line(ctx));
        });

        input.update(&mut app, |input, ctx| {
            // Move cursor to end of first line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_to_line_end(ctx);
            });
            assert!(input.should_insert_newline_on_enter(ctx));

            // Move cursor to end of second line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            // Move cursor to end of third line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(input.should_insert_newline_on_enter(ctx));

            // Move cursor to end of fourth line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            // Move cursor to end of fifth line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));
        });
    })
}

#[test]
fn test_powershell_should_insert_newline_on_enter() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let base_text = r"
            1 slash \
            1 backtick with space `
            1 backtick no space f`
            no backtick
            2 backticks ``
            3 backticks ```
        "
        .unindent();

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        input.update(&mut app, |input, ctx| {
            input.replace_buffer_content(base_text.as_str(), ctx);
            input.editor.update(ctx, |editor, ctx| {
                editor
                    .select_ranges(vec![DisplayPoint::new(0, 0)..DisplayPoint::new(0, 0)], ctx)
                    .unwrap();
                editor.set_shell_family(ShellFamily::PowerShell);
            })
        });

        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), base_text);
            assert!(input.editor.as_ref(ctx).single_cursor_on_first_line(ctx));
        });

        input.update(&mut app, |input, ctx| {
            // Move cursor to end of first line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            // Move cursor to end of second line.
            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(input.should_insert_newline_on_enter(ctx));

            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));

            input.editor.update(ctx, |editor, ctx| {
                editor.move_down(ctx);
                editor.move_to_line_end(ctx);
            });
            assert!(!input.should_insert_newline_on_enter(ctx));
        });
    })
}

#[test]
fn test_workflow_selected() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        input.update(&mut app, |input, ctx| {
            input.user_insert("hello", ctx);
        });

        let workflow = Workflow::new(
            "test",
            "{{p1}} {{parameter_2}} {{p3}} foo {{p1}} {{parameter_2}}",
        )
        .with_arguments(vec![
            Argument::new("p1", ArgumentType::Text),
            Argument::new("parameter_2", ArgumentType::Text),
            Argument::new("p3", ArgumentType::Text),
        ]);

        input.update(&mut app, |input, ctx| {
            input.show_workflows_info_box_on_workflow_selection(
                WorkflowType::Local(workflow),
                WorkflowSource::Global,
                WorkflowSelectionSource::Undefined,
                None,
                ctx,
            );
        });

        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "p1 parameter_2 p3 foo p1 parameter_2"
            );
        });
    });
}

#[test]
fn test_workflow_selected_with_default_value() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        let workflow = Workflow::new("test", "{{p1}}/{{parameter_2}}").with_arguments(vec![
            Argument {
                name: "p1".into(),
                description: None,
                default_value: Some("default_parameter_1".into()),
                arg_type: Default::default(),
            },
            Argument {
                name: "parameter_2".into(),
                description: None,
                default_value: Some("default_parameter_2".into()),
                arg_type: Default::default(),
            },
        ]);

        input.update(&mut app, |input, ctx| {
            input.show_workflows_info_box_on_workflow_selection(
                WorkflowType::Local(workflow),
                WorkflowSource::Global,
                WorkflowSelectionSource::Undefined,
                None,
                ctx,
            );
        });

        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "default_parameter_1/default_parameter_2"
            );
        });
    });
}

#[test]
fn test_multiple_workflows_selected() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        let workflow = Workflow::new("test", "p1 {{foo}} bar")
            .with_arguments(vec![Argument::new("foo", ArgumentType::Text)]);

        input.update(&mut app, |input, ctx| {
            input.show_workflows_info_box_on_workflow_selection(
                WorkflowType::Local(workflow.clone()),
                WorkflowSource::Global,
                WorkflowSelectionSource::Undefined,
                None,
                ctx,
            );
        });

        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "p1 foo bar");
        });

        // "foo" should be the only range highlighted.
        input.update(&mut app, |input, ctx| {
            let text_style_runs = input.editor.read(ctx, |editor, ctx| {
                editor
                    .text_style_runs(ctx)
                    .filter_map(|text_run| {
                        text_run
                            .text_style()
                            .background_color
                            .map(|_| text_run.text().to_owned())
                    })
                    .collect::<Vec<_>>()
            });

            assert_eq!(text_style_runs, ["foo"]);
        });

        // Input the workflow again.
        input.update(&mut app, |input, ctx| {
            input.show_workflows_info_box_on_workflow_selection(
                WorkflowType::Local(workflow),
                WorkflowSource::Global,
                WorkflowSelectionSource::Undefined,
                None,
                ctx,
            );
        });

        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "p1 foo bar");
        });

        // "foo" should be the only range highlighted.
        input.update(&mut app, |input, ctx| {
            let text_style_runs = input.editor.read(ctx, |editor, ctx| {
                editor
                    .text_style_runs(ctx)
                    .filter_map(|text_run| {
                        text_run
                            .text_style()
                            .background_color
                            .map(|_| text_run.text().to_owned())
                    })
                    .collect::<Vec<_>>()
            });

            assert_eq!(text_style_runs, ["foo"]);
        });
    });
}

#[test]
fn test_workflow_argument_tab_with_syntax_highlighting() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        let workflow = Workflow::new("test", "yarn {{cwd}} {{flags}}").with_arguments(vec![
            Argument {
                name: "cwd".into(),
                description: None,
                default_value: Some("--cwd ./".into()),
                arg_type: Default::default(),
            },
            Argument::new("flags", ArgumentType::Text),
        ]);

        input.update(&mut app, |input, ctx| {
            input.show_workflows_info_box_on_workflow_selection(
                WorkflowType::Local(workflow.clone()),
                WorkflowSource::Global,
                WorkflowSelectionSource::Undefined,
                None,
                ctx,
            );

            // Simulates syntax highlighting highlighting a portion of an argument
            input.editor.update(ctx, |editor, ctx| {
                let theme = Appearance::as_ref(ctx).theme();
                let terminal_colors_normal = theme.terminal_colors().normal.to_owned();
                editor.update_buffer_styles(
                    vec![ByteOffset::from(5)..ByteOffset::from(10)],
                    TextStyleOperation::default().set_syntax_color(
                        AnsiColorIdentifier::Yellow
                            .to_ansi_color(&terminal_colors_normal)
                            .into(),
                    ),
                    ctx,
                )
            })
        });

        // Even though there are 2 args, there will be 3 runs
        input.read(&app, |input, ctx| {
            // Buffer text should equal our command w/ defaults inserted
            assert_eq!(input.buffer_text(ctx), "yarn --cwd ./ flags");

            let selected_text = input
                .editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));

            // Currently selected text should be the text for the first arg
            assert_eq!(selected_text, "--cwd ./");

            let text_style_runs = input.editor.read(ctx, |editor, ctx| {
                editor
                    .text_style_runs(ctx)
                    .filter_map(|text_run| {
                        text_run
                            .text_style()
                            .background_color
                            .map(|_| text_run.text().to_owned())
                    })
                    .collect::<Vec<_>>()
            });

            // Even though we have only 2 args, there will be 3 runs b/c of syntax highlighting
            assert_eq!(text_style_runs, ["--cwd", " ./", "flags"]);
        });

        input.update(&mut app, |input, ctx| {
            input.input_shift_tab(ctx);
        });

        input.read(&app, |input, ctx| {
            let selected_text = input
                .editor
                .read(ctx, |editor, ctx| editor.selected_text(ctx));

            // Tab moves over to next argument
            assert_eq!(selected_text, "flags");
        })
    })
}

#[test]
fn test_workflow_view_does_not_panic() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        let workflows = vec![
            Workflow::new("Test Workflow", "echo \"Hello World\""),
            Workflow::new("Test Workflow with Description", "echo \"Hello World\"")
                .with_description("This is a test workflow that prints Hello World!".into()),
            Workflow::new("Test Workflow with Args", "echo \"Hello {{person}}\"").with_arguments(
                vec![
                    Argument::new("person", ArgumentType::Text)
                        .with_description("The person you want to say hello to".to_string()),
                ],
            ),
            Workflow::new("test", "echo \"Hello {{person}}\"")
                .with_description("This is a test workflow that prints Hello {{person}}!".into())
                .with_arguments(vec![
                    Argument::new("person", ArgumentType::Text)
                        .with_description("The person you want to say hello to".to_string()),
                ]),
        ];

        for workflow in workflows {
            let command = workflow.content().to_string();
            input.update(&mut app, |input, ctx| {
                input.show_workflows_info_box_on_workflow_selection(
                    WorkflowType::Local(workflow),
                    WorkflowSource::Global,
                    WorkflowSelectionSource::Undefined,
                    None,
                    ctx,
                );
            });

            input.read(&app, |input, ctx| {
                // Buffer text should equal our command w/ defaults inserted
                assert_eq!(
                    input.buffer_text(ctx),
                    command.replace("{{", "").replace("}}", "")
                );
            });
        }
    })
}

#[test]
fn test_system_insert() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(
            &mut app, None, /* history_file_commands */
            None,
        )
        .await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        input.update(&mut app, |input, ctx| {
            input.system_insert("hello world", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "hello world",
                "Should have inserted 'hello world'"
            );
        });
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
        });
        input.read(&app, |input, ctx| {
            assert!(input.buffer_text(ctx).is_empty(), "Input should be empty");
        });
        input.update(&mut app, |input, ctx| {
            input.system_insert("hello\nworld", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "hello\nworld",
                "Should have inserted 'hello\nworld'"
            );
        });
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
        });
        input.read(&app, |input, ctx| {
            assert!(input.buffer_text(ctx).is_empty(), "Input should be empty");
        });
        input.update(&mut app, |input, ctx| {
            input.system_insert("héłló worlḏ", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "héłló worlḏ",
                "Should have inserted 'héłló worlḏ'"
            );
        });
    });
}

#[test]
fn test_is_cursor_in_valid_position_for_completions_while_typing() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });
        input.update(&mut app, |input, ctx| {
            input.set_active_block_metadata(
                BlockMetadata::new(Some(SessionId::from(0)), Some("~".into())),
                false,
                ctx,
            )
        });

        // If cursor is at end of line, show completions menu
        input.update(&mut app, |input, ctx| {
            input.user_insert("gi", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Buffer now looks like "gi|"
        });
        input.update(&mut app, |input, ctx| {
            assert!(input.is_cursor_in_valid_position_for_completions_while_typing(ctx));
        });

        // If cursor is not at end of line, don't show completions menu
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_start(ctx);
            // Buffer now looks like "|gi"
        });
        input.update(&mut app, |input, ctx| {
            assert!(!input.is_cursor_in_valid_position_for_completions_while_typing(ctx));
        });

        // Even if cursor is at end of line when there's multiple lines, don't show
        // completions unless its at the end of the last line.
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Buffer now looks like " gi|"
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("\ngi", ctx);
            // Buffer currently looks like "gi\ngi|"
            assert_eq!(input.buffer_text(ctx), "gi\ngi");
            assert!(input.is_cursor_in_valid_position_for_completions_while_typing(ctx));
        });

        editor.update(&mut app, |editor, ctx| {
            // Close the tab completion menu if open
            editor.escape(ctx);
            editor.move_up(ctx);
            // Buffer now looks like "gi|\ngi"
        });

        input.update(&mut app, |input, ctx| {
            assert!(!input.is_cursor_in_valid_position_for_completions_while_typing(ctx));
        });

        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Buffer now looks like "gi\ngi|"
        });
        input.update(&mut app, |input, ctx| {
            assert!(input.is_cursor_in_valid_position_for_completions_while_typing(ctx));
        });
    });
}

#[test]
fn test_last_word_insertions() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // last word insertion looks for preceding whitespace character
        let history_file_commands = vec![
            "https://app.warp.dev".to_string(),
            "cargo check\ncargo run --features".to_string(),
        ];
        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, Some(history_file_commands), None)
                .await;

        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });

        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("git test", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git test");
        });

        // Insert while selecting the word `test`
        editor.update(&mut app, |editor, ctx| {
            editor.select_word(&DisplayPoint::new(0, 4), ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.insert_last_word_previous_command(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git --features");
        });

        // Next insert replaces inserted word (not all of current text), with word from second last history command
        input.update(&mut app, |input, ctx| {
            input.insert_last_word_previous_command(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git https://app.warp.dev");
        });

        // Insert is temporary, undo goes back to initial state before first insertion
        // After undo, `test` is currently selected
        editor.update(&mut app, |editor, ctx| {
            editor.undo(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git test");
        });

        // After system edit action (undo), subsequent inserts will insert last word of most recent command
        // After insert, `--features` is currently selected
        input.update(&mut app, |input, ctx| {
            input.insert_last_word_previous_command(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git --features");
        });

        // After user edit action (input), subsequent inserts will insert last word of most recent command
        editor.update(&mut app, |editor, ctx| {
            editor.user_insert("f", ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git f");
        });
        // Cursor after `f`
        input.update(&mut app, |input, ctx| {
            input.insert_last_word_previous_command(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git f--features");
        });

        // After non-edit action (move left), subsequent inserts will insert last word of most recent command
        editor.update(&mut app, |editor, ctx| {
            editor.move_left(/* stop at line start */ false, ctx);
            editor.move_left(/* stop at line start */ false, ctx);
        });
        input.update(&mut app, |input, ctx| {
            input.insert_last_word_previous_command(ctx);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), "git --featuresf--features");
        });
    });
}

#[test]
fn test_alias_expansion_when_invalid_expansion() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let aliases = HashMap::from_iter([("gco".into(), "git checkout".into())]);
        let session_info = SessionInfo::new_for_test().with_aliases(aliases);

        set_alias_expansion_setting(true, &mut app);
        let terminal = add_window_with_bootstrapped_terminal(
            &mut app,
            None, /* history_file_commands */
            Some(session_info),
        )
        .await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });
        input.update(&mut app, |input, ctx| {
            input.set_active_block_metadata(
                BlockMetadata::new(Some(SessionId::from(0)), Some("~".into())),
                false,
                ctx,
            )
        });

        // No expansion if the token is an argument
        input.update(&mut app, |input, ctx| {
            input.user_insert("test gco ", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Cursor is now at "test gco |"
        });
        input.update(&mut app, |input, ctx| {
            input.run_expansion_on_space(ctx);
            assert_eq!(input.buffer_text(ctx), "test gco ");
        });

        // No expansion if the token is not an alias
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("test ", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Cursor is now at "test |"
        });
        input.update(&mut app, |input, ctx| {
            input.run_expansion_on_space(ctx);
            assert_eq!(input.buffer_text(ctx), "test ");
        });
    });
}

#[test]
fn test_alias_expansion_when_alias_includes_itself() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let aliases =
            HashMap::from_iter([("g".into(), "git".into()), ("ls".into(), "ls -G".into())]);
        let session_info = SessionInfo::new_for_test().with_aliases(aliases);

        set_alias_expansion_setting(true, &mut app);
        let terminal = add_window_with_bootstrapped_terminal(
            &mut app,
            None, /* history_file_commands */
            Some(session_info),
        )
        .await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });
        input.update(&mut app, |input, ctx| {
            input.set_active_block_metadata(
                BlockMetadata::new(Some(SessionId::from(0)), Some("~".into())),
                false,
                ctx,
            )
        });

        // An alias that includes itself is not expanded
        input.update(&mut app, |input, ctx| {
            input.user_insert("ls ", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Cursor is now at "ls |"
        });
        input.update(&mut app, |input, ctx| {
            input.run_expansion_on_space(ctx);
            assert_eq!(input.buffer_text(ctx), "ls ");
        });

        // Aliases that are only a substring of the alias value are still expanded
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert("g ", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Cursor is now at "g |"
        });
        input.update(&mut app, |input, ctx| {
            input.run_expansion_on_space(ctx);
            assert_eq!(input.buffer_text(ctx), "git ");
        });
    });
}

#[test]
fn test_alias_expansion_with_abbreviations() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let abbreviations = HashMap::from_iter([("g".into(), "git log".into())]);
        let aliases = HashMap::from_iter([("g".into(), "git".into())]);
        let session_info = SessionInfo::new_for_test()
            .with_aliases(aliases)
            .with_abbreviations(abbreviations);

        set_alias_expansion_setting(true, &mut app);
        let terminal = add_window_with_bootstrapped_terminal(
            &mut app,
            None, /* history_file_commands */
            Some(session_info),
        )
        .await;

        let input = terminal.read(&app, |terminal, _| terminal.input().clone());
        let editor = input.read(&app, |input, _| input.editor().clone());

        input.update(&mut app, |input, ctx| {
            input.set_active_block_metadata(
                BlockMetadata::new(Some(SessionId::from(0)), Some("~".into())),
                false,
                ctx,
            )
        });

        // Abbreviations are expanded and take priority over aliases
        input.update(&mut app, |input, ctx| {
            input.user_insert("g ", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            // Cursor is now at "g |"
        });
        input.update(&mut app, |input, ctx| {
            input.run_expansion_on_space(ctx);
            assert_eq!(input.buffer_text(ctx), "git log ");
        });
    });
}

#[test]

macro_rules! input_mode_prefix_tests {
    ($($name:ident: ($udi_enabled:literal, $input_mode:expr_2021),)*) => {
        $(
            #[test]
            fn $name() {
            }
        )*
    };
}

#[test]

#[test]
fn test_remove_ignored_suggestion_on_command_execution() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;
        let input = terminal.read(&app, |view, _| view.input().clone());

        // First, add a command to ignored suggestions
        let test_command = "echo hi";
        IgnoredSuggestionsModel::handle(&app).update(&mut app, |model, ctx| {
            model.add_ignored_suggestion(
                test_command.to_string(),
                crate::suggestions::ignored_suggestions_model::SuggestionType::ShellCommand,
                ctx,
            );
        });

        // Verify the command is ignored
        let is_ignored_before = IgnoredSuggestionsModel::handle(&app).read(&app, |model, _| {
            model.is_ignored(
                test_command,
                crate::suggestions::ignored_suggestions_model::SuggestionType::ShellCommand,
            )
        });
        assert!(is_ignored_before, "Command should be ignored initially");

        // Execute the command
        input.update(&mut app, |input, ctx| {
            input.clear_buffer_and_reset_undo_stack(ctx);
            input.user_insert(test_command, ctx);
            input.try_execute_command(test_command, ctx);
        });

        // Verify the command is no longer ignored
        let is_ignored_after = IgnoredSuggestionsModel::handle(&app).read(&app, |model, _| {
            model.is_ignored(
                test_command,
                crate::suggestions::ignored_suggestions_model::SuggestionType::ShellCommand,
            )
        });
        assert!(
            !is_ignored_after,
            "Command should no longer be ignored after execution"
        );
    });
}

#[test]
fn test_page_up_and_down_scroll_terminal_from_prompt() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });

        terminal.update(&mut app, |terminal, _| {
            terminal
                .model
                .lock()
                .simulate_block("ls", &"\n".repeat(1000));
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("echo first line\necho second line", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.move_to_buffer_end(ctx);
            editor.handle_action(&EditorAction::PageUp, ctx);
        });

        assert_eq!(
            input.read(&app, |input, ctx| input.buffer_text(ctx)),
            "echo first line\necho second line"
        );
        let scroll_position_after_page_up =
            terminal.read(&app, |terminal, _| terminal.scroll_position());
        assert!(matches!(
            scroll_position_after_page_up,
            ScrollPosition::FixedAtPosition { .. }
        ));

        editor.update(&mut app, |editor, ctx| {
            editor.handle_action(&EditorAction::PageDown, ctx);
        });

        assert_eq!(
            input.read(&app, |input, ctx| input.buffer_text(ctx)),
            "echo first line\necho second line"
        );
        let scroll_position_after_page_down =
            terminal.read(&app, |terminal, _| terminal.scroll_position());
        assert_ne!(
            scroll_position_after_page_down,
            scroll_position_after_page_up
        );
    });
}

#[test]
fn test_page_up_and_down_do_not_scroll_terminal_when_suggestions_are_visible() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let history_file_commands = vec![
            "echo alpha\necho beta".to_string(),
            "git status\ngit diff".to_string(),
        ];
        let terminal =
            add_window_with_bootstrapped_terminal(&mut app, Some(history_file_commands), None)
                .await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });

        terminal.update(&mut app, |terminal, _| {
            terminal
                .model
                .lock()
                .simulate_block("ls", &"\n".repeat(1000));
        });

        input.update(&mut app, |input, ctx| {
            input.handle_action(&InputAction::Up, ctx);
            assert!(input.suggestions_mode_model.as_ref(ctx).is_visible());
        });

        let initial_scroll_position = terminal.read(&app, |terminal, _| terminal.scroll_position());
        let initial_buffer = input.read(&app, |input, ctx| input.buffer_text(ctx));

        editor.update(&mut app, |editor, ctx| {
            editor.handle_action(&EditorAction::PageUp, ctx);
            editor.handle_action(&EditorAction::PageDown, ctx);
        });

        terminal.read(&app, |terminal, _| {
            assert_eq!(terminal.scroll_position(), initial_scroll_position);
        });
        input.read(&app, |input, ctx| {
            assert_eq!(input.buffer_text(ctx), initial_buffer);
            assert!(input.suggestions_mode_model.as_ref(ctx).is_visible());
        });
    });
}

#[test]
fn test_page_up_and_down_scroll_terminal_with_vim_mode_enabled() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });

        terminal.update(&mut app, |terminal, _| {
            terminal
                .model
                .lock()
                .simulate_block("ls", &"\n".repeat(1000));
        });

        AppEditorSettings::handle(&app).update(&mut app, |settings, settings_ctx| {
            let _ = settings.vim_mode.set_value(true, settings_ctx);
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("echo first line\necho second line", ctx);
        });
        editor.update(&mut app, |editor, ctx| {
            editor.vim_keystroke(&Keystroke::parse("escape").unwrap(), ctx);
        });
        editor.read(&app, |editor, ctx| {
            assert_eq!(editor.vim_mode(ctx), Some(VimMode::Normal));
        });

        editor.update(&mut app, |editor, ctx| {
            editor.handle_action(&EditorAction::PageUp, ctx);
        });

        assert_eq!(
            input.read(&app, |input, ctx| input.buffer_text(ctx)),
            "echo first line\necho second line"
        );
        let scroll_position_after_page_up =
            terminal.read(&app, |terminal, _| terminal.scroll_position());
        assert!(matches!(
            scroll_position_after_page_up,
            ScrollPosition::FixedAtPosition { .. }
        ));

        editor.update(&mut app, |editor, ctx| {
            editor.handle_action(&EditorAction::PageDown, ctx);
        });

        assert_eq!(
            input.read(&app, |input, ctx| input.buffer_text(ctx)),
            "echo first line\necho second line"
        );
        let scroll_position_after_page_down =
            terminal.read(&app, |terminal, _| terminal.scroll_position());
        assert_ne!(
            scroll_position_after_page_down,
            scroll_position_after_page_up
        );
    });
}

#[test]
fn test_custom_terminal_page_scroll_binding_applies_when_prompt_is_focused() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let (window_id, terminal) =
            add_window_with_bootstrapped_terminal_and_window_id(&mut app, None, None).await;
        let (input, editor) = terminal.read(&app, |terminal, ctx| {
            let input = terminal.input().clone();
            let editor = input.as_ref(ctx).editor().clone();
            (input, editor)
        });

        terminal.update(&mut app, |terminal, _| {
            terminal
                .model
                .lock()
                .simulate_block("ls", &"\n".repeat(1000));
        });

        app.update(|ctx| {
            ctx.set_custom_trigger(
                "terminal:scroll_up_one_page".to_owned(),
                warpui::keymap::Trigger::Keystrokes(vec![
                    Keystroke::parse("shift-pageup").unwrap(),
                ]),
            );
        });

        let focus_path = [terminal.id(), input.id(), editor.id()];

        let handled = app
            .dispatch_keystroke(
                window_id,
                &focus_path,
                &Keystroke::parse("pageup").unwrap(),
                false,
            )
            .unwrap();
        assert!(!handled);
        terminal.read(&app, |terminal, _| {
            assert_eq!(
                terminal.scroll_position(),
                ScrollPosition::FollowsBottomOfMostRecentBlock
            );
        });

        let handled = app
            .dispatch_keystroke(
                window_id,
                &focus_path,
                &Keystroke::parse("shift-pageup").unwrap(),
                false,
            )
            .unwrap();
        assert!(handled);
        terminal.read(&app, |terminal, _| {
            assert!(matches!(
                terminal.scroll_position(),
                ScrollPosition::FixedAtPosition { .. }
            ));
        });
    });
}

// Helper: open the CLI-agent rich input for the terminal view under test.

/// `unfreeze_agent_input` must NOT clear the buffer. The buffer is cleared via CRDT
/// delete ops emitted by `system_clear_buffer` when `SentRequest` fires, which flow to
/// both the server (for new viewers) and existing viewers (via `InputUpdated`).
/// Clearing the buffer here would cause CRDT inconsistencies (see the function doc).
#[test]
fn unfreeze_agent_input_does_not_clear_buffer() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let tips_model = app.add_model(|_| TipsCompleted::default());

        // Test for ActiveSharer
        let (_, sharer_terminal) = app.add_window(WindowStyle::NotStealFocus, move |ctx| {
            TerminalView::new_for_test(tips_model, None, ctx)
        });
        sharer_terminal.update(&mut app, |view, _| {
            let mut model = view.model.lock();
            model.block_list_mut().set_bootstrapped();
            model.set_shared_session_status(SharedSessionStatus::ActiveSharer);
        });
        let sharer_input = sharer_terminal.read(&app, |view, _| view.input().clone());

        sharer_input.update(&mut app, |input, ctx| {
            input.replace_buffer_content("help me write a test", ctx);
        });
        assert_eq!(
            sharer_input.read(&app, |i, ctx| i.buffer_text(ctx)),
            "help me write a test"
        );

        sharer_input.update(&mut app, |input, ctx| {
            input.unfreeze_agent_input(false, ctx);
        });

        // Buffer must be unchanged — clearing is the responsibility of system_clear_buffer
        // via the SentRequest event, not of this unfreeze function.
        assert_eq!(
            sharer_input.read(&app, |i, ctx| i.buffer_text(ctx)),
            "help me write a test",
            "unfreeze_agent_input must not clear the sharer's buffer"
        );

        // Same for ActiveViewer
        let tips_model2 = app.add_model(|_| TipsCompleted::default());
        let (_, viewer_terminal) = app.add_window(WindowStyle::NotStealFocus, move |ctx| {
            TerminalView::new_for_test(tips_model2, None, ctx)
        });
        viewer_terminal.update(&mut app, |view, _| {
            let mut model = view.model.lock();
            model.block_list_mut().set_bootstrapped();
            model.set_shared_session_status(SharedSessionStatus::executor());
        });
        let viewer_input = viewer_terminal.read(&app, |view, _| view.input().clone());

        viewer_input.update(&mut app, |input, ctx| {
            input.replace_buffer_content("follow-up question", ctx);
        });
        assert_eq!(
            viewer_input.read(&app, |i, ctx| i.buffer_text(ctx)),
            "follow-up question"
        );

        viewer_input.update(&mut app, |input, ctx| {
            input.unfreeze_agent_input(false, ctx);
        });

        assert_eq!(
            viewer_input.read(&app, |i, ctx| i.buffer_text(ctx)),
            "follow-up question",
            "unfreeze_agent_input must not clear the viewer's buffer"
        );
    });
}

/// With the '#' AI Command Search trigger disabled (APP-5557), typing '#' at the start of the
/// buffer must leave it (and any text typed after it) as literal input, and must not open AI
/// Command Search — this is what lets the text be finished and submitted as a shell comment
/// instead of trapping the user in the panel.
#[test]
fn hash_trigger_disabled_keeps_hash_literal_and_does_not_open_ai_command_search() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        InputSettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .enable_ai_command_search_hash_trigger
                .set_value(false, ctx)
                .expect("setting value must succeed");
        });

        let terminal = add_window_with_bootstrapped_terminal(&mut app, None, None).await;
        let input = terminal.read(&app, |terminal, _| terminal.input().clone());

        let open_count = Rc::new(RefCell::new(0));
        let open_count_for_subscription = open_count.clone();
        app.update(|ctx| {
            ctx.subscribe_to_view(&input, move |_, event, _| {
                if matches!(event, Event::ShowCommandSearch(_)) {
                    *open_count_for_subscription.borrow_mut() += 1;
                }
            });
        });

        input.update(&mut app, |input, ctx| {
            input.user_insert("#", ctx);
            input.user_insert(" this is a test comment", ctx);
        });

        input.read(&app, |input, ctx| {
            assert_eq!(
                input.buffer_text(ctx),
                "# this is a test comment",
                "the '#' and the text typed after it must remain literal input"
            );
        });
        assert_eq!(
            *open_count.borrow(),
            0,
            "AI Command Search must not open when the '#' trigger setting is disabled"
        );
    });
}

#[cfg(test)]
mod completion_sources_resolution_tests {

    // A multi-line buffer disables native completions even on a Tab trigger with both toggles on.
}
