//! Widget `Catalog` trait implementations for `AppTheme`. Most widgets
//! use explicit `.style()` closures, so these defaults are rarely invoked
//! directly — they exist to satisfy trait bounds.

use iced::{Background, Border, Color, Shadow, border, widget};

use super::AppTheme;

// ── button ──────────────────────────────────────────────────────────────────

impl widget::button::Catalog for AppTheme {
    type Class<'a> = widget::button::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme, _status| widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: theme.colors.text_primary,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: false,
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: widget::button::Status,
    ) -> widget::button::Style {
        class(self, status)
    }
}

// ── container ───────────────────────────────────────────────────────────────

impl widget::container::Catalog for AppTheme {
    type Class<'a> = widget::container::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_theme| widget::container::Style::default())
    }

    fn style(&self, class: &Self::Class<'_>) -> widget::container::Style {
        class(self)
    }
}

// ── text_input ──────────────────────────────────────────────────────────────

impl widget::text_input::Catalog for AppTheme {
    type Class<'a> = widget::text_input::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme, _status| widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: theme.colors.text_muted,
            placeholder: theme.colors.text_secondary,
            value: theme.colors.text_primary,
            selection: theme.colors.accent,
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: widget::text_input::Status,
    ) -> widget::text_input::Style {
        class(self, status)
    }
}

// ── text_editor (multi-line) ───────────────────────────────────────────────

impl widget::text_editor::Catalog for AppTheme {
    type Class<'a> = widget::text_editor::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme, _status| widget::text_editor::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: theme.colors.text_secondary,
            value: theme.colors.text_primary,
            selection: theme.colors.accent,
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: widget::text_editor::Status,
    ) -> widget::text_editor::Style {
        class(self, status)
    }
}

// ── rule ────────────────────────────────────────────────────────────────────

impl widget::rule::Catalog for AppTheme {
    type Class<'a> = widget::rule::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme| widget::rule::Style {
            color: theme.colors.border,
            radius: border::Radius::default(),
            fill_mode: widget::rule::FillMode::Full,
            snap: false,
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> widget::rule::Style {
        class(self)
    }
}

// ── scrollable ──────────────────────────────────────────────────────────────

impl widget::scrollable::Catalog for AppTheme {
    type Class<'a> = widget::scrollable::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme, _status| widget::scrollable::Style {
            container: widget::container::Style::default(),
            vertical_rail: widget::scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: widget::scrollable::Scroller {
                    background: Background::Color(theme.colors.scrollbar_thumb),
                    border: Border::default(),
                },
            },
            horizontal_rail: widget::scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: widget::scrollable::Scroller {
                    background: Background::Color(theme.colors.scrollbar_thumb),
                    border: Border::default(),
                },
            },
            gap: None,
            auto_scroll: widget::scrollable::AutoScroll {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                shadow: Shadow::default(),
                icon: Color::TRANSPARENT,
            },
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: widget::scrollable::Status,
    ) -> widget::scrollable::Style {
        class(self, status)
    }
}

// ── pane_grid (extends container::Catalog) ──────────────────────────────────

impl widget::pane_grid::Catalog for AppTheme {
    type Class<'a> = widget::pane_grid::StyleFn<'a, Self>;

    fn default<'a>() -> <Self as widget::pane_grid::Catalog>::Class<'a> {
        Box::new(|_theme| widget::pane_grid::Style {
            hovered_region: widget::pane_grid::Highlight {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
            },
            picked_split: widget::pane_grid::Line {
                color: Color::TRANSPARENT,
                width: 1.0,
            },
            hovered_split: widget::pane_grid::Line {
                color: Color::from_rgb(0.5, 0.5, 0.5),
                width: 2.0,
            },
        })
    }

    fn style(
        &self,
        class: &<Self as widget::pane_grid::Catalog>::Class<'_>,
    ) -> widget::pane_grid::Style {
        class(self)
    }
}

// ── checkbox ───────────────────────────────────────────────────────────────

