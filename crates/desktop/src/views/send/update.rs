use std::sync::Arc;

use bitwarden_send::{SendId, SendType, SendView as SdkSendView};
use iced::{Element, Task};

use crate::{
    app::{Outcome, Overlay, RenderCtx, UpdateCtx, View},
    components::toast::Toast,
    debug_fmt::{NoDebug, Summary},
    domain::UserId,
    fl,
    services::sdk::ClientManager,
    theme::AppTheme,
};

use super::{
    SendEvent, SendFilter, SendMessage,
    message::ForUserMessage,
    state::SendView,
    widgets::{
        send_edit::{FormEvent, SendEditMessage, SendForm},
        send_list::{SearchMessage, SendListMessage},
    },
};

// ── Task factory ───────────────────────────────────────────────────────────

impl SendView {
    /// Sync stub today (no SDK encrypt/decrypt yet); returned as a `Task` so
    /// the call shape lines up with `VaultView::load_list_task` and a future
    /// SDK-driven async path is a drop-in.
    pub fn load_list_task(uid: UserId, mgr: &ClientManager) -> Task<SendMessage> {
        let items: Vec<Arc<SdkSendView>> = mgr.list_sends(&uid).into_iter().map(Arc::new).collect();
        Task::done(SendMessage::ListLoaded(uid, Ok(Summary(items))))
    }
}

// ── Update dispatch ────────────────────────────────────────────────────────

impl View for SendView {
    type Message = SendMessage;
    type Event = SendEvent;

    fn update(&mut self, msg: SendMessage, mut ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            SendMessage::ItemList(m) => {
                return self.handle_item_list(&ctx, m);
            }
            SendMessage::Search(SearchMessage::QueryChanged(query)) => {
                self.search_query = query;
                if let Some(uid) = ctx.active_user {
                    self.recompute_filtered(uid, ctx.active_send_filter);
                }
            }
            SendMessage::CloseFormPane => {
                // Hide immediately, then clean up selection without a timer.
                let task = self
                    .selection
                    .sheet_fade
                    .close_with_finalize(SendMessage::FinalizeSheetClose);
                self.pane.close();
                return Outcome::task(task);
            }
            SendMessage::FinalizeSheetClose => {
                if !self.selection.sheet_fade.is_open() {
                    self.selection.clear();
                }
            }
            SendMessage::PaneResized(event) => self.pane.set_ratio(event.ratio),
            SendMessage::SendEdit(m) => {
                return self.handle_send_edit(&mut ctx, m);
            }
            SendMessage::CancelDeleteSelected => self.selection.confirm_delete.close(),
            SendMessage::ConfirmDeleteSelected => {
                return self.handle_confirm_delete(&mut ctx);
            }
            SendMessage::NewItem => {
                self.handle_new_item(&ctx);
            }
            SendMessage::AccountSwitcher(m) => {
                return m.route(&mut *ctx.open_overlay, Overlay::AccountSwitcher);
            }
            SendMessage::ListLoaded(uid, res) => {
                return self.handle_list_loaded(&ctx, uid, res);
            }
            SendMessage::ForUser(msg_uid, inner) => {
                // Single stale-user guard for every SDK completion: drop the
                // result if the active user changed during the await.
                if !ctx.is_active_user(&msg_uid) {
                    tracing::debug!(uid = %msg_uid, ?inner, "send result dropped: active user changed");
                    return Outcome::None;
                }
                return match inner {
                    ForUserMessage::DetailLoaded(id, res) => {
                        self.handle_detail_loaded(&ctx, msg_uid, id, res)
                    }
                    ForUserMessage::SaveCompleted(res) => {
                        self.handle_save_completed(&ctx, msg_uid, res)
                    }
                    ForUserMessage::DeleteCompleted(id, res) => {
                        self.handle_delete_completed(&ctx, msg_uid, id, res)
                    }
                };
            }
            SendMessage::PasswordGenerated(res) => {
                // Mirror the official client: every generated value lands in
                // the user's history, regardless of which form triggered it.
                if let Ok(NoDebug(value)) = &res
                    && let Some(uid) = ctx.active_user.copied()
                {
                    ctx.client_manager.push_history(uid, value.clone());
                }
                return self.handle_password_generated(res);
            }
        }
        Outcome::None
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, SendMessage, AppTheme> {
        self.render(ctx)
    }

