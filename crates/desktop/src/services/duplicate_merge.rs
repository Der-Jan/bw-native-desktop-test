//! Loss-preserving login merge planning. Persistence and authorization stay in the SDK.
use bitwarden_vault::{CipherType, CipherView, FieldType, FieldView};

pub fn plan(primary: &CipherView, others: &[CipherView]) -> Result<CipherView, String> {
    if others.is_empty() {
        return Err("Select at least two entries".into());
    }
    let mut merged = primary.clone();
    if others
        .iter()
        .any(|v| v.reprompt == bitwarden_vault::CipherRepromptType::Password)
    {
        merged.reprompt = bitwarden_vault::CipherRepromptType::Password;
    }
    let mut fields = primary.fields.clone().unwrap_or_default();
    let mut ids = std::collections::HashSet::new();
    ids.insert(primary.id.ok_or("Missing primary ID")?);
    for item in std::iter::once(primary).chain(others) {
        if item.r#type != CipherType::Login
            || item.login.is_none()
            || item.partial
            || item.deleted_date.is_some()
            || item.archived_date.is_some()
            || !item.edit
            || !item.view_password
            || item
                .attachment_decryption_failures
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return Err("Only complete, editable active logins can be merged".into());
        }
        if item.organization_id != primary.organization_id {
            return Err("Merge entries within the same vault".into());
        }
    }
    let logins: Vec<_> = std::iter::once(primary)
        .chain(others)
        .map(|v| v.login.as_ref().unwrap())
        .collect();
    let totp: Vec<_> = logins
        .iter()
        .filter_map(|v| v.totp.as_ref().filter(|v| !v.trim().is_empty()))
        .collect();
    let passkeys: Vec<_> = logins
        .iter()
        .filter_map(|v| v.fido2_credentials.as_ref().filter(|v| !v.is_empty()))
        .collect();
    if totp.len() > 1 {
        return Err(
            "More than one selected entry contains TOTP; review these entries before merging"
                .into(),
        );
    }
    if passkeys.len() > 1 {
        return Err(
            "More than one selected entry contains passkeys; review these entries before merging"
                .into(),
        );
    }
    if let Some(value) = totp.first() {
        merged.login.as_mut().unwrap().totp = Some((*value).clone());
    }
    if let Some(value) = passkeys.first() {
        merged.login.as_mut().unwrap().fido2_credentials = Some((*value).clone());
    }
    let base = primary.login.as_ref().unwrap();
    for item in others {
        if !ids.insert(item.id.ok_or("Missing secondary ID")?) {
            return Err("Select each entry only once".into());
        }
        if item.permissions.as_ref().is_some_and(|p| !p.delete) {
            return Err("A secondary entry cannot be moved to trash".into());
        }
        let login = item.login.as_ref().unwrap();
        if item.attachments.as_ref().is_some_and(|v| !v.is_empty()) {
            return Err("Secondary entries with attachments cannot be merged yet".into());
        }
        let source = item.id.unwrap().to_string();
        let mut add = |label: &str, value: Option<&str>, hidden: bool| {
            if let Some(value) = value.filter(|v| !v.is_empty()) {
                let kind = if hidden {
                    FieldType::Hidden
                } else {
                    FieldType::Text
                };
                // Same name/value/type is already preserved; different values are never overwritten.
                let stem = format!("Merged {label}");
                if fields.iter().any(|f| {
                    f.name
                        .as_deref()
                        .is_some_and(|n| n == stem || n.starts_with(&(stem.clone() + " (")))
                        && f.value.as_deref() == Some(value)
                        && f.r#type == kind
                }) {
                    return;
                }
                let mut name = format!("{stem} ({source})");
                let mut suffix = 2;
                while fields.iter().any(|f| f.name.as_deref() == Some(&name)) {
                    name = format!("{stem} ({source}) {suffix}");
                    suffix += 1;
                }
                fields.push(FieldView {
                    name: Some(name),
                    value: Some(value.into()),
                    r#type: kind,
                    linked_id: None,
                });
            }
        };
        if item.name != primary.name {
            add("name", Some(&item.name), false);
        }
        if item.notes != primary.notes {
            add("notes", item.notes.as_deref(), true);
        }
        if login.username != base.username {
            add("username", login.username.as_deref(), false);
        }
        if login.password != base.password {
            add("password", login.password.as_deref(), true);
        }
        for uri in login.uris.iter().flatten() {
            if !base.uris.iter().flatten().any(|u| u == uri) {
                // Retain matching mode/checksum as well as the URL; primary URI rules stay unchanged.
                add(
                    "URL",
                    Some(&serde_json::to_string(uri).map_err(|_| "Could not preserve URL")?),
                    false,
                );
            }
        }
        for (label, value, original) in [
            (
                "autofill",
                login.autofill_on_page_load,
                base.autofill_on_page_load,
            ),
            (
                "HTTP authentication",
                login.http_authentication_enabled,
                base.http_authentication_enabled,
            ),
        ] {
            if value != original {
                add(
                    label,
                    value.map(|v| if v { "true" } else { "false" }),
                    false,
                );
            }
        }
        for history in item.password_history.iter().flatten() {
            if !primary
                .password_history
                .iter()
                .flatten()
                .any(|h| h == history)
            {
                add(
                    "password history",
                    Some(
                        &serde_json::to_string(history)
                            .map_err(|_| "Could not preserve password history")?,
                    ),
                    true,
                );
            }
        }
        for field in item.fields.iter().flatten() {
            let prior_name = format!(
                "{} ({source})",
                field.name.as_deref().unwrap_or("Unnamed field")
            );
            if fields.iter().any(|f| {
                f == field
                    || (f.value == field.value
                        && f.r#type == field.r#type
                        && f.linked_id == field.linked_id
                        && f.name.as_deref().is_some_and(|n| {
                            n == prior_name || n.starts_with(&(prior_name.clone() + " "))
                        }))
            }) {
                continue;
            }
            if field.r#type == FieldType::Linked {
                return Err(
                    "Secondary linked custom fields must be converted to text before merging"
                        .into(),
                );
            }
            let mut preserved = field.clone();
            if fields.iter().any(|f| f.name == preserved.name) {
                let stem = format!(
                    "{} ({source})",
                    field.name.as_deref().unwrap_or("Unnamed field")
                );
                let mut name = stem.clone();
                let mut index = 2;
                while fields.iter().any(|f| f.name.as_deref() == Some(&name)) {
                    name = format!("{stem} {index}");
                    index += 1;
                }
                preserved.name = Some(name);
            }
            fields.push(preserved);
        }
    }
    merged.fields = Some(fields);
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn login(username: &str, password: &str) -> CipherView {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4(), "name": "Synthetic login", "type": 1,
            "login": {"username":username,"password":password,"uris":[{"uri":"https://example.invalid","match":0}]},
            "collectionIds":[],"favorite":false,"reprompt":0,"organizationUseTotp":false,
            "edit":true,"viewPassword":true,"creationDate":"2026-01-01T00:00:00Z","revisionDate":"2026-01-01T00:00:00Z"
        })).unwrap()
    }
    #[test]
    fn duplicate_merge_keeps_primary_and_preserves_multiple_alternates() {
        let mut primary = login("alice", "primary-password");
        primary.fields = Some(vec![FieldView {
            name: Some("Account".into()),
            value: Some("one".into()),
            r#type: FieldType::Text,
            linked_id: None,
        }]);
        let mut email = login("alice@example.invalid", "other-password");
        email.notes = Some("secondary notes".into());
        email.fields = Some(vec![FieldView {
            name: Some("Account".into()),
            value: Some("two".into()),
            r#type: FieldType::Hidden,
            linked_id: None,
        }]);
        let mut third = login("alice2", "primary-password");
        third.login.as_mut().unwrap().uris.as_mut().unwrap()[0].uri =
            Some("https://other.invalid".into());
        third.reprompt = bitwarden_vault::CipherRepromptType::Password;
        let before = serde_json::to_value(&primary).unwrap();
        let merged = plan(&primary, &[email.clone(), third.clone()]).unwrap();
        assert_eq!(merged.id, primary.id);
        assert_eq!(merged.login, primary.login);
        assert_eq!(merged.name, primary.name);
        assert_eq!(
            merged.reprompt,
            bitwarden_vault::CipherRepromptType::Password
        );
        let fields = merged.fields.as_ref().unwrap();
        for value in [
            "alice@example.invalid",
            "alice2",
            "other-password",
            "secondary notes",
            "two",
        ] {
            assert!(
                fields.iter().any(|f| f.value.as_deref() == Some(value)),
                "missing synthetic value"
            );
        }
        assert!(
            fields
                .iter()
                .any(|f| f.value.as_deref() == Some("other-password")
                    && f.r#type == FieldType::Hidden)
        );
        assert!(fields.iter().any(|f| f.value.as_deref() == Some("two")
            && f.r#type == FieldType::Hidden
            && f.name.as_deref() != Some("Account")));
        assert!(fields.iter().any(|f| {
            f.value
                .as_deref()
                .is_some_and(|v| v.contains("https://other.invalid"))
        }));
        assert_eq!(serde_json::to_value(&primary).unwrap(), before);
        // Retrying after a partial cleanup does not multiply preserved fields.
        let again = plan(&merged, &[email, third]).unwrap();
        assert_eq!(again.fields, merged.fields);
    }
    #[test]
    fn duplicate_merge_transfers_single_totp_and_passkey_sources_and_blocks_multiple_sources() {
        let primary = login("alice", "secret");
        let mut other = login("alice@example.invalid", "secret");
        other.login.as_mut().unwrap().totp = Some("synthetic-totp".into());
        let credential=serde_json::from_value(serde_json::json!({
            "credentialId":"synthetic-id","keyType":"public-key","keyAlgorithm":"ECDSA","keyCurve":"P-256",
            "keyValue":"synthetic-private-material","rpId":"example.invalid","counter":"0","discoverable":"true","creationDate":"2026-01-01T00:00:00Z"
        })).unwrap();
        other.login.as_mut().unwrap().fido2_credentials = Some(vec![credential]);
        let merged = plan(&primary, &[other.clone()]).unwrap();
        assert_eq!(
            merged.login.as_ref().unwrap().totp,
            other.login.as_ref().unwrap().totp
        );
        assert_eq!(
            merged.login.as_ref().unwrap().fido2_credentials,
            other.login.as_ref().unwrap().fido2_credentials
        );
        assert_eq!(
            merged.login.as_ref().unwrap().username,
            primary.login.as_ref().unwrap().username
        );
        // Two entries are ambiguous even when the TOTP/passkey values happen to match.
        assert!(
            plan(&merged, &[other.clone()])
                .unwrap_err()
                .contains("TOTP")
        );
        other.login.as_mut().unwrap().totp = None;
        let mut primary_with_key = merged.clone();
        primary_with_key.login.as_mut().unwrap().totp = None;
        assert!(
            plan(&primary_with_key, &[other.clone()])
                .unwrap_err()
                .contains("passkeys")
        );
        let empty = login("bob", "secret");
        assert!(plan(&empty, &[primary, other]).is_ok());
    }

    #[test]
    fn duplicate_merge_rejects_invalid_or_incomplete_sources() {
        let primary = login("alice", "secret");
        assert!(plan(&primary, &[]).is_err());
        assert!(plan(&primary, std::slice::from_ref(&primary)).is_err());
        let mut other = login("alice@example.invalid", "other");
        other.partial = true;
        assert!(plan(&primary, &[other.clone()]).is_err());
        other.partial = false;
        other.edit = false;
        assert!(plan(&primary, &[other.clone()]).is_err());
        other.edit = true;
        other.deleted_date = Some(chrono::Utc::now());
        assert!(plan(&primary, &[other.clone()]).is_err());
        other.deleted_date = None;
        other.organization_id = Some(bitwarden_core::OrganizationId::new_v4());
        assert!(plan(&primary, &[other]).is_err());
    }
}
