//! Bounded wheel interpolation local to the list. Precise pixel events pass
//! straight through. No global timer, and idle lists request no redraws.
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Vector,
    advanced::{
        Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
};

use crate::theme::AppTheme;

pub type ResetKey = (usize, usize, u8, u8);
pub struct SmoothScroll<'a, Message> {
    inner: Element<'a, Message, AppTheme>,
    id: iced::widget::Id,
    reset: ResetKey,
}
impl<'a, Message> SmoothScroll<'a, Message> {
    pub fn new(
        inner: impl Into<Element<'a, Message, AppTheme>>,
        id: iced::widget::Id,
        reset: ResetKey,
    ) -> Self {
        Self {
            inner: inner.into(),
            id,
            reset,
        }
    }
}

// A fixed, short interval avoids the old exponential tail and its large first
// jump. Repeated steps share the current position and finish within 60 ms of
// the newest event; no long backlog builds up. Direction reversal is immediate.
#[derive(Default)]
struct Motion {
    remaining: f32,
    last: Option<iced::time::Instant>,
    end: Option<iced::time::Instant>,
}
impl Motion {
    fn cancel(&mut self) {
        self.remaining = 0.0;
        self.last = None;
        self.end = None;
    }
    fn active(&self) -> bool {
        self.last.is_some()
    }
    fn queue(&mut self, delta: f32, now: iced::time::Instant) -> f32 {
        if !delta.is_finite() || delta == 0.0 {
            self.cancel();
            return 0.0;
        }
        let carried = if self.active() && self.remaining.signum() == delta.signum() {
            self.advance(now)
        } else {
            self.cancel();
            0.0
        };
        let immediate = delta.signum() * delta.abs().min(1.0);
        self.remaining += delta - immediate;
        if self.remaining != 0.0 {
            self.last = Some(now);
            self.end = Some(now + std::time::Duration::from_millis(60));
        }
        carried + immediate
    }
    fn advance(&mut self, now: iced::time::Instant) -> f32 {
        let (Some(last), Some(end)) = (self.last, self.end) else {
            return 0.0;
        };
        if now >= end {
            let distance = self.remaining;
            self.cancel();
            return distance;
        }
        let fraction = now.saturating_duration_since(last).as_secs_f32()
            / end.saturating_duration_since(last).as_secs_f32();
        let distance = self.remaining * fraction.clamp(0.0, 1.0);
        self.remaining -= distance;
        self.last = Some(now.max(last));
        distance
    }
}
struct State {
    motion: Motion,
    modifiers: iced::keyboard::Modifiers,
    reset: ResetKey,
}

/// Query the actual native scroll range, avoiding animation at list boundaries.
struct Range {
    id: iced::widget::Id,
    delta: f32,
    can_move: bool,
}
impl Operation for Range {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        visit(self);
    }
    fn scrollable(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        content: Rectangle,
        translation: Vector,
        _state: &mut dyn iced::advanced::widget::operation::scrollable::Scrollable,
    ) {
        if id == Some(&self.id) {
            let max = (content.height - bounds.height).max(0.0);
            let offset = translation.y.clamp(0.0, max);
            self.delta = (offset + self.delta).clamp(0.0, max) - offset;
            self.can_move = self.delta.abs() > 0.01;
        }
    }
}