    fn overlays<'a>(&'a self, ctx: &RenderCtx<'a>) -> Vec<Element<'a, SendMessage, AppTheme>> {
        crate::components::list_pane::overlays(self.render_sheet(ctx), self.render_overlay(ctx))
    }
}

// ── Per-variant handlers ───────────────────────────────────────────────────

impl SendView {
    fn handle_item_list(&mut self, ctx: &UpdateCtx<'_>, msg: SendListMessage) -> Outcome<Self> {
        match msg {
            SendListMessage::ItemSelected(idx) => {
                self.selection.item = Some(idx);
                self.selection.id = ctx
                    .active_user
                    .and_then(|uid| self.items.get(uid))
                    .and_then(|ic| ic.cached.get(idx))
                    .and_then(|i| i.id);
                let Some(id) = self.selection.id else {
                    return Outcome::None;
                };
                let Some(uid) = ctx.active_user.copied() else {
                    return Outcome::None;
                };
                // Sync stub today; deliver via `Task::done` so the rest of
                // the view's flow (loading state, stale-uid guard) keeps the
                // same shape as the future async path.
                let result = ctx
                    .client_manager
                    .full_send(&uid, id)
                    .map(|v| NoDebug(Box::new(v)))
                    .ok_or_else(|| format!("send {id} not found"));
                return Outcome::task(Task::done(SendMessage::ForUser(
                    uid,
                    ForUserMessage::DetailLoaded(id, result),
                )));
            }
            SendListMessage::Scrolled(viewport) => {
                self.list_scroll.track(viewport);
            }
            SendListMessage::MoreOptions(_) => {}
        }
        Outcome::None
    }

    fn handle_new_item(&mut self, ctx: &UpdateCtx<'_>) {
        // The sub-filter dictates the default type. File sends currently have
        // no creation path beyond the placeholder "Choose file" button — the
        // form renders a disabled filename/size for them.
        let send_type = match ctx.active_send_filter {
            SendFilter::File => SendType::File,
            SendFilter::Text | SendFilter::AllItems => SendType::Text,
        };
        self.selection.clear();
        self.selection.form = Some(SendForm::new(send_type));
        self.selection.sheet_fade.open();
        self.pane.open();
    }

    fn handle_send_edit(&mut self, ctx: &mut UpdateCtx<'_>, msg: SendEditMessage) -> Outcome<Self> {
        let Some(form) = self.selection.form.as_mut() else {
            return Outcome::None;
        };
        match form.update(msg) {
            FormEvent::None => Outcome::None,
            FormEvent::Cancel => {
                let task = self
                    .selection
                    .sheet_fade
                    .close_with_finalize(SendMessage::FinalizeSheetClose);
                self.pane.close();
                Outcome::task(task)
            }
            FormEvent::Save => {
                if !form.is_valid() {
                    return Outcome::toast(Toast::warning(fl!("toast-required-fields"), None));
                }
                let Some(uid) = ctx.active_user.copied() else {
                    return Outcome::None;
                };
                form.saving = true;
                let view = form.to_send_view();
                // Sync save (no SDK encrypt yet); commit + dispatch the
                // completion message so handle_save_completed runs the same
                // path as the future async case.
                let saved = ctx.client_manager.save_send(uid, view);
                Outcome::task(Task::done(SendMessage::ForUser(
                    uid,
                    ForUserMessage::SaveCompleted(Ok(NoDebug(Box::new(saved)))),
                )))
            }
            FormEvent::Delete => {
                self.selection.confirm_delete.open();
                Outcome::None
            }
            FormEvent::CopyLink(url) => Outcome::event(SendEvent::ClipboardCopyRequested {
                value: url,
                sensitivity: crate::services::clipboard::Sensitivity::Normal,
                toast_label: fl!("send-toast-copied-link"),
            }),
            FormEvent::CopyPassword(pw) => Outcome::event(SendEvent::ClipboardCopyRequested {
                value: pw,
                sensitivity: crate::services::clipboard::Sensitivity::Sensitive,
                toast_label: fl!("send-toast-copied-password"),
            }),
            FormEvent::RegeneratePassword => Outcome::event(SendEvent::RegeneratePasswordRequested),
        }
    }

