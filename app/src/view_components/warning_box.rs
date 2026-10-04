//! A reusable warning callout component with optional action button.
use markdown_parser::{FormattedText, FormattedTextInline, FormattedTextLine};
use warp_core::ui::color::blend::Blend;
use warpui::EventContext;
use warpui::color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Element, Expanded, Flex,
    FormattedTextElement, Hoverable, HyperlinkLens, MainAxisSize, MouseStateHandle, ParentElement,
    Radius, Text,
};
use warpui::platform::Cursor;

use crate::appearance::Appearance;
use crate::themes::theme::Fill as ThemeFill;
use crate::ui_components::icons::Icon;

impl WarningBoxButtonConfig {
}

impl WarningBoxConfig {

}

pub struct WarningBoxButtonConfig {
    pub label: String,
    pub mouse_state: MouseStateHandle,
    pub on_click: Box<dyn Fn(&mut EventContext) + 'static>,
}

pub struct WarningBoxConfig {
    pub icon: Icon,
    pub title: WarningBoxTitle,
    pub description: Option<String>,

    /// Optional max width. If provided, the WarningBox will not exceed this width,
    /// but can shrink on smaller screens.
    pub width: Option<f32>,

    pub margin_top: f32,

    pub button: Option<WarningBoxButtonConfig>,
}

pub enum WarningBoxTitle {
    Text(String),
    Formatted(FormattedTextInline),
}
