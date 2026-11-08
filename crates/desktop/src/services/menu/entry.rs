//! Menu data model — the tree, sections, entries, and the dynamic-children
//! mechanism.
//!
//! [`MenuTree`] / [`MenuSection`] / [`MenuEntry`] form a three-tier shape:
//! the tree owns top-level sections (File, Edit, …), each section owns
//! ordered entries, and entries are either leaves (shortcut + action) or
//! submenus whose children are fixed at build time
//! ([`MenuChildren::Static`]) or computed from live state at render time
//! ([`MenuChildren::Dynamic`]).
//!
//! Labels are owned `String`s constructed via the [`crate::fl!`] macro,
//! which validates against the Fluent catalogue at compile time. The whole
//! tree is rebuilt by [`super::menu_tree`] on language change.

use std::borrow::Cow;

use bitwarden_vault::CipherType;

use super::shortcut::Shortcut;
use crate::{domain::UserId, services::sdk::AccountEntry};

// ── Enable gating ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MenuState {
    pub is_locked: bool,
    pub has_accounts: bool,
    pub has_lockable_accounts: bool,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            is_locked: true,
            has_accounts: false,
            has_lockable_accounts: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum EnabledWhen {
    Always,
    Unlocked,
    HasAccounts,
    HasLockable,
    /// Permanently disabled — entry renders grayed-out. Used as a placeholder
    /// for menu items that exist on the official client but have no behaviour
    /// in this stub (Edit → Undo / Redo / Cut / Copy / Paste / Select all).
    /// Keyboard shortcuts still pass through to focused text widgets
    /// unaffected, since they have no [`MenuAction`].
    Never,
}

impl EnabledWhen {
    pub fn check(self, state: &MenuState) -> bool {
        match self {
            Self::Always => true,
            Self::Unlocked => !state.is_locked,
            Self::HasAccounts => state.has_accounts,
            Self::HasLockable => state.has_lockable_accounts,
            Self::Never => false,
        }
    }
}

// ── Actions ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    Quit,
    LockAllVaults,
    /// Lock a specific user's vault. Emitted by the File → Lock vault
    /// per-account submenu ([`DynamicSubmenu::PerLockableAccount`]).
    LockAccount(UserId),
    /// Log a specific user out. Emitted by the File → Log out per-account
    /// submenu ([`DynamicSubmenu::PerKnownAccount`]).
    LogOutAccount(UserId),
    SyncNow,
    SearchVault,
    Minimize,
    HideToTray,
    ToggleAlwaysOnTop,
    Close,
    About,
    Settings,
    Generator,
    GeneratorHistory,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Import,
    Export,
    NewFolder,
    /// Open the vault with an empty cipher form of the given type.
    /// Wired from the File → New login (Cmd+N) entry, the File → New item
    /// submenu (Cmd+Shift+L/C/I/S/K), and the +New dropdown.
    NewItem(CipherType),
    CopyUsername,
    CopyPassword,
    CopyTotp,
    FingerprintPhrase,
    /// Static external URL — Help-menu social/store/legal links.
    OpenStaticUrl(&'static str),
    /// The active user's web vault. `None` opens the root; `Some("#/path")`
    /// appends an in-app route (account-menu items).
    OpenWebVault(Option<&'static str>),
}

// ── Tree / section / entry ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct MenuTree {
    pub sections: Vec<MenuSection>,
}

impl MenuTree {
    pub fn new(sections: Vec<MenuSection>) -> Self {
        Self { sections }
    }
}

#[derive(Clone)]
pub struct MenuSection {
    pub label: String,
    pub entries: Vec<MenuEntry>,
}

#[derive(Clone)]
pub struct MenuEntry {
    pub label: String,
    pub shortcut: Option<Shortcut>,
    pub enabled: EnabledWhen,
    pub action: Option<MenuAction>,
    pub children: MenuChildren,
}

/// Children of a [`MenuEntry`]. The three variants make the leaf case
/// (`None`), the fixed-set case (`Static`), and the live-state case
/// (`Dynamic`) explicit at the type level. Renderers walk via
/// [`MenuEntry::effective_children`] so the static / dynamic split is
/// invisible to them.
#[derive(Clone)]
pub enum MenuChildren {
    None,
    Static(Vec<MenuEntry>),
    Dynamic(DynamicSubmenu),
}

