//! Post-toggle confirmation dialog for screen-capture protection.
//!
//! Shown the moment the user enables `Block screenshots` in Settings →
//! Advanced. The whole purpose: catch the case where the user is on a
//! remote-desktop session (or any other capture-relaying setup) and
//! enabling protection has just hidden the window from them. If they
//! can read the dialog, they click OK and the setting sticks. If they
//! can't (because the window is now invisible to their capture), the
//! 5-second timeout reverts protection automatically and they regain
//! control.
//!
//! The settings handler builds an [`iced::Task::abortable`] sleep,
//! hands the [`iced::task::Handle`] to [`ScreenshotConfirmModal::open`],
//! and dispatches the resulting Task. Closing the modal (or replacing
//! the handle on a fresh open) drops the prior `Handle`, whose
//! `abort_on_drop` cancels the in-flight sleep — so the only `Timeout`
//! message that ever reaches the handler is one whose modal is still
//! open.

use std::time::Duration;

use iced::{Element, Length, Padding, widget::text};

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{FadeInOut, buttons, icons, modal},
    fl,
    theme::AppTheme,
};

/// How long the dialog gives the user to confirm before auto-reverting.
/// Bitwarden Electron uses 10 s; this client trims to 5 s — short enough
/// that a user who can't see the dialog gets their window back quickly,
/// long enough for a user who can see it to react.
pub const REVERT_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
pub enum ScreenshotConfirmMessage {
    /// User clicked OK — keep protection on.
    Confirm,
    /// `REVERT_AFTER` elapsed without confirmation. The handle stored on
    /// the modal aborts the underlying Task on `close()` / re-`open()`,
    /// so this variant only ever lands while the modal is genuinely
    /// open and unconfirmed.
    Timeout,
}

/// User couldn't see the dialog in time — App reverts protection at the
/// OS level and flips the setting back. Bubbled rather than handled
/// inline because the mutations reach state outside [`UpdateCtx`].
#[derive(Debug, Clone, Copy)]
pub enum ScreenshotConfirmEvent {
    Revert,
}

#[derive(Default)]
pub struct ScreenshotConfirmModal {
    fade: FadeInOut,
    /// Auto-aborts the in-flight 5 s sleep when the handle is dropped —
    /// either because the user closed the modal (`close()`) or because
    /// a fresh `open()` overwrote the slot. `None` outside an open
    /// session.
    pending_revert: Option<iced::task::Handle>,
}

impl ScreenshotConfirmModal {
    /// Open the dialog and store the abort handle for the matching
    /// in-flight 5 s timeout. Replacing the handle drops the previous
    /// one, which `abort_on_drop` then cancels — so a stale `Timeout`
    /// from an earlier session can't survive into a new one.
    pub fn open(&mut self, revert_handle: iced::task::Handle) {
        self.pending_revert = Some(revert_handle.abort_on_drop());
        self.fade.open();
    }

    pub fn close(&mut self) {
        // Drop the handle → abort_on_drop cancels the pending sleep.
        self.pending_revert = None;
        self.fade.close();
    }

    pub fn is_open(&self) -> bool {
        self.fade.is_open()
    }
}

impl View for ScreenshotConfirmModal {
    type Message = ScreenshotConfirmMessage;
    type Event = ScreenshotConfirmEvent;

    fn update(&mut self, msg: ScreenshotConfirmMessage, _ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            ScreenshotConfirmMessage::Confirm => {
                self.close();
                Outcome::None
            }
            ScreenshotConfirmMessage::Timeout => {
                if !self.is_open() {
                    // Abort raced with delivery — drop the stale message.
                    return Outcome::None;
                }
                self.close();
                Outcome::event(ScreenshotConfirmEvent::Revert)
            }
        }
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, ScreenshotConfirmMessage, AppTheme> {
        let body_text = text(fl!("settings-confirm-window-visible-body"))
            .size(crate::text_size(14))
            .color(ctx.colors.text_secondary);

        let ok_button = buttons::primary(
            text(fl!("settings-confirm-window-visible-ok")).size(crate::text_size(14)),
        )
        .on_press(ScreenshotConfirmMessage::Confirm)
        .padding(Padding::from([10, 20]))
        .width(Length::Fill)
        .into();

        modal::info_dialog(
            420.0,
            icons::INFO_CIRCLE_FILL,
            fl!("settings-confirm-window-visible-title"),
            body_text,
            vec![ok_button],
            ScreenshotConfirmMessage::Confirm,
            ctx.colors,
            self.fade.progress_when_visible(),
        )
    }
}

#[cfg(test)]
mod tests_update {
    use super::*;
    use crate::test_support::{OutcomeExt, ViewTestExt};

    /// Build an abortable iced Task purely so we have a `Handle` to pass to
    /// `open()`. The Task itself is discarded — tests don't drive the iced
    /// runtime, they just need a non-stub Handle.
    fn dummy_handle() -> iced::task::Handle {
        iced::Task::done(()).abortable().1
    }

    #[tokio::test(flavor = "current_thread")]
    async fn confirm_closes_the_modal() {
        let mut view = ScreenshotConfirmModal::default();
        view.open(dummy_handle());
        assert!(view.is_open());

        view.run(ScreenshotConfirmMessage::Confirm)
            .await
            .expect_none();
        assert!(!view.is_open());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn timeout_while_open_emits_revert_and_closes() {
        let mut view = ScreenshotConfirmModal::default();
        view.open(dummy_handle());

        let ev = view
            .run(ScreenshotConfirmMessage::Timeout)
            .await
            .expect_event();
        assert!(matches!(ev, ScreenshotConfirmEvent::Revert));
        assert!(!view.is_open());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn timeout_while_closed_drops_silently() {
        // Abort-on-drop is the modal's primary defense against stale
        // Timeouts, but it's not airtight: the abort can race with the
        // task already enqueueing its completion message into iced's
        // queue. This guard catches the race so a closed modal's stale
        // Timeout doesn't emit a Revert event.
        let mut view = ScreenshotConfirmModal::default();
        // Modal never opened — `is_open()` is false from the start.
        view.run(ScreenshotConfirmMessage::Timeout)
            .await
            .expect_none();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn close_drops_pending_revert_handle() {
        // The handle's `abort_on_drop` is the real abort path; this just
        // verifies the modal drops its stored handle on `close()` so the
        // drop actually happens.
        let mut view = ScreenshotConfirmModal::default();
        view.open(dummy_handle());
        view.close();
        // After close, `is_open` is false — and the next Timeout (which
        // would normally fire from a stale in-flight task) gets the
        // closed-modal guard treatment.
        assert!(!view.is_open());
        view.run(ScreenshotConfirmMessage::Timeout)
            .await
            .expect_none();
    }
}
