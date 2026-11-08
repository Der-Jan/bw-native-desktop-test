//! Import data modal — wired to the sidebar Import button.
//!
//! Visual stub: dropdowns are populated with real data (vaults from the
//! active user's organizations, folders from the SDK, file format from the
//! upstream importer list), but submitting only fires an "unimplemented"
//! toast. The Choose-file button is not yet wired to a native picker either.

use bitwarden_core::OrganizationId;
use iced::{
    Alignment, Element, Fill, Length,
    widget::{column, container, row, text, text_editor},
};

use crate::{
    app::{Outcome, RenderCtx, UpdateCtx, View},
    components::{FadeInOut, buttons, inputs, modal},
    fl,
    services::sdk::{Collection, Organization, VaultChoice},
    theme::{AppColors, AppTheme, RADIUS_LG},
};

// ── Import format catalog ─────────────────────────────────────────────────
//
// Ported from upstream `clients/libs/importer/src/models/import-options.ts`.
// Exposed as `&'static [ImportFormat]` so the picker doesn't allocate per
// frame; `Display` returns the human-readable name iced shows in the list.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportFormat {
    pub id: &'static str,
    pub name: &'static str,
}

impl std::fmt::Display for ImportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

const FEATURED_FORMATS: &[ImportFormat] = &[
    ImportFormat {
        id: "bitwardenjson",
        name: "Bitwarden (json)",
    },
    ImportFormat {
        id: "bitwardencsv",
        name: "Bitwarden (csv)",
    },
    ImportFormat {
        id: "chromecsv",
        name: "Chrome",
    },
    ImportFormat {
        id: "dashlanecsv",
        name: "Dashlane (csv)",
    },
    ImportFormat {
        id: "firefoxcsv",
        name: "Firefox (csv)",
    },
    ImportFormat {
        id: "keepass2xml",
        name: "KeePass 2 (xml)",
    },
    ImportFormat {
        id: "lastpasscsv",
        name: "LastPass",
    },
    ImportFormat {
        id: "safaricsv",
        name: "Safari and macOS (csv)",
    },
    ImportFormat {
        id: "1password1pux",
        name: "1Password (1pux/json)",
    },
];

