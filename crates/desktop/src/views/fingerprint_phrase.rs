//! Account → Fingerprint phrase modal.
//!
//! Single-shot dialog: opened from the menu with a precomputed phrase, closed
//! via the Close button or backdrop click. The "Learn more" link opens
//! Bitwarden's fingerprint help page in the default browser. The `Copy`
//! action surfaces as an event because pushing to the clipboard + emitting
//! the success toast lives at the App level — both reach state outside
//! `UpdateCtx` (the [`crate::services::clipboard::ClipboardManager`] and the
//! toast sink).

use iced::{
    Alignment, Element, Length, Padding,
    widget::{row, text, text::Wrapping},
};

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{FadeInOut, buttons, icons, modal},
    fl,
    services::clipboard,
    theme::AppTheme,
};

/// Actions emitted from the Account → Fingerprint phrase modal.
#[derive(Debug, Clone, Copy)]
pub enum FingerprintMessage {
    /// User dismissed the modal (Close button or backdrop click).
    Close,
    /// "Learn more" pressed — opens the help page in the default browser.
    OpenLearnMore,
    /// Copy icon pressed — pushes the phrase onto the clipboard.
    Copy,
}

/// Events bubbled up to App. Side effects (clipboard, browser launch) live
/// at the App level because they touch state outside [`UpdateCtx`].
#[derive(Debug, Clone)]
pub enum FingerprintEvent {
    /// Open the fingerprint help page in the default browser.
    OpenLearnMore,
    /// Push the current phrase onto the clipboard and surface a copy toast.
    Copy(String),
}

const LEARN_MORE_URL: &str = "https://bitwarden.com/help/fingerprint-phrase/";

#[derive(Default)]
pub struct FingerprintModal {
    fade: FadeInOut,
    phrase: String,
}

impl FingerprintModal {
    pub fn open_with(&mut self, phrase: String) {
        self.phrase = phrase;
        self.fade.open();
    }
}

impl View for FingerprintModal {
    type Message = FingerprintMessage;
    type Event = FingerprintEvent;

    fn update(&mut self, msg: FingerprintMessage, _ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            FingerprintMessage::Close => {
                self.fade.close();
                Outcome::None
            }
            FingerprintMessage::OpenLearnMore => {
                self.fade.close();
                Outcome::event(FingerprintEvent::OpenLearnMore)
            }
            FingerprintMessage::Copy => {
                if self.phrase.is_empty() {
                    return Outcome::None;
                }
                Outcome::event(FingerprintEvent::Copy(self.phrase.clone()))
            }
        }
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, FingerprintMessage, AppTheme> {
        let phrase_row = row![
            text(self.phrase.as_str())
                .size(crate::text_size(14))
                .color(ctx.colors.text_primary)
                .wrapping(Wrapping::None),
            buttons::icon_button(icons::BWI_COPY, FingerprintMessage::Copy, ctx.colors),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let learn_more = buttons::primary(
            row![
                text(fl!("menu-fingerprint-learn-more")).size(crate::text_size(14)),
                icons::BWI_EXTERNAL_LINK
                    .render::<FingerprintMessage, AppTheme>(12.0, ctx.colors.card_bg),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(FingerprintMessage::OpenLearnMore)
        .padding(Padding::from([10, 20]))
        .width(Length::Fill)
        .into();

        let close_button = buttons::secondary(
            text(fl!("menu-fingerprint-close"))
                .size(crate::text_size(14))
                .color(ctx.colors.accent),
        )
        .on_press(FingerprintMessage::Close)
        .padding(Padding::from([10, 20]))
        .width(Length::Fill)
        .into();

        modal::info_dialog(
            440.0,
            icons::INFO_CIRCLE_FILL,
            fl!("menu-fingerprint-title"),
            phrase_row,
            vec![learn_more, close_button],
            FingerprintMessage::Close,
            ctx.colors,
            self.fade.progress_when_visible(),
        )
    }
}

/// Open the fingerprint help page in the default browser.
pub fn open_learn_more() {
    clipboard::launch_url(LEARN_MORE_URL);
}

#[cfg(test)]
mod tests_snapshot {
    use super::*;
    use crate::test_support::{self, ViewTestExt};

    #[tokio::test(flavor = "current_thread")]
    async fn fingerprint_modal() {
        let mut view = FingerprintModal::default();
        view.open_with("apple banana carrot dolphin eagle".to_owned());
        test_support::settle_animations();

        view.assert_themed_snapshots("fingerprint_modal").await;
    }
}

#[cfg(test)]
mod tests_interaction {
    use super::*;
    use crate::{app::App, fl, test_support};

    #[tokio::test(flavor = "current_thread")]
    async fn close_button_emits_close() {
        test_support::init();
        let mut view = FingerprintModal::default();
        view.open_with("apple banana carrot dolphin eagle".to_owned());
        test_support::settle_animations();

        let app = App::test();
        let element = view.view(&app.render_ctx_main());
        let messages = test_support::drive_element(element, |ui| {
            ui.click(fl!("menu-fingerprint-close").as_str())
                .expect("Close button");
        });

        test_support::assert_emitted(&messages, "Close", |m| {
            matches!(m, FingerprintMessage::Close)
        });
    }

    #[tokio::test(flavor = "current_thread")]
    async fn learn_more_button_emits_open_learn_more() {
        test_support::init();
        let mut view = FingerprintModal::default();
        view.open_with("apple banana carrot dolphin eagle".to_owned());
        test_support::settle_animations();

        let app = App::test();
        let element = view.view(&app.render_ctx_main());
        let messages = test_support::drive_element(element, |ui| {
            ui.click(fl!("menu-fingerprint-learn-more").as_str())
                .expect("Learn more button");
        });

        test_support::assert_emitted(&messages, "OpenLearnMore", |m| {
            matches!(m, FingerprintMessage::OpenLearnMore)
        });
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::test_support::{OutcomeExt, ViewTestExt};

    #[tokio::test(flavor = "current_thread")]
    async fn close_closes_fade() {
        let mut view = FingerprintModal::default();
        view.open_with("apple".into());
        view.run(FingerprintMessage::Close).await.expect_none();
        assert!(!view.fade.is_open());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn open_learn_more_closes_fade_and_emits_event() {
        let mut view = FingerprintModal::default();
        view.open_with("apple".into());
        let ev = view
            .run(FingerprintMessage::OpenLearnMore)
            .await
            .expect_event();
        assert!(matches!(ev, FingerprintEvent::OpenLearnMore));
        assert!(!view.fade.is_open());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn copy_emits_event_with_phrase() {
        let mut view = FingerprintModal::default();
        view.open_with("apple banana".into());
        let ev = view.run(FingerprintMessage::Copy).await.expect_event();
        match ev {
            FingerprintEvent::Copy(p) => assert_eq!(p, "apple banana"),
            _ => panic!("expected Copy"),
        }
        // Modal stays open after Copy — copy_and_toast still drops onto the
        // clipboard, but the dialog isn't dismissed.
        assert_eq!(view.phrase, "apple banana");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn copy_with_empty_phrase_returns_none() {
        let mut view = FingerprintModal::default();
        view.run(FingerprintMessage::Copy).await.expect_none();
    }
}
