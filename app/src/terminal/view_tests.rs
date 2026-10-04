use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::pin;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{Local, Utc};
use parking_lot::FairMutex;
use session_sharing_protocol::common::CLIAgentSessionState;
use warp_cli::agent::Harness;
use warp_terminal::model::escape_sequences::{BRACKETED_PASTE_END, BRACKETED_PASTE_START, C0};
use warpui::notification::UserNotification;
use warpui::platform::WindowStyle;
use warpui::{App, EntityIdSet, Presenter, ReadModel, WindowInvalidation};

use super::*;
use crate::auth::user::TEST_USER_UID;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{CloudObjectMetadata, CloudObjectPermissions};
use crate::code_review::comments::{
    AttachedReviewComment, AttachedReviewCommentTarget, CommentOrigin,
};
use crate::context_chips::prompt::Prompt;
use crate::editor::{AutosuggestionLocation, AutosuggestionType, CrdtOperation};
use crate::features::FeatureFlag;
use crate::pane_group::focus_state::PaneGroupFocusState;
use crate::pane_group::pane::PaneStack;
use crate::pane_group::{BackingView, TerminalPaneId};
use crate::server::ids::{ClientId, SyncId};
use crate::server::team_scope::RequestTeamScope;
use crate::settings::import::model::ImportedConfigModel;
use crate::settings::{ AppEditorSettings, RightClickBehavior, WarpPromptSeparator};
use crate::tab::NewSessionMenuItem;
use crate::terminal::alt_screen::should_intercept_mouse;
use crate::terminal::block_list_element::{SnackbarPoint, SnackbarTranslationMode};
use crate::terminal::block_list_viewport::{ClampingMode, ScrollLines};
use crate::terminal::model::ansi::{self, BootstrappedValue, InitShellValue, PreexecValue};
use crate::terminal::model::blocks::{TotalIndex, insert_block};
use crate::terminal::model::grid::Dimensions as _;
use crate::terminal::model::terminal_model::WithinBlock;
use crate::terminal::shared_session::shared_handlers::{
    RemoteUpdateGuard,
};
use crate::terminal::shared_session::{SharedSessionSource, SharedSessionStatus};
use crate::terminal::{ MockTerminalManager, TerminalManager, TerminalModel, should_right_click_paste,
};
use crate::test_util::terminal::{ initialize_app_for_terminal_view,
};
use crate::test_util::{ assert_eventually};
use crate::view_components::find::FindWithinBlockState;
use crate::workspace::view::tests::{initialize_app as initialize_workspace_app, mock_workspace};
use crate::workspace::{ToastStack, WorkspaceAction};
use crate::workspaces::user_workspaces::TeamlessScopeForTest;









































/// Bootstraps the terminal model with one completed block and one active long-running block.
fn bootstrap_with_long_running_block(view: &mut TerminalView) {
    let mut model = view.model.lock();
    model.init_shell(InitShellValue {
        session_id: 0.into(),
        shell: "zsh".to_owned(),
        ..Default::default()
    });
    model.bootstrapped(BootstrappedValue {
        shell: "zsh".to_owned(),
        ..Default::default()
    });
    model.simulate_block("ls", "file.txt");
    model.simulate_long_running_block("long-command", "output");
}








struct TestTerminalManager {
    model: Arc<FairMutex<TerminalModel>>,
    _view: ViewHandle<TerminalView>,
}

impl TerminalManager for TestTerminalManager {
    fn model(&self) -> Arc<FairMutex<TerminalModel>> {
        self.model.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}














#[test]
fn command_first_word_and_suffix_preserves_leading_whitespace() {
    assert_eq!(
        command_first_word_and_suffix("  myssh arg"),
        Some(("myssh", " arg"))
    );
}

#[test]
fn command_first_word_and_suffix_handles_alias_without_args() {
    assert_eq!(
        command_first_word_and_suffix("  myssh"),
        Some(("myssh", ""))
    );
}




























impl TerminalView {


