//! The application menu tree — the single source of truth for both the
//! custom title-bar dropdown (Win/Linux) and the native muda menu (macOS).
//!
//! Built fresh by [`menu_tree`] on app start and on every language change.
//! Every label flows through the [`crate::fl!`] macro, which validates the
//! Fluent message ID against the catalogue at compile time.
//!
//! Also hosts [`find_shortcut_action`] which the keyboard-event handler
//! uses to dispatch shortcuts that aren't already routed through a focused
//! widget.

use bitwarden_vault::CipherType;

use super::{
    DynamicSubmenu,
    EnabledWhen::*,
    MenuAction::{self, *},
    MenuChildren, MenuEntry, MenuState, MenuTree,
    entry::{item, lit, section, sep},
    shortcut::{cmd, cmd_shift},
};
use crate::fl;

/// Build the full menu tree against the currently-loaded language.
/// Called once at app start and again whenever the user changes language.
pub fn menu_tree() -> MenuTree {
    MenuTree::new(vec![
        section(fl!("menu-file"), vec![
            item(fl!("menu-file-new-login"))
                .key(cmd('n'))
                .when(Unlocked)
                .action(NewItem(CipherType::Login)),
            item(fl!("menu-file-new-item")).when(Unlocked).children(vec![
                item(fl!("menu-file-new-item-login"))
                    .key(cmd_shift('l'))
                    .action(NewItem(CipherType::Login)),
                item(fl!("menu-file-new-item-card"))
                    .key(cmd_shift('c'))
                    .action(NewItem(CipherType::Card)),
                item(fl!("menu-file-new-item-identity"))
                    .key(cmd_shift('i'))
                    .action(NewItem(CipherType::Identity)),
                item(fl!("menu-file-new-item-secure-note"))
                    .key(cmd_shift('s'))
                    .action(NewItem(CipherType::SecureNote)),
                item(fl!("menu-file-new-item-ssh-key"))
                    .key(cmd_shift('k'))
                    .action(NewItem(CipherType::SshKey)),
                item(fl!("menu-file-new-item-bank-account"))
                    .key(cmd_shift('b'))
                    .action(NewItem(CipherType::BankAccount)),
                item(fl!("menu-file-new-item-drivers-license"))
                    .key(cmd_shift('d'))
                    .action(NewItem(CipherType::DriversLicense)),
                item(fl!("menu-file-new-item-passport"))
                    .key(cmd_shift('p'))
                    .action(NewItem(CipherType::Passport)),
            ]),
            item(fl!("menu-file-new-folder")).when(Unlocked).action(NewFolder),
            sep(),
            item(fl!("menu-file-sync-now")).when(HasAccounts).action(SyncNow),
            item(fl!("menu-file-import")).when(Never).action(Import),
            item(fl!("menu-file-export")).when(Unlocked).action(Export),
            sep(),
            item(fl!("menu-file-settings"))
                .key(cmd(','))
                .when(Always)
                .action(Settings),
            item(fl!("menu-file-lock-vault"))
                .when(HasLockable)
                .dynamic(DynamicSubmenu::PerLockableAccount),
            item(fl!("menu-file-lock-all-vaults"))
                .key(cmd('l'))
                .when(HasAccounts)
                .action(LockAllVaults),
            item(fl!("menu-file-log-out"))
                .when(HasAccounts)
                .dynamic(DynamicSubmenu::PerKnownAccount),
            sep(),
            item(fl!("menu-file-quit")).action(Quit),
        ]),
        section(fl!("menu-edit"), vec![
            // Undo/Redo/Cut/Copy/Paste/Select-all: text widgets handle these
            // via the keyboard already; menu wiring would need a focused-
            // widget dispatcher we haven't built. Disabled placeholders for
            // now — see docs/todo.md.
            item(fl!("menu-edit-undo")).key(cmd('z')).when(Never),
            item(fl!("menu-edit-redo")).key(cmd('y')).when(Never),
            sep(),
            item(fl!("menu-edit-cut")).key(cmd('x')).when(Never),
            item(fl!("menu-edit-copy")).key(cmd('c')).when(Never),
            item(fl!("menu-edit-paste")).key(cmd('v')).when(Never),
            sep(),
            item(fl!("menu-edit-select-all")).key(cmd('a')).when(Never),
            sep(),
            item(fl!("menu-edit-copy-username"))
                .key(cmd('u'))
                .when(Unlocked)
                .action(CopyUsername),
            item(fl!("menu-edit-copy-password"))
                .key(cmd('p'))
                .when(Unlocked)
                .action(CopyPassword),
            item(fl!("menu-edit-copy-totp"))
                .key(cmd('t'))
                .when(Unlocked)
                .action(CopyTotp),
        ]),
        section(fl!("menu-view"), vec![
            item(fl!("menu-view-search"))
                .key(cmd('f'))
                .when(Unlocked)
                .action(SearchVault),
            sep(),
            item(fl!("menu-view-generator"))
                .key(cmd('g'))
                .when(Unlocked)
                .action(Generator),
        ]),
        section(fl!("menu-account"), vec![
            item(fl!("menu-account-change-password"))
                .when(Unlocked)
                .action(OpenWebVault(Some("#/settings/security/change-master-password"))),
            item(fl!("menu-account-two-step"))
                .when(Unlocked)
                .action(OpenWebVault(Some("#/settings/security/two-factor"))),
            item(fl!("menu-account-fingerprint"))
                .when(Unlocked)
                .action(FingerprintPhrase),
            sep(),
            item(fl!("menu-account-delete"))
                .when(Unlocked)
                .action(OpenWebVault(Some("#/settings/security/delete-account"))),
        ]),
        section(fl!("menu-window"), vec![
            item(fl!("menu-window-minimize")).key(cmd('m')).action(Minimize),
            item(fl!("menu-window-hide-to-tray")).key(cmd_shift('m')).action(HideToTray),
            item(fl!("menu-window-always-on-top")).key(cmd_shift('t')).action(ToggleAlwaysOnTop),
            sep(),
            item(fl!("menu-window-close")).key(cmd('w')).action(Close),
        ]),
        section(fl!("menu-help"), vec![
            item(fl!("menu-help-feedback")).action(OpenStaticUrl("https://bitwarden.com/help")),
            item(fl!("menu-help-bug")).action(OpenStaticUrl("https://github.com/bitwarden/clients/issues")),
            item(fl!("menu-help-legal")).children(vec![
                item(fl!("menu-help-legal-tos")).action(OpenStaticUrl("https://bitwarden.com/terms/")),
                item(fl!("menu-help-legal-privacy")).action(OpenStaticUrl("https://bitwarden.com/privacy/")),
            ]),
            sep(),
            item(fl!("menu-help-follow")).children(vec![
                lit("Blog").action(OpenStaticUrl("https://blog.bitwarden.com")),
                lit("Twitter").action(OpenStaticUrl("https://twitter.com/bitwarden")),
                lit("Facebook").action(OpenStaticUrl("https://www.facebook.com/bitwarden/")),
                lit("GitHub").action(OpenStaticUrl("https://github.com/bitwarden")),
                lit("Mastodon").action(OpenStaticUrl("https://fosstodon.org/@bitwarden")),
            ]),
            sep(),
            item(fl!("menu-help-web-vault")).action(OpenWebVault(None)),
            sep(),
            item(fl!("menu-help-browser-extension")).children(vec![
                lit("Chrome").action(OpenStaticUrl(
                    "https://chromewebstore.google.com/detail/bitwarden-free-password-m/nngceckbapebfimnlniiiahkandclblb",
                )),
                lit("Firefox").action(OpenStaticUrl(
                    "https://addons.mozilla.org/firefox/addon/bitwarden-password-manager/",
                )),
                lit("Opera").action(OpenStaticUrl(
                    "https://addons.opera.com/extensions/details/bitwarden-free-password-manager/",
                )),
                lit("Edge").action(OpenStaticUrl(
                    "https://microsoftedge.microsoft.com/addons/detail/jbkfoedolllekgbhcbcoahefnbanhhlh",
                )),
                lit("Safari").action(OpenStaticUrl("https://bitwarden.com/download/")),
            ]),
            sep(),
            item(fl!("menu-help-about")).action(About),
        ]),
    ])
}