const REGULAR_FORMATS: &[ImportFormat] = &[
    ImportFormat {
        id: "keepassxcsv",
        name: "KeePassX (csv)",
    },
    ImportFormat {
        id: "1password1pif",
        name: "1Password (1pif)",
    },
    ImportFormat {
        id: "1passwordwincsv",
        name: "1Password 6 and 7 Windows (csv)",
    },
    ImportFormat {
        id: "1passwordmaccsv",
        name: "1Password 6 and 7 Mac (csv)",
    },
    ImportFormat {
        id: "dashlanejson",
        name: "Dashlane (json)",
    },
    ImportFormat {
        id: "roboformcsv",
        name: "RoboForm (csv)",
    },
    ImportFormat {
        id: "keepercsv",
        name: "Keeper (csv)",
    },
    ImportFormat {
        id: "enpasscsv",
        name: "Enpass (csv)",
    },
    ImportFormat {
        id: "enpassjson",
        name: "Enpass (json)",
    },
    ImportFormat {
        id: "protonpass",
        name: "ProtonPass (zip/json)",
    },
    ImportFormat {
        id: "safeincloudxml",
        name: "SafeInCloud (xml)",
    },
    ImportFormat {
        id: "pwsafexml",
        name: "Password Safe - pwsafe.org (xml)",
    },
    ImportFormat {
        id: "stickypasswordxml",
        name: "Sticky Password (xml)",
    },
    ImportFormat {
        id: "msecurecsv",
        name: "mSecure (csv)",
    },
    ImportFormat {
        id: "truekeycsv",
        name: "True Key (csv)",
    },
    ImportFormat {
        id: "passwordbossjson",
        name: "Password Boss (json)",
    },
    ImportFormat {
        id: "zohovaultcsv",
        name: "Zoho Vault (csv)",
    },
    ImportFormat {
        id: "splashidcsv",
        name: "SplashID (csv)",
    },
    ImportFormat {
        id: "passworddragonxml",
        name: "Password Dragon (xml)",
    },
    ImportFormat {
        id: "padlockcsv",
        name: "Padlock (csv)",
    },
    ImportFormat {
        id: "passboltcsv",
        name: "Passbolt (csv)",
    },
    ImportFormat {
        id: "clipperzhtml",
        name: "Clipperz (html)",
    },
    ImportFormat {
        id: "aviracsv",
        name: "Avira (csv)",
    },
    ImportFormat {
        id: "saferpasscsv",
        name: "SaferPass (csv)",
    },
    ImportFormat {
        id: "upmcsv",
        name: "Universal Password Manager (csv)",
    },
    ImportFormat {
        id: "ascendocsv",
        name: "Ascendo DataVault (csv)",
    },
    ImportFormat {
        id: "meldiumcsv",
        name: "Meldium (csv)",
    },
    ImportFormat {
        id: "passkeepcsv",
        name: "PassKeep (csv)",
    },
    ImportFormat {
        id: "arccsv",
        name: "Arc",
    },
    ImportFormat {
        id: "edgecsv",
        name: "Edge",
    },
    ImportFormat {
        id: "operacsv",
        name: "Opera",
    },
    ImportFormat {
        id: "vivaldicsv",
        name: "Vivaldi",
    },
    ImportFormat {
        id: "bravecsv",
        name: "Brave",
    },
    ImportFormat {
        id: "gnomejson",
        name: "GNOME Passwords and Keys/Seahorse (json)",
    },
    ImportFormat {
        id: "blurcsv",
        name: "Blur (csv)",
    },
    ImportFormat {
        id: "passwordagentcsv",
        name: "Password Agent (csv)",
    },
    ImportFormat {
        id: "passpackcsv",
        name: "Passpack (csv)",
    },
    ImportFormat {
        id: "passmanjson",
        name: "Passman (json)",
    },
    ImportFormat {
        id: "avastcsv",
        name: "Avast Passwords (csv)",
    },
    ImportFormat {
        id: "avastjson",
        name: "Avast Passwords (json)",
    },
    ImportFormat {
        id: "fsecurefsk",
        name: "F-Secure KEY (fsk)",
    },
    ImportFormat {
        id: "kasperskytxt",
        name: "Kaspersky Password Manager (txt)",
    },
    ImportFormat {
        id: "remembearcsv",
        name: "RememBear (csv)",
    },
    ImportFormat {
        id: "passwordwallettxt",
        name: "PasswordWallet (txt)",
    },
    ImportFormat {
        id: "mykicsv",
        name: "Myki (csv)",
    },
    ImportFormat {
        id: "securesafecsv",
        name: "SecureSafe (csv)",
    },
    ImportFormat {
        id: "logmeoncecsv",
        name: "LogMeOnce (csv)",
    },
    ImportFormat {
        id: "blackberrycsv",
        name: "BlackBerry Password Keeper (csv)",
    },
    ImportFormat {
        id: "buttercupcsv",
        name: "Buttercup (csv)",
    },
    ImportFormat {
        id: "codebookcsv",
        name: "Codebook (csv)",
    },
    ImportFormat {
        id: "encryptrcsv",
        name: "Encryptr (csv)",
    },
    ImportFormat {
        id: "yoticsv",
        name: "Yoti (csv)",
    },
    ImportFormat {
        id: "nordpasscsv",
        name: "Nordpass (csv)",
    },
    ImportFormat {
        id: "psonojson",
        name: "Psono (json)",
    },
    ImportFormat {
        id: "passkyjson",
        name: "Passky (json)",
    },
    ImportFormat {
        id: "passwordxpcsv",
        name: "Password XP (csv)",
    },
    ImportFormat {
        id: "netwrixpasswordsecure",
        name: "Netwrix Password Secure (csv)",
    },
    ImportFormat {
        id: "passworddepot17xml",
        name: "Password Depot 17 (xml)",
    },
];

