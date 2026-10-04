use ai::LLMId;
use onboarding::{
    AgentOnboardingView, OfferVariant, OnboardingAuthState, OnboardingIntention, SelectedSettings,
    UICustomizationSettings,
};
use session_sharing_protocol::common::SessionId;
use warp_core::features::FeatureFlag;
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::elements::Empty;
use warpui::platform::WindowStyle;
use warpui::{
    App, AppContext, Element, Entity, EntityId, SingletonEntity, TypedActionView, View, ViewHandle,
};

use super::{ AuthOnboardingState, AuthOnboardingTarget,
    HAS_COMPLETED_ONBOARDING_KEY, NewWorkspaceSource, RootView, WorkspaceArgs,
    has_completed_local_onboarding,
};
use crate::GlobalResourceHandles;
use crate::appearance::Appearance;
use crate::auth::AuthStateProvider;
use crate::auth::auth_manager::AuthManager;
use crate::auth::login_slide::{LoginSlideSource, LoginSlideView};
use crate::server::server_api::ServerApiProvider;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::test_util::settings::initialize_settings_for_tests;
use crate::themes::onboarding_theme_picker_themes;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::FtueAccountClass;

fn initialize_app(app: &mut App) {
    app.update(crate::settings::init_and_register_user_preferences);
    app.add_singleton_model(|_ctx| ServerApiProvider::new_for_test());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
}









/// If the user hasn't completed local onboarding, the helper must leave the
/// server-side flag untouched — onboarding hasn't actually happened yet.
#[test]
fn test_sync_noop_when_local_onboarding_not_completed() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        // Do not set HAS_COMPLETED_ONBOARDING_KEY; it defaults to false.
        app.update(|ctx| {
            AuthStateProvider::as_ref(ctx).get().set_is_onboarded(false);
        });

        app.update(|ctx| {
            let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
            RootView::sync_local_onboarding_to_server(&auth_state, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                AuthStateProvider::as_ref(ctx).get().is_onboarded(),
                Some(false),
                "sync should not have changed is_onboarded when local onboarding is incomplete"
            );
        });
    });
}


struct SsoLinkTestHarnessView {
    login_slide_view: ViewHandle<LoginSlideView>,
    onboarding_view: ViewHandle<AgentOnboardingView>,
}

impl Entity for SsoLinkTestHarnessView {
    type Event = ();
}

impl View for SsoLinkTestHarnessView {
    fn ui_name() -> &'static str {
        "SsoLinkTestHarnessView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl TypedActionView for SsoLinkTestHarnessView {
    type Action = ();
}




fn assert_pending_workspace_retargeted(
    target: &AuthOnboardingTarget,
    session_id: SessionId,
    case: &str,
) {
    let AuthOnboardingTarget::Workspace(args) = target else {
        panic!("{case}: expected a pending workspace target, found an existing terminal");
    };
    assert!(
        matches!(
            args.workspace_setting,
            NewWorkspaceSource::SharedSessionAsViewer { session_id: id } if id == session_id
        ),
        "{case}: should retarget to the requested session"
    );
}

fn pending_workspace_args(app: &mut App) -> Box<WorkspaceArgs> {
    Box::new(WorkspaceArgs {
        global_resource_handles: GlobalResourceHandles::mock(app),
        server_time: None,
        workspace_setting: NewWorkspaceSource::Empty {
            previous_active_window: None,
            shell: None,
        },
    })
}

fn root_view_for_join_test(app: &mut App) -> ViewHandle<RootView> {
    crate::workspace::view::tests::initialize_app(app);
    let global_resource_handles = GlobalResourceHandles::mock(app);
    let (_, root_view) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        RootView::new(
            global_resource_handles,
            NewWorkspaceSource::Empty {
                previous_active_window: None,
                shell: None,
            },
            ctx,
        )
    });
    root_view
}

fn act_join_shared_session(
    app: &mut App,
    root_view: &ViewHandle<RootView>,
    session_id: SessionId,
    case: &str,
) {
    let handled = root_view.update(app, |root_view, ctx| {
        root_view.join_shared_session_in_existing_window(&session_id, ctx)
    });
    assert!(handled, "{case}: expected the link to be handled");
}




