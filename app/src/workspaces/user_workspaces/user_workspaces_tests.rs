use std::time::Duration;

use warpui::elements::Empty;
use warpui::{AddSingletonModel, App, Element, TypedActionView, View};

use super::*;
use crate::server::server_api::team::MockTeamClient;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::Workspace;

#[derive(Default)]
struct CachedResources {
    workspaces: Vec<Workspace>,
}

fn initialize_window_team_test_app(app: &mut App, workspaces: Vec<Workspace>) {
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|ctx| {
        UserWorkspaces::mock(
            Arc::new(MockTeamClient::new()),
            Arc::new(MockWorkspaceClient::new()),
            workspaces,
            ctx,
        )
    });
}

const TEST_GCP_AUDIENCE: &str = "//iam.googleapis.com/projects/123456/locations/global/workloadIdentityPools/warp-pool/providers/warp-provider";

#[test]
fn warp_agent_cli_upgrade_link_is_channel_aware_and_user_bound() {
    let user_uid = UserUid::new("user-123");

    assert_eq!(
        UserWorkspaces::warp_agent_cli_upgrade_link(Some(user_uid)),
        format!(
            "{}{STRIPE_SUBSCRIPTION_INTERVAL_PAGE_PREFIX}/user/{user_uid}?source=warp-agent-cli",
            ChannelState::server_root_url(),
        )
    );
}

#[test]
fn warp_agent_cli_upgrade_link_uses_channel_aware_fallback_without_a_user() {
    assert_eq!(
        UserWorkspaces::warp_agent_cli_upgrade_link(None),
        format!(
            "{}{STRIPE_SUBSCRIPTION_INTERVAL_PAGE_PREFIX}?source=warp-agent-cli",
            ChannelState::server_root_url().trim_end_matches('/'),
        )
    );
}

#[derive(Default)]
struct TeamContextTestView;

impl Entity for TeamContextTestView {
    type Event = ();
}

impl View for TeamContextTestView {
    fn ui_name() -> &'static str {
        "TeamContextTestView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl TypedActionView for TeamContextTestView {
    type Action = ();
}

#[test]
fn link_sharing_fails_open_without_a_workspace() {
    App::test((), |mut app| async move {
        initialize_window_team_test_app(&mut app, vec![]);

        app.read(|ctx| {
            let user_workspaces = UserWorkspaces::as_ref(ctx);
            let scope = TeamlessScopeForTest;
            assert!(user_workspaces.is_anyone_with_link_sharing_enabled(&scope));
            assert!(user_workspaces.is_direct_link_sharing_enabled(&scope));
        });
    })
}

#[test]
fn test_team_switcher_hidden_with_zero_teams() {
    // When the user is in no workspace / no teams, `can_switch_teams` must return
    // false so the pill does not render.
    App::test((), |mut app| async move {
        initialize_window_team_test_app(&mut app, vec![]);
        app.read(|ctx| {
            assert!(
                !UserWorkspaces::as_ref(ctx).can_switch_teams(),
                "0 teams: switcher should be hidden"
            );
        });
    })
}

#[test]
fn test_purchase_addon_credits_forwards_teamless_team_uid() {
    App::test((), |mut app| async move {
        let mut workspace_client = MockWorkspaceClient::new();
        workspace_client
            .expect_purchase_addon_credits()
            .withf(|team_uid, credits| team_uid.is_none() && *credits == 1_000)
            .times(1)
            .returning(|_, _| {
                Ok(PurchaseAddonCreditsOutcome::CheckoutRequired {
                    checkout_url: "https://example.com/checkout".to_string(),
                })
            });

        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(
                Arc::new(MockTeamClient::new()),
                Arc::new(workspace_client),
                vec![],
                ctx,
            )
        });

        UserWorkspaces::handle(&app).update(&mut app, |user_workspaces, ctx| {
            user_workspaces.purchase_addon_credits(None, 1_000, ctx);
        });

        // Give the spawned client call time to run so the mock expectation is
        // exercised before the test ends.
        warpui::r#async::Timer::after(Duration::from_millis(100)).await;
    })
}

#[test]
fn test_purchase_addon_credits_forwards_team_uid_when_present() {
    App::test((), |mut app| async move {
        let mut workspace_client = MockWorkspaceClient::new();
        workspace_client
            .expect_purchase_addon_credits()
            .withf(|team_uid, credits| *team_uid == Some(123.into()) && *credits == 2_000)
            .times(1)
            .returning(|_, _| {
                Ok(PurchaseAddonCreditsOutcome::CheckoutRequired {
                    checkout_url: "https://example.com/checkout".to_string(),
                })
            });

        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(
                Arc::new(MockTeamClient::new()),
                Arc::new(workspace_client),
                vec![],
                ctx,
            )
        });

        UserWorkspaces::handle(&app).update(&mut app, |user_workspaces, ctx| {
            user_workspaces.purchase_addon_credits(Some(123.into()), 2_000, ctx);
        });

        // Give the spawned client call time to run so the mock expectation is
        // exercised before the test ends.
        warpui::r#async::Timer::after(Duration::from_millis(100)).await;
    })
}
