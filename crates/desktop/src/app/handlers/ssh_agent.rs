use crate::{
    app::{
        App, Message, WindowMessage,
        window::{WindowInfo, WindowKind},
    },
    services::{
        preferences::SshPromptBehavior,
        ssh_agent::{Decision, UiEvent},
    },
};
use iced::Task;
impl App {
    pub(crate) fn refresh_ssh_agent(&mut self) -> Task<Message> {
        self.ssh_agent
            .configure(self.settings.ssh_agent && !self.shutting_down);
        let session = if self.shutting_down {
            None
        } else {
            self.active_user
                .and_then(|uid| self.client_manager.agent_session_for(&uid))
        };
        let mode = self
            .active_user
            .map(|uid| self.settings.preferences_for(&uid).ssh_prompt_behavior)
            .unwrap_or(SshPromptBehavior::Always);
        self.ssh_agent.set_context(session, mode);
        if self.ssh_approval.as_ref().is_some_and(|a| !a.current()) {
            return self.cancel_ssh_approval();
        }
        Task::none()
    }
    pub(crate) fn cancel_ssh_approval(&mut self) -> Task<Message> {
        if let Some(approval) = self.ssh_approval.take() {
            approval.respond(Decision::Deny);
        }
        self.close_ssh_approval_window()
    }
    fn close_ssh_approval_window(&mut self) -> Task<Message> {
        self.ssh_approval_window
            .take()
            .map_or_else(Task::none, |(id, _)| iced::window::close(id))
    }
    pub(crate) fn decide_ssh_approval(
        &mut self,
        id: uuid::Uuid,
        decision: Decision,
    ) -> Task<Message> {
        if self.ssh_approval.as_ref().is_some_and(|a| a.id == id)
            && let Some(approval) = self.ssh_approval.take()
        {
            approval.respond(
                if decision == Decision::UntilLock && !approval.can_remember {
                    Decision::Deny
                } else {
                    decision
                },
            );
            return self.close_ssh_approval_window();
        }
        Task::none()
    }
    pub(crate) fn focus_ssh_approval(&self, id: iced::window::Id) -> Task<Message> {
        let Some(approval) = self.ssh_approval.as_ref() else {
            return Task::none();
        };
        if !approval.current()
            || !self
                .ssh_approval_window
                .is_some_and(|(window, request)| window == id && request == approval.id)
        {
            return Task::none();
        }
        Task::batch([
            iced::widget::operation::focus(crate::components::ssh_approval::focus_id(
                approval.default_decision,
            )),
            iced::window::gain_focus(id),
            iced::window::request_user_attention(id, Some(iced::window::UserAttention::Critical)),
            crate::services::screenshot_protection::apply(id, !self.settings.allow_screenshots)
                .discard(),
        ])
    }
    pub(crate) fn handle_ssh_agent_event(&mut self, event: UiEvent) -> Task<Message> {
        match event {
            UiEvent::Approval(approval) => {
                if !approval.current() || self.ssh_approval.is_some() {
                    approval.respond(Decision::Deny);
                    return Task::none();
                }
                self.open_overlay = None;
                // A fresh native surface is activated by KDE/Wayland; winit's
                // focus_window on an existing Wayland surface is a no-op.
                // This window contains public request metadata only.
                let size = iced::Size::new(720.0, 520.0);
                let (id, open) = iced::window::open(iced::window::Settings {
                    size,
                    min_size: Some(iced::Size::new(700.0, 500.0)),
                    position: iced::window::Position::Centered,
                    level: iced::window::Level::AlwaysOnTop,
                    decorations: false,
                    exit_on_close_request: false,
                    icon: crate::assets::window_icon(),
                    ..Default::default()
                });
                self.ssh_approval_window = Some((id, approval.id));
                self.ssh_approval = Some(approval);
                self.windows
                    .insert(id, WindowInfo::new(WindowKind::SshApproval, size));
                open.map(|id| Message::Window(WindowMessage::Opened(id)))
            }
            UiEvent::Dismiss(id) => {
                if self.ssh_approval.as_ref().is_some_and(|a| a.id == id) {
                    self.ssh_approval = None;
                    return self.close_ssh_approval_window();
                }
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ssh_agent::test_ui_approval;
    #[tokio::test]
    async fn native_prompt_is_separate_dismisses_on_close_and_rejects_stale_decisions() {
        let (_manager, approval) = test_ui_approval(true, false).await;
        let mut app = App::test();
        let main = app.main_window_id();
        let _ = app.handle_ssh_agent_event(UiEvent::Approval(approval.clone()));
        let (window, request) = app.ssh_approval_window.unwrap();
        assert_ne!(main, window);
        assert_eq!(app.windows[&window].kind, WindowKind::SshApproval);
        assert_eq!(request, approval.id);
        let _ = app.decide_ssh_approval(uuid::Uuid::new_v4(), Decision::Once);
        assert!(app.ssh_approval.is_some());
        let _ = app.handle_window_message(WindowMessage::CloseRequested(window));
        assert!(app.ssh_approval.is_none());
        assert!(app.ssh_approval_window.is_none());
        assert!(!app.shutting_down);
        let _ = app.handle_window_message(WindowMessage::Closed(window));
        assert!(!app.shutting_down);
    }
    #[tokio::test]
    async fn old_window_close_and_dismiss_cannot_cancel_a_replacement_prompt() {
        let (_first_manager, first) = test_ui_approval(false, false).await;
        let mut app = App::test();
        let _ = app.handle_ssh_agent_event(UiEvent::Approval(first.clone()));
        let old_window = app.ssh_approval_window.unwrap().0;
        let _ = app.decide_ssh_approval(first.id, Decision::Deny);
        let (_second_manager, second) = test_ui_approval(true, true).await;
        let _ = app.handle_ssh_agent_event(UiEvent::Approval(second.clone()));
        let new_window = app.ssh_approval_window.unwrap().0;
        let _ = app.handle_window_message(WindowMessage::Closed(old_window));
        let _ = app.handle_ssh_agent_event(UiEvent::Dismiss(first.id));
        assert_eq!(app.ssh_approval.as_ref().unwrap().id, second.id);
        assert_eq!(app.ssh_approval_window.unwrap().0, new_window);
        let _ = app.cancel_ssh_approval();
        assert!(app.ssh_approval_window.is_none());
    }
    #[tokio::test]
    async fn locking_revokes_a_pending_prompt_and_cancellation_before_open_still_closes_its_surface()
     {
        let (manager, approval) = test_ui_approval(true, false).await;
        let uid = manager.user_ids()[0];
        let mut app = App::test();
        app.client_manager = manager;
        app.active_user = Some(uid);
        let _ = app.handle_ssh_agent_event(UiEvent::Approval(approval.clone()));
        let window = app.ssh_approval_window.unwrap().0;
        let _ = app.lock_user(&uid);
        let _ = app.refresh_ssh_agent();
        assert!(!approval.current());
        assert!(app.ssh_approval.is_none());
        assert!(app.ssh_approval_window.is_none());
        let close = app.handle_window_message(WindowMessage::Opened(window));
        assert!(close.units() > 0);
        assert!(!app.shutting_down);
    }
}
