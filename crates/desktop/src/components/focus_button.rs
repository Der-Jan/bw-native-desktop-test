//! Keyboard focus for the pinned Iced button, which is otherwise mouse-only.
//! Messages carry the approval UUID supplied by the caller: a queued Enter can
//! never confirm a replacement request. Tab navigation stays inside the dialog.
use crate::theme::{AppTheme, RADIUS_PILL};
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Vector,
    advanced::{
        Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree, operation::focusable::Focusable, tree},
    },
    keyboard::{self, Key, key::Named},
    widget::Id,
};

pub struct FocusButton<'a, M> {
    content: Element<'a, M, AppTheme>,
    id: Id,
    initial_focus: bool,
    press: M,
    next: M,
    previous: M,
}
impl<'a, M> FocusButton<'a, M> {
    pub fn new(
        content: impl Into<Element<'a, M, AppTheme>>,
        id: Id,
        initial_focus: bool,
        press: M,
        next: M,
        previous: M,
    ) -> Self {
        Self {
            content: content.into(),
            id,
            initial_focus,
            press,
            next,
            previous,
        }
    }
}
#[derive(Default)]
struct State {
    focused: bool,
}
impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
    }
    fn unfocus(&mut self) {
        self.focused = false;
    }
}
impl<M: Clone> Widget<M, AppTheme, Renderer> for FocusButton<'_, M> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            focused: self.initial_focus,
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.focusable(
            Some(&self.id),
            layout.bounds(),
            tree.state.downcast_mut::<State>(),
        );
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        if tree.state.downcast_ref::<State>().focused
            && let Event::Keyboard(event) = event
        {
            if let keyboard::Event::KeyPressed {
                key,
                modifiers,
                repeat: false,
                ..
            } = event
            {
                let message = match key {
                    Key::Named(Named::Enter | Named::Space) if modifiers.is_empty() => {
                        Some(&self.press)
                    }
                    Key::Named(Named::Tab)
                        if !modifiers.control() && !modifiers.alt() && !modifiers.logo() =>
                    {
                        Some(if modifiers.shift() {
                            &self.previous
                        } else {
                            &self.next
                        })
                    }
                    _ => None,
                };
                if let Some(message) = message {
                    shell.publish(message.clone());
                }
            }
            // Include repeats and key releases: typing must not reach a field
            // underneath the modal. Escape is handled by the App's UUID-aware modal path.
            shell.capture_event();
            return;
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &AppTheme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        if tree.state.downcast_ref::<State>().focused {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds().expand(2.0),
                    border: Border::default()
                        .color(theme.colors.accent)
                        .width(1.0)
                        .rounded(RADIUS_PILL),
                    ..Default::default()
                },
                Color::TRANSPARENT,
            );
        }
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, M, AppTheme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
impl<'a, M: Clone + 'a> From<FocusButton<'a, M>> for Element<'a, M, AppTheme> {
    fn from(button: FocusButton<'a, M>) -> Self {
        Self::new(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{components::buttons, test_support};
    use iced::widget::text;
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Msg {
        Allow(uuid::Uuid),
        Next,
        Previous,
    }
    fn element(id: uuid::Uuid) -> Element<'static, Msg, AppTheme> {
        let press = Msg::Allow(id);
        FocusButton::new(
            buttons::primary(text("Allow once")).on_press(press.clone()),
            Id::new("test-approval-default"),
            true,
            press,
            Msg::Next,
            Msg::Previous,
        )
        .into()
    }
    #[test]
    fn enter_activates_the_default_with_the_original_request_id_and_captures_the_event() {
        test_support::init();
        let id = uuid::Uuid::new_v4();
        let mut ui = test_support::simulator(element(id));
        assert_eq!(ui.tap_key(Named::Enter), iced::event::Status::Captured);
        assert_eq!(ui.into_messages().collect::<Vec<_>>(), vec![Msg::Allow(id)]);
    }
    #[test]
    fn repeated_or_modified_enter_cannot_confirm_and_tab_stays_scoped() {
        test_support::init();
        let mut ui = test_support::simulator(element(uuid::Uuid::new_v4()));
        let mut repeated = iced_test::simulator::press_key(Named::Enter, None);
        if let Event::Keyboard(keyboard::Event::KeyPressed { repeat, .. }) = &mut repeated {
            *repeat = true;
        }
        let mut modified = iced_test::simulator::press_key(Named::Enter, None);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut modified {
            *modifiers = keyboard::Modifiers::CTRL;
        }
        ui.simulate([repeated, modified]);
        assert_eq!(ui.tap_key(Named::Tab), iced::event::Status::Captured);
        let mut reverse = iced_test::simulator::press_key(Named::Tab, None);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut reverse {
            *modifiers = keyboard::Modifiers::SHIFT;
        }
        ui.simulate([reverse]);
        assert_eq!(
            ui.into_messages().collect::<Vec<_>>(),
            vec![Msg::Next, Msg::Previous]
        );
    }
}
