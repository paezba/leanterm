use session_sharing_protocol::common::SessionId;
use warpui::{App, SingletonEntity, TypedActionView, ViewHandle};

use super::{SharingDialog, SharingDialogAction};
use crate::auth::UserUid;
use crate::cloud_object::Owner;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::model::view::CloudViewModel;
use crate::drive::sharing::SharingAccessLevel;
use crate::server::ids::{ClientId, ServerId, SyncId};
use crate::terminal::TerminalView;
use crate::terminal::shared_session::manager::Manager;
use crate::terminal::shared_session::SharedSessionStatus;
use crate::test_util::terminal::{ initialize_app_for_terminal_view,
};
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspaces::team::{Team, TeamVisibility};
use crate::workspaces::user_profiles::UserProfiles;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::{
    EnforceableSetting, TeamLinkSharingSettings, TeamSettings, Workspace, WorkspaceSettings,
};

fn set_shared_session_status(
    terminal: &ViewHandle<TerminalView>,
    status: SharedSessionStatus,
    app: &mut App,
) {
    terminal.update(app, |view, _| {
        view.model.lock().set_shared_session_status(status);
    });
}

fn assert_session_link_state(
    terminal: &ViewHandle<TerminalView>,
    dialog: &ViewHandle<SharingDialog>,
    expected_session_id: Option<SessionId>,
    app: &App,
) {
    terminal.read(app, |view, ctx| {
        let shared_session_status = view.model.lock().shared_session_status().clone();
        let manager = Manager::as_ref(ctx);
        assert_eq!(
            manager.session_id_for_link(&view.id(), &shared_session_status),
            expected_session_id
        );
        assert_eq!(
            manager.has_session_link(&view.id(), &shared_session_status),
            expected_session_id.is_some()
        );
    });

    dialog.read(app, |dialog, ctx| {
        assert_eq!(
            dialog.has_shared_session_link(ctx),
            expected_session_id.is_some()
        );
    });
}


fn team_with_link_sharing(uid: i64, name: &str, permitted: bool) -> Team {
    let permission = EnforceableSetting {
        value: permitted,
        is_enforced_by_workspace: false,
    };
    Team {
        uid: uid.into(),
        name: name.to_string(),
        color: None,
        invite_link: None,
        members: vec![],
        pending_email_invites: vec![],
        invite_link_domain_restrictions: vec![],
        billing_metadata: Default::default(),
        stripe_customer_id: None,
        settings: TeamSettings {
            link_sharing: TeamLinkSharingSettings {
                anyone_with_link_sharing_enabled: permission.clone(),
                direct_link_sharing_enabled: permission,
            },
            ..Default::default()
        },
        is_eligible_for_discovery: false,
        has_billing_history: false,
        visibility: TeamVisibility::Open,
    }
}

fn install_workspace_with_teams(app: &mut App, teams: Vec<Team>) {
    let workspace = Workspace {
        uid: "workspace_uid123456789".to_string().into(),
        name: "test".to_string(),
        stripe_customer_id: None,
        teams,
        open_teams: vec![],
        billing_metadata: Default::default(),
        bonus_grants_purchased_this_month: Default::default(),
        billing_cycle_usage: None,
        has_billing_history: false,
        settings: WorkspaceSettings::default(),
        invite_link_domain_restrictions: vec![],
        pending_email_invites: vec![],
        is_eligible_for_discovery: false,
        members: vec![],
        total_requests_used_since_last_refresh: 0,
    };
    let workspace_uid = workspace.uid;

    let user_workspaces = UserWorkspaces::handle(&*app);
    user_workspaces.update(app, |user_workspaces, ctx| {
        user_workspaces.update_workspaces(vec![workspace], ctx);
        user_workspaces.set_current_workspace_uid(workspace_uid, ctx);
    });
}


fn initialize_app_for_drive_object_dialog(app: &mut App) {
    initialize_app_for_terminal_view(app);
    app.add_singleton_model(CloudViewModel::mock);
    app.add_singleton_model(|_| UserProfiles::new(Vec::new()));
}

fn add_shareable_object(app: &mut App) -> ServerId {
    let object_uid: ServerId = 789.into();
    let mut object = CloudWorkflow::new_local(
        CloudWorkflowModel {
            data: Workflow::new("shared workflow", "echo shared"),
        },
        Owner::User {
            user_uid: UserUid::new("owner"),
        },
        None,
        ClientId::default(),
    );
    object.id = SyncId::ServerId(object_uid);

    let cloud_model = CloudModel::handle(&*app);
    cloud_model.update(app, |cloud_model, _| {
        cloud_model.add_object(object.id, object);
    });
    object_uid
}

fn permissions_change_reached_object(app: &App, object_uid: ServerId) -> bool {
    app.read(|ctx| {
        CloudModel::as_ref(ctx)
            .get_by_uid(&object_uid.uid())
            .expect("the targeted object should be in the cloud model")
            .metadata()
            .pending_changes_statuses
            .has_pending_permissions_change
    })
}

fn set_link_permissions(
    dialog: &ViewHandle<SharingDialog>,
    access_level: Option<SharingAccessLevel>,
    app: &mut App,
) {
    dialog.update(app, |dialog, ctx| {
        dialog.handle_action(&SharingDialogAction::SetLinkPermissions(access_level), ctx);
    });
}