impl widget::checkbox::Catalog for AppTheme {
    type Class<'a> = widget::checkbox::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme, status| {
            let is_checked = matches!(
                status,
                widget::checkbox::Status::Active { is_checked: true }
                    | widget::checkbox::Status::Hovered { is_checked: true }
            );
            widget::checkbox::Style {
                background: Background::Color(if is_checked {
                    theme.colors.accent
                } else {
                    Color::TRANSPARENT
                }),
                icon_color: Color::WHITE,
                border: Border::default()
                    .color(if is_checked {
                        theme.colors.accent
                    } else {
                        theme.colors.border
                    })
                    .width(1.5)
                    .rounded(4),
                text_color: Some(theme.colors.text_primary),
            }
        })
    }

    fn style(
        &self,
        class: &Self::Class<'_>,
        status: widget::checkbox::Status,
    ) -> widget::checkbox::Style {
        class(self, status)
    }
}

// ── svg ─────────────────────────────────────────────────────────────────────

impl widget::svg::Catalog for AppTheme {
    type Class<'a> = widget::svg::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_theme, _status| widget::svg::Style::default())
    }

    fn style(&self, class: &Self::Class<'_>, status: widget::svg::Status) -> widget::svg::Style {
        class(self, status)
    }
}

// ── text ────────────────────────────────────────────────────────────────────

impl iced::widget::text::Catalog for AppTheme {
    type Class<'a> = widget::text::StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_theme| widget::text::Style::default())
    }

    fn style(&self, class: &Self::Class<'_>) -> widget::text::Style {
        class(self)
    }
}

// ── pick_list ───────────────────────────────────────────────────────────────

impl widget::pick_list::Catalog for AppTheme {
    type Class<'a> = widget::pick_list::StyleFn<'a, Self>;

    fn default<'a>() -> <Self as widget::pick_list::Catalog>::Class<'a> {
        Box::new(|theme: &AppTheme, status| {
            let (border_color, text_color) = match status {
                widget::pick_list::Status::Hovered | widget::pick_list::Status::Opened { .. } => {
                    (theme.colors.accent, theme.colors.text_primary)
                }
                widget::pick_list::Status::Active => {
                    (theme.colors.border, theme.colors.text_primary)
                }
                widget::pick_list::Status::Disabled => {
                    (theme.colors.border, theme.colors.text_muted)
                }
            };
            widget::pick_list::Style {
                text_color,
                background: Background::Color(Color::TRANSPARENT),
                placeholder_color: theme.colors.text_secondary,
                handle_color: theme.colors.text_secondary,
                border: Border::default().color(border_color).width(1.0).rounded(4),
            }
        })
    }

    fn style(
        &self,
        class: &<Self as widget::pick_list::Catalog>::Class<'_>,
        status: widget::pick_list::Status,
    ) -> widget::pick_list::Style {
        class(self, status)
    }
}

// ── overlay::menu (dropdown panel for pick_list) ───────────────────────────

impl iced::overlay::menu::Catalog for AppTheme {
    type Class<'a> = iced::overlay::menu::StyleFn<'a, Self>;

    fn default<'a>() -> <Self as iced::overlay::menu::Catalog>::Class<'a> {
        Box::new(|theme: &AppTheme| iced::overlay::menu::Style {
            background: Background::Color(theme.colors.card_bg),
            border: Border::default()
                .color(theme.colors.border)
                .width(1.0)
                .rounded(4),
            text_color: theme.colors.text_primary,
            selected_text_color: theme.colors.text_primary,
            selected_background: Background::Color(theme.colors.item_hover),
            shadow: Shadow::default(),
        })
    }

    fn style(
        &self,
        class: &<Self as iced::overlay::menu::Catalog>::Class<'_>,
    ) -> iced::overlay::menu::Style {
        class(self)
    }
}

// ── combo_box (searchable pick_list) ───────────────────────────────────────
// Blanket trait over text_input::Catalog + menu::Catalog, both impl'd above.

impl widget::combo_box::Catalog for AppTheme {}