/// Featured first, then regular sorted by display name. Computed once and
/// reused across `modal_view` calls so the format dropdown doesn't pay a
/// `sort_by_key` per frame.
static SORTED_FORMATS: std::sync::LazyLock<Vec<ImportFormat>> = std::sync::LazyLock::new(|| {
    let mut out = Vec::with_capacity(FEATURED_FORMATS.len() + REGULAR_FORMATS.len());
    out.extend(FEATURED_FORMATS.iter().copied());
    let mut regular: Vec<ImportFormat> = REGULAR_FORMATS.to_vec();
    regular.sort_by_key(|f| f.name);
    out.extend(regular);
    out
});

fn all_formats() -> Vec<ImportFormat> {
    SORTED_FORMATS.clone()
}

// ── State ─────────────────────────────────────────────────────────────────

pub struct ImportView {
    pub(super) fade: FadeInOut,
    vault_choices: Vec<VaultChoice>,
    selected_vault: VaultChoice,
    /// Folder names with the localized placeholder at the top. Loaded
    /// asynchronously after open. Used only when `selected_vault` is
    /// `Personal`.
    folder_choices: Vec<String>,
    selected_folder: Option<String>,
    /// Full collection list across orgs. Filtered per-render by the
    /// currently-selected organization. Synchronously seeded by App on open.
    collections: Vec<Collection>,
    selected_collection: Option<String>,
    selected_format: Option<ImportFormat>,
    paste_contents: text_editor::Content,
}

// ── Messages + Events ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ImportMessage {
    Close,
    Submit,
    VaultSelected(VaultChoice),
    FolderSelected(String),
    CollectionSelected(String),
    FormatSelected(ImportFormat),
    PasteAction(text_editor::Action),
    ChooseFile,
    /// Folder names from the SDK list. The async block flattens
    /// `FolderView` to the only piece we need for the dropdown — `Err` is
    /// swallowed in the App handler so the variant just carries names.
    FoldersLoaded(Vec<String>),
}

pub enum ImportEvent {
    /// Submit + Choose file both bubble this; App routes to a toast.
    Unimplemented,
}

impl View for ImportView {
    type Message = ImportMessage;
    type Event = ImportEvent;

    fn update(&mut self, msg: ImportMessage, _ctx: UpdateCtx<'_>) -> Outcome<Self> {
        match msg {
            ImportMessage::Close => self.fade.close(),
            ImportMessage::Submit | ImportMessage::ChooseFile => {
                return Outcome::event(ImportEvent::Unimplemented);
            }
            ImportMessage::VaultSelected(v) => {
                // Reset the per-vault sub-selection so the collection /
                // folder dropdown doesn't carry stale state across switches.
                match &v {
                    VaultChoice::Personal => {
                        self.selected_folder = Some(fl!("import-modal-folder-placeholder"));
                        self.selected_collection = None;
                    }
                    VaultChoice::Org { .. } => {
                        self.selected_collection = Some(fl!("import-modal-collection-placeholder"));
                    }
                }
                self.selected_vault = v;
            }
            ImportMessage::FolderSelected(f) => self.selected_folder = Some(f),
            ImportMessage::CollectionSelected(c) => self.selected_collection = Some(c),
            ImportMessage::FormatSelected(f) => self.selected_format = Some(f),
            ImportMessage::PasteAction(action) => self.paste_contents.perform(action),
            ImportMessage::FoldersLoaded(names) => self.set_folders(names),
        }
        Outcome::None
    }

    fn should_render(&self) -> bool {
        self.fade.is_visible()
    }

