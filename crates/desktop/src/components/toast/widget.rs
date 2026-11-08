//! `Manager` widget impl + the per-toast row builder (`toast_view`).
//!
//! The manager wraps an underlying content `Element` and overlays the toast
//! stack. Most of the trait methods just forward to the wrapped content;
//! the interesting bits are `state` / `diff` (timer bookkeeping) and
//! `overlay` (which spawns [`super::overlay::ToastOverlay`]).

use std::{cell::Cell, rc::Rc, time::Instant};

use iced::{
    Alignment, Border, Color, Element, Event, Fill, Length, Rectangle, Size, Vector,
    advanced::{
        Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse::{self, Cursor},
        overlay,
        renderer::{self},
        widget::{self, Tree},
    },
    widget::{button, column, container, row, text},
};

use crate::{
    components::icons,
    theme::{AppTheme, RADIUS_MD},
};

use super::{
    PROGRESS_BAR_HEIGHT, TOAST_MAX_WIDTH, Toast,
    overlay::ToastOverlay,
    visuals::{ToastProgressBar, ToastTimer, ToastVisuals, refresh_visuals},
};

/// Wraps the app's main content and overlays a stack of toasts.
pub struct Manager<'a, Message> {
    content: Element<'a, Message, AppTheme>,
    toasts: Vec<Element<'a, Message, AppTheme>>,
    /// Per-toast animation cells aligned 1:1 with `toasts`. The overlay
    /// writes on each tick; row style closures read at draw time.
    cells: Vec<Rc<Cell<ToastVisuals>>>,
    on_close: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> Manager<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(
        content: impl Into<Element<'a, Message, AppTheme>>,
        toasts: &'a [Toast],
        on_close: impl Fn(usize) -> Message + 'a,
    ) -> Self {
        // Cells start hidden; `diff()` reseeds them from the persisted
        // timer state for the first draw.
        let cells: Vec<Rc<Cell<ToastVisuals>>> = toasts
            .iter()
            .map(|_| Rc::new(Cell::new(ToastVisuals::HIDDEN)))
            .collect();

        let toast_elements = toasts
            .iter()
            .enumerate()
            .map(|(index, toast)| toast_view(index, toast, cells[index].clone(), &on_close))
            .collect();

        Self {
            content: content.into(),
            toasts: toast_elements,
            cells,
            on_close: Box::new(on_close),
        }
    }
}

/// `text::Style` closure painting `Color::WHITE` scaled by the visuals
/// cell's `alpha`, optionally further dimmed by `factor`.
fn white_alpha_style(
    cell: Rc<Cell<ToastVisuals>>,
    factor: f32,
) -> impl Fn(&AppTheme) -> iced::widget::text::Style {
    move |_theme: &AppTheme| iced::widget::text::Style {
        color: Some(Color::WHITE.scale_alpha(cell.get().alpha * factor)),
    }
}

fn toast_view<'a, Message: 'a + Clone>(
    index: usize,
    toast: &'a Toast,
    cell: Rc<Cell<ToastVisuals>>,
    on_close: &(impl Fn(usize) -> Message + 'a),
) -> Element<'a, Message, AppTheme> {
    let status = toast.status;

    let icon_elem = text(status.icon().to_string())
        .font(icons::FONT)
        .size(22.0)
        .style(white_alpha_style(cell.clone(), 1.0));

    let title_elem = text(toast.title.as_str())
        .font(crate::app_font_bold())
        .size(crate::text_size(14))
        .style(white_alpha_style(cell.clone(), 1.0));

    let body_elem = text(toast.body.as_str())
        .size(crate::text_size(12))
        .style(white_alpha_style(cell.clone(), 0.92));

    let close_icon = text(icons::BWI_CLOSE.char().to_string())
        .font(icons::BWI_FONT)
        .size(18.0)
        .style(white_alpha_style(cell.clone(), 1.0));

    let close_btn = button(close_icon)
        .on_press((on_close)(index))
        .padding([2, 4])
        .style(|_theme: &AppTheme, _status| button::Style {
            background: None,
            text_color: Color::WHITE,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        });

    let body_row = container(
        row![
            icon_elem,
            column![title_elem, body_elem].spacing(2).width(Fill),
            close_btn,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 14])
    .width(Fill);

    let progress_bar: Element<'a, Message, AppTheme> = Element::new(ToastProgressBar {
        cell: cell.clone(),
        height: PROGRESS_BAR_HEIGHT,
        status,
    });

    let inner = column![body_row, progress_bar].width(Fill);

    let outer_cell = cell;
    container(inner)
        .max_width(TOAST_MAX_WIDTH)
        .style(move |theme: &AppTheme| {
            let alpha = outer_cell.get().alpha;
            container::Style::default()
                .background(status.background(&theme.colors).scale_alpha(alpha))
                .border(iced::border::rounded(RADIUS_MD))
        })
        .into()
}

