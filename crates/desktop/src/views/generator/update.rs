//! `GeneratorView::update` — message dispatch for the generator modal.

use iced::Element;

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::toast::Toast,
    fl,
    theme::AppTheme,
};

use super::{
    GeneratorView,
    message::{GeneratorEvent, GeneratorMessage},
    state::{Mode, accept_digits, bump_clamped},
};

impl View for GeneratorView {
    type Message = GeneratorMessage;
    type Event = GeneratorEvent;

    fn update(&mut self, msg: GeneratorMessage, ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            GeneratorMessage::Close => {
                self.fade.close();
                Outcome::None
            }
            GeneratorMessage::SelectTab(tab) => {
                if self.active_tab == tab {
                    return Outcome::None;
                }
                self.active_tab = tab;
                self.current = None;
                self.regenerate_event()
            }
            GeneratorMessage::ShowHistory => {
                self.mode = Mode::History;
                Outcome::None
            }
            GeneratorMessage::BackToGenerator => {
                self.mode = Mode::Generator;
                // Seed the value card if the modal was opened directly into
                // history mode and the active tab has never generated yet.
                if self.current.is_none() {
                    return self.regenerate_event();
                }
                Outcome::None
            }
            GeneratorMessage::ClearHistory => {
                self.history.clear();
                Outcome::event(GeneratorEvent::ClearHistory)
            }
            GeneratorMessage::CopyCurrent => match self.current.clone() {
                Some(v) if !v.is_empty() => Outcome::event(GeneratorEvent::Copy(v)),
                _ => Outcome::None,
            },
            GeneratorMessage::CopyHistoryEntry(idx) => {
                // History is rendered reverse-chronologically; the idx the
                // row passes is into that reversed view, so translate back
                // to the storage order (oldest-first).
                let len = self.history.len();
                if idx >= len {
                    return Outcome::None;
                }
                let storage_idx = len - 1 - idx;
                let value = self.history[storage_idx].value.clone();
                Outcome::event(GeneratorEvent::Copy(value))
            }
            GeneratorMessage::Regenerate => self.regenerate_event(),
            // Password tab
            GeneratorMessage::SetLength(raw) => {
                self.set_digit_field(raw, |s, r| s.password.length = r)
            }
            GeneratorMessage::BumpLength(delta) => self.mutate_and_regen(|s| {
                s.password.length = bump_clamped(&s.password.length, delta, 14, 5, 128);
            }),
            GeneratorMessage::ToggleLowercase(v) => {
                self.mutate_and_regen(|s| s.password.lowercase = v)
            }
            GeneratorMessage::ToggleUppercase(v) => {
                self.mutate_and_regen(|s| s.password.uppercase = v)
            }
            GeneratorMessage::ToggleNumbers(v) => self.mutate_and_regen(|s| s.password.numbers = v),
            GeneratorMessage::ToggleSpecial(v) => self.mutate_and_regen(|s| s.password.special = v),
            GeneratorMessage::SetMinNumber(raw) => {
                self.set_digit_field(raw, |s, r| s.password.min_number = r)
            }
            GeneratorMessage::BumpMinNumber(delta) => self.mutate_and_regen(|s| {
                s.password.min_number = bump_clamped(&s.password.min_number, delta, 1, 0, 9);
            }),
            GeneratorMessage::SetMinSpecial(raw) => {
                self.set_digit_field(raw, |s, r| s.password.min_special = r)
            }
            GeneratorMessage::BumpMinSpecial(delta) => self.mutate_and_regen(|s| {
                s.password.min_special = bump_clamped(&s.password.min_special, delta, 1, 0, 9);
            }),
            GeneratorMessage::ToggleAvoidAmbiguous(v) => {
                self.mutate_and_regen(|s| s.password.avoid_ambiguous = v)
            }
            // Passphrase tab
            GeneratorMessage::SetNumWords(raw) => {
                self.set_digit_field(raw, |s, r| s.passphrase.num_words = r)
            }
            GeneratorMessage::BumpNumWords(delta) => self.mutate_and_regen(|s| {
                s.passphrase.num_words = bump_clamped(&s.passphrase.num_words, delta, 6, 3, 20);
            }),
            GeneratorMessage::SetWordSeparator(s) => self.mutate_and_regen(|st| {
                // Single-char only — matches the official client even though
                // the SDK accepts a full String here.
                st.passphrase.word_separator = s.chars().take(1).collect();
            }),
            GeneratorMessage::TogglePassphraseCapitalize(v) => {
                self.mutate_and_regen(|s| s.passphrase.capitalize = v)
            }
            GeneratorMessage::TogglePassphraseIncludeNumber(v) => {
                self.mutate_and_regen(|s| s.passphrase.include_number = v)
            }
            // Username tab
            GeneratorMessage::SelectUsernameKind(k) => {
                self.mutate_and_regen(|s| s.username.kind = k)
            }
            GeneratorMessage::ToggleUsernameCapitalize(v) => {
                self.mutate_and_regen(|s| s.username.capitalize = v)
            }
            GeneratorMessage::ToggleUsernameIncludeNumber(v) => {
                self.mutate_and_regen(|s| s.username.include_number = v)
            }
            GeneratorMessage::SetEmail(s) => self.mutate_and_regen(|st| st.username.email = s),
            GeneratorMessage::SetDomain(s) => self.mutate_and_regen(|st| st.username.domain = s),
            GeneratorMessage::Generated(Ok(value)) => {
                self.current = Some(value.clone());
                if let Some(uid) = ctx.active_user.copied() {
                    self.history = ctx.client_manager.push_history(uid, value);
                }
                Outcome::None
            }
            GeneratorMessage::Generated(Err(err)) => {
                tracing::warn!(%err, "generator request failed");
                Outcome::toast(Toast::warning(err, Some(&fl!("generator-toast-failed"))))
            }
        }
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, GeneratorMessage, AppTheme> {
        self.render(ctx)
    }
}

