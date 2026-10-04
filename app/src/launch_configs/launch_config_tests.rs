
use crate::app_state::TabGroupSnapshot;
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








