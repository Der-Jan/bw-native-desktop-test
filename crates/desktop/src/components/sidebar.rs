//! App-level sidebar chrome shared between the Vault and Send screens.
//!
//! State (mode, tree open flags, active filters) lives on `App` since the
//! sidebar persists across authenticated screens. Sidebar messages are
//! handled by `App`: section clicks may trigger a screen switch, and
//! per-screen filter changes are forwarded to the view that owns them.

mod navigation_tree;

use std::collections::HashSet;

use bitwarden_vault::CipherType;
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Padding,
    widget::{Space, column, container, row, scrollable, svg, text},
};

use crate::{
    components::{self, buttons, icons},
    fl,
    services::sdk::{Collection, Organization},
    theme::{AppColors, AppTheme},
    // Filter types are owned by the views whose lists they filter; sidebar
    // imports them rather than defining them so the semantics stay with the
    // view that interprets them.
    views::{
        send::SendFilter,
        vault::{
            CollectionFilter, FolderFilter, VaultFilter, VaultScope, VaultSection,
            widgets::cipher_edit::FolderOption,
        },
    },
};

const RAIL_WIDTH: f32 = 50.0;
const PANEL_WIDTH: f32 = 232.0;
const RAIL_ICON_SIZE: f32 = 18.0;
const RAIL_BTN_SIZE: f32 = 36.0;
const ITEM_RADIUS: f32 = 6.0;
const SIDEBAR_H_PAD: f32 = 8.0;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Collapsed,
    Expanded,
}

/// The "which screen / module am I in" nav facet. Generator / Import /
/// Export are placeholders without their own screens yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    Vault,
    Send,
    Generator,
    Import,
    Export,
}

pub use crate::services::vault_navigation::NavigationGroup as SidebarGroup;

/// Sidebar chrome state that persists across screen switches.
pub struct SidebarState {
    pub mode: SidebarMode,
    pub active_section: NavSection,
    pub active_vault_filter: VaultFilter,
    pub active_send_filter: SendFilter,
    pub collapsed_groups: HashSet<SidebarGroup>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            mode: SidebarMode::Expanded,
            active_section: NavSection::Vault,
            active_vault_filter: VaultFilter::default(),
            active_send_filter: SendFilter::AllItems,
            collapsed_groups: HashSet::new(),
        }
    }
}

impl SidebarState {
    pub fn group_is_open(&self, group: SidebarGroup) -> bool {
        !self.collapsed_groups.contains(&group)
    }
    pub fn toggle_group(&mut self, group: SidebarGroup) {
        if !self.collapsed_groups.remove(&group) {
            self.collapsed_groups.insert(group);
        }
    }
}

/// A user changes one facet; the other selections remain intact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VaultFilterChange {
    Scope(VaultScope),
    Category(Option<CipherType>),
    Folder(FolderFilter),
    Collection(CollectionFilter),
    Section(VaultSection),
}
impl VaultFilterChange {
    pub fn apply(self, filter: VaultFilter) -> VaultFilter {
        match self {
            Self::Scope(scope) => filter.with_scope(scope),
            Self::Folder(folder) => filter.with_folder(folder),
            Self::Collection(collection) => filter.with_collection(collection),
            Self::Category(category) => filter.with_category(category),
            Self::Section(section) => filter.with_section(section),
        }
    }
    fn selected(self, filter: VaultFilter) -> bool {
        match self {
            Self::Scope(scope) => filter.scope == scope,
            Self::Folder(folder) => filter.folder == folder,
            Self::Collection(collection) => filter.collection == collection,
            Self::Category(kind) => filter.category == kind,
            Self::Section(section) => filter.section == section,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SidebarMessage {
    ToggleSidebarMode,
    SectionSelected(NavSection),
    ToggleGroup(SidebarGroup),
    VaultFilterSelected(VaultFilterChange),
    SendFilterSelected(SendFilter),
}

// ── Entry point ────────────────────────────────────────────────────────────

pub fn view<'a>(
    state: &SidebarState,
    organizations: &[Organization],
    folders: &[FolderOption],
    collections: &[Collection],
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    if state.mode == SidebarMode::Expanded {
        expanded_panel(state, organizations, folders, collections, colors)
    } else {
        icon_rail(state.active_section, colors)
    }
}

// ── Icon rail (collapsed) ──────────────────────────────────────────────────

fn icon_rail<'a>(
    active_section: NavSection,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    let shield = container(
        svg(svg::Handle::from_memory(crate::assets::BITWARDEN_SHIELD))
            .width(28)
            .height(28),
    )
    .padding([12, 0])
    .width(RAIL_WIDTH)
    .align_x(Alignment::Center);

    let nav_text = colors.nav_text;
    let sidebar_selected = colors.sidebar_selected;
    let nav_item_hover = colors.nav_item_hover;

    let rail_btn =
        move |icon: icons::BwiIcon, section: NavSection| -> Element<'a, SidebarMessage, AppTheme> {
            let is_active = active_section == section;
            buttons::ghost(
                container(icon.render(RAIL_ICON_SIZE, nav_text))
                    .width(RAIL_BTN_SIZE)
                    .height(RAIL_BTN_SIZE)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
                is_active,
                sidebar_selected,
                nav_item_hover,
                ITEM_RADIUS,
            )
            .on_press(SidebarMessage::SectionSelected(section))
            .padding(0)
            .width(RAIL_WIDTH)
            .into()
        };

