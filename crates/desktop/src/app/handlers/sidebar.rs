//! Sidebar message routing. Section clicks may flip between `Screen::Vault`
//! and `Screen::Send`; filter changes are stored on `self.sidebar` and pushed
//! down via the affected view's `apply_filter`.

use iced::Task;

use crate::{
    app::{App, Message},
    components::sidebar::{NavSection, SidebarMessage, SidebarMode},
    domain::Screen,
};

impl App {
    pub(crate) fn handle_sidebar_message(&mut self, msg: SidebarMessage) -> Task<Message> {
        if self
            .active_user
            .is_some_and(|uid| self.client_manager.is_remote(&uid))
            && matches!(
                msg,
                SidebarMessage::SectionSelected(
                    NavSection::Send | NavSection::Import | NavSection::Export
                ) | SidebarMessage::SendFilterSelected(_)
            )
        {
            self.push_toast(crate::components::toast::Toast::warning("Der synchronisierte Tresor ist schreibgeschützt. Send, Import und Export folgen später.",None));
            return Task::none();
        }
        match msg {
            SidebarMessage::ToggleSidebarMode => {
                self.sidebar.mode = match self.sidebar.mode {
                    SidebarMode::Collapsed => SidebarMode::Expanded,
                    SidebarMode::Expanded => SidebarMode::Collapsed,
                };
                self.settings.sidebar_collapsed = self.sidebar.mode == SidebarMode::Collapsed;
                self.settings.save();
                Task::none()
            }
            SidebarMessage::ToggleGroup(group) => {
                self.sidebar.toggle_group(group);
                self.settings.collapsed_navigation_groups = self.sidebar.collapsed_groups.clone();
                self.settings.save();
                Task::none()
            }
            SidebarMessage::SectionSelected(section) => match section {
                NavSection::Vault => {
                    self.sidebar.active_section = section;
                    self.switch_to_vault()
                }
                NavSection::Send => {
                    self.sidebar.active_section = section;
                    self.switch_to_send()
                }
                NavSection::Generator => {
                    // Modal, not a screen — leave `active_section` pointing
                    // at the underlying screen so the highlight tracks where
                    // the user returns when the modal closes.
                    self.open_generator_modal()
                }
                NavSection::Import => self.open_import_modal(),
                NavSection::Export => self.open_export_modal(),
            },
            SidebarMessage::VaultFilterSelected(filter) => {
                let mut filter = filter.apply(self.sidebar.active_vault_filter);
                // A collection from a different organization must not silently hide all items.
                if let crate::views::vault::CollectionFilter::Collection(id) = filter.collection
                    && self.active_user.is_some_and(|uid| {
                        !self.client_manager.list_collections(&uid).iter().any(|c| {
                            c.id == id
                                && match filter.scope {
                                    crate::views::vault::VaultScope::All => true,
                                    crate::views::vault::VaultScope::Personal => false,
                                    crate::views::vault::VaultScope::Organization(org) => {
                                        c.organization_id == org
                                    }
                                }
                        })
                    })
                {
                    filter.collection = crate::views::vault::CollectionFilter::All;
                }
                if filter.scope == crate::views::vault::VaultScope::Personal {
                    filter.collection = crate::views::vault::CollectionFilter::All;
                }
                self.sidebar.active_vault_filter = filter;
                self.remember_active_filter();
                self.sidebar.active_section = NavSection::Vault;
                if let Some(uid) = self.active_user {
                    self.views.vault.apply_filter(&uid, filter);
                }
                self.switch_to_vault()
            }
            SidebarMessage::SendFilterSelected(filter) => {
                self.sidebar.active_send_filter = filter;
                self.sidebar.active_section = NavSection::Send;
                if let Some(uid) = self.active_user {
                    self.views.send.apply_filter(&uid, filter);
                }
                self.switch_to_send()
            }
        }
    }

