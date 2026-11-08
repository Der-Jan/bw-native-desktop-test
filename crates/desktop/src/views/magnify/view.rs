//! Magnify launcher rendering. Three branches matching
//! `designs/magnify/locked.jpg`, `unlocked.jpg`, and `searching.jpg`. Only
//! the outermost container paints a background so the rounded edges aren't
//! clipped by inner fills.
//!
//! **Tree-shape stability:** in `Mode::Unlocked`, the outer column always
//! starts with the same `search_row` widget at index 0, with the optional
//! results + footer pushed below. This keeps the `text_input`'s position in
//! the widget tree identical whether the query is empty or not — iced
//! matches widget state by tree position, so a structural shift between
//! "empty" and "searching" branches would drop the input's focus state on
//! the first keystroke.

use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length,
    widget::{column, container, mouse_area, row, scrollable, svg, text, text_input},
};

use crate::{
    app::RenderCtx,
    components::{self, icons, separator_h},
    fl,
    services::favicon::FaviconService,
    theme::{
        AppColors, AppTheme, MAGNIFY_OVERLAY_ALPHA_FAINT, MAGNIFY_SURFACE_ALPHA, RADIUS_PILL,
        RADIUS_XL,
    },
    views::magnify::{
        MAGNIFY_RESULTS_SCROLL_ID, MAGNIFY_SEARCH_ID, MagnifyMessage, MagnifyView, Mode, dims,
        widgets::{chip, footer, row::view as row_view},
    },
};

/// Build the launcher window's view tree. The outer container draws the
/// rounded background; the per-window theme returns `background: TRANSPARENT`
/// so the OS window stays see-through outside the rounded region.
pub(crate) fn view<'a>(
    state: &'a MagnifyView,
    ctx: &RenderCtx<'a>,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let body: Element<'a, MagnifyMessage, AppTheme> = match state.mode {
        Mode::Locked => locked_body(ctx.colors),
        Mode::Unlocked => unlocked_body(
            state,
            ctx.favicon,
            ctx.show_favicons,
            ctx.active_user,
            ctx.colors,
        ),
    };

    container(body)
        .width(Length::Fixed(dims::WIDTH))
        .height(Fill)
        .style(|theme: &AppTheme| {
            // Translucent surface — the OS window is set to `transparent: true`
            // and the per-window theme returns `background: TRANSPARENT`, so
            // pixels outside this rounded fill stay see-through and the alpha
            // here lets the desktop bleed through subtly inside.
            let surface = Color {
                a: MAGNIFY_SURFACE_ALPHA,
                ..theme.colors.card_bg
            };
            container::Style::default()
                .background(Background::Color(surface))
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(RADIUS_XL),
                )
        })
        .into()
}

// ── Locked state ──────────────────────────────────────────────────────────

fn locked_body<'a>(colors: &'a AppColors) -> Element<'a, MagnifyMessage, AppTheme> {
    let lock_icon = container(icons::BWI_LOCK.render(20.0, colors.text_primary))
        .width(32)
        .height(32)
        .center_x(32)
        .center_y(32);

    let labels = column![
        text(fl!("magnify-locked-title"))
            .size(crate::text_size(14))
            .color(colors.text_primary)
            .font(crate::app_font_bold()),
        text(fl!("magnify-locked-subtitle"))
            .size(crate::text_size(12))
            .color(colors.text_secondary),
    ]
    .spacing(2);

    // Display-only hint: pressing Enter triggers OpenMainWindow (handled in
    // app::handlers::magnify). Same chip styling as the searching footer
    // hints — the locked state is a tip, not a button.
    let open_hint = chip::hint(
        chip::keybind(icons::ARROW_RETURN_LEFT.render(11.0, colors.text_primary)),
        fl!("magnify-open-bitwarden"),
        colors,
    );

    container(
        row![
            lock_icon,
            labels,
            iced::widget::Space::new().width(Fill),
            open_hint,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([0, 16])
    .width(Fill)
    .height(Length::Fixed(dims::COLLAPSED_HEIGHT))
    .align_y(Alignment::Center)
    .into()
}

// ── Unlocked: stable column with search_row at index 0 ────────────────────

fn unlocked_body<'a>(
    state: &'a MagnifyView,
    favicon: &'a FaviconService,
    show_favicons: bool,
    active_user: Option<&'a crate::domain::UserId>,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let is_searching = !state.query.is_empty();

    let search_row = search_row(state.query.as_str(), is_searching, colors);

    let mut col = column![search_row].width(Fill).height(Fill);

    if is_searching {
        col = col
            .push(separator_h())
            .push(results_list(
                state,
                favicon,
                show_favicons,
                active_user,
                colors,
            ))
            .push(separator_h())
            .push(footer::view(colors));
    }

    col.into()
}

