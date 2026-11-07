//! `SendForm` — the editable state shown in the right-hand pane when a
//! send is selected or when the user clicks "New". Mirrors
//! `CipherForm`: the form owns a self-sufficient copy of the view and
//! reconciles back to a `SendView` on save.

use bitwarden_send::{AuthType, SendFileView, SendTextView, SendType, SendView};
use chrono::{DateTime, Duration, Utc};
use iced::widget::text_editor;

/// Result of dispatching a `SendEditMessage` into `SendForm::update`.
/// Mirrors `CipherForm`'s `FormEvent` but with extensions for the form's
/// copy-to-clipboard actions (link, password) and the regenerate-password
/// request which the containing view needs to promote to `SendEvent`s
/// (the actual SDK call lives at App level — it needs `views.generator`).
pub enum FormEvent {
    None,
    Save,
    Cancel,
    Delete,
    CopyLink(String),
    CopyPassword(String),
    RegeneratePassword,
}

/// Hard-coded deletion date preset durations. Picking one replaces the
/// form's `deletion_date` with `now + duration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionPreset {
    OneHour,
    OneDay,
    TwoDays,
    ThreeDays,
    SevenDays,
    FourteenDays,
    ThirtyDays,
}

impl DeletionPreset {
    pub const ALL: [DeletionPreset; 7] = [
        DeletionPreset::OneHour,
        DeletionPreset::OneDay,
        DeletionPreset::TwoDays,
        DeletionPreset::ThreeDays,
        DeletionPreset::SevenDays,
        DeletionPreset::FourteenDays,
        DeletionPreset::ThirtyDays,
    ];

    pub fn duration(self) -> Duration {
        match self {
            DeletionPreset::OneHour => Duration::hours(1),
            DeletionPreset::OneDay => Duration::days(1),
            DeletionPreset::TwoDays => Duration::days(2),
            DeletionPreset::ThreeDays => Duration::days(3),
            DeletionPreset::SevenDays => Duration::days(7),
            DeletionPreset::FourteenDays => Duration::days(14),
            DeletionPreset::ThirtyDays => Duration::days(30),
        }
    }
}

/// Form-side "who can view" selector. Maps onto the SDK's `AuthType` plus
/// the password / emails payload captured by the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Link,
    People,
    Password,
}

impl AccessType {
    pub const ALL: [AccessType; 3] = [AccessType::Link, AccessType::People, AccessType::Password];
}

pub struct SendForm {
    pub(super) original: Option<SendView>,
    /// A `SendView::id` of `None` marks this as a not-yet-saved draft.
    pub(super) id: Option<bitwarden_send::SendId>,
    pub(super) access_id: Option<String>,

    pub(super) name: String,
    pub(super) send_type: SendType,

    // Text-send fields
    pub(super) text_content: text_editor::Content,
    pub(super) text_hidden: bool,

    // File-send fields (always display-only in this stub)
    pub(super) file_name: String,
    pub(super) file_size_name: Option<String>,

    // Deletion date
    pub(super) deletion_date: DateTime<Utc>,
    pub(super) deletion_preset: DeletionPreset,

    // Access control
    pub(super) access_type: AccessType,
    pub(super) password: String,
    pub(super) password_revealed: bool,
    pub(super) emails_content: text_editor::Content,

    // Additional options
    pub(super) max_access_count_raw: String,
    pub(super) access_count: u32,
    pub(super) hide_email: bool,
    pub(super) notes_content: text_editor::Content,

    pub saving: bool,
}

impl SendForm {
    /// Open a fresh form for a new send. Defaults: 7-day deletion, link-only
    /// access, no password, no note.
    pub fn new(send_type: SendType) -> Self {
        let deletion_preset = DeletionPreset::SevenDays;
        let deletion_date = Utc::now() + deletion_preset.duration();
        Self {
            original: None,
            id: None,
            access_id: None,

            name: String::new(),
            send_type,

            text_content: text_editor::Content::new(),
            text_hidden: false,

            file_name: String::new(),
            file_size_name: None,

            deletion_date,
            deletion_preset,

            access_type: AccessType::Link,
            password: String::new(),
            password_revealed: false,
            emails_content: text_editor::Content::new(),

            max_access_count_raw: String::new(),
            access_count: 0,
            hide_email: false,
            notes_content: text_editor::Content::new(),

            saving: false,
        }
    }

