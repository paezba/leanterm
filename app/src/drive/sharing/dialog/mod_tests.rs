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