    fn is_vertically_scrollable(&self, app: &AppContext) -> bool {
        let total_block_heights = self
            .model
            .lock()
            .block_list()
            .block_heights()
            .summary()
            .height;
        let visible_rows = self.content_element_height_lines(app);
        heights_approx_gt(total_block_heights, visible_rows)
    }
}

fn read_from_clipboard(ctx: &mut ViewContext<TerminalView>) -> String {
    TerminalView::read_from_clipboard(Some(ShellFamily::Posix), ctx)
}


const BODY_PREFIX: &str = "Latest output: ";





// Regression test for WAR-3433 on find bar selection crash.















// #[test]
// fn test_navigate_blocks_inverted_blocklist() {
//     run_navigation_test(InputMode::PinnedToTop);
// }
















#[test]
fn test_banner_for_incompatible_plugins() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal =
            MockTerminalManager::create_new_terminal_view_window_for_test(&mut app, None);

        SessionSettings::handle(&app).update(&mut app, |session_settings, ctx| {
            let _ = session_settings.honor_ps1.set_value(true, ctx);
        });

        terminal.update(&mut app, |view, _ctx| {
            let mut model = view.model.lock();
            model.init_shell(InitShellValue {
                session_id: 0.into(),
                shell: "zsh".to_owned(),
                ..Default::default()
            });
            model.bootstrapped(BootstrappedValue {
                shell: "zsh".to_owned(),
                shell_plugins: Some(HashSet::from(["p10k_unsupported".to_string()])),
                ..Default::default()
            });
        });

        // This is asynchronous because we're waiting for the bootstrap event
        // to be sent from the terminal model to the terminal view.
        assert_eventually!(
            200 => terminal.read(&app, |view, _ctx| view
                .is_incompatible_configuration_banner_open),
            "Banner did not open in time"
        );
    })
}

/// Regression test for #9011: the slow-bootstrap banner used to persist
/// indefinitely when shell integration never sent the bootstrap signal
/// (e.g. the user's shell `exec`s into `expect` before Warp's integration
/// runs). The auto-dismiss timer scheduled when the banner opens must
/// eventually close it.
#[test]
fn test_slow_bootstrap_banner_auto_dismisses() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal =
            MockTerminalManager::create_new_terminal_view_window_for_test(&mut app, None);

        // Open the banner directly and schedule a short-duration auto-dismiss
        // timer. We bypass `on_bootstrap_failed_timer_complete` (which itself
        // waits the 7-second bootstrap timeout) to keep this test fast — the
        // important behavior under test is the auto-dismiss path.
        terminal.update(&mut app, |view, ctx| {
            view.is_slow_bootstrap_banner_open = true;
            view.slow_bootstrap_banner_auto_dismiss_handle = Some(
                view.start_slow_bootstrap_banner_auto_dismiss_timer(Duration::from_millis(50), ctx),
            );
        });

        assert!(terminal.read(&app, |view, _ctx| view.is_slow_bootstrap_banner_open));

        assert_eventually!(
            200 => terminal.read(&app, |view, _ctx| !view.is_slow_bootstrap_banner_open
                && view.slow_bootstrap_banner_auto_dismiss_handle.is_none()),
            "Slow bootstrap banner did not auto-dismiss"
        );
    })
}


// Regression test for GH#3548 / GH#6093: the "Seems like your completions are not
// working" banner must offer a permanent "Don't show me again" dismissal that is
// persisted, while the "x" close button keeps its existing per-session behavior.

// Regression test for GH#3548 / GH#6093: once the banner has been permanently
// dismissed it must not reopen on subsequent sessions.