/// Top row containing the search input. The trailing widget swaps between
/// the Bitwarden shield (empty state) and the "esc" chip (searching), but
/// the `text_input` always sits at the same position in the row's children
/// so iced preserves its focus state across the transition.
fn search_row<'a>(
    query: &'a str,
    is_searching: bool,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let input = search_input(query);
    let trailing: Element<'a, MagnifyMessage, AppTheme> = if is_searching {
        mouse_area(
            container(
                text("esc")
                    .size(crate::text_size(12))
                    .color(colors.text_secondary),
            )
            .padding([4, 10])
            .style(|theme: &AppTheme| {
                container::Style::default()
                    .background(Background::Color(Color {
                        a: MAGNIFY_OVERLAY_ALPHA_FAINT,
                        ..Color::WHITE
                    }))
                    .border(
                        Border::default()
                            .color(theme.colors.border)
                            .width(1.0)
                            .rounded(RADIUS_PILL),
                    )
            }),
        )
        .on_press(MagnifyMessage::Hide)
        .into()
    } else {
        svg(svg::Handle::from_memory(crate::assets::BITWARDEN_SHIELD))
            .width(18)
            .height(18)
            // The shield asset has `fill="white"` baked in. Tint to the
            // theme's muted text color so it reads correctly on the light
            // theme's pale surface (where pure white would disappear) and
            // softens to a non-glaring tone on the dark theme.
            .style(|theme: &AppTheme, _status| iced::widget::svg::Style {
                color: Some(theme.colors.text_muted),
            })
            .into()
    };

    container(row![input, trailing].spacing(12).align_y(Alignment::Center))
        .padding([0, 16])
        .width(Fill)
        .height(Length::Fixed(dims::COLLAPSED_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

fn results_list<'a>(
    state: &'a MagnifyView,
    favicon: &'a FaviconService,
    show_favicons: bool,
    active_user: Option<&'a crate::domain::UserId>,
    colors: &'a AppColors,
) -> Element<'a, MagnifyMessage, AppTheme> {
    let rows: Vec<Element<'a, MagnifyMessage, AppTheme>> = state
        .results
        .iter()
        .enumerate()
        .map(|(i, item)| {
            row_view(
                i,
                item.as_ref(),
                i == state.selected,
                favicon,
                show_favicons,
                active_user,
                colors,
            )
        })
        .collect();

    if rows.is_empty() {
        container(
            text(fl!("magnify-no-results"))
                .size(crate::text_size(14))
                .color(colors.text_secondary),
        )
        .padding([12, 16])
        .width(Fill)
        .into()
    } else {
        scrollable(column(rows).width(Fill))
            .id(MAGNIFY_RESULTS_SCROLL_ID)
            .on_scroll(MagnifyMessage::Scrolled)
            .height(Fill)
            .style(components::rail_scroll_style)
            .into()
    }
}

// ── Search input ──────────────────────────────────────────────────────────

fn search_input<'a>(query: &'a str) -> Element<'a, MagnifyMessage, AppTheme> {
    text_input(&fl!("magnify-search-placeholder"), query)
        .id(MAGNIFY_SEARCH_ID)
        .on_input(MagnifyMessage::QueryChanged)
        .icon(icons::SEARCH.input_icon(16.0, text_input::Side::Left))
        .size(crate::text_size(16))
        .padding([10, 8])
        .width(Fill)
        .style(|theme: &AppTheme, _status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: theme.colors.text_muted,
            placeholder: theme.colors.text_muted,
            value: theme.colors.text_primary,
            // Match the result-row selection so the launcher reads as one
            // visual system — selected text in the input and the
            // highlighted result both use the same brighter-blue.
            selection: theme.colors.magnify_selected,
        })
        .into()
}
