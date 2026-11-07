//! Immediate visibility state shared by dialogs and sheets.
//! The historical type name is retained for existing callers; no transitions,
//! frame subscriptions or focus delays are scheduled.

use iced::{Task, widget};

#[derive(Debug, Clone, Default)]
pub struct FadeInOut {
    open: bool,
}
impl FadeInOut {
    pub fn open(&mut self) {
        self.open = true;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn close_with_finalize<M: Clone + Send + 'static>(&mut self, finalize: M) -> Task<M> {
        self.close();
        Task::done(finalize)
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn progress_if_visible(&self) -> Option<f32> {
        self.open.then_some(1.0)
    }
    pub fn is_visible(&self) -> bool {
        self.open
    }
    pub fn progress_when_visible(&self) -> f32 {
        self.progress_if_visible().expect("overlay must be open")
    }
}

/// Focus on the first mounted frame, without a timer.
pub fn focus_after_open<M: Send + 'static>(id: widget::Id) -> Task<M> {
    widget::operation::focus(id)
}
