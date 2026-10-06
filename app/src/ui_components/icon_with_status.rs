use leanterm_core::ui::icons::Icon as LeantermIcon;
use leanterm_core::ui::theme::color::internal_colors;
use leanterm_core::ui::theme::{Fill as LeantermThemeFill, LeantermTheme};
use leanterm_ui::elements::{ConstrainedBox, Container, CornerRadius, Element, Radius};

/// The inner glyph occupies `NEUTRAL_GLYPH_RATIO * total_size`, matching the old sizing where a
/// 24px container held a 16px glyph (16/24 ≈ 0.667).
const NEUTRAL_GLYPH_RATIO: f32 = 16.0 / 24.0;

/// What to render inside the circle.
pub(crate) enum IconWithStatusVariant {
    /// A generic icon with a given color on an overlay background.
    Neutral {
        icon: LeantermIcon,
        icon_color: LeantermThemeFill,
    },
    /// A pre-built icon element on an overlay background.
    NeutralElement { icon_element: Box<dyn Element> },
}

/// Renders a pane icon in a circle that fills the `total_size` bounding box.
pub(crate) fn render_icon_with_status(
    variant: IconWithStatusVariant,
    total_size: f32,
    theme: &LeantermTheme,
) -> Box<dyn Element> {
    let icon_element = match variant {
        IconWithStatusVariant::Neutral { icon, icon_color } => {
            icon.to_leanterm_ui_icon(icon_color).finish()
        }
        IconWithStatusVariant::NeutralElement { icon_element } => icon_element,
    };
    let glyph = total_size * NEUTRAL_GLYPH_RATIO;
    let padding = (total_size - glyph) / 2.;
    let inner = ConstrainedBox::new(icon_element)
        .with_width(glyph)
        .with_height(glyph)
        .finish();
    Container::new(inner)
        .with_uniform_padding(padding)
        .with_background(internal_colors::fg_overlay_2(theme))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(total_size / 2.)))
        .finish()
}
