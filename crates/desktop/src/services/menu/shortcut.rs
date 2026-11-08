//! Keyboard shortcut representation. Used by both the custom title-bar menu
//! (which renders the shortcut display text alongside the label) and the
//! native muda menu (which converts to `muda::accelerator::Accelerator`).

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKey {
    Char(char),
}

#[derive(Clone, Copy)]
pub struct Shortcut {
    /// Ctrl on Win/Linux, Cmd on macOS.
    pub ctrl_cmd: bool,
    pub shift: bool,
    pub key: ShortcutKey,
}

pub(super) const fn cmd(key: char) -> Shortcut {
    Shortcut {
        ctrl_cmd: true,
        shift: false,
        key: ShortcutKey::Char(key),
    }
}

pub(super) const fn cmd_shift(key: char) -> Shortcut {
    Shortcut {
        ctrl_cmd: true,
        shift: true,
        key: ShortcutKey::Char(key),
    }
}

impl Shortcut {
    /// Platform-aware display text (e.g. "Ctrl+N" on Win, "Cmd+N" on Mac).
    pub fn display(&self) -> String {
        let mut s = String::new();
        if cfg!(target_os = "macos") {
            if self.ctrl_cmd {
                s.push_str("Cmd+");
            }
            if self.shift {
                s.push_str("Shift+");
            }
        } else {
            if self.ctrl_cmd {
                s.push_str("Ctrl+");
            }
            if self.shift {
                s.push_str("Shift+");
            }
        }
        match self.key {
            ShortcutKey::Char(c) if c.is_ascii_alphabetic() => s.push(c.to_ascii_uppercase()),
            ShortcutKey::Char(c) => s.push(c),
        }
        s
    }

    pub fn to_accelerator(self) -> Option<muda::accelerator::Accelerator> {
        self.display().parse().ok()
    }

    pub fn matches(&self, key: &eframe::egui::Key, modifiers: eframe::egui::Modifiers) -> bool {
        if modifiers.command != self.ctrl_cmd {
            return false;
        }
        if modifiers.shift != self.shift {
            return false;
        }

        let ShortcutKey::Char(character) = self.key;
        eframe::egui::Key::from_name(&character.to_ascii_uppercase().to_string()) == Some(*key)
            && !modifiers.alt
    }
}
