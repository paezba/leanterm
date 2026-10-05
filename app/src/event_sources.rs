
use serde::{Deserialize, Serialize};
use warp_core::interval_timer::TimingDataPoint;
pub use warp_terminal::ImageProtocol;

use crate::cloud_object::model::generic_string_model::GenericStringObjectId;
use crate::cloud_object::{GenericStringObjectFormat, Space};
use crate::drive::CloudObjectTypeAndId;
use crate::notebooks::{NotebookId, NotebookLocation};
use crate::search::command_search::searcher::CommandSearchItemAction;
use crate::server::ids::ServerId;
use crate::terminal::model::session::SessionId;
use crate::workflows::{WorkflowId, WorkflowSelectionSource, WorkflowSource};

#[derive(Clone, Serialize, Deserialize)]
pub struct BootstrappingInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// `true` if the bootstrapping process was triggered by an RC file snippet.
    ///
    /// This should only be true if `is_subshell` is true.
    pub was_triggered_by_rc_file: bool,
    /// The total time it took to bootstrap the shell, in seconds.
    pub bootstrap_duration_seconds: Option<f64>,
    /// The time it took to source the user's rcfiles, in seconds.  May be None
    /// if we weren't able to get that information from the shell.
    pub rcfiles_duration_seconds: Option<f64>,
    /// The difference between the total bootstrap time and the rcfile sourcing
    /// time, which roughly equals the time cost of running our bootstrap
    /// script.  Will be None if `bootstrap_duration_seconds` or
    /// `rcfiles_duration_seconds` is None.
    pub warp_attributed_bootstrap_duration_seconds: Option<f64>,
    pub shell_version: Option<String>,
    pub terminal_session_id: Option<SessionId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SlowBootstrapInfo {
    pub shell: &'static str,
    pub is_ssh: bool,
    pub is_subshell: bool,
    pub is_wsl: bool,
    pub is_msys2: bool,
    /// Contents of the bootstrap block when the slow bootstrap was detected.
    /// This includes both command and output content from the block.
    pub bootstrap_block_contents: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppStartupInfo {
    pub is_session_restoration_on: bool,
    /// Whether or not a screen reader is enabled at the time the app is
    /// launched.  Should be set to None if we do not know for sure.
    pub is_screen_reader_enabled: Option<bool>,
    pub from_relaunch: bool,
    pub is_crash_reporting_enabled: bool,
    pub timing_data: Vec<TimingDataPoint>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum DownloadSource {
    Website,
    Homebrew,
}

// For use when recording what type of cloud object a particular telemetry is for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryCloudObjectType {
    Workflow,
    Notebook,
    Folder,
    GenericStringObject(GenericStringObjectFormat),
}

impl From<&CloudObjectTypeAndId> for TelemetryCloudObjectType {
    fn from(cloud_object_type_and_id: &CloudObjectTypeAndId) -> Self {
        match cloud_object_type_and_id {
            CloudObjectTypeAndId::Notebook(_) => Self::Notebook,
            CloudObjectTypeAndId::Workflow(_) => Self::Workflow,
            CloudObjectTypeAndId::Folder(_) => Self::Folder,
            CloudObjectTypeAndId::GenericStringObject { object_type, .. } => {
                Self::GenericStringObject(*object_type)
            }
        }
    }
}

/// For use when recording how a user has access to a cloud object.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TelemetrySpace {
    /// The object is owned by the current user.
    Personal,
    /// The object is owned by a team the user is on.
    Team,
    /// The object was shared with the user.
    Shared,
}

impl From<Space> for TelemetrySpace {
    fn from(space: Space) -> Self {
        match space {
            Space::Personal => Self::Personal,
            Space::Team { .. } => Self::Team,
            Space::Shared => Self::Shared,
        }
    }
}

