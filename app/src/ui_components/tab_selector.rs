use warp_core::ui::appearance::Appearance;
use warpui::elements::{
    Border, Container, CrossAxisAlignment, Element, Empty, Fill, Flex, MouseStateHandle,
    ParentElement,
};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};

use crate::ui_components::blended_colors;

impl SettingsTab {
}

pub struct SettingsTab {
    pub label: String,
    pub mouse_state: MouseStateHandle,
}
