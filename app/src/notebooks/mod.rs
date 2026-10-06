mod context_menu;
pub mod editor;
pub mod file;
pub mod link;
mod styles;

use leanterm_ui::AppContext;

/// Initialize notebooks-related keybindings.
pub fn init(app: &mut AppContext) {
    self::file::init(app);
    self::editor::view::init(app);
}

pub mod telemetry;
