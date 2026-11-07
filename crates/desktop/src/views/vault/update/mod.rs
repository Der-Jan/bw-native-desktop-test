//! `VaultView::update` dispatch + the `load_list_task` factory.
//!
//! `update()` is a thin dispatch into `handle_*` methods that each own one
//! branch of the message tree. Per-domain handlers live in:
//!
//! - [`list`] — the cipher list, filter recomputation, list-loaded.
//! - [`detail`] — read-only detail pane: clipboard ops, detail-loaded.
//! - [`form`] — edit/new form, save/delete, form-options-loaded.

mod detail;
mod form;
mod list;

use std::sync::Arc;

use bitwarden_vault::{FieldView, LoginView, SshKeyView};
use iced::{Element, Task};

use crate::{
    app::{Outcome, Overlay, RenderCtx, UpdateCtx, View},
    debug_fmt::{NoDebug, Summary},
    domain::UserId,
    services::sdk::ClientManager,
    theme::AppTheme,
};

use super::{
    VaultEvent, VaultFilter, VaultMessage,
    message::ForUserMessage,
    state::VaultView,
    widgets::{cipher_detail, search_bar::SearchMessage},
};

// ── Task factory ───────────────────────────────────────────────────────────

impl VaultView {
    /// Build the task that decrypts the user's vault list and lands as
    /// `VaultMessage::ListLoaded`. Called from App handlers (unlock, user
    /// switch, sync) — the factory lives here so all `Task::perform` calls
    /// that produce `VaultMessage`s stay within the owning view.
    pub fn load_list_task(uid: UserId, mgr: &ClientManager) -> Task<VaultMessage> {
        let Some(client) = mgr.client_for(&uid) else {
            return Task::done(VaultMessage::ListLoaded(
                uid,
                Err(format!("unknown user {uid}")),
            ));
        };
        Task::perform(
            async move {
                client.list_vault().await.map(|(items, folders)| {
                    (
                        Summary(items.into_iter().map(Arc::new).collect::<Vec<_>>()),
                        NoDebug(
                            folders
                                .into_iter()
                                .filter_map(|f| {
                                    super::widgets::cipher_edit::FolderOption::try_from(f).ok()
                                })
                                .collect(),
                        ),
                    )
                })
            },
            move |result| VaultMessage::ListLoaded(uid, result),
        )
    }
}

// ── Update dispatch ────────────────────────────────────────────────────────

impl View for VaultView {
    type Message = VaultMessage;
    type Event = VaultEvent;