    fn handle_confirm_delete(&mut self, ctx: &mut UpdateCtx<'_>) -> Outcome<Self> {
        self.selection.confirm_delete.close();
        let Some(send_id) = self.selection.id else {
            // New-item form hasn't been saved yet — "delete" just dismisses
            // the draft via the same immediate sheet cleanup as cancel.
            let task = self
                .selection
                .sheet_fade
                .close_with_finalize(SendMessage::FinalizeSheetClose);
            self.pane.close();
            return Outcome::task(task);
        };
        let Some(uid) = ctx.active_user.copied() else {
            return Outcome::None;
        };
        ctx.client_manager.delete_send(&uid, send_id);
        Outcome::task(Task::done(SendMessage::ForUser(
            uid,
            ForUserMessage::DeleteCompleted(send_id, Ok(())),
        )))
    }

    fn handle_list_loaded(
        &mut self,
        ctx: &UpdateCtx<'_>,
        msg_uid: UserId,
        result: Result<Summary<Vec<Arc<SdkSendView>>>, String>,
    ) -> Outcome<Self> {
        match result {
            Ok(Summary(items)) => {
                tracing::info!(uid = %msg_uid, count = items.len(), "send list loaded");
                let cache = self.items.entry(msg_uid).or_default();
                cache.all = items;
                if ctx.is_active_user(&msg_uid) {
                    self.recompute_filtered(&msg_uid, ctx.active_send_filter);
                }
            }
            Err(err) => {
                tracing::error!(uid = %msg_uid, %err, "list_sends failed");
            }
        }
        Outcome::None
    }

    /// Caller (`SendMessage::ForUser` dispatch) has already verified the
    /// uid matches the active user.
    fn handle_detail_loaded(
        &mut self,
        _ctx: &UpdateCtx<'_>,
        _msg_uid: UserId,
        id: SendId,
        result: Result<NoDebug<Box<SdkSendView>>, String>,
    ) -> Outcome<Self> {
        match result {
            Ok(NoDebug(view)) => {
                if view.r#type == SendType::Item {
                    return Outcome::toast(Toast::warning(fl!("send-item-unsupported"), None));
                }
                if self.selection.id == Some(id) {
                    self.selection.form = Some(SendForm::edit(*view));
                    self.selection.sheet_fade.open();
                    self.pane.open();
                }
            }
            Err(err) => {
                tracing::error!(send_id = %id, %err, "full_send failed");
                return Outcome::toast(Toast::error(
                    fl!("send-toast-load-failed-body"),
                    Some(&fl!("send-toast-load-failed-title")),
                ));
            }
        }
        Outcome::None
    }

    /// Caller (`SendMessage::ForUser` dispatch) has already verified the
    /// uid matches the active user.
    fn handle_save_completed(
        &mut self,
        _ctx: &UpdateCtx<'_>,
        msg_uid: UserId,
        result: Result<NoDebug<Box<SdkSendView>>, String>,
    ) -> Outcome<Self> {
        match result {
            Ok(NoDebug(view)) => {
                // Re-bind selection to the persisted id (new sends start with
                // id=None; `save_send` assigns one) so subsequent reloads
                // keep the form open.
                self.selection.id = view.id;
                self.selection.form = Some(SendForm::edit(*view));
                self.selection.sheet_fade.open();
                Outcome::event(SendEvent::ItemSaved { uid: msg_uid })
            }
            Err(err) => {
                tracing::error!(%err, "save_send failed");
                if let Some(form) = self.selection.form.as_mut() {
                    form.saving = false;
                }
                Outcome::toast(Toast::error(
                    fl!("send-toast-save-failed-body"),
                    Some(&fl!("send-toast-save-failed-title")),
                ))
            }
        }
    }

    fn handle_password_generated(
        &mut self,
        result: Result<NoDebug<String>, String>,
    ) -> Outcome<Self> {
        match result {
            Ok(NoDebug(value)) => {
                if let Some(form) = self.selection.form.as_mut() {
                    form.apply_generated_password(value);
                }
                Outcome::None
            }
            Err(err) => {
                tracing::warn!(%err, "send password regenerate failed");
                Outcome::toast(Toast::warning(err, Some(&fl!("generator-toast-failed"))))
            }
        }
    }

    /// Caller (`SendMessage::ForUser` dispatch) has already verified the
    /// uid matches the active user.
    fn handle_delete_completed(
        &mut self,
        _ctx: &UpdateCtx<'_>,
        msg_uid: UserId,
        send_id: SendId,
        result: Result<(), String>,
    ) -> Outcome<Self> {
        match result {
            Ok(()) => {
                self.selection.clear();
                self.pane.close();
                Outcome::event(SendEvent::ItemDeleted { uid: msg_uid })
            }
            Err(err) => {
                tracing::error!(send_id = %send_id, %err, "delete_send failed");
                Outcome::toast(Toast::error(
                    fl!("send-toast-delete-failed-body"),
                    Some(&fl!("send-toast-delete-failed-title")),
                ))
            }
        }
    }
}