#[test]
fn test_bash_vim_banner_already_shown() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal =
            MockTerminalManager::create_new_terminal_view_window_for_test(&mut app, None);

        // Ensure the terminal is the active session.
        terminal.update(&mut app, |view, ctx| {
            let terminal_pane_id = TerminalPaneId::dummy_terminal_pane_id();
            let focus_state = ctx.add_model(|_| {
                PaneGroupFocusState::new(terminal_pane_id.into(), Some(terminal_pane_id), false)
            });
            let focus_handle = PaneFocusHandle::new(terminal_pane_id.into(), focus_state);
            view.set_focus_handle(focus_handle, ctx);
        });

        // The banner has already been shown and dismissed.
        VimBannerSettings::handle(&app).update(&mut app, |banner_settings, ctx| {
            let _ = banner_settings
                .vim_keybindings_banner_state
                .set_value(BannerState::Dismissed, ctx);
        });

        // Ensure Warp's vim keybindings are off.
        AppEditorSettings::handle(&app).update(&mut app, |editor_settings, ctx| {
            let _ = editor_settings.vim_mode.set_value(false, ctx);
        });

        // Bootstrap a bash session with vi mode enabled.
        terminal.update(&mut app, |view, _ctx| {
            let mut model = view.model.lock();
            model.init_shell(InitShellValue {
                session_id: 0.into(),
                shell: "bash".to_owned(),
                ..Default::default()
            });
            model.bootstrapped(BootstrappedValue {
                shell: "bash".to_owned(),
                shell_options: Some(HashSet::from(["vi_mode".to_string()])),
                ..Default::default()
            });
        });

        // This is asynchronous because we're waiting for the bootstrap event
        // to be sent from the terminal model to the terminal view.
        assert_eventually!(
            // Since the user already dismissed the banner, it should not
            // be shown again.
            terminal.read(&app, |terminal, _terminal_ctx| {
                terminal.inline_banners_state.vim_banner_state.is_none()
            }),
            "Banner should not have opened"
        );
    })
}

















/// Subscribes to the terminal view's PTY writes so tests can assert on the bytes forwarded to the
/// shell.
fn capture_pty_writes(
    app: &mut App,
    terminal: &ViewHandle<TerminalView>,
) -> Rc<RefCell<Vec<Vec<u8>>>> {
    let pty_writes: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let writes = pty_writes.clone();
    app.update(|ctx| {
        ctx.subscribe_to_view(terminal, move |_, event, _| {
            if let Event::WriteBytesToPty { bytes } = event {
                writes.borrow_mut().push(bytes.to_vec());
            }
        });
    });
    pty_writes
}































// Regression test for https://github.com/warpdotdev/warp/issues/9059.
// Codex's listener doesn't emit Blocked-state events (it only forwards opaque
// OSC 9 notifications as Stop), so auto-toggling rich input would trap arrow
// keys when Codex shows interactive option menus. Auto-toggle must not fire
// for agents whose handlers report `supports_rich_status() == false`.




























#[test]
fn visible_bootstrap_block_leaves_focus_on_tab_rename_editor() {
    App::test((), |mut app| async move {
        initialize_workspace_app(&mut app);
        let workspace = mock_workspace(&mut app);
        let (window, terminal) = workspace.update(&mut app, |workspace, ctx| {
            workspace.rename_tab(0, ctx);
            let terminal = workspace
                .active_tab_pane_group()
                .as_ref(ctx)
                .active_session_view(ctx)
                .expect("tab should contain a terminal");
            (ctx.window_id(), terminal)
        });
        assert!(workspace.read(&app, |workspace, ctx| {
            workspace.is_inline_rename_editor_focused(ctx)
        }));
        let focused_before = app.focused_view_id(window);

        terminal.update(&mut app, |view, ctx| {
            view.handle_terminal_event(&ModelEvent::VisibleBootstrapBlock, ctx);
        });

        assert_eq!(app.focused_view_id(window), focused_before);
        assert!(workspace.read(&app, |workspace, ctx| {
            workspace.is_inline_rename_editor_focused(ctx)
        }));
    });
}

