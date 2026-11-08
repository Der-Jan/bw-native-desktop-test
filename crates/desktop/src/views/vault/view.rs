//! `VaultView::view` plus `sheet_view` / `modal_view` / view-internal builders.

use std::sync::Arc;

use bitwarden_vault::CipherListView;
use iced::{
    Border, Color, Element, Fill, Font,
    widget::{container, text, text::Wrapping, tooltip},
};

use crate::{
    app::{Overlay, RenderCtx},
    components::{
        account_switcher,
        bottom_sheet::{self, SHEET_BREAKPOINT_PX, SHEET_TOP_INSET_PX, SHEET_TOP_RADIUS_PX},
        list_pane,
    },
    fl,
    theme::{AppColors, AppTheme},
};

use super::{
    CollectionFilter, FolderFilter, VaultMessage, VaultScope, VaultSection,
    state::VaultView,
    widgets::{
        cipher_detail::{self, CipherDetailMessage},
        cipher_edit, item_list, search_bar,
    },
};

impl VaultView {
    pub(super) fn render<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, VaultMessage, AppTheme> {
        let active_user = ctx.active_user.expect("Screen::Vault without active_user");
        let user_cache = self.items.get(active_user);
        let cached_items: &[Arc<CipherListView>] =
            user_cache.map(|ic| ic.cached.as_slice()).unwrap_or(&[]);

        // Below `SHEET_BREAKPOINT_PX` the right pane is composed at the App
        // level as a bottom sheet (`render_sheet`), so build it here only in
        // wide mode.
        let right_pane = (ctx.window_width >= SHEET_BREAKPOINT_PX
            && (self.selection.detail.is_some() || self.selection.form.is_some()))
        .then(|| self.detail_or_form_pane(ctx.colors, 0.0, ctx.remote_vault));

        list_pane::render(
            &self.pane,
            self.list_content(ctx, cached_items),
            right_pane,
            VaultMessage::PaneResized,
            ctx.window_width,
        )
    }

