#[cfg(not(target_family = "wasm"))]
use std::cell::RefCell;
#[cfg(not(target_family = "wasm"))]
use std::rc::Rc;

#[cfg(not(target_family = "wasm"))]
use warpui::App;

use super::*;
#[cfg(not(target_family = "wasm"))]
use crate::server::server_api::team::MockTeamClient;
#[cfg(not(target_family = "wasm"))]
use crate::workspace::view::tests::{
    initialize_app, initialize_app_with_team_client, mock_workspace,
};
#[cfg(not(target_family = "wasm"))]
use crate::workspaces::team::DiscoveryOptions;

#[cfg(not(target_family = "wasm"))]
#[test]
fn joining_a_workspace_team_opens_only_a_new_scoped_window() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let source_workspace = mock_workspace(&mut app);
        let source_window_id = source_workspace.update(&mut app, |_, ctx| ctx.window_id());
        let source_team_uid: ServerId = 123.into();
        let joined_team_uid: ServerId = 456.into();
        UserWorkspaces::handle(&app).update(&mut app, |user_workspaces, ctx| {
            user_workspaces.register_window(source_window_id, Some(source_team_uid), ctx);
        });
        let teams_page = source_workspace.update(&mut app, |_, ctx| {
            ctx.add_typed_action_view(TeamsPageView::new)
        });

        let changed_window_ids = Rc::new(RefCell::new(Vec::new()));
        let changed_window_ids_for_subscription = changed_window_ids.clone();
        app.update(|ctx| {
            ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), move |_, event, _| {
                if let UserWorkspacesEvent::WindowTeamChanged { window_id } = event {
                    changed_window_ids_for_subscription
                        .borrow_mut()
                        .push(*window_id);
                }
            });
        });
        let initial_window_count = app.window_ids().len();

        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_model_event(
                &UserWorkspacesEvent::JoinTeamInWorkspaceSuccess {
                    team_uid: joined_team_uid,
                },
                ctx,
            );
        });

        assert_eq!(app.window_ids().len(), initial_window_count + 1);
        app.read(|ctx| {
            let user_workspaces = UserWorkspaces::as_ref(ctx);
            assert_eq!(
                user_workspaces.team_uid_for_window(source_window_id),
                Some(source_team_uid)
            );
            let joined_window_ids = ctx
                .window_ids()
                .filter(|window_id| {
                    user_workspaces.team_uid_for_window(*window_id) == Some(joined_team_uid)
                })
                .collect::<Vec<_>>();
            assert_eq!(joined_window_ids.len(), 1);
            assert_eq!(
                changed_window_ids.borrow().as_slice(),
                joined_window_ids.as_slice()
            );
        });
    });
}