impl GeneratorView {
    pub(super) fn regenerate_event(&self) -> Outcome<Self> {
        Outcome::event(GeneratorEvent::Generate(self.current_request()))
    }

    fn mutate_and_regen(&mut self, f: impl FnOnce(&mut Self)) -> Outcome<Self> {
        f(self);
        self.regenerate_event()
    }

    fn set_digit_field(
        &mut self,
        raw: String,
        assign: impl FnOnce(&mut Self, String),
    ) -> Outcome<Self> {
        if accept_digits(&raw) {
            assign(self, raw);
            self.regenerate_event()
        } else {
            Outcome::None
        }
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::{
        services::sdk::PasswordHistoryEntry,
        test_support::{OutcomeExt, ViewTestExt},
    };

    fn entry(value: &str) -> PasswordHistoryEntry {
        PasswordHistoryEntry {
            value: value.to_string(),
            created: chrono::Utc::now(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn copy_history_entry_idx_zero_is_newest() {
        // History is stored oldest-first but rendered reverse-chronologically.
        // Row index 0 maps to the last storage entry (newest).
        let mut view = GeneratorView::new();
        view.set_history(vec![entry("oldest"), entry("middle"), entry("newest")]);

        let ev = view
            .run(GeneratorMessage::CopyHistoryEntry(0))
            .await
            .expect_event();
        match ev {
            GeneratorEvent::Copy(v) => assert_eq!(v, "newest"),
            _ => panic!("expected Copy"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn copy_history_entry_last_idx_is_oldest() {
        // Mirrors the previous test in the other direction: the last
        // rendered row (idx == len-1) maps to storage index 0.
        let mut view = GeneratorView::new();
        view.set_history(vec![entry("oldest"), entry("middle"), entry("newest")]);

        let ev = view
            .run(GeneratorMessage::CopyHistoryEntry(2))
            .await
            .expect_event();
        match ev {
            GeneratorEvent::Copy(v) => assert_eq!(v, "oldest"),
            _ => panic!("expected Copy"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn copy_history_entry_out_of_bounds_returns_none() {
        // Defensive: a row click for an index past the cache shouldn't
        // crash. Could happen on rapid clear-then-click sequences.
        let mut view = GeneratorView::new();
        view.set_history(vec![entry("only")]);

        view.run(GeneratorMessage::CopyHistoryEntry(5))
            .await
            .expect_none();
    }
}
