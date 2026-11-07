//! `SendView` struct + view-local domain types.

use std::{collections::HashMap, sync::Arc};

use bitwarden_send::{SendId, SendView as SdkSendView};
use iced::Task;

use crate::{
    components::{FadeInOut, collapsible_pane::CollapsiblePane, virtual_list},
    domain::UserId,
};

use super::{SendMessage, widgets::send_edit::SendForm};

// ── View-local domain ──────────────────────────────────────────────────────

/// Filter applied to the send list. Selected from the sidebar; the sidebar
/// imports this type from here so the filter shape stays owned by the send
/// view that defines its semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendFilter {
    /// "Send" parent — every send.
    AllItems,
    Text,
    File,
}

/// The currently-selected send. Sends open directly into the edit form
/// when a row is clicked, so there's no read-only "detail" step — `form`
/// is populated either on "New" or on row click (once `DetailLoaded`
/// fires).
#[derive(Default)]
pub(super) struct Selection {
    pub(super) item: Option<usize>,
    pub(super) id: Option<SendId>,
    pub(super) form: Option<SendForm>,
    /// Armed-delete state for the inline confirmation modal.
    pub(super) confirm_delete: FadeInOut,
    /// Bottom-sheet (narrow-mode) in/out animation. Form-setting paths call
    /// `.open()`; `CloseFormPane` calls `.close()` and schedules a delayed
    /// `clear()` per the FadeInOut delayed-cleanup pattern.
    pub(super) sheet_fade: FadeInOut,
}

impl Selection {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// `all` is the full list from the SDK; `cached` is `all` with the
/// current filter + search applied.
#[derive(Default)]
pub(super) struct ItemCache {
    pub(super) all: Vec<Arc<SdkSendView>>,
    pub(super) cached: Vec<Arc<SdkSendView>>,
}

const INITIAL_FORM_PANE_RATIO: f32 = 0.4;

pub struct SendView {
    pub(super) search_query: String,
    /// Persistent list/form split. Mounted from construction and only
    /// resized to toggle form visibility — see `components::collapsible_pane`.
    pub(super) pane: CollapsiblePane,
    pub(super) selection: Selection,
    pub(super) items: HashMap<UserId, ItemCache>,
    pub(super) list_scroll: virtual_list::ScrollState,
}

impl SendView {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            pane: CollapsiblePane::new(INITIAL_FORM_PANE_RATIO),
            selection: Selection::default(),
            items: HashMap::new(),
            list_scroll: virtual_list::ScrollState::default(),
        }
    }

    /// Called by App when the active send filter changes. Mirrors
    /// `VaultView::apply_filter`.
    pub fn apply_filter(&mut self, uid: &UserId, filter: SendFilter) {
        self.selection.clear();
        self.pane.close();
        self.recompute_filtered(uid, filter);
    }

    /// Reset transient state when switching users. The per-user `items`
    /// cache is preserved in the map.
    pub fn reset(&mut self, uid: &UserId, filter: SendFilter) {
        self.search_query.clear();
        self.selection.clear();
        self.pane.close();
        self.list_scroll = virtual_list::ScrollState::default();
        self.recompute_filtered(uid, filter);
    }

    pub fn remove_user_items(&mut self, uid: &UserId) {
        self.items.remove(uid);
    }

    /// Clear the search and focus the input. Used on every transition into
    /// the Send screen (sidebar tab, user switch) and by the future
    /// `File → Search Sends` menu item.
    pub(crate) fn focus_search_task(&mut self) -> Task<SendMessage> {
        self.search_query.clear();
        iced::widget::operation::focus(super::widgets::send_list::SEND_SEARCH_ID)
    }

    /// Focus the search input *without* clearing the query. Fired on every
    /// transition into the Send screen (sidebar tab, user switch) so the
    /// user can type immediately. Mirrors `VaultView::auto_focus_task`.
    pub(crate) fn auto_focus_task(&self) -> Task<SendMessage> {
        iced::widget::operation::focus(super::widgets::send_list::SEND_SEARCH_ID)
    }
}

impl SendView {
    /// A replacement native surface has a fresh widget tree. Reapply the cached
    /// viewport so virtualization and the scrollbar agree on the first frame.
    pub fn restore_surface_scroll<M: Send + 'static>(&self) -> iced::Task<M> {
        iced::widget::operation::scroll_to(
            super::widgets::send_list::LIST_ID,
            iced::widget::scrollable::AbsoluteOffset {
                x: None,
                y: Some(self.list_scroll.offset_y),
            },
        )
    }
}
