//! Leanterm UI Components module contains functions and structs that implement our internal components
//! used for the apps design (our buttons with styling, headers and panels etc.) as well definition
//! of colors (aka blended colors from the figma designs derived from Leanterm theme) and icons used
//! within the app.
pub(crate) mod avatar;
pub(crate) mod blended_colors;
pub mod buttons;
pub(crate) mod code_block;
pub(crate) mod color_dot;
pub(crate) mod dialog;
pub(crate) mod icon_with_status;
pub(crate) mod inline_action_header;
pub(crate) mod inline_action_icons;
pub(crate) mod item_highlight;
pub mod json_tree;
pub(crate) mod keyboard_navigable_buttons;
pub(crate) mod menu_button;
pub(crate) mod red_notification_dot;
pub(crate) mod render_file_search_row;
pub mod tab_selector;
pub(crate) mod window_focus_dimming;

pub use leanterm_core::ui::icons;

const BORDER_RADIUS: f32 = 4.;
