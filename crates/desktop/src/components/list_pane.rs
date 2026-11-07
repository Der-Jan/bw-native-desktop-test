//! Shared chrome for the Vault and Send screens: a "list pane" with a
//! 28-pt title, a primary "+ New" button, the account-switcher avatar, a
//! search row, and the scrolling item body below. Both screens used to
//! reimplement this layout side by side; the differences (new-button
//! payload, search widget, list body) flow in as `Element` parameters so
//! the helpers don't bake either caller's specifics into the shared
//! scaffolding.

use iced::{
    Alignment, Border, Element, Fill, Padding,
    widget::{Button, column, container, pane_grid::ResizeEvent, row, text},
};

use crate::{
    components::{
        bottom_sheet::SHEET_BREAKPOINT_PX, buttons, collapsible_pane::CollapsiblePane, icons,
    },
    theme::{AppColors, AppTheme},
};

/// Builds the "+ New" primary button used at the top of both the Vault and
/// Send list panes. The vault wraps the returned `Button` in a `DropDown`
/// for the cipher-type picker; send uses it as-is for the new-file flow,
/// so we hand back the [`Button`] builder rather than an `Element`.
pub fn new_item_button<'a, M: 'a + Clone>(
    label: impl Into<String>,
    on_press: M,
    colors: &'a AppColors,
) -> Button<'a, M, AppTheme> {
    buttons::primary(
        row![
            icons::PLUS.render(14.0, colors.card_bg),
            text(label.into()).size(14),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(on_press)
    .padding(Padding {
        top: 8.0,
        right: 16.0,
        bottom: 8.0,
        left: 12.0,
    })
}

/// Compose the list-pane column: header (title + spacer + new-button + avatar)
/// over the search row over the list body. `new_button` is passed as an
/// `Element` so the caller can wrap [`new_item_button`] in a `DropDown` or
/// hand it in directly.
pub fn layout<'a, M: 'a>(
    title: impl Into<String>,
    new_button: impl Into<Element<'a, M, AppTheme>>,
    avatar: Element<'a, M, AppTheme>,
    search: Element<'a, M, AppTheme>,
    body: Element<'a, M, AppTheme>,
    colors: &'a AppColors,
) -> Element<'a, M, AppTheme> {
    let title_el = text(title.into())
        .size(28)
        .color(colors.text_primary)
        .font(crate::APP_FONT_BOLD);

    layout_with_heading(title_el.into(), new_button, avatar, search, body)
}

/// Header variant for a title with a secondary filter summary.
pub fn layout_with_heading<'a, M: 'a>(
    heading: Element<'a, M, AppTheme>,
    new_button: impl Into<Element<'a, M, AppTheme>>,
    avatar: Element<'a, M, AppTheme>,
    search: Element<'a, M, AppTheme>,
    body: Element<'a, M, AppTheme>,
) -> Element<'a, M, AppTheme> {
    let content_header = container(
        row![container(heading).width(Fill), new_button.into(), avatar]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .padding([16, 24])
    .width(Fill);

    let search_row = container(search)
        .padding(Padding {
            top: 0.0,
            right: 24.0,
            bottom: 8.0,
            left: 24.0,
        })
        .width(Fill);

    column![content_header, search_row, body]
        .width(Fill)
        .height(Fill)
        .into()
}

/// Standard `View::overlays` body for the Vault and Send list screens:
/// the narrow-mode bottom sheet (when present) under the delete-confirm
/// modal (when present). Filters out `None` so the result skips empty
/// slots — z-order is preserved (sheet below, modal above).
pub fn overlays<'a, M>(
    sheet: Option<Element<'a, M, AppTheme>>,
    modal: Option<Element<'a, M, AppTheme>>,
) -> Vec<Element<'a, M, AppTheme>> {
    [sheet, modal].into_iter().flatten().collect()
}

/// Wrap the list pane content in the rounded-top-left `background`-coloured
/// container shared by Vault and Send. Above [`SHEET_BREAKPOINT_PX`] the
/// `right_pane` (the detail or form) renders alongside the list inside a
/// [`crate::components::collapsible_pane`]; below it, the right pane is
/// expected to render separately as a bottom sheet at the App level, so
/// callers pass `None` (or skip building it altogether) in narrow mode.
pub fn render<'a, M, R>(
    pane: &'a CollapsiblePane,
    list_content: Element<'a, M, AppTheme>,
    right_pane: Option<Element<'a, M, AppTheme>>,
    on_resize: R,
    window_width: f32,
) -> Element<'a, M, AppTheme>
where
    M: 'a + Clone,
    R: Fn(ResizeEvent) -> M + 'a,
{
    let content_area_inner: Element<'a, M, AppTheme> = if window_width >= SHEET_BREAKPOINT_PX {
        crate::components::collapsible_pane::view(pane, list_content, right_pane, on_resize)
    } else {
        list_content
    };

    container(content_area_inner)
        .width(Fill)
        .height(Fill)
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.background)
                .border(Border::default().rounded(iced::border::top_left(10)))
        })
        .into()
}