    let toggle_btn: Element<'a, SidebarMessage, AppTheme> = buttons::ghost_icon(
        container(icons::BWI_ANGLE_RIGHT.render(32.0, nav_text))
            .align_x(Alignment::Center)
            .width(Fill),
        nav_item_hover,
    )
    .on_press(SidebarMessage::ToggleSidebarMode)
    .padding([6, 0])
    .width(Fill)
    .into();

    container(
        column![
            shield,
            rail_btn(icons::BWI_VAULT, NavSection::Vault),
            rail_btn(icons::BWI_SEND, NavSection::Send),
            rail_btn(icons::BWI_GENERATE, NavSection::Generator),
            rail_btn(icons::BWI_IMPORT, NavSection::Import),
            rail_btn(icons::BWI_DOWNLOAD, NavSection::Export),
            Space::new().height(Fill),
            container(toggle_btn).padding([0, 4]),
        ]
        .spacing(4)
        .align_x(Alignment::Center)
        .padding(Padding {
            top: 0.0,
            right: 4.0,
            bottom: 10.0,
            left: 4.0,
        }),
    )
    .width(Length::Fixed(RAIL_WIDTH))
    .height(Fill)
    .style(|theme: &AppTheme| container::Style::default().background(theme.colors.header_bg))
    .into()
}

// ── Expanded panel ─────────────────────────────────────────────────────────

fn group_label<'a>(label: String, colors: &AppColors) -> Element<'a, SidebarMessage, AppTheme> {
    container(
        text(label)
            .size(crate::text_size(12))
            .color(colors.nav_text),
    )
    .padding([6, 12])
    .into()
}

fn group_header<'a>(
    label: String,
    group: SidebarGroup,
    state: &SidebarState,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    let chevron = if state.group_is_open(group) {
        icons::BWI_ANGLE_UP
    } else {
        icons::BWI_ANGLE_DOWN
    };
    buttons::ghost_icon(
        row![
            text(label)
                .size(crate::text_size(12))
                .color(colors.nav_text),
            Space::new().width(Fill),
            chevron.render(18.0, colors.nav_text)
        ]
        .align_y(Alignment::Center),
        colors.nav_item_hover,
    )
    .on_press(SidebarMessage::ToggleGroup(group))
    .padding([6, 12])
    .width(Fill)
    .into()
}