/// Common metadata to include in all Warp Drive telemetry events that act on a specific object.
/// Events that only apply to a single object type may use specific metadata like [`WorkflowTelemetryMetadata`],
/// [`NotebookTelemetryMetadata`], or [`EnvVarTelemetryMetadata`] instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudObjectTelemetryMetadata {
    pub object_type: TelemetryCloudObjectType,
    /// The server UID of the object. This only exists for objects that have been synced to the
    /// server.
    pub object_uid: Option<ServerId>,
    /// The space through which the user has access to the object.
    pub space: Option<TelemetrySpace>,
    /// If the object is owned by a team, this is the owning team's UID. For shared objects, the
    /// user might not be on the team.
    pub team_uid: Option<ServerId>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkflowTelemetryMetadata {
    pub workflow_categories: Option<Vec<String>>,
    pub workflow_source: WorkflowSource,
    pub workflow_space: Option<TelemetrySpace>,
    pub workflow_selection_source: WorkflowSelectionSource,
    // This field is only populated for cloud workflows that have been synced to the server
    pub workflow_id: Option<WorkflowId>,
    // Any referenced workflow enums that have been synced to the cloud
    pub enum_ids: Vec<GenericStringObjectId>,
}

/// Metadata to include in all notebook telemetry events.
///
/// There are 4 expected configurations:
/// * Personal cloud notebooks: `notebook_id` is `Some`, `team_uid` is `None`, and location is `PersonalCloud`
/// * Team cloud notebooks: `notebook_id` is `Some`, `team_uid` is `Some`, and location is `Team`
/// * Local file-based notebooks: `notebook_id` and `team_uid` are `None`, and location is `LocalFile`
/// * Remote file-based notebooks: `notebook_id` and `team_uid` are `None`, and location is `RemoteFile`
///
/// This representation allows for invalid combinations, but makes querying the data easier (for
/// example, to find all notebook events for a given team).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NotebookTelemetryMetadata {
    /// The notebook ID, only available for cloud notebooks that have been synced to the server.
    pub notebook_id: Option<NotebookId>,
    /// The team UID, only available for cloud notebooks in a shared team.
    pub team_uid: Option<ServerId>,
    pub space: Option<TelemetrySpace>,
    /// Where the notebook is canonically located.
    pub location: NotebookLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_table_count: Option<usize>,
}

impl NotebookTelemetryMetadata {
    pub fn new(
        notebook_id: impl Into<Option<NotebookId>>,
        team_uid: impl Into<Option<ServerId>>,
        location: impl Into<NotebookLocation>,
        space: Option<TelemetrySpace>,
    ) -> Self {
        Self {
            notebook_id: notebook_id.into(),
            team_uid: team_uid.into(),
            location: location.into(),
            space,
            markdown_table_count: None,
        }
    }

