use std::time::Duration;

use regex::Regex;
use settings::{PrivatePreferences, PublicPreferences};
use warp_graphql::billing::{
    PurchaseAddOnCreditsPolicy as GqlPurchaseAddOnCreditsPolicy, Tier as GqlTier,
};
use warp_graphql::queries::get_workspaces_metadata_for_user::{
    User as GqlUser, UserProfile as GqlUserProfile, UserPurchasePolicyBillingMetadata,
    UserPurchasePolicyTier,
};
use warp_graphql::workspace::Workspace as GqlWorkspace;
use warpui::elements::Empty;
use warpui::platform::WindowStyle;
use warpui::{AddSingletonModel, App, Element, TypedActionView, View, ViewHandle, WindowId};
use warpui_extras::user_preferences;

use super::*;
use crate::auth::AuthManager;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::CloudObject;
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::server_api::ServerApiProvider;
use crate::server::server_api::team::{MockTeamClient, TeamClient};
use crate::server::sync_queue::SyncQueue;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings::{ CodeSettings,
};
use crate::system::SystemStats;
use crate::workspaces::team::{DiscoverableWorkspace, Team};
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::update_manager::TeamUpdateManager;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::Workspace;

#[derive(Default)]
struct CachedResources {
    workspaces: Vec<Workspace>,
}

fn initialize_app(
    app: &mut App,
    resources: CachedResources,
    team_client: Arc<dyn TeamClient>,
    workspace_client: Arc<dyn WorkspaceClient>,
) {
    initialize_app_with_auth(
        app,
        resources,
        team_client,
        workspace_client,
        AuthStateProvider::new_for_test(),
    );
}