fn expanded_panel<'a>(
    state: &SidebarState,
    organizations: &[Organization],
    folders: &[FolderOption],
    collections: &[Collection],
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    let active_section = state.active_section;
    let on_vault = active_section == NavSection::Vault;
    let on_send = active_section == NavSection::Send;

    let mut items: Vec<Element<'a, SidebarMessage, AppTheme>> = Vec::new();

    let logo = container(
        svg(svg::Handle::from_memory(
            crate::assets::PASSWORD_MANAGER_LOGO,
        ))
        .width(Fill)
        .height(Length::Shrink),
    )
    .padding(Padding {
        top: 4.0,
        right: 16.0,
        bottom: 6.0,
        left: 16.0,
    });
    items.push(logo.into());

    let active = on_vault.then_some(state.active_vault_filter);
    items.push(group_header(
        fl!("sidebar-group-vaults"),
        SidebarGroup::Vaults,
        state,
        colors,
    ));
    if state.group_is_open(SidebarGroup::Vaults) {
        items.push(vault_nav_button(
            fl!("sidebar-filter-all-vaults"),
            icons::BWI_VAULT.render(17.0, colors.nav_text),
            VaultFilterChange::Scope(VaultScope::All),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-my-vault"),
            icons::BWI_USER.render(17.0, colors.nav_text),
            VaultFilterChange::Scope(VaultScope::Personal),
            active,
            colors,
        ));
        for org in organizations {
            items.push(vault_nav_button(
                org.name.clone(),
                icons::BUILDING.render(17.0, colors.nav_text),
                VaultFilterChange::Scope(VaultScope::Organization(org.id)),
                active,
                colors,
            ));
        }
    }
    items.push(group_header(
        fl!("sidebar-group-view"),
        SidebarGroup::View,
        state,
        colors,
    ));
    if state.group_is_open(SidebarGroup::View) {
        items.push(vault_nav_button(
            fl!("sidebar-filter-active-items"),
            icons::BWI_VAULT.render(17.0, colors.nav_text),
            VaultFilterChange::Section(VaultSection::Active),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-favorites"),
            icons::BWI_STAR.render(17.0, colors.nav_text),
            VaultFilterChange::Section(VaultSection::Favorites),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-archive"),
            icons::BWI_ARCHIVE.render(17.0, colors.nav_text),
            VaultFilterChange::Section(VaultSection::Archive),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-trash"),
            icons::BWI_TRASH.render(17.0, colors.nav_text),
            VaultFilterChange::Section(VaultSection::Trash),
            active,
            colors,
        ));
    }
    items.push(group_header(
        fl!("sidebar-group-types"),
        SidebarGroup::Types,
        state,
        colors,
    ));
    if state.group_is_open(SidebarGroup::Types) {
        items.push(vault_nav_button(
            fl!("sidebar-filter-all-types"),
            icons::BWI_VAULT.render(17.0, colors.nav_text),
            VaultFilterChange::Category(None),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-logins"),
            icons::BWI_LOGIN.render(17.0, colors.nav_text),
            VaultFilterChange::Category(Some(CipherType::Login)),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-cards"),
            icons::BWI_CREDIT_CARD.render(17.0, colors.nav_text),
            VaultFilterChange::Category(Some(CipherType::Card)),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-identities"),
            icons::BWI_IDENTITY.render(17.0, colors.nav_text),
            VaultFilterChange::Category(Some(CipherType::Identity)),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-notes"),
            icons::BWI_NOTE.render(17.0, colors.nav_text),
            VaultFilterChange::Category(Some(CipherType::SecureNote)),
            active,
            colors,
        ));
        items.push(vault_nav_button(
            fl!("sidebar-filter-ssh-keys"),
            icons::BWI_KEY.render(17.0, colors.nav_text),
            VaultFilterChange::Category(Some(CipherType::SshKey)),
            active,
            colors,
        ));
    }
    items.push(group_header(
        fl!("sidebar-group-folders"),
        SidebarGroup::Folders,
        state,
        colors,
    ));
    if state.group_is_open(SidebarGroup::Folders) {
        for (label, filter) in [
            (fl!("sidebar-filter-all-folders"), FolderFilter::All),
            (fl!("sidebar-filter-no-folder"), FolderFilter::None),
        ] {
            items.push(vault_nav_button(
                label,
                icons::BWI_FOLDER.render(17.0, colors.nav_text),
                VaultFilterChange::Folder(filter),
                active,
                colors,
            ));
        }
        for node in navigation_tree::rows(folders.iter().map(|f| (f.id, f.name.clone()))) {
            let entry = if let Some(id) = node.id {
                vault_nav_button(
                    node.name,
                    icons::BWI_FOLDER.render(17.0, colors.nav_text),
                    VaultFilterChange::Folder(FolderFilter::Folder(id)),
                    active,
                    colors,
                )
            } else {
                group_label(node.name, colors)
            };
            items.push(
                container(entry)
                    .padding(Padding {
                        left: (node.depth.min(8) * 12) as f32,
                        ..Padding::ZERO
                    })
                    .into(),
            );
        }
    }
    if !matches!(state.active_vault_filter.scope, VaultScope::Personal) {
        items.push(group_header(
            fl!("sidebar-group-collections"),
            SidebarGroup::Collections,
            state,
            colors,
        ));
        if state.group_is_open(SidebarGroup::Collections) {
            for (label, filter) in [
                (fl!("sidebar-filter-all-collections"), CollectionFilter::All),
                (fl!("sidebar-filter-no-collection"), CollectionFilter::None),
            ] {
                items.push(vault_nav_button(
                    label,
                    icons::BWI_FOLDER.render(17.0, colors.nav_text),
                    VaultFilterChange::Collection(filter),
                    active,
                    colors,
                ));
            }
            for org in organizations.iter().filter(|org| !matches!(state.active_vault_filter.scope, VaultScope::Organization(id) if id != org.id)) {
                let org_collections: Vec<_> = collections.iter().filter(|c| c.organization_id == org.id).map(|c| (c.id, c.name.clone())).collect();
                if org_collections.is_empty() { continue; }
                if state.active_vault_filter.scope == VaultScope::All { items.push(group_label(org.name.clone(), colors)); }
                for node in navigation_tree::rows(org_collections) {
                    let entry = if let Some(id) = node.id {
                        vault_nav_button(node.name, icons::BWI_FOLDER.render(17.0, colors.nav_text), VaultFilterChange::Collection(CollectionFilter::Collection(id)), active, colors)
                    } else { group_label(node.name, colors) };
                    items.push(container(entry).padding(Padding { left: (node.depth.min(8) * 12) as f32, ..Padding::ZERO }).into());
                }
            }
        }
    }

    items.push(parent_header_row(
        fl!("sidebar-section-send"),
        icons::BWI_SEND,
        state.group_is_open(SidebarGroup::Send),
        on_send && state.active_send_filter == SendFilter::AllItems,
        SidebarMessage::SendFilterSelected(SendFilter::AllItems),
        SidebarMessage::ToggleGroup(SidebarGroup::Send),
        colors,
    ));

    if state.group_is_open(SidebarGroup::Send) {
        let active = if on_send {
            Some(state.active_send_filter)
        } else {
            None
        };
        items.push(send_nav_button(
            fl!("sidebar-filter-text-send"),
            icons::FILE_TEXT.render(17.0, colors.nav_text),
            SendFilter::Text,
            active,
            colors,
        ));
        items.push(send_nav_button(
            fl!("sidebar-filter-file-send"),
            icons::FILE_EARMARK.render(17.0, colors.nav_text),
            SendFilter::File,
            active,
            colors,
        ));
    }

    items.push(standalone_item(
        fl!("sidebar-item-generator"),
        icons::BWI_GENERATE.render(17.0, colors.nav_text),
        NavSection::Generator,
        active_section,
        colors,
    ));
    items.push(standalone_item(
        fl!("sidebar-item-import"),
        icons::BWI_IMPORT.render(17.0, colors.nav_text),
        NavSection::Import,
        active_section,
        colors,
    ));
    items.push(standalone_item(
        fl!("sidebar-item-export"),
        icons::BWI_DOWNLOAD.render(17.0, colors.nav_text),
        NavSection::Export,
        active_section,
        colors,
    ));

    let separator = container(components::separator_h()).padding([4.0, SIDEBAR_H_PAD]);

    let collapse_btn: Element<'a, SidebarMessage, AppTheme> = buttons::ghost_icon(
        row![icons::BWI_ANGLE_LEFT.render(32.0, colors.nav_text),].align_y(Alignment::Center),
        colors.nav_item_hover,
    )
    .on_press(SidebarMessage::ToggleSidebarMode)
    .padding([8, 16])
    .width(Fill)
    .into();

    container(
        column![
            scrollable(column(items).spacing(2).padding([0.0, SIDEBAR_H_PAD])).height(Fill),
            separator,
            container(collapse_btn).padding([0.0, SIDEBAR_H_PAD]),
        ]
        .padding([10, 0]),
    )
    .width(Length::Fixed(PANEL_WIDTH))
    .height(Fill)
    .style(|theme: &AppTheme| container::Style::default().background(theme.colors.header_bg))
    .into()
}