/// Names a class of runtime-computed submenu. Fully resolved at render
/// time from the active accounts via [`DynamicSubmenu::resolve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicSubmenu {
    /// One entry per currently-unlocked account; clicking locks that
    /// account. Combined with [`EnabledWhen::HasLockable`] so the parent
    /// is disabled when no entries would appear.
    PerLockableAccount,
    /// One entry per known account; clicking logs that account out.
    /// Combined with [`EnabledWhen::HasAccounts`].
    PerKnownAccount,
}

impl DynamicSubmenu {
    pub fn resolve(self, accounts: &[AccountEntry]) -> Vec<MenuEntry> {
        match self {
            Self::PerLockableAccount => accounts
                .iter()
                .filter(|a| !a.locked)
                .map(|a| lit(&a.email).action(MenuAction::LockAccount(a.user_id)))
                .collect(),
            Self::PerKnownAccount => accounts
                .iter()
                .map(|a| lit(&a.email).action(MenuAction::LogOutAccount(a.user_id)))
                .collect(),
        }
    }
}

impl MenuEntry {
    pub fn key(mut self, s: Shortcut) -> Self {
        self.shortcut = Some(s);
        self
    }
    pub fn when(mut self, e: EnabledWhen) -> Self {
        self.enabled = e;
        self
    }
    pub fn action(mut self, a: MenuAction) -> Self {
        self.action = Some(a);
        self
    }
    pub fn children(mut self, items: Vec<MenuEntry>) -> Self {
        self.children = MenuChildren::Static(items);
        self
    }
    pub fn dynamic(mut self, kind: DynamicSubmenu) -> Self {
        self.children = MenuChildren::Dynamic(kind);
        self
    }

    pub fn is_separator(&self) -> bool {
        self.label.is_empty()
            && matches!(self.children, MenuChildren::None)
            && self.action.is_none()
    }

    pub fn is_submenu(&self) -> bool {
        !matches!(self.children, MenuChildren::None)
    }

    pub fn is_enabled(&self, state: &MenuState) -> bool {
        self.enabled.check(state)
    }

    /// Children to render this frame. Borrows the fixed slice for static
    /// submenus; expands a [`DynamicSubmenu`] against `accounts` into an
    /// owned `Vec` otherwise. Both renderers walk this uniformly so the
    /// static / dynamic split stays out of their loops.
    pub fn effective_children<'a>(&'a self, accounts: &[AccountEntry]) -> Cow<'a, [MenuEntry]> {
        match &self.children {
            MenuChildren::None => Cow::Borrowed(&[]),
            MenuChildren::Static(v) => Cow::Borrowed(v.as_slice()),
            MenuChildren::Dynamic(kind) => Cow::Owned(kind.resolve(accounts)),
        }
    }

    pub fn shortcut_display(&self) -> Option<String> {
        self.shortcut.map(|s| s.display())
    }
}

// ── Builders ───────────────────────────────────────────────────────────────

/// Build a top-level menu section (File, Edit, …).
pub fn section(label: String, entries: Vec<MenuEntry>) -> MenuSection {
    MenuSection { label, entries }
}

/// Build a menu entry with the given label. Chain `.key()`, `.when()`,
/// `.action()`, `.children()`, or `.dynamic()` to configure.
pub fn item(label: String) -> MenuEntry {
    MenuEntry {
        label,
        shortcut: None,
        enabled: EnabledWhen::Always,
        action: None,
        children: MenuChildren::None,
    }
}

/// Visual separator inside a menu — empty-label entry with no action.
pub fn sep() -> MenuEntry {
    MenuEntry {
        label: String::new(),
        shortcut: None,
        enabled: EnabledWhen::Always,
        action: None,
        children: MenuChildren::None,
    }
}

/// Entry with a **literal** label — rendered verbatim, no Fluent lookup.
/// Use for brand and platform names (Chrome, iOS, …) that shouldn't be
/// localized. The runtime resolver for [`DynamicSubmenu`] also uses this
/// for email labels.
pub fn lit(s: &str) -> MenuEntry {
    item(s.to_string())
}