    /// Open the form populated from an existing `SendView`.
    pub fn edit(view: SendView) -> Self {
        let text_content = view
            .text
            .as_ref()
            .and_then(|t| t.text.clone())
            .map(|t| text_editor::Content::with_text(&t))
            .unwrap_or_default();
        let text_hidden = view.text.as_ref().map(|t| t.hidden).unwrap_or(false);
        let file_name = view
            .file
            .as_ref()
            .map(|f| f.file_name.clone())
            .unwrap_or_default();
        let file_size_name = view.file.as_ref().and_then(|f| f.size_name.clone());

        let (access_type, password) = if view.has_password {
            (AccessType::Password, String::new())
        } else if !view.emails.is_empty() {
            (AccessType::People, String::new())
        } else {
            (AccessType::Link, String::new())
        };
        let emails_content = text_editor::Content::with_text(&view.emails.join(", "));

        let notes_content = view
            .notes
            .as_deref()
            .map(text_editor::Content::with_text)
            .unwrap_or_default();

        let max_access_count_raw = view
            .max_access_count
            .map(|n| n.to_string())
            .unwrap_or_default();

        // Classify the current deletion delta as closest preset. Purely
        // cosmetic — the actual deletion_date is what gets saved.
        let preset = classify_preset(view.deletion_date);

        Self {
            original: Some(view.clone()),
            id: view.id,
            access_id: view.access_id,
            name: view.name,
            send_type: view.r#type,
            text_content,
            text_hidden,
            file_name,
            file_size_name,
            deletion_date: view.deletion_date,
            deletion_preset: preset,
            access_type,
            password,
            password_revealed: false,
            emails_content,
            max_access_count_raw,
            access_count: view.access_count,
            hide_email: view.hide_email,
            notes_content,
            saving: false,
        }
    }

    /// Project the form back into the SDK's `SendView` shape. Called on
    /// save; `ClientManager::save_send` takes the result.
    pub fn to_send_view(&self) -> SendView {
        let notes_str = self.notes_content.text();
        let notes = if notes_str.trim().is_empty() {
            None
        } else {
            Some(notes_str)
        };

        let text = if matches!(self.send_type, SendType::Text) {
            let body = self.text_content.text();
            Some(SendTextView {
                text: if body.is_empty() { None } else { Some(body) },
                hidden: self.text_hidden,
            })
        } else {
            None
        };

        let file = if matches!(self.send_type, SendType::File) {
            Some(SendFileView {
                id: self
                    .original
                    .as_ref()
                    .and_then(|o| o.file.as_ref())
                    .and_then(|f| f.id.clone()),
                file_name: self.file_name.clone(),
                size: self
                    .original
                    .as_ref()
                    .and_then(|o| o.file.as_ref())
                    .and_then(|f| f.size.clone()),
                size_name: self.file_size_name.clone(),
            })
        } else {
            None
        };

        let emails: Vec<String> = match self.access_type {
            AccessType::People => parse_emails(&self.emails_content.text()),
            _ => Vec::new(),
        };

        let auth_type = match self.access_type {
            AccessType::Link => AuthType::None,
            AccessType::People => AuthType::Email,
            AccessType::Password => AuthType::Password,
        };

        let new_password = match self.access_type {
            AccessType::Password if !self.password.is_empty() => Some(self.password.clone()),
            _ => None,
        };
        let has_password = match self.access_type {
            AccessType::Password => {
                !self.password.is_empty()
                    || self
                        .original
                        .as_ref()
                        .map(|o| o.has_password)
                        .unwrap_or(false)
            }
            _ => false,
        };

        let max_access_count = parse_limit(&self.max_access_count_raw);

        SendView {
            id: self.id,
            access_id: self.access_id.clone(),
            name: self.name.clone(),
            notes,
            // `key` is generated server-side on create; we just carry over
            // whatever the original had. The in-memory stub never touches it.
            key: self.original.as_ref().and_then(|o| o.key.clone()),
            new_password,
            has_password,
            r#type: self.send_type,
            file,
            text,
            data: self.original.as_ref().and_then(|o| o.data.clone()),
            max_access_count,
            access_count: self.access_count,
            disabled: self.original.as_ref().map(|o| o.disabled).unwrap_or(false),
            hide_email: self.hide_email,
            revision_date: Utc::now(),
            deletion_date: self.deletion_date,
            expiration_date: self.original.as_ref().and_then(|o| o.expiration_date),
            emails,
            auth_type,
        }
    }