// ── Filter helpers ─────────────────────────────────────────────────────────

impl SendView {
    pub(super) fn recompute_filtered(&mut self, uid: &UserId, filter: SendFilter) {
        let query = self.search_query.to_lowercase();
        if let Some(cache) = self.items.get_mut(uid) {
            cache.cached = filter_items(&cache.all, filter, &query);
        }
        if let Some(id) = self.selection.id {
            self.selection.item = self
                .items
                .get(uid)
                .and_then(|c| c.cached.iter().position(|i| i.id == Some(id)));
        } else {
            self.selection.item = None;
        }
        self.list_scroll.offset_y = 0.0;
    }
}

fn filter_items(
    all: &[Arc<SdkSendView>],
    filter: SendFilter,
    query: &str,
) -> Vec<Arc<SdkSendView>> {
    let items = all.iter().filter(|item| match filter {
        SendFilter::AllItems => true,
        SendFilter::Text => matches!(item.r#type, SendType::Text),
        SendFilter::File => matches!(item.r#type, SendType::File),
    });
    if query.is_empty() {
        items.cloned().collect()
    } else {
        items
            .filter(|item| item.name.to_lowercase().contains(query))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::{
        app::App, debug_fmt::NoDebug, domain::UserId, test_support::OutcomeExt,
        views::send::message::ForUserMessage,
    };

    /// Build a fake SaveCompleted result so we don't have to construct a
    /// full `SdkSendView` in tests that only care about the dispatch.
    fn save_completed_payload() -> Result<NoDebug<Box<SdkSendView>>, String> {
        Err("test stub".to_string())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn for_user_drops_message_when_active_user_changed() {
        // The dispatch site in `SendView::update` checks `is_active_user`
        // once per ForUser batch and drops the entire message if the user
        // switched during the await. This is the load-bearing guard; the
        // individual handlers below the dispatch trust the uid is current.
        let active = UserId::new_v4();
        let stale = UserId::new_v4();

        let mut app = App::test();
        app.active_user = Some(active);

        let mut view = SendView::new();
        view.update(
            SendMessage::ForUser(
                stale,
                ForUserMessage::SaveCompleted(save_completed_payload()),
            ),
            app.update_ctx(),
        )
        .expect_none();
        // Nothing to assert on view state — the message was dropped before
        // any handler ran. The point of the test is that no panic / no
        // event fires when uid != active.
    }

    #[tokio::test(flavor = "current_thread")]
    async fn for_user_dispatches_when_uid_matches_active() {
        let uid = UserId::new_v4();
        let mut app = App::test();
        app.active_user = Some(uid);

        let mut view = SendView::new();
        // Err payload → handle_save_completed emits an error toast. The
        // assertion here is "we got past the stale-user guard and reached
        // the handler" (verified by the toast emission).
        let toast = view
            .update(
                SendMessage::ForUser(uid, ForUserMessage::SaveCompleted(save_completed_payload())),
                app.update_ctx(),
            )
            .expect_toast();
        assert!(matches!(
            toast.status,
            crate::components::toast::ToastStatus::Error
        ));
    }
}