// ── Shared row helpers ─────────────────────────────────────────────────────

/// Parent row for a collapsible section. The label side is a selectable
/// filter (`select_msg`); the chevron toggles the tree (`toggle_msg`). When
/// `is_selected`, an outer container paints the darkened background across
/// both halves.
fn parent_header_row<'a>(
    label: String,
    icon: icons::BwiIcon,
    is_open: bool,
    is_selected: bool,
    select_msg: SidebarMessage,
    toggle_msg: SidebarMessage,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    // When already selected, clicking the label is a no-op — suppress the
    // hover highlight. The chevron stays hoverable since toggling the tree
    // is still meaningful.
    let label_hover_bg = if is_selected {
        Color::TRANSPARENT
    } else {
        colors.nav_item_hover
    };
    let label_btn = buttons::ghost(
        row![
            icon.render(18.0, colors.nav_text),
            text(label)
                .size(crate::text_size(16))
                .color(colors.nav_text),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        false,
        colors.sidebar_selected,
        label_hover_bg,
        ITEM_RADIUS,
    )
    .on_press(select_msg)
    .padding([8, 12])
    .width(Fill);

    let chevron = if is_open {
        icons::BWI_ANGLE_UP
    } else {
        icons::BWI_ANGLE_DOWN
    };
    let chevron_btn = buttons::ghost_icon(
        container(chevron.render(21.0, colors.nav_text))
            .padding([0, 8])
            .align_y(Alignment::Center),
        colors.nav_item_hover,
    )
    .on_press(toggle_msg)
    .padding([8, 4]);

    let inner = row![label_btn, chevron_btn]
        .spacing(0)
        .align_y(Alignment::Center);

    let selected_bg = colors.sidebar_selected;
    container(inner)
        .style(move |_theme: &AppTheme| {
            let bg = if is_selected {
                Background::Color(selected_bg)
            } else {
                Background::Color(Color::TRANSPARENT)
            };
            container::Style::default()
                .background(bg)
                .border(Border::default().rounded(ITEM_RADIUS))
        })
        .into()
}