fn open_team(uid: &str, name: &str) -> DiscoverableTeam {
    DiscoverableTeam {
        team_uid: uid.to_string(),
        num_members: 2,
        name: name.to_string(),
        team_accepting_invites: true,
    }
}
fn discoverable_workspace(
    uid: i64,
    name: &str,
    open_teams: Vec<DiscoverableTeam>,
) -> DiscoverableWorkspace {
    DiscoverableWorkspace {
        workspace_uid: ServerId::from(uid).into(),
        name: name.to_string(),
        open_teams,
        member_count: 4,
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn workspace_discovery_page_routes_each_option_through_the_expected_client_boundary() {
    App::test((), |mut app| async move {
        let workspace_with_teams = discoverable_workspace(
            10,
            "Workspace With Teams",
            vec![open_team(&ServerId::from(11).to_string(), "Engineering")],
        );
        let workspace_without_teams =
            discoverable_workspace(20, "Workspace Without Teams", Vec::new());
        let legacy_team = open_team(&ServerId::from(30).to_string(), "Legacy Team");
        let workspace_with_teams_uid = workspace_with_teams.workspace_uid;
        let workspace_without_teams_uid = workspace_without_teams.workspace_uid;
        let open_team_uid =
            ServerId::from_string_lossy(&workspace_with_teams.open_teams[0].team_uid);
        let legacy_team_uid = ServerId::from_string_lossy(&legacy_team.team_uid);

        let mut team_client = MockTeamClient::new();
        team_client
            .expect_get_discovery_options()
            .returning(move || {
                Ok(DiscoveryOptions {
                    workspaces: vec![
                        workspace_with_teams.clone(),
                        workspace_without_teams.clone(),
                    ],
                    legacy_teams: vec![legacy_team.clone()],
                })
            });
        team_client
            .expect_join_workspace_from_discovery()
            .withf(move |workspace_uid, team_uid| {
                *workspace_uid == workspace_without_teams_uid && team_uid.is_none()
            })
            .times(1)
            .return_once(|_, _| Err(anyhow::anyhow!("workspace-only join rejected")));
        team_client
            .expect_join_workspace_from_discovery()
            .withf(move |workspace_uid, team_uid| {
                *workspace_uid == workspace_with_teams_uid && *team_uid == Some(open_team_uid)
            })
            .times(1)
            .return_once(|_, _| Err(anyhow::anyhow!("workspace team join rejected")));
        team_client
            .expect_join_team_with_team_discovery()
            .withf(move |team_uid| *team_uid == legacy_team_uid)
            .times(1)
            .return_once(|_| Err(anyhow::anyhow!("legacy team join rejected")));

        initialize_app_with_team_client(&mut app, Arc::new(team_client));
        let workspace = mock_workspace(&mut app);
        let teams_page = workspace.update(&mut app, |_, ctx| {
            ctx.add_typed_action_view(TeamsPageView::new)
        });

        let (fetch_sender, fetch_receiver) = async_channel::unbounded();
        let (join_sender, join_receiver) = async_channel::unbounded();
        app.update(|ctx| {
            ctx.subscribe_to_model(
                &UserWorkspaces::handle(ctx),
                move |_, event: &UserWorkspacesEvent, _| match event {
                    UserWorkspacesEvent::FetchDiscoveryOptionsSuccess(_) => {
                        let _ = fetch_sender.try_send(());
                    }
                    UserWorkspacesEvent::JoinWorkspaceFromDiscoveryRejected(err)
                    | UserWorkspacesEvent::JoinTeamWithTeamDiscoveryRejected(err) => {
                        let _ = join_sender.try_send(err.to_string());
                    }
                    _ => {}
                },
            );
        });

        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.on_page_selected(false, ctx);
        });
        fetch_receiver
            .recv()
            .await
            .expect("expected discovery fetch");
        teams_page.read(&app, |teams_page, _| {
            assert_eq!(teams_page.discoverable_workspaces_states.len(), 2);
            assert_eq!(teams_page.discoverable_teams_states.len(), 1);
            assert_eq!(
                WorkspaceDiscoveryAction::for_workspace(
                    &teams_page.discoverable_workspaces_states[0].workspace
                )
                .label(),
                "Continue"
            );
            assert_eq!(
                WorkspaceDiscoveryAction::for_workspace(
                    &teams_page.discoverable_workspaces_states[1].workspace
                )
                .label(),
                "Join"
            );
        });

        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_action(
                &TeamsPageAction::ShowWorkspaceTeams {
                    workspace_uid: workspace_with_teams_uid,
                },
                ctx,
            );
        });
        teams_page.read(&app, |teams_page, _| {
            let selected = teams_page
                .workspace_discovery_screen
                .selected_workspace(&teams_page.discoverable_workspaces_states)
                .expect("Continue should open the selected workspace");
            assert_eq!(selected.workspace.name, "Workspace With Teams");
            assert_eq!(selected.open_team_states.len(), 1);
            assert_eq!(selected.open_team_states[0].team.name, "Engineering");
        });
        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_action(&TeamsPageAction::ShowDiscoveryOptions, ctx);
        });
        teams_page.read(&app, |teams_page, _| {
            assert_eq!(
                teams_page.workspace_discovery_screen,
                WorkspaceDiscoveryScreen::Options
            );
        });

        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_action(
                &TeamsPageAction::JoinWorkspaceFromDiscovery {
                    workspace_uid: workspace_without_teams_uid,
                    team_uid: None,
                },
                ctx,
            );
        });
        teams_page.read(&app, |teams_page, _| {
            assert_eq!(
                teams_page.discovery_join_target,
                Some(DiscoveryJoinTarget::Workspace {
                    workspace_uid: workspace_without_teams_uid,
                    team_uid: None,
                })
            );
        });
        assert_eq!(
            join_receiver
                .recv()
                .await
                .expect("expected workspace-only join result"),
            "workspace-only join rejected"
        );
        teams_page.read(&app, |teams_page, _| {
            assert!(teams_page.discovery_join_target.is_none());
        });

        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_action(
                &TeamsPageAction::ShowWorkspaceTeams {
                    workspace_uid: workspace_with_teams_uid,
                },
                ctx,
            );
            teams_page.handle_action(
                &TeamsPageAction::JoinWorkspaceFromDiscovery {
                    workspace_uid: workspace_with_teams_uid,
                    team_uid: Some(open_team_uid),
                },
                ctx,
            );
        });
        teams_page.read(&app, |teams_page, _| {
            assert_eq!(
                teams_page.discovery_join_target,
                Some(DiscoveryJoinTarget::Workspace {
                    workspace_uid: workspace_with_teams_uid,
                    team_uid: Some(open_team_uid),
                })
            );
        });
        assert_eq!(
            join_receiver
                .recv()
                .await
                .expect("expected workspace team join result"),
            "workspace team join rejected"
        );
        teams_page.read(&app, |teams_page, _| {
            assert!(teams_page.discovery_join_target.is_none());
            assert_eq!(
                teams_page.workspace_discovery_screen,
                WorkspaceDiscoveryScreen::OpenTeams(workspace_with_teams_uid)
            );
        });
        teams_page.update(&mut app, |teams_page, ctx| {
            teams_page.handle_action(&TeamsPageAction::ShowDiscoveryOptions, ctx);
            teams_page.handle_action(
                &TeamsPageAction::JoinTeamWithTeamDiscovery {
                    team_uid: legacy_team_uid,
                },
                ctx,
            );
        });
        teams_page.read(&app, |teams_page, _| {
            assert_eq!(
                teams_page.discovery_join_target,
                Some(DiscoveryJoinTarget::LegacyTeam(legacy_team_uid))
            );
        });
        assert_eq!(
            join_receiver
                .recv()
                .await
                .expect("expected legacy team join result"),
            "legacy team join rejected"
        );
        teams_page.read(&app, |teams_page, _| {
            assert!(teams_page.discovery_join_target.is_none());
            assert_eq!(
                teams_page.workspace_discovery_screen,
                WorkspaceDiscoveryScreen::Options
            );
        });
    });
}