fn initialize_app_with_auth(
    app: &mut App,
    resources: CachedResources,
    team_client: Arc<dyn TeamClient>,
    workspace_client: Arc<dyn WorkspaceClient>,
    auth_state_provider: AuthStateProvider,
) {
    // Add the necessary singleton models to the App
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(TeamTesterStatus::new);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(CloudModel::mock);
    app.add_singleton_model(|ctx| {
        UserWorkspaces::mock(
            team_client.clone(),
            workspace_client.clone(),
            resources.workspaces,
            ctx,
        )
    });
    app.add_singleton_model(|ctx| TeamUpdateManager::new(team_client.clone(), None, ctx));
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_| ServerApiProvider::new_for_test());
    app.add_singleton_model(|_| auth_state_provider);
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
    app.add_singleton_model(|_| {
        PublicPreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    app.add_singleton_model(|_| {
        PrivatePreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });

    app.add_singleton_model(CodeSettings::new_with_defaults);

    // The start of polling is normally triggered by authentication completion, but
    // we need to do it manually for tests.
    TeamTesterStatus::handle(app).update(app, |team_tester, ctx| {
        team_tester.initiate_data_pollers(false, ctx);
    });
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





/// Registers a fresh window on `team` and returns its id, so tests can build a
/// [`TeamScope`] via [`UserWorkspaces::team_context_for_window_for_test`].
fn window_on_team(app: &mut App, team: &Team) -> WindowId {
    let window_id = WindowId::new();
    UserWorkspaces::handle(app).update(app, |user_workspaces, ctx| {
        user_workspaces.set_team_for_window(window_id, team.uid, ctx);
    });
    window_id
}






const TEST_GCP_AUDIENCE: &str = "//iam.googleapis.com/projects/123456/locations/global/workloadIdentityPools/warp-pool/providers/warp-provider";
const TEST_GCP_SA_EMAIL: &str = "warp-geap@test-project.iam.gserviceaccount.com";













fn team_selection(team: Option<Option<String>>) -> warp_cli::scope::TeamSelection {
    warp_cli::scope::TeamSelection { team }
}

fn object_scope(team: Option<Option<String>>, personal: bool) -> warp_cli::scope::ObjectScope {
    warp_cli::scope::ObjectScope {
        team_selection: team_selection(team),
        personal,
    }
}










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

fn create_test_window(app: &mut App) -> (WindowId, ViewHandle<TeamContextTestView>) {
    app.add_window(WindowStyle::NotStealFocus, |_| TeamContextTestView)
}




fn set_team_remote_session_policy(team: &mut Team, allow_ai: bool, patterns: &[&str]) {
    team.settings
        .ai_permissions
        .allow_ai_in_remote_sessions
        .value = allow_ai;
    team.settings.ai_permissions.remote_session_regex_list = patterns
        .iter()
        .map(|pattern| Regex::new(pattern).expect("test pattern should compile"))
        .collect();
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




fn gql_tier(purchase_policy: Option<GqlPurchaseAddOnCreditsPolicy>) -> GqlTier {
    GqlTier {
        name: "Free".to_string(),
        description: "Free tier".to_string(),
        warp_ai_policy: None,
        team_size_policy: None,
        shared_notebooks_policy: None,
        shared_workflows_policy: None,
        session_sharing_policy: None,
        ai_autonomy_policy: None,
        telemetry_data_collection_policy: None,
        ugc_data_collection_policy: None,
        usage_based_pricing_policy: None,
        codebase_context_policy: None,
        byo_api_key_policy: None,
        byo_endpoint_policy: None,
        managed_byok_byoe_policy: None,
        purchase_add_on_credits_policy: purchase_policy,
        enterprise_pay_as_you_go_policy: None,
        enterprise_credits_auto_reload_policy: None,
        multi_admin_policy: None,
        native_workspaces_policy: None,
        ambient_agents_policy: None,
        usage_visibility_policy: None,
    }
}




fn apply_workspaces_metadata(app: &mut App, metadata: WorkspacesMetadataResponse) {
    UserWorkspaces::handle(app).update(app, |user_workspaces, ctx| {
        user_workspaces.on_workspaces_updated(
            Ok(WorkspacesMetadataWithPricing {
                metadata,
                pricing_info: None,
            }),
            ctx,
        );
    });
}

fn current_team_names(user_workspaces: &UserWorkspaces) -> Vec<String> {
    user_workspaces
        .current_workspace()
        .map(|workspace| {
            workspace
                .teams
                .iter()
                .map(|team| team.name.clone())
                .collect()
        })
        .unwrap_or_default()
}





fn gql_premium_purchase_policy() -> GqlPurchaseAddOnCreditsPolicy {
    GqlPurchaseAddOnCreditsPolicy {
        enabled: false,
        premium_enabled: true,
        price_premium_bps: 1000,
    }
}

fn gql_user(
    user_purchase_policy: Option<GqlPurchaseAddOnCreditsPolicy>,
    workspaces: Vec<GqlWorkspace>,
) -> GqlUser {
    GqlUser {
        profile: GqlUserProfile {
            uid: "test-user".to_string(),
        },
        ai_credit_availability: warp_graphql::ai::AICreditAvailability {
            available: true,
            denial_reason: warp_graphql::ai::AICreditAvailabilityDenialReason::None,
            credit_source: None,
        },
        billing_metadata: user_purchase_policy.map(|policy| UserPurchasePolicyBillingMetadata {
            tier: UserPurchasePolicyTier {
                purchase_add_on_credits_policy: Some(policy),
            },
        }),
        workspaces,
        experiments: None,
        discoverable_teams: vec![],
    }
}

fn discovery_options_for_test() -> DiscoveryOptions {
    DiscoveryOptions {
        workspaces: vec![DiscoverableWorkspace {
            workspace_uid: ServerId::from(10).into(),
            name: "Discoverable Workspace".to_string(),
            open_teams: vec![DiscoverableTeam {
                team_uid: ServerId::from(11).to_string(),
                num_members: 2,
                name: "Open Team".to_string(),
                team_accepting_invites: true,
            }],
            member_count: 4,
        }],
        legacy_teams: vec![DiscoverableTeam {
            team_uid: ServerId::from(12).to_string(),
            num_members: 3,
            name: "Legacy Team".to_string(),
            team_accepting_invites: true,
        }],
    }
}