    fn view<'a>(&'a self, ctx: &RenderCtx<'a>) -> Element<'a, ImportMessage, AppTheme> {
        let progress = self.fade.progress_when_visible();

        let header =
            modal::dialog_header(fl!("import-modal-title"), ImportMessage::Close, ctx.colors);

        let personal_label = fl!("import-modal-vault-personal");
        let vault_picker = inputs::select_field(
            fl!("import-modal-vault-label"),
            Some(self.selected_vault.clone()),
            self.vault_choices.clone(),
            move |v: &VaultChoice| v.label(&personal_label),
            ImportMessage::VaultSelected,
            ctx.colors,
        );

        let sub_picker: Element<'a, ImportMessage, AppTheme> = match &self.selected_vault {
            VaultChoice::Personal => inputs::select_field(
                fl!("import-modal-folder-label"),
                self.selected_folder.clone(),
                self.folder_choices.clone(),
                |s: &String| s.clone(),
                ImportMessage::FolderSelected,
                ctx.colors,
            ),
            VaultChoice::Org { id, .. } => inputs::select_field(
                fl!("import-modal-collection-label"),
                self.selected_collection.clone(),
                self.collection_choices(*id),
                |s: &String| s.clone(),
                ImportMessage::CollectionSelected,
                ctx.colors,
            ),
        };

        let destination_card = section_card(
            fl!("import-modal-section-destination"),
            column![vault_picker, sub_picker].spacing(12).into(),
            ctx.colors,
        );

        let format_picker = inputs::select_field(
            fl!("import-modal-file-format-label"),
            self.selected_format,
            all_formats(),
            |f: &ImportFormat| f.name.to_string(),
            ImportMessage::FormatSelected,
            ctx.colors,
        );

        let file_row = row![
            buttons::secondary(text(fl!("import-modal-choose-file")).size(crate::text_size(14)))
                .on_press(ImportMessage::ChooseFile)
                .padding([8, 16]),
            text(fl!("import-modal-no-file"))
                .size(crate::text_size(14))
                .color(ctx.colors.text_secondary),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let paste_editor = text_editor(&self.paste_contents)
            .padding([10, 12])
            .height(Length::Shrink)
            .min_height(80.0)
            .max_height(240.0)
            .on_action(ImportMessage::PasteAction);
        let paste_field =
            inputs::field_frame(fl!("import-modal-paste-label"), paste_editor, ctx.colors);

        let data_card = section_card(
            fl!("import-modal-section-data"),
            column![
                format_picker,
                text(fl!("import-modal-file-helper"))
                    .size(crate::text_size(12))
                    .color(ctx.colors.text_secondary),
                file_row,
                paste_field,
            ]
            .spacing(12)
            .into(),
            ctx.colors,
        );

        let footer = modal::footer_actions(
            fl!("import-modal-submit"),
            Some(ImportMessage::Submit),
            fl!("import-modal-cancel"),
            ImportMessage::Close,
        );

        let body = column![header, destination_card, data_card, footer]
            .spacing(16)
            .padding(modal::BODY_PADDING)
            .width(Fill);

        modal::dialog(
            580.0,
            None,
            |c| c.card_bg,
            progress,
            container(body),
            ImportMessage::Close,
        )
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────

impl ImportView {
    pub fn new() -> Self {
        Self {
            fade: FadeInOut::default(),
            vault_choices: vec![VaultChoice::Personal],
            selected_vault: VaultChoice::Personal,
            folder_choices: Vec::new(),
            selected_folder: None,
            collections: Vec::new(),
            selected_collection: None,
            selected_format: None,
            paste_contents: text_editor::Content::new(),
        }
    }

    pub fn open(&mut self) {
        self.fade.open();
        self.selected_format = None;
        self.paste_contents = text_editor::Content::new();
        self.selected_vault = VaultChoice::Personal;
        // Reset folder dropdown to just the placeholder until the async
        // SDK call completes via FoldersLoaded.
        let placeholder = fl!("import-modal-folder-placeholder");
        self.folder_choices = vec![placeholder.clone()];
        self.selected_folder = Some(placeholder);
        self.selected_collection = None;
    }

    pub fn set_organizations(&mut self, orgs: &[Organization]) {
        self.vault_choices = VaultChoice::list_with_personal(orgs);
    }

    pub fn set_collections(&mut self, collections: Vec<Collection>) {
        self.collections = collections;
    }

    fn set_folders(&mut self, names: Vec<String>) {
        let placeholder = fl!("import-modal-folder-placeholder");
        let mut choices = Vec::with_capacity(names.len() + 1);
        choices.push(placeholder);
        choices.extend(names);
        self.folder_choices = choices;
        // Keep the placeholder selected — user picks a folder explicitly.
    }

    /// Collection options for the currently-selected organization, with the
    /// localized placeholder at the top.
    fn collection_choices(&self, org_id: OrganizationId) -> Vec<String> {
        let placeholder = fl!("import-modal-collection-placeholder");
        let mut out = vec![placeholder];
        for c in &self.collections {
            if c.organization_id == org_id {
                out.push(c.name.clone());
            }
        }
        out
    }
}

/// Inset card matching the design: subtle background tile + rounded
/// corners, with an accent-coloured heading inside the card alongside the
/// body — distinct from `components::section_card` which stacks the heading
/// above a shadowed card.
fn section_card<'a>(
    heading: String,
    body: Element<'a, ImportMessage, AppTheme>,
    colors: &'a AppColors,
) -> Element<'a, ImportMessage, AppTheme> {
    let inner = column![
        text(heading)
            .size(crate::text_size(14))
            .font(crate::app_font_bold())
            .color(colors.accent),
        body,
    ]
    .spacing(12);