fn vault_nav_button<'a>(
    label: impl Into<String>,
    icon: Element<'a, SidebarMessage, AppTheme>,
    filter: VaultFilterChange,
    active: Option<VaultFilter>,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    nav_row(
        label,
        icon,
        active.is_some_and(|active| filter.selected(active)),
        SidebarMessage::VaultFilterSelected(filter),
        colors,
    )
}

fn send_nav_button<'a>(
    label: impl Into<String>,
    icon: Element<'a, SidebarMessage, AppTheme>,
    filter: SendFilter,
    active: Option<SendFilter>,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    nav_row(
        label,
        icon,
        active == Some(filter),
        SidebarMessage::SendFilterSelected(filter),
        colors,
    )
}

fn nav_row<'a>(
    label: impl Into<String>,
    icon: Element<'a, SidebarMessage, AppTheme>,
    is_selected: bool,
    on_press: SidebarMessage,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    buttons::ghost(
        row![
            icon,
            text(label.into())
                .size(crate::text_size(16))
                .color(colors.nav_text)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        is_selected,
        colors.sidebar_selected,
        colors.nav_item_hover,
        ITEM_RADIUS,
    )
    .on_press(on_press)
    .padding(Padding {
        top: 6.0,
        right: 12.0,
        bottom: 6.0,
        left: 28.0,
    })
    .width(Fill)
    .into()
}

fn standalone_item<'a>(
    label: impl Into<String>,
    icon: Element<'a, SidebarMessage, AppTheme>,
    section: NavSection,
    active_section: NavSection,
    colors: &AppColors,
) -> Element<'a, SidebarMessage, AppTheme> {
    let is_active = active_section == section;

    buttons::ghost(
        row![
            icon,
            text(label.into())
                .size(crate::text_size(16))
                .color(colors.nav_text)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        is_active,
        colors.sidebar_selected,
        colors.nav_item_hover,
        ITEM_RADIUS,
    )
    .on_press(SidebarMessage::SectionSelected(section))
    .padding([6, 12])
    .width(Fill)
    .into()
}