impl<Message> Widget<Message, AppTheme, iced::Renderer> for Manager<'_, Message>
where
    Message: Clone,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn tag(&self) -> widget::tree::Tag {
        struct Marker;
        widget::tree::Tag::of::<Marker>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Vec::<Option<ToastTimer>>::new())
    }

    fn children(&self) -> Vec<Tree> {
        std::iter::once(Tree::new(&self.content))
            .chain(self.toasts.iter().map(Tree::new))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let timers = tree.state.downcast_mut::<Vec<Option<ToastTimer>>>();

        // Drop dismissed entries (overlay nulls the slot when fade-out
        // completes) before re-syncing with the new toast list.
        timers.retain(Option::is_some);

        match (timers.len(), self.toasts.len()) {
            (old, new) if old > new => timers.truncate(new),
            (old, new) if old < new => {
                let now = Instant::now();
                timers.extend(std::iter::repeat_n(
                    Some(ToastTimer {
                        created: now,
                        dismissing: None,
                        hovered: false,
                    }),
                    new - old,
                ));
            }
            _ => {}
        }

        // Seed each visuals cell from the persisted timer state so the
        // first draw of a frame has correct values even if the overlay's
        // update hasn't run yet (e.g. mouse-event-triggered rebuilds).
        let now = Instant::now();
        for (cell, slot) in self.cells.iter().zip(timers.iter()) {
            let visuals = match slot {
                Some(timer) => refresh_visuals(timer, now),
                None => ToastVisuals::HIDDEN,
            };
            cell.set(visuals);
        }

        let children: Vec<&Element<'_, Message, AppTheme>> = std::iter::once(&self.content)
            .chain(self.toasts.iter())
            .collect();
        tree.diff_children(&children);
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &AppTheme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
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

    fn operate<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation<()>,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, AppTheme, iced::Renderer>> {
        let timers = tree.state.downcast_mut::<Vec<Option<ToastTimer>>>();
        let (content_state, toasts_state) = tree.children.split_at_mut(1);

        let content_overlay = self.content.as_widget_mut().overlay(
            &mut content_state[0],
            layout,
            renderer,
            viewport,
            translation,
        );

        let toasts_overlay = (!self.toasts.is_empty()).then(|| {
            overlay::Element::new(Box::new(ToastOverlay {
                position: layout.bounds().position() + translation,
                viewport: *viewport,
                toasts: &mut self.toasts,
                trees: toasts_state,
                timers,
                cells: &self.cells,
                on_close: &self.on_close,
            }))
        });

        let overlays: Vec<_> = content_overlay.into_iter().chain(toasts_overlay).collect();
        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

impl<'a, Message: 'a + Clone> From<Manager<'a, Message>>
    for Element<'a, Message, AppTheme, iced::Renderer>
{
    fn from(manager: Manager<'a, Message>) -> Self {
        Element::new(manager)
    }
}