    pub fn with_markdown_table_count(mut self, markdown_table_count: usize) -> Self {
        self.markdown_table_count = Some(markdown_table_count);
        self
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvVarTelemetryMetadata {
    /// The object ID, only available for cloud env vars that have been synced to the server.
    pub object_id: Option<GenericStringObjectId>,
    /// The team UID, only available for cloud env vars in a shared team.
    pub team_uid: Option<ServerId>,
    pub space: TelemetrySpace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenedSharingDialogEvent {
    pub source: SharingDialogSource,

    /// Metadata for the object being shared, if it's a Warp Drive object.
    #[serde(flatten)]
    pub object_metadata: Option<CloudObjectTelemetryMetadata>,
}

/// How the user opened the Warp Drive sharing dialog.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SharingDialogSource {
    /// The sharing button in the pane header.
    PaneHeader,
    /// The per-pane command palette entry (includes keybindings).
    CommandPalette,
    /// The Warp Drive index context menu.
    DriveIndex,
    /// The sharing dialog was auto-opened from shared session creation.
    StartedSessionShare,
    /// The user intented into Warp with an email address to invite.
    InviteeRequest,
    /// The user jumped from an inherited ACL to its definition on a parent object.
    InheritedPermission,
    /// The onboarding block shown after users create new personal objects.
    OnboardingBlock,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum TabRenameEvent {
    OpenedEditor,
    CustomNameSet,
    CustomNameCleared,
}

/// The possible sources notifications can turned on from.
#[derive(Clone, Serialize, Deserialize)]
pub enum NotificationsTurnedOnSource {
    Settings,
    Banner,
}

/// The possible types of toggles in the find bar
#[derive(Clone, Serialize, Deserialize)]
pub enum FindOption {
    CaseSensitive,
    FindInBlock,
    Regex,
}

/// The possible ways to trigger command x-ray
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandXRayTrigger {
    Hover,
    Keystroke,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum PaletteSource {
    PrefixChange,
    Keybinding,
    CtrlTab { shift_pressed_initially: bool },
    WarpDrive,
    QuitModal,
    LogOutModal,
    IntegrationTest,
    ContextChip,
    PaneHeader,
    TitleBarSearchBar,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum FileTreeSource {
    /// Opened from the pane header toolbelt button.
    PaneHeader,
    Keybinding,
    LeftPanelToolbelt,
    ForceOpened,
}

#[cfg(feature = "local_fs")]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodePanelsFileOpenEntrypoint {
    CodeReview,
    ProjectExplorer,
    GlobalSearch,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WarpDriveSource {
    Legacy,
    LeftPanelToolbelt,
    ForceOpened,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CommandSearchResultType {
    History,
    Workflow,
    EnvVarCollection,
    ViewInWarpDrive,
    Project,
}

impl From<&CommandSearchItemAction> for CommandSearchResultType {
    fn from(action: &CommandSearchItemAction) -> Self {
        use crate::search::command_search::searcher::CommandSearchItemAction::*;
        match action {
            AcceptHistory(_) | ExecuteHistory(_) => Self::History,
            AcceptWorkflow(_) => Self::Workflow,
            AcceptEnvVarCollection(_) => Self::EnvVarCollection,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CloseTarget {
    App,
    Window,
    Tab,
    Pane,
    EditorTab,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum PtySpawnMode {
    /// The pty was spawned using the terminal server.
    TerminalServer,
    /// We tried to spawn the pty using the terminal server, but something went
    /// wrong so we fell back to spawning it directly.
    FallbackToDirect,
    /// The terminal server is not in use, and we spawned the pty directly
    /// (in tests, for example).
    Direct,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SaveAsWorkflowModalSource {
    Block,
    Input,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum LaunchConfigUiLocation {
    CommandPalette,
    AppMenu,
    TabMenu,
    Uri,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SecretInteraction {
    RevealSecret,
    HideSecret,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AnonymousUserSignupEntrypoint {
    HitDriveObjectLimit,
    LoginGatedFeature,
    SignUpButton,
    RenotificationBlock,
    NextCommandSuggestionsUpgradeBanner,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum UndoCloseItemType {
    Window,
    Tab,
    Pane,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PromptChoice {
    PS1,
    Default,
    Custom { builtin_chips: Vec<String> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ToggleBlockFilterSource {
    /// This includes the keybinding and the command palette items.
    Binding,
    ContextMenu,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TierLimitHitEvent {
    pub team_uid: ServerId,
    pub feature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuUsageStats {
    /// The number of logical CPUs on the system.
    pub num_cpus: usize,

    /// The maximum CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `max_usage / num_cpus * 100`.
    pub max_usage: f32,

    /// The average CPU usage over the measurement interval.
    ///
    /// This number is in the range [0, num_cpus].  The CPU utilization, as a
    /// percentage, can be determined via `avg_usage / num_cpus * 100`.
    pub avg_usage: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryUsageStats {
    pub total_application_usage_bytes: usize,
    pub total_blocks: usize,
    pub total_lines: usize,

    /// Statistics about blocks that have been seen in the past 5 minutes.
    pub active_block_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [5m, 1h).
    pub inactive_5m_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [1h, 24h).
    pub inactive_1h_stats: BlockMemoryUsageStats,
    /// Statistics about blocks that haven't been seen since [24h, ..).
    pub inactive_24h_stats: BlockMemoryUsageStats,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockMemoryUsageStats {
    pub num_blocks: usize,
    pub num_lines: usize,
    pub estimated_memory_usage_bytes: usize,
}

/// How the user triggered the [`AddTabWithShell`] event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AddTabWithShellSource {
    CommandPalette,
    ShellSelectorMenu,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginEventSource {
    OnboardingSlide,
    AuthModal,
}