    container(inner)
        .padding(16)
        .width(Fill)
        .style(|theme: &AppTheme| {
            iced::widget::container::Style::default()
                .background(theme.colors.background)
                .border(iced::Border::default().rounded(RADIUS_LG))
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn merged_length_matches_inputs() {
        // Sanity: the static can't have lost or duplicated entries during
        // construction.
        let merged = all_formats();
        assert_eq!(merged.len(), FEATURED_FORMATS.len() + REGULAR_FORMATS.len());
    }

    #[test]
    fn featured_formats_lead_in_declaration_order() {
        // The dropdown promise: featured entries appear first, in the order
        // they're declared in the source — *not* alphabetised.
        let merged = all_formats();
        for (i, expected) in FEATURED_FORMATS.iter().enumerate() {
            assert_eq!(
                merged[i].id, expected.id,
                "featured slot {i} expected {} but got {}",
                expected.id, merged[i].id
            );
        }
    }

    #[test]
    fn tail_after_featured_is_alphabetical() {
        // The cheap durability assertion: post-featured tail must be
        // non-decreasing by `name`. Hard-coding individual positions would
        // break every time the upstream importer list grows.
        let merged = all_formats();
        let tail = &merged[FEATURED_FORMATS.len()..];
        for window in tail.windows(2) {
            assert!(
                window[0].name <= window[1].name,
                "tail not sorted: {:?} > {:?}",
                window[0].name,
                window[1].name
            );
        }
    }

    #[test]
    fn no_duplicate_ids_across_merged_table() {
        // Both halves of the table are large and easy to copy-paste-collide.
        // `id` is what flows into the SDK importer dispatch, so a duplicate
        // means one importer is silently unreachable.
        let merged = all_formats();
        let mut seen: HashSet<&str> = HashSet::with_capacity(merged.len());
        for f in &merged {
            assert!(seen.insert(f.id), "duplicate import format id: {:?}", f.id);
        }
    }

    #[test]
    fn no_id_collision_between_featured_and_regular() {
        // Belt-and-braces: the previous test catches any duplicate, but
        // pinning down "the two halves don't overlap" makes the regression
        // message actionable when it fires.
        let featured: HashSet<&str> = FEATURED_FORMATS.iter().map(|f| f.id).collect();
        for f in REGULAR_FORMATS {
            assert!(
                !featured.contains(f.id),
                "{:?} appears in both FEATURED and REGULAR",
                f.id
            );
        }
    }
}
