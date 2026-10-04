use std::path::PathBuf;

use super::{CommandTemplate, LaunchConfig, PaneTemplateType, TabTemplate};
use crate::app_state::{
    AppState, BranchSnapshot, LeafContents, LeafSnapshot, NotebookPaneSnapshot, PaneFlex,
    PaneNodeSnapshot, SplitDirection, TabGroupSnapshot, TabSnapshot, TerminalPaneSnapshot,
    WindowSnapshot,
};
use crate::drive::OpenWarpDriveObjectSettings;
use crate::tab::SelectedTabColor;
use crate::themes::theme::AnsiColorIdentifier;
use crate::workspace::tab_group::TabGroupId;












// ---------------------------------------------------------------------------
// Tab groups (#13898)
// ---------------------------------------------------------------------------




fn group(name: &str, id: TabGroupId) -> TabGroupSnapshot {
    TabGroupSnapshot {
        id,
        name: Some(name.to_string()),
        color: SelectedTabColor::Color(AnsiColorIdentifier::Blue),
        collapsed: false,
        pinned: false,
    }
}