    /// In narrow mode (`window_width < SHEET_BREAKPOINT_PX`) with a detail
    /// selected, returns the bottom-sheet element that the app composes on
    /// top of the entire window (including sidebar and title bar). Returns
    /// `None` otherwise.
    pub(super) fn render_sheet<'a>(
        &'a self,
        ctx: &RenderCtx<'a>,
    ) -> Option<Element<'a, VaultMessage, AppTheme>> {
        if ctx.window_width >= SHEET_BREAKPOINT_PX {
            return None;
        }
        if self.selection.detail.is_none() && self.selection.form.is_none() {
            return None;
        }
        let progress = self.selection.sheet_fade.progress_if_visible()?;
        let pane = self.detail_or_form_pane(ctx.colors, SHEET_TOP_RADIUS_PX, ctx.remote_vault);
        Some(bottom_sheet::view(
            pane,
            SHEET_TOP_INSET_PX,
            progress,
            Some(VaultMessage::CloseCipherDetail),
        ))
    }

    /// Returns the delete-confirmation modal when armed, `None` otherwise.
    /// Composed by the App on top of the vault view so the backdrop covers
    /// the sidebar and title bar.
    pub(super) fn render_overlay<'a>(
        &'a self,
        ctx: &RenderCtx<'a>,
    ) -> Option<Element<'a, VaultMessage, AppTheme>> {
        let progress = self.selection.confirm_delete.progress_if_visible()?;
        let item_name = self
            .selection
            .detail
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("");

        Some(crate::components::modal::confirm_dialog(
            fl!("vault-delete-modal-title"),
            fl!("vault-delete-modal-body", name = item_name),
            fl!("vault-delete-modal-confirm"),
            fl!("vault-delete-modal-cancel"),
            VaultMessage::ConfirmDeleteSelected,
            VaultMessage::CancelDeleteSelected,
            ctx.colors,
            progress,
        ))
    }

    /// Builds the right-side pane content: either the editable `cipher_edit`
    /// when `selection.form.is_some()`, or the read-only `cipher_detail`.
    fn detail_or_form_pane<'a>(
        &'a self,
        colors: &'a AppColors,
        top_radius: f32,
        read_only: bool,
    ) -> Element<'a, VaultMessage, AppTheme> {
        if let Some(form) = self.selection.form.as_ref() {
            cipher_edit::view(form, colors, top_radius).map(Into::into)
        } else {
            let item = self
                .selection
                .detail
                .as_ref()
                .expect("detail_or_form_pane called without a selection");
            cipher_detail::view(item, colors, top_radius, read_only).map(|msg| match msg {
                CipherDetailMessage::Close => VaultMessage::CloseCipherDetail,
                other => VaultMessage::CipherDetail(other),
            })
        }
    }

    /// Builds the list pane content (header + search + item list).
    fn list_content<'a>(
        &'a self,
        ctx: &RenderCtx<'a>,
        cached_items: &'a [Arc<CipherListView>],
    ) -> Element<'a, VaultMessage, AppTheme> {
        let active_email = ctx
            .active_email
            .expect("Screen::Vault without active_email");

        let new_button: Element<'a, VaultMessage, AppTheme> = if ctx.remote_vault {
            iced::widget::Space::new().into()
        } else {
            list_pane::new_item_button(
                fl!("vault-new-button"),
                VaultMessage::OpenNewItemPicker,
                ctx.colors,
            )
            .into()
        };

        let account_switcher_open = ctx.open_overlay == Some(Overlay::AccountSwitcher);
        let avatar = account_switcher::header_switcher(
            active_email,
            ctx.accounts,
            account_switcher_open,
            ctx.colors,
        )
        .map(Into::into);

        let search = search_bar::view(&self.search_query).map(Into::into);
        let search = if ctx.remote_vault {
            let label = if ctx.syncing {
                "Synchronisierung läuft …".to_owned()
            } else {
                format!(
                    "Schreibgeschützt · Stand: {}",
                    ctx.last_sync
                        .map(|t| t
                            .with_timezone(&chrono::Local)
                            .format("%d.%m.%Y %H:%M")
                            .to_string())
                        .unwrap_or("noch kein Snapshot".into())
                )
            };
            let mut content =
                iced::widget::column![iced::widget::text(label).size(crate::text_size(12)), search]
                    .spacing(8);
            if let Some(error) = ctx.sync_error {
                content=content.push(iced::widget::text(format!("Aktualisierung fehlgeschlagen: {error} · Gespeicherter Stand bleibt verfügbar.")).size(crate::text_size(12)));
            }
            content.into()
        } else {
            search
        };
        let item_list = item_list::view(
            ctx,
            cached_items,
            self.selection.item,
            self.list_scroll,
            self.sort,
        )
        .map(Into::into);

        let title = match ctx.active_vault_filter.scope {
            VaultScope::All => fl!("sidebar-filter-all-vaults"),
            VaultScope::Personal => fl!("sidebar-filter-my-vault"),
            VaultScope::Organization(id) => ctx
                .active_user
                .and_then(|uid| self.organizations_for(uid))
                .and_then(|orgs| orgs.iter().find(|org| org.id == id))
                .map(|org| org.name.clone())
                .unwrap_or_else(|| fl!("sidebar-group-vaults")),
        };
        let filters = self.filter_summary(ctx);
        let mut heading = iced::widget::column![heading_label(
            title,
            28,
            crate::app_font_bold(),
            ctx.colors.text_primary
        )]
        .width(Fill)
        .spacing(4);
        if !filters.is_empty() {
            heading = heading.push(heading_label(
                filters.join(" · "),
                14,
                crate::app_font(),
                ctx.colors.text_secondary,
            ));
        }
        list_pane::layout_with_heading(heading.into(), new_button, avatar, search, item_list)
    }
    fn filter_summary(&self, ctx: &RenderCtx<'_>) -> Vec<String> {
        use bitwarden_vault::CipherType;
        let filter = ctx.active_vault_filter;
        let cache = ctx.active_user.and_then(|uid| self.items.get(uid));
        let mut labels = Vec::new();
        if let Some(kind) = filter.category {
            labels.push(match kind {
                CipherType::Login => fl!("sidebar-filter-logins"),
                CipherType::Card => fl!("sidebar-filter-cards"),
                CipherType::Identity => fl!("sidebar-filter-identities"),
                CipherType::SecureNote => fl!("sidebar-filter-notes"),
                CipherType::SshKey => fl!("sidebar-filter-ssh-keys"),
                CipherType::BankAccount => fl!("detail-section-bank-account"),
                CipherType::DriversLicense => fl!("detail-section-drivers-license"),
                CipherType::Passport => fl!("detail-section-passport"),
            });
        }
        match filter.section {
            VaultSection::Active => {}
            VaultSection::Favorites => labels.push(fl!("sidebar-filter-favorites")),
            VaultSection::Archive => labels.push(fl!("sidebar-filter-archive")),
            VaultSection::Trash => labels.push(fl!("sidebar-filter-trash")),
        }
        match filter.folder {
            FolderFilter::All => {}
            FolderFilter::None => labels.push(fl!("sidebar-filter-no-folder")),
            FolderFilter::Folder(id) => {
                if let Some(folder) = cache.and_then(|c| c.folders.iter().find(|f| f.id == id)) {
                    labels.push(fl!("vault-filter-folder", name = folder.name.clone()));
                }
            }
        }
        match filter.collection {
            CollectionFilter::All => {}
            CollectionFilter::None => labels.push(fl!("sidebar-filter-no-collection")),
            CollectionFilter::Collection(id) => {
                if let Some(collection) =
                    cache.and_then(|c| c.collections.iter().find(|f| f.id == id))
                {
                    let name = if filter.scope == VaultScope::All {
                        cache
                            .and_then(|c| {
                                c.organizations
                                    .iter()
                                    .find(|o| o.id == collection.organization_id)
                            })
                            .map(|org| format!("{} / {}", org.name, collection.name))
                            .unwrap_or_else(|| collection.name.clone())
                    } else {
                        collection.name.clone()
                    };
                    labels.push(fl!("vault-filter-collection", name = name));
                }
            }
        }
        if !self.search_query.is_empty() {
            labels.push(fl!(
                "vault-filter-search",
                query = self.search_query.clone()
            ));
        }
        labels
    }
}

/// Keep the list header compact even alongside a wide detail pane. Hover
/// exposes the full scope/filter text without expanding the header height.
fn heading_label<'a>(
    value: String,
    size: u32,
    font: Font,
    color: Color,
) -> Element<'a, VaultMessage, AppTheme> {
    tooltip(
        // The pinned renderer can retain an empty height-limited ellipsis
        // paragraph after relayout. Clip a non-wrapping label instead.
        container(
            text(value.clone())
                .size(crate::text_size(size))
                .font(font)
                .color(color)
                .width(Fill)
                .wrapping(Wrapping::None),
        )
        .width(Fill)
        .clip(true),
        container(text(value).size(crate::text_size(14)))
            .max_width(600)
            .padding(8)
            .style(|theme: &AppTheme| {
                container::Style::default()
                    .background(theme.colors.card_bg)
                    .border(
                        Border::default()
                            .color(theme.colors.border)
                            .width(1.0)
                            .rounded(crate::theme::RADIUS_SM),
                    )
            }),
        tooltip::Position::Bottom,
    )
    .into()
}
