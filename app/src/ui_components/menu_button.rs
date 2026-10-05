use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpui::elements::{
    ChildAnchor, ChildView, MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Stack,
};
use warpui::platform::Cursor;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{AppContext, Element, EventContext, View, ViewHandle};

use super::buttons::{highlight, icon_button};
use super::icons::Icon;
use crate::appearance::Appearance;
