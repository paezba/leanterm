use leanterm_ui::Element;
use leanterm_ui::elements::{
    Align, Container, CrossAxisAlignment, Flex, MouseStateHandle, ParentElement, Shrinkable,
};
use leanterm_ui::fonts::Weight;
use leanterm_ui::keymap::Keystroke;
use leanterm_ui::ui_components::button::ButtonVariant;
use leanterm_ui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use pathfinder_color::ColorU;

use super::render_block_banner;
use crate::appearance::Appearance;
use crate::terminal::view::{RememberForLeantermification, TerminalAction};
use crate::themes::theme::Fill;
use crate::ui_components::blended_colors;

const CLOSE_BUTTON_DIAMETER: f32 = 20.0;
const STANDARD_PADDING: f32 = 8.0;

pub struct LeantermifyBannerState {
    /// The subshell command that triggered the banner.
    pub command: String,
    pub height: f32,
    pub accept_button_mouse_state: MouseStateHandle,
    pub dont_ask_button_mouse_state: MouseStateHandle,
    pub dismiss_button_mouse_state: MouseStateHandle,

    /// This keybinding gets rendered in the Leantermification banner, but we can't look it up
    /// during render as a &mut AppContext is not available then. This needs to get
    /// looked up during action handling and cached here.
    pub initialize_leantermify_keybinding: Option<Keystroke>,
    pub hover_state: MouseStateHandle,
}

impl LeantermifyBannerState {
    pub fn new(command: String, initialize_leantermify_keybinding: Option<Keystroke>) -> Self {
        Self {
            command,
            height: 0.0,
            initialize_leantermify_keybinding,
            accept_button_mouse_state: Default::default(),
            dont_ask_button_mouse_state: Default::default(),
            dismiss_button_mouse_state: Default::default(),
            hover_state: Default::default(),
        }
    }

    pub fn title(&self) -> &str {
        "Leantermify subshell"
    }

    pub fn action(&self) -> TerminalAction {
        TerminalAction::TriggerSubshellBootstrap
    }

    fn remember_for_leantermification(
        &self,
        should_remember: bool,
    ) -> RememberForLeantermification {
        if should_remember {
            RememberForLeantermification::RememberSubshellCommand(self.command.to_owned())
        } else {
            RememberForLeantermification::DoNotRememberSubshellCommand
        }
    }
}

/// This banner is shown when the user runs a command which is recognized as a subshell-compatible
/// command. It asks if they want to bootstrap a subshell and, if so, whether we should ask again
/// next time they run the same command.
pub fn render_leantermification_banner(
    state: &LeantermifyBannerState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let yes_button = render_yes_button(
        state,
        &state.initialize_leantermify_keybinding,
        &state.accept_button_mouse_state,
        appearance,
    );

    let remember = state.remember_for_leantermification(true);
    let dont_ask_button = Container::new(
        appearance
            .ui_builder()
            .button(
                ButtonVariant::Text,
                state.dont_ask_button_mouse_state.clone(),
            )
            .with_text_label("Do not show again".to_owned())
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(TerminalAction::DismissLeantermifyBanner(
                    remember.to_owned(),
                ));
            })
            .finish(),
    )
    .with_margin_right(16.)
    .finish();

    let do_not_remember = state.remember_for_leantermification(false);
    let close_button = appearance
        .ui_builder()
        .close_button(
            CLOSE_BUTTON_DIAMETER,
            state.dismiss_button_mouse_state.clone(),
        )
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(TerminalAction::DismissLeantermifyBanner(
                do_not_remember.to_owned(),
            ));
        })
        .finish();

    let col = Flex::column()
        .with_child(
            Flex::row()
                .with_child(Align::new(yes_button).finish())
                .with_child(
                    Shrinkable::new(1., Align::new(dont_ask_button).right().finish()).finish(),
                )
                .with_child(Align::new(close_button).finish())
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .finish(),
        )
        .with_cross_axis_alignment(CrossAxisAlignment::Start);

    render_block_banner(
        |_hover_state| col.finish(),
        state.hover_state.clone(),
        appearance.theme(),
    )
}

fn render_yes_button(
    state: &LeantermifyBannerState,
    initialize_leantermification_keybinding: &Option<Keystroke>,
    mouse_state: &MouseStateHandle,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let yes_button = match initialize_leantermification_keybinding {
        Some(keystroke) => appearance
            .ui_builder()
            .keyboard_shortcut_button(state.title().to_owned(), keystroke, mouse_state.clone())
            .with_style(UiComponentStyles {
                height: Some(36.),
                padding: Some(Coords {
                    top: 0.,
                    bottom: 0.,
                    left: STANDARD_PADDING,
                    right: STANDARD_PADDING,
                }),
                ..Default::default()
            }),
        None => appearance
            .ui_builder()
            .button(ButtonVariant::Basic, mouse_state.clone())
            .with_text_label(state.title().to_owned())
            .with_style(UiComponentStyles {
                background: Some(Fill::Solid(ColorU::transparent_black()).into()),
                height: Some(36.),
                font_size: Some(appearance.ui_font_size() + 2.),
                font_weight: Some(Weight::Bold),
                font_color: Some(blended_colors::text_main(
                    appearance.theme(),
                    appearance.theme().background(),
                )),
                border_color: Some(appearance.theme().surface_3().into()),
                border_width: Some(1.),
                padding: Some(Coords::uniform(STANDARD_PADDING)),
                ..Default::default()
            })
            .with_hovered_styles(UiComponentStyles {
                background: Some(appearance.theme().surface_3().into()),
                border_color: Some(blended_colors::accent(appearance.theme()).into()),
                ..Default::default()
            }),
    };
    let action = state.action();
    yes_button
        .build()
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.to_owned()))
        .finish()
}
