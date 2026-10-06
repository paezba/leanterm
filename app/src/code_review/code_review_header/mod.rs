mod header_revamp;

use leanterm_ui::elements::{ChildView, ConstrainedBox, Container};
use leanterm_ui::ui_components::components::Coords;
use leanterm_ui::{Element, ViewHandle};

use crate::appearance::Appearance;
use crate::view_components::action_button::ActionButton;

// This is a best effort guess of the size of all of the elements in the header to know when we should start to wrap to the second row

pub(crate) const HEADER_BUTTON_PADDING: Coords = Coords {
    top: 2.,
    bottom: 2.,
    left: 6.,
    right: 6.,
};

pub struct CodeReviewHeader {}

impl CodeReviewHeader {
    pub fn new() -> Self {
        Self {}
    }

    pub(super) fn render_maximize_pane_button(
        &self,
        maximize_button: &ViewHandle<ActionButton>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        Container::new(
            ConstrainedBox::new(ChildView::new(maximize_button).finish())
                .with_height(appearance.ui_font_size() + 10.)
                .with_width(appearance.ui_font_size() + 10.)
                .finish(),
        )
        .with_margin_left(8.)
        .with_margin_right(6.)
        .finish()
    }
}