    /// Compositional MVU update. Returns a task (for async work the view
    /// owns) and an optional event (cross-cutting fact for App to route).
    ///
    /// `ctx` carries the SDK handle + active user; it's built fresh on every
    /// `App::update` call so the view can construct `Task::perform` calls
    /// without owning shared state.
    fn update(&mut self, msg: VaultMessage, mut ctx: UpdateCtx<'_>) -> Outcome<Self> {
        self.sort = ctx.vault_sort;
        // Detail buttons share the SDK copy path with list/menu actions. Never
        // copy from an older detail view while a new selection is loading.
        let msg = match (&msg, self.selection.id) {
            (
                VaultMessage::CipherDetail(cipher_detail::CipherDetailMessage::CopyUsername),
                Some(id),
            ) => {
                VaultMessage::ItemList(super::widgets::item_list::ItemListMessage::CopyUsername(id))
            }
            (
                VaultMessage::CipherDetail(cipher_detail::CipherDetailMessage::CopyPassword),
                Some(id),
            ) => {
                VaultMessage::ItemList(super::widgets::item_list::ItemListMessage::CopyPassword(id))
            }
            (
                VaultMessage::CipherDetail(
                    cipher_detail::CipherDetailMessage::CopyUsername
                    | cipher_detail::CipherDetailMessage::CopyPassword,
                ),
                None,
            ) => return Outcome::None,
            _ => msg,
        };
        if let Some(outcome) = self.authorize_action(&msg, &ctx) {
            return outcome;
        }
        match msg {
            VaultMessage::MagnifyCopy(id, field) => {
                return Outcome::event(VaultEvent::MagnifyCopyAuthorized(id, field));
            }
            VaultMessage::Authorization(message) => return self.update_authorization(message, ctx),
            VaultMessage::ItemList(m) => {
                return self.handle_item_list(&ctx, m);
            }
            VaultMessage::Search(SearchMessage::QueryChanged(query)) => {
                self.search_query = query;
                if let Some(uid) = ctx.active_user {
                    self.recompute_filtered(uid, ctx.active_vault_filter);
                }
            }
            VaultMessage::CloseCipherDetail => {
                // Hide immediately, then clean up selection without a timer.
                let task = self
                    .selection
                    .sheet_fade
                    .close_with_finalize(VaultMessage::FinalizeSheetClose);
                self.pane.close();
                return Outcome::task(task);
            }
            VaultMessage::FinalizeSheetClose => {
                if !self.selection.sheet_fade.is_open() {
                    self.selection.clear();
                }
            }
            VaultMessage::PaneResized(event) => self.pane.set_ratio(event.ratio),
            VaultMessage::CipherDetail(m) => {
                return self.handle_cipher_detail(&ctx, m);
            }
            VaultMessage::CancelDeleteSelected => self.selection.confirm_delete.close(),
            VaultMessage::ConfirmDeleteSelected => {
                return self.handle_confirm_delete(&ctx);
            }
            VaultMessage::CipherEdit(m) => {
                return self.handle_cipher_edit(&ctx, m);
            }
            VaultMessage::AccountSwitcher(m) => {
                return m.route(&mut *ctx.open_overlay, Overlay::AccountSwitcher);
            }
            VaultMessage::OpenNewItemPicker => {
                return Outcome::event(VaultEvent::OpenNewItemPicker);
            }
            VaultMessage::NewItem(t) => {
                return self.handle_new_item(&mut ctx, t);
            }
            VaultMessage::ListLoaded(uid, res) => {
                return self.handle_list_loaded(&ctx, uid, res);
            }
            VaultMessage::ForUser(msg_uid, inner) => {
                // Single stale-user guard for every SDK completion: drop the
                // result if the active user changed during the await.
                if !ctx.is_active_user(&msg_uid) {
                    tracing::debug!(uid = %msg_uid, ?inner, "vault result dropped: active user changed");
                    return Outcome::None;
                }
                return match inner {
                    ForUserMessage::LoginCopied { password, result } => match result {
                        Ok(NoDebug(value)) => list::clipboard_outcome(
                            value,
                            if password {
                                crate::services::clipboard::Sensitivity::Sensitive
                            } else {
                                crate::services::clipboard::Sensitivity::Normal
                            },
                            if password {
                                crate::fl!("vault-toast-copied-password")
                            } else {
                                crate::fl!("vault-toast-copied-username")
                            },
                        ),
                        Err(_) => Outcome::toast(crate::components::toast::Toast::error(
                            crate::fl!("vault-toast-decrypt-failed-body"),
                            None,
                        )),
                    },
                    ForUserMessage::DetailLoaded(id, res) => {
                        self.handle_detail_loaded(&ctx, msg_uid, id, res)
                    }
                    ForUserMessage::FormOptionsLoaded(crate::debug_fmt::NoDebug(opts)) => {
                        self.handle_form_options_loaded(&ctx, msg_uid, opts)
                    }
                    ForUserMessage::SaveCompleted(res) => {
                        self.handle_save_completed(&ctx, msg_uid, res)
                    }
                    ForUserMessage::DeleteCompleted(id, res) => {
                        self.handle_delete_completed(&ctx, msg_uid, id, res)
                    }
                };
            }
            VaultMessage::AutoFocusSearchDelayed => {
                return Outcome::task(self.auto_focus_task());
            }
        }
        Outcome::None
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, VaultMessage, AppTheme> {
        self.render(ctx)
    }

    /// Bottom-sheet (narrow-mode cipher detail) and the delete-confirm
    /// sub-modal, in z-order: sheet first (lower), confirm on top.
    fn overlays<'a>(&'a self, ctx: &RenderCtx<'a>) -> Vec<Element<'a, VaultMessage, AppTheme>> {
        let mut overlays = crate::components::list_pane::overlays(
            self.render_sheet(ctx),
            self.render_overlay(ctx),
        );
        if let Some(prompt) = self.authorization_overlay(ctx) {
            overlays.push(prompt);
        }
        overlays
    }
}

impl VaultView {
    /// Called by App when the active vault filter changes. Clears the
    /// current selection (what's selected may no longer be in the list) and
    /// recomputes the cached list for the active user.
    pub fn apply_filter(&mut self, uid: &UserId, filter: VaultFilter) {
        self.selection.clear();
        self.pane.close();
        self.recompute_filtered(uid, filter);
    }
}

// ── Selection accessors ────────────────────────────────────────────────────
//
// Shared walk of `self.selection.detail.as_ref()` into the login / fields
// sub-objects. Removes the repeated four-line chain from every handler arm
// that needs a value to copy.

impl VaultView {
    pub(super) fn selected_login(&self) -> Option<&LoginView> {
        self.selection.detail.as_ref()?.login.as_ref()
    }