    /// Read-only access to the form's current name. Used by callers that
    /// need to render it outside the form (e.g. delete confirmation modal).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// True once all required fields are populated. Called from the view
    /// handler's `FormEvent::Save` branch to decide between running the
    /// save task and showing a "please fill in required fields" toast.
    pub fn is_valid(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }
        match self.send_type {
            SendType::Text => !self.text_content.text().trim().is_empty(),
            SendType::File => !self.file_name.trim().is_empty(),
            SendType::Item => false,
        }
    }

    /// External URL for the send. Stubbed: the SDK's real create path would
    /// hand back a key fragment; for now this is just a placeholder the
    /// copy-link button hands to the clipboard.
    pub(super) fn send_link(&self) -> Option<String> {
        self.access_id
            .as_ref()
            .map(|id| format!("http://vault.bitwarden.test/#/send/{id}"))
    }

    /// Remaining views, used for the hint under the limit-views field.
    pub(super) fn views_left(&self) -> Option<u32> {
        parse_limit(&self.max_access_count_raw).map(|max| max.saturating_sub(self.access_count))
    }

    // ── Mutators used by `update.rs` ──────────────────────────────────────

    pub(super) fn set_name(&mut self, value: String) {
        self.name = value;
    }

    pub(super) fn set_text_hidden(&mut self, value: bool) {
        self.text_hidden = value;
    }

    pub(super) fn set_access_type(&mut self, value: AccessType) {
        self.access_type = value;
    }

    pub(super) fn set_password(&mut self, value: String) {
        self.password = value;
    }

    pub(super) fn toggle_password_reveal(&mut self) {
        self.password_revealed = !self.password_revealed;
    }

    /// Apply a password generated by `ClientManager::generate_password`.
    /// Reveal the field so the user sees the new value immediately —
    /// matches the cipher form's regenerate behaviour.
    pub fn apply_generated_password(&mut self, value: String) {
        self.password = value;
        self.password_revealed = true;
    }

    pub(super) fn set_deletion_preset(&mut self, preset: DeletionPreset) {
        self.deletion_preset = preset;
        self.deletion_date = Utc::now() + preset.duration();
    }

    pub(super) fn set_max_access_count_raw(&mut self, value: String) {
        if value.is_empty() || value.chars().all(|c| c.is_ascii_digit()) {
            self.max_access_count_raw = value;
        }
    }

    pub(super) fn bump_max_access_count(&mut self, delta: i32) {
        let current = parse_limit(&self.max_access_count_raw).unwrap_or(0) as i32;
        let next = (current + delta).max(0) as u32;
        self.max_access_count_raw = if next == 0 {
            String::new()
        } else {
            next.to_string()
        };
    }
}

/// Pick the preset whose duration most closely matches the given date's
/// distance from `now`. Ties go to the smaller preset.
fn classify_preset(deletion_date: DateTime<Utc>) -> DeletionPreset {
    let now = Utc::now();
    let delta = deletion_date.signed_duration_since(now);
    let mut best = DeletionPreset::SevenDays;
    let mut best_diff = Duration::MAX;
    for preset in DeletionPreset::ALL {
        let diff = (preset.duration() - delta).abs();
        if diff < best_diff {
            best_diff = diff;
            best = preset;
        }
    }
    best
}

fn parse_limit(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok().filter(|&n| n > 0)
}

fn parse_emails(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod sdk4_item_send_tests {
    use super::*;

    #[test]
    fn unsupported_item_sends_cannot_be_saved_as_text_or_file_sends() {
        let mut form = SendForm::new(SendType::Item);
        form.name = "Synthetic item".into();
        form.file_name = "synthetic.txt".into();
        form.text_content = iced::widget::text_editor::Content::with_text("synthetic text");
        assert!(!form.is_valid());
    }
}