impl<Message> Widget<Message, AppTheme, Renderer> for SmoothScroll<'_, Message> {
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
        self.inner
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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
        self.inner.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            motion: Motion::default(),
            modifiers: iced::keyboard::Modifiers::empty(),
            reset: self.reset,
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.inner)]
    }
    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.reset != self.reset {
            state.motion.cancel();
            state.reset = self.reset;
        }
        tree.diff_children(std::slice::from_ref(&self.inner));
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
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        let state = tree.state.downcast_mut::<State>();
        let child = &mut tree.children[0];
        let inside = cursor.is_over(layout.bounds().intersection(viewport).unwrap_or_default());
        let mut emitted = None;
        match event {
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) => {
                // X11 may repeat an unchanged modifier report for every wheel
                // event. Those reports must not cancel the settling tail.
                if state.modifiers != *modifiers {
                    state.modifiers = *modifiers;
                    state.motion.cancel();
                }
            }
            Event::Keyboard(_)
            | Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Window(iced::window::Event::Unfocused) => state.motion.cancel(),
            Event::Mouse(mouse::Event::CursorMoved { .. }) if !inside => state.motion.cancel(),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { .. },
            }) => state.motion.cancel(),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x, y },
            }) if inside
                && !shell.is_event_captured()
                && !state.modifiers.shift()
                && *x == 0.0
                && y.is_finite()
                && *y != 0.0 =>
            {
                let delta = -*y * 60.0;
                let mut range = Range {
                    id: self.id.clone(),
                    delta,
                    can_move: false,
                };
                self.inner
                    .as_widget_mut()
                    .operate(child, layout, renderer, &mut range);
                if range.can_move {
                    emitted = Some(state.motion.queue(range.delta, iced::time::Instant::now()));
                } else {
                    state.motion.cancel();
                }
            }
            Event::Window(iced::window::Event::RedrawRequested(now)) if state.motion.active() => {
                if inside {
                    let delta = state.motion.advance(*now);
                    let mut range = Range {
                        id: self.id.clone(),
                        delta,
                        can_move: false,
                    };
                    self.inner
                        .as_widget_mut()
                        .operate(child, layout, renderer, &mut range);
                    if delta.abs() > 0.01 {
                        if range.can_move {
                            emitted = Some(range.delta);
                        } else {
                            state.motion.cancel();
                        }
                    }
                } else {
                    state.motion.cancel();
                }
            }
            _ => {}
        }
        if state.motion.active() {
            // Keep the redraw chain alive across on_scroll model updates.
            // A timed request can be superseded by the immediate rebuild.
            shell.request_redraw();
        }
        if let Some(delta) = emitted {
            // A synthetic wheel event inherits capture from earlier siblings;
            // that can consume the tail without moving the native scroll state.
            // Apply the exact delta as an operation and notify through a fresh
            // redraw shell instead. Capturing a wheel must not capture redraws.
            let mut operation = iced::advanced::widget::operation::scrollable::scroll_by::<()>(
                self.id.clone(),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: delta },
            );
            self.inner
                .as_widget_mut()
                .operate(child, layout, renderer, &mut operation);
            let mut messages = Vec::new();
            let mut local_shell = Shell::new(&mut messages);
            self.inner.as_widget_mut().update(
                child,
                &Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                )),
                layout,
                cursor,
                renderer,
                &mut local_shell,
                viewport,
            );
            shell.merge(local_shell, |message| message);
            if matches!(event, Event::Mouse(mouse::Event::WheelScrolled { .. })) {
                shell.capture_event();
            }
            return;
        }
        self.inner
            .as_widget_mut()
            .update(child, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner.as_widget().mouse_interaction(
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
    ) -> Option<overlay::Element<'a, Message, AppTheme, Renderer>> {
        self.inner.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message> From<SmoothScroll<'a, Message>> for Element<'a, Message, AppTheme>
where
    Message: 'a,
{
    fn from(value: SmoothScroll<'a, Message>) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{
        Point,
        widget::{Space, scrollable},
    };
    use std::time::Duration;

    fn list(reset: ResetKey) -> Element<'static, scrollable::Viewport, AppTheme> {
        SmoothScroll::new(
            scrollable(Space::new().height(4000))
                .height(400)
                .width(200)
                .id(iced::widget::Id::new("scroll-test"))
                .on_scroll(|v| v),
            iced::widget::Id::new("scroll-test"),
            reset,
        )
        .into()
    }
    fn event(
        element: &mut Element<'_, scrollable::Viewport, AppTheme>,
        tree: &mut Tree,
        renderer: &Renderer,
        node: &layout::Node,
        event: Event,
    ) -> Vec<scrollable::Viewport> {
        let mut messages = Vec::new();
        element.as_widget_mut().update(
            tree,
            &event,
            Layout::new(node),
            mouse::Cursor::Available(Point::new(50.0, 50.0)),
            renderer,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(Size::new(200.0, 400.0)),
        );
        messages
    }
    #[test]
    fn wheel_settles_to_exact_distance_and_pixel_events_remain_immediate() {
        let renderer = iced::futures::executor::block_on(<Renderer as renderer::Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU test renderer");
        let mut element = list((0, 1, 0, 0));
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 400.0)),
        );
        let now = iced::time::Instant::now();
        let messages = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
        );
        let first = messages.last().unwrap().absolute_offset().y;
        assert!(first > 0.0 && first < 60.0);
        let messages = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Window(iced::window::Event::RedrawRequested(
                now + Duration::from_millis(250),
            )),
        );
        assert!((messages.last().unwrap().absolute_offset().y - 60.0).abs() < 0.01);
        assert!(!tree.state.downcast_ref::<State>().motion.active());
        let messages = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -4.25 },
            }),
        );
        assert!((messages.last().unwrap().absolute_offset().y - 64.25).abs() < 0.01);
        assert!(!tree.state.downcast_ref::<State>().motion.active());
    }
    #[test]
    fn wheel_has_small_immediate_response_and_uniform_bounded_intermediate_positions() {
        let now = iced::time::Instant::now();
        let mut motion = Motion::default();
        let mut position = motion.queue(60.0, now);
        assert_eq!(position, 1.0);
        for frame in 1..=6 {
            position += motion.advance(now + Duration::from_millis(frame * 10));
            let expected = 1.0 + 59.0 * frame as f32 / 6.0;
            assert!((position - expected).abs() < 0.001);
            assert_eq!(motion.active(), frame < 6);
        }
        assert_eq!(position, 60.0);
    }
    #[test]
    fn repeated_steps_preserve_distance_and_reversal_cancels_old_direction() {
        let now = iced::time::Instant::now();
        let mut motion = Motion::default();
        let mut distance = 0.0;
        for step in 0..10 {
            distance += motion.queue(60.0, now + Duration::from_millis(step * 8));
        }
        distance += motion.advance(now + Duration::from_millis(500));
        assert!((distance - 600.0).abs() < 0.01);
        assert!(!motion.active());
        let mut motion = Motion::default();
        motion.queue(60.0, now);
        assert!(motion.queue(-60.0, now) < 0.0);
        assert!(motion.advance(now + Duration::from_millis(250)) < 0.0);
        assert!(!motion.active());
        assert_eq!(motion.advance(now), 0.0);
        assert!(!motion.active());
    }
    #[test]
    fn dataset_or_sort_change_cancels_pending_wheel_movement() {
        let mut element = list((0, 1, 0, 0));
        let mut tree = Tree::new(&element);
        tree.state
            .downcast_mut::<State>()
            .motion
            .queue(60.0, iced::time::Instant::now());
        element = list((0, 1, 1, 0));
        tree.diff(element.as_widget());
        assert!(!tree.state.downcast_ref::<State>().motion.active());
    }
    #[test]
    fn precise_input_cancels_the_wheel_tail_and_boundaries_remain_idle() {
        let renderer = iced::futures::executor::block_on(<Renderer as renderer::Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU test renderer");
        let mut element = list((0, 1, 0, 0));
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 400.0)),
        );
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
        );
        assert!(!tree.state.downcast_ref::<State>().motion.active());
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
        );
        assert!(tree.state.downcast_ref::<State>().motion.active());
        let messages = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -4.25 },
            }),
        );
        assert!((messages.last().unwrap().absolute_offset().y - 5.25).abs() < 0.01);
        assert!(!tree.state.downcast_ref::<State>().motion.active());
    }
    #[test]
    fn captured_parent_redraw_cannot_discard_pending_distance() {
        let renderer = iced::futures::executor::block_on(<Renderer as renderer::Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU test renderer");
        let mut element = list((0, 1, 0, 0));
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 400.0)),
        );
        let now = iced::time::Instant::now();
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
        );
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        shell.capture_event();
        element.as_widget_mut().update(
            &mut tree,
            &Event::Window(iced::window::Event::RedrawRequested(
                now + Duration::from_millis(250),
            )),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(50.0, 50.0)),
            &renderer,
            &mut shell,
            &Rectangle::with_size(Size::new(200.0, 400.0)),
        );
        assert!((messages.last().unwrap().absolute_offset().y - 60.0).abs() < 0.01);
        assert!(!tree.state.downcast_ref::<State>().motion.active());
    }
    #[test]
    fn redundant_modifier_reports_from_wheel_input_do_not_cancel_movement() {
        let renderer = iced::futures::executor::block_on(<Renderer as renderer::Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU test renderer");
        let mut element = list((0, 1, 0, 0));
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 400.0)),
        );
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
        );
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                iced::keyboard::Modifiers::empty(),
            )),
        );
        assert!(tree.state.downcast_ref::<State>().motion.active());
        let _ = event(
            &mut element,
            &mut tree,
            &renderer,
            &node,
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                iced::keyboard::Modifiers::SHIFT,
            )),
        );
        assert!(!tree.state.downcast_ref::<State>().motion.active());
    }
}
