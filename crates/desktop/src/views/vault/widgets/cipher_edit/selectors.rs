//! Shared ownership selectors (folder, organization, collections) and the
//! generic bordered dropdown trigger used by the collections multi-select.
//!
//! Card-type and identity-type selectors live next to their section cards in
//! `sections/card.rs` and `sections/identity.rs`.

use iced::{
    Alignment, Background, Border, Color, Element, Fill,
    widget::{Space, checkbox, column, container, row, text},
};

use crate::{
    components::{
        buttons, icons,
        inputs::{multi_select_field, search_select_field},
    },
    fl,
    theme::{AppColors, AppTheme, RADIUS_LG, RADIUS_SM},
};

use super::{
    message::CipherEditMessage,
    state::{CipherForm, CollectionOption, FolderChoice, OrgChoice},
};

pub(super) fn folder_selector<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let selected = match form.modified.folder_id {
        None => FolderChoice::None,
        Some(id) => form
            .folders
            .iter()
            .find(|f| f.id == id)
            .cloned()
            .map(FolderChoice::Folder)
            .unwrap_or(FolderChoice::None),
    };
    let placeholder = selected.to_string();

    search_select_field(
        &form.folder_combo_state,
        fl!("form-folder"),
        placeholder,
        Some(selected),
        |choice: FolderChoice| CipherEditMessage::FolderSelected(choice.id()),
        CipherEditMessage::FolderComboClosed,
        colors,
    )
}

pub(super) fn org_selector<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let selected = match form.modified.organization_id {
        None => OrgChoice::None,
        Some(id) => form
            .organizations
            .iter()
            .find(|o| o.id == id)
            .cloned()
            .map(OrgChoice::Org)
            .unwrap_or(OrgChoice::None),
    };
    let placeholder = selected.to_string();

    search_select_field(
        &form.org_combo_state,
        fl!("form-organization"),
        placeholder,
        Some(selected),
        |choice: OrgChoice| CipherEditMessage::OrgSelected(choice.id()),
        CipherEditMessage::OrgComboClosed,
        colors,
    )
}

pub(super) fn collections_selector<'a>(
    form: &'a CipherForm,
    colors: &'a AppColors,
) -> Element<'a, CipherEditMessage, AppTheme> {
    let org_id = form.modified.organization_id;
    let scoped: Vec<&CollectionOption> = form
        .collections
        .iter()
        .filter(|c| Some(c.organization_id) == org_id)
        .collect();

    let summary = if form.modified.collection_ids.is_empty() {
        fl!("form-collections-none")
    } else {
        let count = form.modified.collection_ids.len();
        fl!("form-collections-selected", count = count)
    };

    let trigger: Element<'a, CipherEditMessage, AppTheme> = {
        let label_text = text(summary.clone())
            .size(crate::text_size(14))
            .color(colors.text_primary);
        let chevron = icons::BWI_ANGLE_DOWN.render(14.0, colors.text_secondary);
        let content = row![label_text, Space::new().width(Fill), chevron]
            .spacing(8)
            .align_y(Alignment::Center);

        buttons::ghost(
            container(content).padding([8, 12]).width(Fill),
            false,
            Color::TRANSPARENT,
            colors.item_hover,
            RADIUS_SM,
        )
        .on_press(CipherEditMessage::CollectionsDropdownToggled)
        .padding(0)
        .width(Fill)
        .style(|theme: &AppTheme, _status| iced::widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: theme.colors.text_primary,
            border: Border::default()
                .color(theme.colors.border)
                .width(1.0)
                .rounded(RADIUS_SM),
            shadow: iced::Shadow::default(),
            snap: false,
        })
        .into()
    };

    let mut options: Vec<Element<'a, CipherEditMessage, AppTheme>> = Vec::new();
    if scoped.is_empty() {
        options.push(
            container(
                text(fl!("form-collections-empty-in-org"))
                    .size(crate::text_size(12))
                    .color(colors.text_muted),
            )
            .padding([8, 12])
            .into(),
        );
    } else {
        for c in scoped {
            let selected = form.modified.collection_ids.contains(&c.id);
            let cid = c.id;
            options.push(
                buttons::ghost(
                    row![
                        checkbox(selected).size(crate::text_size(16)).spacing(0),
                        text(c.name.clone())
                            .size(crate::text_size(14))
                            .color(colors.text_primary),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    false,
                    Color::TRANSPARENT,
                    colors.item_hover,
                    0.0,
                )
                .on_press(CipherEditMessage::CollectionToggled(cid))
                .padding([6, 12])
                .width(Fill)
                .into(),
            );
        }
    }

    let panel: Element<'a, CipherEditMessage, AppTheme> = container(column(options).spacing(0))
        .width(Fill)
        .padding([4, 0])
        .style(|theme: &AppTheme| {
            container::Style::default()
                .background(theme.colors.card_bg)
                .border(
                    Border::default()
                        .color(theme.colors.border)
                        .width(1.0)
                        .rounded(RADIUS_LG),
                )
        })
        .into();

    multi_select_field(
        fl!("form-collections"),
        trigger,
        panel,
        form.collections_dropdown_open,
        CipherEditMessage::CollectionsDropdownToggled,
        colors,
    )
}