    pub(crate) fn remember_active_filter(&mut self) {
        if let Some(uid) = self.active_user {
            let filter = self.sidebar.active_vault_filter;
            if self.settings.vault_filters.get(&uid) != Some(&filter) {
                self.settings.vault_filters.insert(uid, filter);
                self.settings.save();
            }
        }
    }
    pub(crate) fn restore_active_filter(&mut self) {
        self.sidebar.active_vault_filter = self
            .active_user
            .and_then(|uid| self.settings.vault_filters.get(&uid).copied())
            .unwrap_or_default();
    }

    /// Transition to a list screen (Vault or Send) and focus the search input.
    /// Same-screen calls (filter switch) preserve the query so the user can
    /// refine inside the new filter; cross-screen calls land via
    /// `apply_filter` reset. Clicking the sidebar while on Login mustn't flip
    /// the screen underneath the login flow.
    fn switch_to_list_screen(&mut self, target: Screen) -> Task<Message> {
        let (auto_focus, focus, load) = match target {
            Screen::Vault => (
                self.views.vault.auto_focus_task().map(Into::into),
                self.views.vault.focus_search_task().map(Into::into),
                self.active_user.map(|uid| self.load_vault_list_task(uid)),
            ),
            Screen::Send => (
                self.views.send.auto_focus_task().map(Into::into),
                self.views.send.focus_search_task().map(Into::into),
                self.active_user.map(|uid| self.load_send_list_task(uid)),
            ),
            Screen::Loading | Screen::Login => return Task::none(),
        };

        if self.screen == target {
            return auto_focus;
        }
        if !matches!(self.screen, Screen::Vault | Screen::Send) {
            return Task::none();
        }
        self.set_screen(target);
        match load {
            Some(load) => Task::batch([load, focus]),
            None => focus,
        }
    }

    fn switch_to_vault(&mut self) -> Task<Message> {
        self.switch_to_list_screen(Screen::Vault)
    }

    fn switch_to_send(&mut self) -> Task<Message> {
        self.switch_to_list_screen(Screen::Send)
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use crate::{
        components::sidebar::VaultFilterChange,
        views::vault::{VaultFilter, VaultScope, VaultSection},
    };
    use bitwarden_core::OrganizationId;
    use bitwarden_vault::CipherType;
    #[tokio::test]
    async fn account_filter_restore_is_isolated_and_defaults_for_new_accounts() {
        let mut app = App::test();
        let first = bitwarden_core::UserId::new_v4();
        let second = bitwarden_core::UserId::new_v4();
        let filter = VaultFilter::default().with_category(Some(CipherType::SshKey));
        app.settings.vault_filters.insert(first, filter);
        app.active_user = Some(first);
        app.restore_active_filter();
        assert_eq!(app.sidebar.active_vault_filter, filter);
        app.active_user = Some(second);
        app.restore_active_filter();
        assert_eq!(app.sidebar.active_vault_filter, VaultFilter::default());
        app.active_user = Some(first);
        app.restore_active_filter();
        assert_eq!(app.sidebar.active_vault_filter, filter);
    }
    #[tokio::test]
    async fn sidebar_scope_type_and_view_changes_preserve_other_facets() {
        let mut app = App::test();
        let first = OrganizationId::new_v4();
        let second = OrganizationId::new_v4();
        let mut select = |change| {
            let _ = app.handle_sidebar_message(SidebarMessage::VaultFilterSelected(change));
            app.sidebar.active_vault_filter
        };
        select(VaultFilterChange::Scope(VaultScope::Organization(first)));
        assert_eq!(
            select(VaultFilterChange::Category(Some(CipherType::SshKey))).scope,
            VaultScope::Organization(first)
        );
        assert_eq!(
            select(VaultFilterChange::Section(VaultSection::Favorites)).category,
            Some(CipherType::SshKey)
        );
        let switched = select(VaultFilterChange::Scope(VaultScope::Organization(second)));
        assert_eq!(switched.category, Some(CipherType::SshKey));
        assert_eq!(switched.section, VaultSection::Favorites);
        let all_vaults = select(VaultFilterChange::Scope(VaultScope::All));
        assert_eq!(all_vaults.category, Some(CipherType::SshKey));
        select(VaultFilterChange::Category(None));
        assert_eq!(
            select(VaultFilterChange::Section(VaultSection::Active)),
            VaultFilter::default()
        );
    }
}
