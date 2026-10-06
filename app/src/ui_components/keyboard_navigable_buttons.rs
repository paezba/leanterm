use leanterm_ui::elements::{Container, CrossAxisAlignment, Flex, ParentElement};
use leanterm_ui::keymap::FixedBinding;
use leanterm_ui::ui_components::button::Button;
use leanterm_ui::ui_components::components::UiComponent;
use leanterm_ui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

const MARGIN_BETWEEN_BUTTONS: f32 = 4.;
const HAS_OPTIONS: &str = "HasOptions";

pub fn init(app: &mut AppContext) {
    use leanterm_ui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "enter",
            KeyboardNavigableButtonsAction::Enter,
            id!(KeyboardNavigableButtons::ui_name()) & id!(HAS_OPTIONS),
        ),
        FixedBinding::new(
            "numpadenter",
            KeyboardNavigableButtonsAction::Enter,
            id!(KeyboardNavigableButtons::ui_name()) & id!(HAS_OPTIONS),
        ),
        FixedBinding::new(
            "up",
            KeyboardNavigableButtonsAction::ArrowUp,
            id!(KeyboardNavigableButtons::ui_name()) & id!(HAS_OPTIONS),
        ),
        FixedBinding::new(
            "down",
            KeyboardNavigableButtonsAction::ArrowDown,
            id!(KeyboardNavigableButtons::ui_name()) & id!(HAS_OPTIONS),
        ),
    ]);
}

#[derive(Debug, Clone)]
pub enum KeyboardNavigableButtonsAction {
    HoveredIn(usize),
    ButtonClicked(usize),

    ArrowUp,
    ArrowDown,
    Enter,
}

pub enum KeyboardNavigableButtonsEvent {}

pub type ButtonBuilder = Box<dyn Fn(bool, &leanterm_ui::AppContext) -> Button>;
pub type OnButtonClickFn = Box<dyn Fn(&mut ViewContext<KeyboardNavigableButtons>)>;

pub struct KeyboardNavigableButtonBuilder {
    button_builder: ButtonBuilder,
    /// Called when the button is selected through click or enter.
    on_click: OnButtonClickFn,
}

impl KeyboardNavigableButtonBuilder {}

/// A view that wraps buttons to make them keyboard navigable.
/// Mouse hover and keyboard navigation both update the same selection index.
/// When hovering stops, the selection remains on the last selected button.
/// Note that this view must be focused for keyboard shortcuts to work -
/// the parent view likely needs to focus this view manually.
pub struct KeyboardNavigableButtons {
    button_builders: Vec<KeyboardNavigableButtonBuilder>,
    selected_button_index: usize,
}

impl KeyboardNavigableButtons {
    fn selected_button_index(&self) -> usize {
        self.selected_button_index
    }
}

impl View for KeyboardNavigableButtons {
    fn ui_name() -> &'static str {
        "KeyboardNavigableButtons"
    }

    fn render(&self, app: &leanterm_ui::AppContext) -> Box<dyn leanterm_ui::Element> {
        let mut content = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for (index, button_builder) in self.button_builders.iter().enumerate() {
            let is_selected = index == self.selected_button_index();
            let button = (button_builder.button_builder)(is_selected, app);
            let mut hoverable = button.build();

            hoverable = hoverable
                .additional_on_hover(move |is_hovered, ctx, _app, _pos| {
                    if is_hovered {
                        ctx.dispatch_typed_action(KeyboardNavigableButtonsAction::HoveredIn(index));
                    }
                })
                .on_click(move |ctx, _app, _pos| {
                    ctx.dispatch_typed_action(KeyboardNavigableButtonsAction::ButtonClicked(index));
                });
            let margin_bottom = if index == self.button_builders.len() - 1 {
                0.
            } else {
                MARGIN_BETWEEN_BUTTONS
            };
            content.add_child(
                Container::new(hoverable.finish())
                    .with_margin_bottom(margin_bottom)
                    .finish(),
            );
        }
        content.finish()
    }

    fn keymap_context(&self, _app: &AppContext) -> leanterm_ui::keymap::Context {
        let mut context = Self::default_keymap_context();
        if !self.button_builders.is_empty() {
            context.set.insert(HAS_OPTIONS);
        }
        context
    }
}

impl TypedActionView for KeyboardNavigableButtons {
    type Action = KeyboardNavigableButtonsAction;

    fn handle_action(
        &mut self,
        action: &KeyboardNavigableButtonsAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            KeyboardNavigableButtonsAction::HoveredIn(index) => {
                self.selected_button_index = *index;
            }
            KeyboardNavigableButtonsAction::ButtonClicked(index) => {
                if let Some(builder) = self.button_builders.get(*index) {
                    (builder.on_click)(ctx);
                }
            }
            KeyboardNavigableButtonsAction::ArrowUp => {
                self.selected_button_index =
                    (self.selected_button_index + self.button_builders.len() - 1)
                        % self.button_builders.len();
            }
            KeyboardNavigableButtonsAction::ArrowDown => {
                self.selected_button_index =
                    (self.selected_button_index + 1) % self.button_builders.len();
            }
            KeyboardNavigableButtonsAction::Enter => {
                if let Some(builder) = self.button_builders.get(self.selected_button_index()) {
                    (builder.on_click)(ctx);
                }
            }
        };
        ctx.notify();
    }
}

impl Entity for KeyboardNavigableButtons {
    type Event = KeyboardNavigableButtonsEvent;
}