    pub(super) fn selected_field(&self, idx: usize) -> Option<&FieldView> {
        self.selection.detail.as_ref()?.fields.as_deref()?.get(idx)
    }

    pub(super) fn selected_login_uri(&self, idx: usize) -> Option<&str> {
        cipher_detail::login_uri_at(self.selected_login()?, idx)
    }

    pub(super) fn selected_ssh_key(&self) -> Option<&SshKeyView> {
        self.selection.detail.as_ref()?.ssh_key.as_ref()
    }
}

#[cfg(test)]
mod tests_sheet_close {
    use super::*;
    use crate::test_support::fixtures::make_cipher_view;
    use crate::test_support::{OutcomeExt, ViewTestExt};

    fn view_with_open_sheet() -> VaultView {
        let mut view = VaultView::new();
        view.selection.detail = Some(make_cipher_view());
        view.selection.item = Some(0);
        view.selection.sheet_fade.open();
        view
    }

    #[tokio::test(flavor = "current_thread")]
    async fn close_cipher_detail_hides_sheet_before_immediate_cleanup() {
        // Visibility changes immediately; the completion task owns cleanup.
        let mut view = view_with_open_sheet();
        assert!(view.selection.detail.is_some());
        assert!(view.selection.sheet_fade.is_open());

        let _outcome = view.run(VaultMessage::CloseCipherDetail).await;

        // sheet_fade.is_open() flips false the moment close starts.
        assert!(!view.selection.sheet_fade.is_open());
        // Cleanup has not been dispatched by this update-only test.
        assert!(
            view.selection.detail.is_some(),
            "cleanup waits for its completion message"
        );
        assert_eq!(view.selection.item, Some(0));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalize_sheet_close_clears_selection() {
        // Cleanup completion has no sleep or animation dependency.
        let mut view = view_with_open_sheet();
        view.run(VaultMessage::CloseCipherDetail).await;
        // Apply the immediate completion:
        view.run(VaultMessage::FinalizeSheetClose)
            .await
            .expect_none();

        assert!(view.selection.detail.is_none());
        assert!(view.selection.item.is_none());
        assert!(view.selection.id.is_none());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn old_sheet_cleanup_does_not_clear_reopened_selection() {
        let mut view = view_with_open_sheet();
        view.run(VaultMessage::CloseCipherDetail).await;
        view.selection.sheet_fade.open();
        view.run(VaultMessage::FinalizeSheetClose)
            .await
            .expect_none();
        assert!(view.selection.detail.is_some());
        assert!(view.selection.sheet_fade.is_visible());
    }
}