impl MenuTree {
    /// Resolve a keyboard event to a [`MenuAction`] by walking every
    /// shortcut bound in the tree. Used by the App-level keyboard handler
    /// for shortcuts that don't land on a focused widget first.
    pub fn find_shortcut_action(
        &self,
        key: &eframe::egui::Key,
        modifiers: eframe::egui::Modifiers,
        state: &MenuState,
    ) -> Option<MenuAction> {
        for sec in &self.sections {
            if let Some(action) = find_in_entries(&sec.entries, key, modifiers, state) {
                return Some(action);
            }
        }
        None
    }
}

fn find_in_entries(
    entries: &[MenuEntry],
    key: &eframe::egui::Key,
    modifiers: eframe::egui::Modifiers,
    state: &MenuState,
) -> Option<MenuAction> {
    for entry in entries {
        if let Some(s) = entry.shortcut
            && s.matches(key, modifiers)
            && entry.is_enabled(state)
            && let Some(action) = entry.action
        {
            return Some(action);
        }
        // Dynamic submenu entries don't carry shortcuts (they're pure
        // click-to-act per-account rows), so only the static branch
        // recurses.
        if let MenuChildren::Static(children) = &entry.children
            && let Some(action) = find_in_entries(children, key, modifiers, state)
        {
            return Some(action);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::menu::{EnabledWhen, shortcut::ShortcutKey};
    use eframe::egui::{Key, Modifiers};

    fn key_char(c: &str) -> Key {
        Key::from_name(&c.to_ascii_uppercase()).unwrap()
    }

    fn unlocked() -> MenuState {
        MenuState {
            is_locked: false,
            has_accounts: true,
            has_lockable_accounts: true,
        }
    }

    fn locked_no_accounts() -> MenuState {
        MenuState {
            is_locked: true,
            has_accounts: false,
            has_lockable_accounts: false,
        }
    }

    fn locked_with_accounts() -> MenuState {
        MenuState {
            is_locked: true,
            has_accounts: true,
            has_lockable_accounts: false,
        }
    }

    // The platform-aware modifier — `Modifiers::COMMAND` resolves to LOGO on
    // macOS and CTRL elsewhere, matching `Shortcut::matches`.
    fn cmd_only() -> Modifiers {
        Modifiers::COMMAND
    }
    fn cmd_shift_mods() -> Modifiers {
        Modifiers::COMMAND | Modifiers::SHIFT
    }

    // ── find_shortcut_action: enable-gates ────────────────────────────────

    #[test]
    fn cmd_n_blocked_when_locked() {
        let tree = menu_tree();
        // File → New login is `Unlocked`-gated.
        assert_eq!(
            tree.find_shortcut_action(&key_char("n"), cmd_only(), &locked_no_accounts()),
            None
        );
    }

    #[test]
    fn cmd_n_routes_to_new_login_when_unlocked() {
        let tree = menu_tree();
        let action = tree.find_shortcut_action(&key_char("n"), cmd_only(), &unlocked());
        assert_eq!(
            action,
            Some(MenuAction::NewItem(bitwarden_vault::CipherType::Login))
        );
    }

    #[test]
    fn cmd_l_requires_has_accounts() {
        let tree = menu_tree();
        // Lock all vaults requires HasAccounts even when the vault is locked.
        assert_eq!(
            tree.find_shortcut_action(&key_char("l"), cmd_only(), &locked_no_accounts()),
            None
        );
        assert_eq!(
            tree.find_shortcut_action(&key_char("l"), cmd_only(), &locked_with_accounts()),
            Some(MenuAction::LockAllVaults)
        );
    }

    #[test]
    fn never_gated_entries_never_fire() {
        let tree = menu_tree();
        // Edit → Undo is `EnabledWhen::Never`. The shortcut must always
        // return `None` so it falls through to focused text widgets.
        for state in [unlocked(), locked_no_accounts(), locked_with_accounts()] {
            assert_eq!(
                tree.find_shortcut_action(&key_char("z"), cmd_only(), &state),
                None,
                "cmd+Z fired in state {state:?}",
            );
        }
    }

    #[test]
    fn shortcut_in_submenu_resolves() {
        let tree = menu_tree();
        // File → New item → Secure note (cmd+shift+S) lives inside a sub.
        let action = tree.find_shortcut_action(&key_char("s"), cmd_shift_mods(), &unlocked());
        assert_eq!(
            action,
            Some(MenuAction::NewItem(bitwarden_vault::CipherType::SecureNote))
        );
    }

    #[test]
    fn unbound_combination_returns_none() {
        let tree = menu_tree();
        // No menu binds cmd+shift+`q`.
        assert_eq!(
            tree.find_shortcut_action(&key_char("q"), cmd_shift_mods(), &unlocked()),
            None
        );
    }

    #[test]
    fn shortcut_requires_correct_modifiers() {
        let tree = menu_tree();
        // cmd+`n` is bound; `n` alone (no modifiers) is not.
        assert_eq!(
            tree.find_shortcut_action(&key_char("n"), Modifiers::NONE, &unlocked()),
            None
        );
        // Adding shift shouldn't match either.
        assert_eq!(
            tree.find_shortcut_action(&key_char("n"), cmd_shift_mods(), &unlocked()),
            None
        );
    }

    // ── menu_tree() table-wide invariants ─────────────────────────────────

    /// Recursively collect every `(modifiers, key)` triple bound by an entry
    /// that *can* fire (anything but `Never`).
    fn collect_active_shortcuts<'a>(
        entries: &'a [MenuEntry],
        out: &mut Vec<((bool, bool, ShortcutKey), &'a str)>,
    ) {
        for entry in entries {
            if let Some(s) = entry.shortcut
                && !matches!(entry.enabled, EnabledWhen::Never)
            {
                out.push(((s.ctrl_cmd, s.shift, s.key), entry.label.as_str()));
            }
            if let MenuChildren::Static(children) = &entry.children {
                collect_active_shortcuts(children, out);
            }
        }
    }

    #[test]
    fn active_shortcuts_have_no_collisions() {
        // A duplicate `(ctrl_cmd, shift, key)` between two non-`Never` entries
        // means `find_shortcut_action` returns whichever appears first in the
        // walk — the second one is silently unreachable.
        let tree = menu_tree();
        let mut all = Vec::new();
        for sec in &tree.sections {
            collect_active_shortcuts(&sec.entries, &mut all);
        }
        for (i, (a, label_a)) in all.iter().enumerate() {
            for (b, label_b) in all.iter().skip(i + 1) {
                // `assert!` rather than `assert_ne!` to avoid requiring
                // `Debug` on `ShortcutKey` purely for test diagnostics.
                assert!(
                    a != b,
                    "shortcut collision between {label_a:?} and {label_b:?}"
                );
            }
        }
    }

    #[test]
    fn active_shortcuts_round_trip_through_muda() {
        // Each shortcut display string must be parseable as a
        // `muda::accelerator::Accelerator` — otherwise the native macOS menu
        // silently drops it. Cheap to assert here because our `display()`
        // and muda's parser are the contract we own.
        let tree = menu_tree();
        let mut all = Vec::new();
        for sec in &tree.sections {
            collect_active_shortcuts(&sec.entries, &mut all);
        }
        for ((ctrl_cmd, shift, key), label) in all {
            let s = super::super::shortcut::Shortcut {
                ctrl_cmd,
                shift,
                key,
            };
            assert!(
                s.to_accelerator().is_some(),
                "shortcut for {label:?} ({}) didn't parse as a muda accelerator",
                s.display()
            );
        }
    }
}
