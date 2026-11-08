//! Native muda menu (macOS app menu, optionally Win/Linux). Builds the OS-
//! level menu tree from a [`super::MenuTree`]; the OS owns the resulting
//! `Menu` for the lifetime of the process.
//!
//! Two refresh paths:
//!
//! - **Enabled state** — [`NativeMenuHandle::sync_enabled`] toggles each
//!   leaf's enabled flag against a fresh [`super::MenuState`]. Called from
//!   the frontend account-menu refresh.
//! - **Dynamic submenu contents** — [`NativeMenuHandle::sync_dynamic`]
//!   clears each dynamic submenu and rebuilds it from the current accounts.
//!   Also called from `refresh_accounts_cache`.

use std::collections::HashMap;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use muda::{Menu, PredefinedMenuItem};
use muda::{MenuItem as MudaMenuItem, Submenu};

use super::{DynamicSubmenu, EnabledWhen, MenuAction, MenuState};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use super::{MenuChildren, MenuEntry, MenuTree, should_use_native_title_bar};

use crate::services::sdk::AccountEntry;

pub struct NativeMenuHandle {
    pub actions: HashMap<muda::MenuId, MenuAction>,
    pub items: Vec<(MudaMenuItem, EnabledWhen)>,
    /// Submenu handles for [`MenuChildren::Dynamic`] entries plus the kind
    /// each one expands to. Held so [`Self::sync_dynamic`] can refill them
    /// post-attach when the live account state changes.
    dynamic_submenus: Vec<(Submenu, DynamicSubmenu)>,
    /// IDs registered by the last [`Self::sync_dynamic`] call. Evicted from
    /// `actions` on the next call so stale per-account ids don't accumulate.
    dynamic_action_ids: Vec<muda::MenuId>,
}

impl NativeMenuHandle {
    /// Resolve a muda `MenuId` to a `MenuAction` if it belongs to the native
    /// app menu. Returns `None` for unrelated ids (e.g. tray menu events).
    /// The caller drains `muda::MenuEvent::receiver()` — a global singleton
    /// shared with the tray menu — and dispatches by id.
    pub fn resolve(&self, id: &muda::MenuId) -> Option<MenuAction> {
        self.actions.get(id).copied()
    }

    pub fn sync_enabled(&self, state: &MenuState) {
        for (item, when) in &self.items {
            item.set_enabled(when.check(state));
        }
    }

    /// Refill every dynamic submenu's children from the current accounts.
    /// Clears the previous per-account muda items and registers the new
    /// ones so click → `MenuAction::LockAccount(uid)` / `LogOutAccount(uid)`
    /// keeps working across lock / unlock / login / logout events.
    pub fn sync_dynamic(&mut self, accounts: &[AccountEntry]) {
        // Drop stale per-account action mappings before re-registering.
        for id in self.dynamic_action_ids.drain(..) {
            self.actions.remove(&id);
        }

        for (submenu, kind) in &self.dynamic_submenus {
            // Clear existing children by index. `remove_at(0)` shifts later
            // items down; loop until empty.
            while !submenu.items().is_empty() {
                let _ = submenu.remove_at(0);
            }

            let resolved = kind.resolve(accounts);
            let muda_items: Vec<MudaMenuItem> = resolved
                .iter()
                .map(|entry| {
                    let item = MudaMenuItem::new(&entry.label, true, None);
                    if let Some(action) = entry.action {
                        self.actions.insert(item.id().clone(), action);
                        self.dynamic_action_ids.push(item.id().clone());
                    }
                    item
                })
                .collect();
            let refs: Vec<&dyn muda::IsMenuItem> = muda_items
                .iter()
                .map(|i| i as &dyn muda::IsMenuItem)
                .collect();
            let _ = submenu.append_items(&refs);
        }
    }
}

/// Returns `None` if native menu is not enabled.
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn attach_menu(raw_id: u64, tree: &MenuTree) -> Option<NativeMenuHandle> {
    if !should_use_native_title_bar() {
        return None;
    }

    let mut actions = HashMap::new();
    let mut items = Vec::new();
    let mut dynamic_submenus = Vec::new();

    let menu = Menu::new();
    for section in &tree.sections {
        let submenu = Submenu::new(format!("&{}", section.label), true);
        append_entries_to_submenu(
            &submenu,
            &section.entries,
            &mut actions,
            &mut items,
            &mut dynamic_submenus,
        );
        let _ = menu.append(&submenu);
    }

    #[cfg(target_os = "macos")]
    {
        let _ = raw_id;
        menu.init_for_nsapp();
    }
    #[cfg(target_os = "windows")]
    {
        unsafe {
            let _ = menu.init_for_hwnd(raw_id as isize);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        // `should_use_native_title_bar()` returns false on Linux so the
        // early return above fires first; reaching here would mean someone
        // forced the native path on an unsupported platform.
        let _ = raw_id;
        let _ = menu;
        return None;
    }

    // The `Menu` and its `MenuItem`s must outlive the process — muda's
    // `init_for_{nsapp,hwnd}` hands ownership to the OS, which holds the
    // references until shutdown. Dropping the `Menu` would leave the OS
    // with a dangling pointer. Leaking is the idiomatic way to hand
    // ownership off to the platform menu system. The `Submenu` handles we
    // kept in `dynamic_submenus` are `Arc`-backed inside muda and stay
    // valid after the leak so post-attach mutation works.
    std::mem::forget(menu);

    Some(NativeMenuHandle {
        actions,
        items,
        dynamic_submenus,
        dynamic_action_ids: Vec::new(),
    })
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn append_entries_to_submenu(
    submenu: &Submenu,
    entries: &[MenuEntry],
    actions: &mut HashMap<muda::MenuId, MenuAction>,
    items: &mut Vec<(MudaMenuItem, EnabledWhen)>,
    dynamic_submenus: &mut Vec<(Submenu, DynamicSubmenu)>,
) {
    let menu_items: Vec<Box<dyn muda::IsMenuItem>> = entries
        .iter()
        .map(|entry| -> Box<dyn muda::IsMenuItem> {
            if entry.is_separator() {
                Box::new(PredefinedMenuItem::separator())
            } else {
                match &entry.children {
                    MenuChildren::Static(children) => {
                        let sub = Submenu::new(&entry.label, true);
                        append_entries_to_submenu(&sub, children, actions, items, dynamic_submenus);
                        Box::new(sub)
                    }
                    MenuChildren::Dynamic(kind) => {
                        // Empty at attach time; first `sync_dynamic` after
                        // the SDK load populates per-account children.
                        let sub = Submenu::new(&entry.label, true);
                        dynamic_submenus.push((sub.clone(), *kind));
                        Box::new(sub)
                    }
                    MenuChildren::None => {
                        let accel = entry.shortcut.and_then(|s| s.to_accelerator());
                        let item = MudaMenuItem::new(&entry.label, true, accel);
                        if let Some(action) = entry.action {
                            actions.insert(item.id().clone(), action);
                        }
                        items.push((item.clone(), entry.enabled));
                        Box::new(item)
                    }
                }
            }
        })
        .collect();
    let refs: Vec<&dyn muda::IsMenuItem> = menu_items.iter().map(|b| b.as_ref()).collect();
    let _ = submenu.append_items(&refs);
}
