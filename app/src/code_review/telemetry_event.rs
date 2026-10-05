use std::fmt::Display;

use serde::{Serialize, Serializer};
use serde_with::SerializeDisplay;

/// Terminal status of a `GitDialog`. Captures both async-op outcomes and
/// pre-confirmation user cancels in a single enum.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum GitDialogStatus {
    /// User confirmed the dialog and the underlying git operation succeeded.
    #[serde(rename = "succeeded")]
    Succeeded,
    /// User confirmed the dialog and the underlying git operation failed.
    #[serde(rename = "failed")]
    Failed,
    /// User cancelled the dialog (ESC / close button / cancel button) before
    /// the async op ran.
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Entry points for opening the code review pane.
#[derive(Clone, Copy, Debug, SerializeDisplay, Default)]
pub enum CodeReviewPaneEntrypoint {
    /// Opened via the git diff chip (git changes button in AI control panel).
    GitDiffChip,
    // Opened via the pane header
    PaneHeader,
    // Opened via the code mode v2 right panel button
    RightPanel,
    /// Opened via other means (unknown entry point).
    #[default]
    Other,
}

impl Display for CodeReviewPaneEntrypoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitDiffChip => write!(f, "git_diff_chip"),
            Self::PaneHeader => write!(f, "pane_header"),
            Self::RightPanel => write!(f, "right_panel"),
            Self::Other => write!(f, "other"),
        }
    }
}