const MEMBER_EMAIL: &str = "member@example.com";

#[test]
fn unresolved_workspace_keeps_create_team_ui() {
    assert_eq!(
        TeamsWidget::page_sections_for(None, Some(MEMBER_EMAIL), false),
        vec![TeamsPageSection::CreateTeam]
    );
}

#[cfg(target_family = "wasm")]
#[test]
fn wasm_does_not_expose_open_workspace_teams() {
    let mut workspace = workspace_with_member(MEMBER_EMAIL, MembershipRole::User, true);
    workspace.open_teams = vec![open_team("0000000000000000000002", "Second Team")];

    let states = TeamsPageView::open_team_states_for_workspace(Some(&workspace));

    assert!(states.is_empty());
}

#[test]
fn disabled_row_renders_dimmed_and_tooltipped() {
    let appearance = Appearance::mock();

    assert_eq!(
        item_row_text_color(&appearance, true),
        appearance.theme().disabled_ui_text_color(),
    );
    assert_ne!(
        item_row_text_color(&appearance, true),
        item_row_text_color(&appearance, false),
        "a disabled row must render in a visibly different color than an active row"
    );
    assert_eq!(
        disabled_member_tooltip_text(true),
        Some(DISABLED_MEMBER_TOOLTIP_TEXT)
    );
    assert_eq!(disabled_member_tooltip_text(false), None);
}
