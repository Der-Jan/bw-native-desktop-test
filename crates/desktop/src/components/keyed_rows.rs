//! Reconcile virtual row state by actual identity, including large jumps and
//! simultaneous removals/insertions. The pinned Iced keyed Column uses a
//! contiguous-edit heuristic which does not preserve those cases correctly.
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Vector,
    advanced::{
        Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
};

use crate::theme::AppTheme;

pub struct KeyedRows<'a, Key, Message> {
    inner: Element<'a, Message, AppTheme>,
    keys: Vec<Key>,
}
impl<'a, Key: Copy, Message: 'a> KeyedRows<'a, Key, Message> {
    pub fn new(rows: impl IntoIterator<Item = (Key, Element<'a, Message, AppTheme>)>) -> Self {
        let (keys, children): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
        Self {
            inner: iced::widget::Column::with_children(children).into(),
            keys,
        }
    }
}
struct Keys<Key>(Vec<Key>);
impl<Key: Copy + Eq + 'static, Message> Widget<Message, AppTheme, Renderer>
    for KeyedRows<'_, Key, Message>
{
    fn size(&self) -> Size<Length> {
        self.inner.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.inner.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.as_widget_mut().layout(tree, renderer, limits)
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
        self.inner
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Keys<Key>>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(Keys(self.keys.clone()))
    }
    fn children(&self) -> Vec<Tree> {
        self.inner.as_widget().children()
    }
    fn diff(&self, tree: &mut Tree) {
        let keys = &mut tree.state.downcast_mut::<Keys<Key>>().0;
        if *keys != self.keys {
            let mut previous: Vec<_> = keys
                .iter()
                .copied()
                .zip(std::mem::take(&mut tree.children))
                .collect();
            tree.children = self
                .keys
                .iter()
                .map(|key| {
                    previous
                        .iter()
                        .position(|(old, _)| old == key)
                        .map(|index| previous.swap_remove(index).1)
                        .unwrap_or_else(Tree::empty)
                })
                .collect();
            keys.clone_from(&self.keys);
        }
        // Normal Column only diffs/layouts children and does not use its own
        // tree state. Empty slots initialize new rows through their real tags.
        self.inner.as_widget().diff(tree);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.inner
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.inner
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, AppTheme, Renderer>> {
        self.inner
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Key: Copy + Eq + 'static, Message: 'a> From<KeyedRows<'a, Key, Message>>
    for Element<'a, Message, AppTheme>
{
    fn from(value: KeyedRows<'a, Key, Message>) -> Self {
        Self::new(value)
    }
}
