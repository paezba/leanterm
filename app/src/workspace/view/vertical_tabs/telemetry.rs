use serde_json::{Value, json};

use crate::workspace::tab_settings::{
    VerticalTabsCompactSubtitle, VerticalTabsDisplayGranularity, VerticalTabsPrimaryInfo,
    VerticalTabsTabItemMode, VerticalTabsViewMode,
};

/// Where in the vertical tabs UI a clickable diff-stats or GitHub PR chip
/// was rendered when the user clicked it.
#[derive(Clone, Copy, Debug)]
pub enum VerticalTabsChipEntrypoint {
    /// The chip was rendered on a row representing a single pane
    /// (display granularity: Panes).
    Pane,
    /// The chip was rendered on a row representing a tab group
    /// (display granularity: Tabs).
    Tab,
    /// The chip was rendered inside the detail sidecar that appears on row hover.
    DetailsSidecar,
}


