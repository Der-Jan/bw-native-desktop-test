//! `CipherForm::update` — the big match that maps form messages onto
//! `CipherView` mutations.

use bitwarden_vault::{
    BankAccountView, CipherRepromptType, CipherView, DriversLicenseView, FieldType, FieldView,
    IdentityView, LoginUriView, PassportView, UriMatchType,
};

use super::{
    message::{CipherEditMessage, FormEvent},
    state::CipherForm,
};

impl CipherForm {
    pub fn update(&mut self, msg: CipherEditMessage) -> FormEvent {
        use CipherEditMessage::*;
        match msg {
            // Item details
            NameChanged(s) => self.modified.name = s,
            NotesAction(action) => {
                // `text_editor` owns its buffer + cursor; we mirror the
                // plain-text value back onto the CipherView so saves / diff
                // logic keep working without knowing about the editor state.
                self.notes_content.perform(action);
                self.modified.notes = opt_string(self.notes_content.text());
            }
            FavoriteToggled => self.modified.favorite = !self.modified.favorite,
            RepromptToggled => {
                self.modified.reprompt = match self.modified.reprompt {
                    CipherRepromptType::None => CipherRepromptType::Password,
                    CipherRepromptType::Password => CipherRepromptType::None,
                };
            }

            // Ownership
            FolderSelected(id) => {
                self.modified.folder_id = id;
            }
            FolderComboClosed => {
                // combo_box has no public API to reset its internal `value`;
                // rebuilding the State via `set_folders` is the only way.
                let folders = std::mem::take(&mut self.folders);
                self.set_folders(folders);
            }
            OrgSelected(id) => {
                self.modified.organization_id = id;
                // Clearing the org clears any collections that were scoped to it.
                if id.is_none() {
                    self.modified.collection_ids.clear();
                } else {
                    self.modified.collection_ids.retain(|cid| {
                        self.collections
                            .iter()
                            .any(|c| &c.id == cid && Some(c.organization_id) == id)
                    });
                }
            }
            OrgComboClosed => {
                let orgs = std::mem::take(&mut self.organizations);
                self.set_organizations(orgs);
            }
            CollectionsDropdownToggled => {
                self.collections_dropdown_open = !self.collections_dropdown_open;
            }
            CollectionToggled(cid) => {
                if let Some(pos) = self.modified.collection_ids.iter().position(|c| c == &cid) {
                    self.modified.collection_ids.remove(pos);
                } else {
                    self.modified.collection_ids.push(cid);
                }
            }

            // Login
            UsernameChanged(s) => {
                if let Some(l) = self.modified.login.as_mut() {
                    l.username = opt_string(s);
                }
            }
            PasswordChanged(s) => {
                if let Some(l) = self.modified.login.as_mut() {
                    l.password = opt_string(s);
                }
            }
            TotpChanged(s) => {
                if let Some(l) = self.modified.login.as_mut() {
                    l.totp = opt_string(s);
                }
            }
            UriChanged(idx, s) => {
                if let Some(l) = self.modified.login.as_mut() {
                    let uris = l.uris.get_or_insert_default();
                    if let Some(u) = uris.get_mut(idx) {
                        u.uri = opt_string(s);
                    }
                }
            }
            UriAdded => {
                if let Some(l) = self.modified.login.as_mut() {
                    l.uris.get_or_insert_default().push(LoginUriView {
                        uri: Some(String::new()),
                        r#match: Some(UriMatchType::Domain),
                        uri_checksum: None,
                    });
                }
            }
            UriRemoved(idx) => {
                if let Some(l) = self.modified.login.as_mut()
                    && let Some(uris) = l.uris.as_mut()
                    && idx < uris.len()
                {
                    uris.remove(idx);
                }
            }
            PasskeyRemoved(idx) => {
                if let Some(l) = self.modified.login.as_mut()
                    && let Some(creds) = l.fido2_credentials.as_mut()
                    && idx < creds.len()
                {
                    creds.remove(idx);
                }
            }

            // Card
            CardCardholderChanged(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.cardholder_name = opt_string(s);
                }
            }
            CardBrandSelected(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.brand = s;
                }
            }
            CardNumberChanged(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.number = opt_string(s);
                }
            }
            CardExpMonthSelected(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.exp_month = s;
                }
            }
            CardExpYearChanged(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.exp_year = opt_string(s);
                }
            }
            CardCodeChanged(s) => {
                if let Some(c) = self.modified.card.as_mut() {
                    c.code = opt_string(s);
                }
            }

            // Identity
            IdentityTitleSelected(s) => {
                if let Some(i) = self.modified.identity.as_mut() {
                    i.title = s;
                }
            }
            IdentityFirstNameChanged(s) => {
                identity_set(&mut self.modified, |i| &mut i.first_name, s)
            }
            IdentityMiddleNameChanged(s) => {
                identity_set(&mut self.modified, |i| &mut i.middle_name, s)
            }
            IdentityLastNameChanged(s) => identity_set(&mut self.modified, |i| &mut i.last_name, s),
            IdentityUsernameChanged(s) => identity_set(&mut self.modified, |i| &mut i.username, s),
            IdentityCompanyChanged(s) => identity_set(&mut self.modified, |i| &mut i.company, s),
            IdentitySsnChanged(s) => identity_set(&mut self.modified, |i| &mut i.ssn, s),
            IdentityPassportChanged(s) => {
                identity_set(&mut self.modified, |i| &mut i.passport_number, s)
            }
            IdentityLicenseChanged(s) => {
                identity_set(&mut self.modified, |i| &mut i.license_number, s)
            }
            IdentityEmailChanged(s) => identity_set(&mut self.modified, |i| &mut i.email, s),
            IdentityPhoneChanged(s) => identity_set(&mut self.modified, |i| &mut i.phone, s),
            IdentityAddress1Changed(s) => identity_set(&mut self.modified, |i| &mut i.address1, s),
            IdentityAddress2Changed(s) => identity_set(&mut self.modified, |i| &mut i.address2, s),
            IdentityAddress3Changed(s) => identity_set(&mut self.modified, |i| &mut i.address3, s),
            IdentityCityChanged(s) => identity_set(&mut self.modified, |i| &mut i.city, s),
            IdentityStateChanged(s) => identity_set(&mut self.modified, |i| &mut i.state, s),
            IdentityPostalCodeChanged(s) => {
                identity_set(&mut self.modified, |i| &mut i.postal_code, s)
            }
            IdentityCountryChanged(s) => identity_set(&mut self.modified, |i| &mut i.country, s),

            // Bank account
            BankNameChanged(s) => bank_set(&mut self.modified, |b| &mut b.bank_name, s),
            BankNameOnAccountChanged(s) => {
                bank_set(&mut self.modified, |b| &mut b.name_on_account, s)
            }
            BankAccountTypeSelected(s) => {
                if let Some(b) = self.modified.bank_account.as_mut() {
                    b.account_type = s;
                }
            }
            BankAccountNumberChanged(s) => {
                bank_set(&mut self.modified, |b| &mut b.account_number, s)
            }
            BankRoutingNumberChanged(s) => {
                bank_set(&mut self.modified, |b| &mut b.routing_number, s)
            }
            BankBranchNumberChanged(s) => bank_set(&mut self.modified, |b| &mut b.branch_number, s),
            BankPinChanged(s) => bank_set(&mut self.modified, |b| &mut b.pin, s),
            BankSwiftCodeChanged(s) => bank_set(&mut self.modified, |b| &mut b.swift_code, s),
            BankIbanChanged(s) => bank_set(&mut self.modified, |b| &mut b.iban, s),
            BankContactPhoneChanged(s) => {
                bank_set(&mut self.modified, |b| &mut b.bank_contact_phone, s)
            }

            // Drivers license
            DlFirstNameChanged(s) => dl_set(&mut self.modified, |d| &mut d.first_name, s),
            DlMiddleNameChanged(s) => dl_set(&mut self.modified, |d| &mut d.middle_name, s),
            DlLastNameChanged(s) => dl_set(&mut self.modified, |d| &mut d.last_name, s),
            DlDateOfBirthChanged(s) => {
                self.dates.birth = s;
                if let Some(value) = self.modified.drivers_license.as_mut()
                    && let Ok(date) = parse_date(&self.dates.birth)
                {
                    value.date_of_birth = date;
                }
            }
            DlLicenseNumberChanged(s) => dl_set(&mut self.modified, |d| &mut d.license_number, s),
            DlIssuingCountryChanged(s) => dl_set(&mut self.modified, |d| &mut d.issuing_country, s),
            DlIssuingStateChanged(s) => dl_set(&mut self.modified, |d| &mut d.issuing_state, s),
            DlIssuingAuthorityChanged(s) => {
                dl_set(&mut self.modified, |d| &mut d.issuing_authority, s)
            }
            DlIssueDateChanged(s) => {
                self.dates.issued = s;
                if let Some(value) = self.modified.drivers_license.as_mut()
                    && let Ok(date) = parse_date(&self.dates.issued)
                {
                    value.issue_date = date;
                }
            }
            DlExpirationDateChanged(s) => {
                self.dates.expires = s;
                if let Some(value) = self.modified.drivers_license.as_mut()
                    && let Ok(date) = parse_date(&self.dates.expires)
                {
                    value.expiration_date = date;
                }
            }
            DlLicenseClassChanged(s) => dl_set(&mut self.modified, |d| &mut d.license_class, s),

            // Passport
            PpGivenNameChanged(s) => pp_set(&mut self.modified, |p| &mut p.given_name, s),
            PpSurnameChanged(s) => pp_set(&mut self.modified, |p| &mut p.surname, s),
            PpDateOfBirthChanged(s) => {
                self.dates.birth = s;
                if let Some(value) = self.modified.passport.as_mut()
                    && let Ok(date) = parse_date(&self.dates.birth)
                {
                    value.date_of_birth = date;
                }
            }
            PpSexChanged(s) => pp_set(&mut self.modified, |p| &mut p.sex, s),
            PpBirthPlaceChanged(s) => pp_set(&mut self.modified, |p| &mut p.birth_place, s),
            PpNationalityChanged(s) => pp_set(&mut self.modified, |p| &mut p.nationality, s),
            PpPassportNumberChanged(s) => pp_set(&mut self.modified, |p| &mut p.passport_number, s),
            PpPassportTypeChanged(s) => pp_set(&mut self.modified, |p| &mut p.passport_type, s),
            PpNationalIdNumberChanged(s) => pp_set(
                &mut self.modified,
                |p| &mut p.national_identification_number,
                s,
            ),
            PpIssuingCountryChanged(s) => pp_set(&mut self.modified, |p| &mut p.issuing_country, s),
            PpIssuingAuthorityChanged(s) => {
                pp_set(&mut self.modified, |p| &mut p.issuing_authority, s)
            }
            PpIssueDateChanged(s) => {
                self.dates.issued = s;
                if let Some(value) = self.modified.passport.as_mut()
                    && let Ok(date) = parse_date(&self.dates.issued)
                {
                    value.issue_date = date;
                }
            }
            PpExpirationDateChanged(s) => {
                self.dates.expires = s;
                if let Some(value) = self.modified.passport.as_mut()
                    && let Ok(date) = parse_date(&self.dates.expires)
                {
                    value.expiration_date = date;
                }
            }

            // Custom fields
            CustomFieldAdded => {
                let fields = self.modified.fields.get_or_insert_default();
                fields.push(FieldView {
                    name: Some(String::new()),
                    value: Some(String::new()),
                    r#type: FieldType::Text,
                    linked_id: None,
                });
            }
            CustomFieldRemoved(idx) => {
                if let Some(fields) = self.modified.fields.as_mut()
                    && idx < fields.len()
                {
                    fields.remove(idx);
                }
            }
            CustomFieldTypeSelected(idx, ty) => {
                if let Some(fields) = self.modified.fields.as_mut()
                    && let Some(f) = fields.get_mut(idx)
                {
                    f.r#type = ty;
                    // Boolean fields represent value as "true"/"false" strings;
                    // reset to a known default when changing to/from bool so the
                    // widget rendering doesn't try to parse a free-form string.
                    if matches!(ty, FieldType::Boolean)
                        && !matches!(f.value.as_deref(), Some("true") | Some("false"))
                    {
                        f.value = Some("false".to_string());
                    }
                }
            }
            CustomFieldNameChanged(idx, s) => {
                if let Some(fields) = self.modified.fields.as_mut()
                    && let Some(f) = fields.get_mut(idx)
                {
                    f.name = opt_string(s);
                }
            }
            CustomFieldValueChanged(idx, s) => {
                if let Some(fields) = self.modified.fields.as_mut()
                    && let Some(f) = fields.get_mut(idx)
                {
                    f.value = opt_string(s);
                }
            }
            CustomFieldBoolToggled(idx) => {
                if let Some(fields) = self.modified.fields.as_mut()
                    && let Some(f) = fields.get_mut(idx)
                {
                    let next = !matches!(f.value.as_deref(), Some("true"));
                    f.value = Some(if next { "true" } else { "false" }.to_string());
                }
            }

            // Flow
            Save => return FormEvent::Save,
            Cancel => return FormEvent::Cancel,
        }
        FormEvent::None
    }
}

fn opt_string(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

fn identity_set(
    cv: &mut CipherView,
    pick: impl Fn(&mut IdentityView) -> &mut Option<String>,
    s: String,
) {
    if let Some(i) = cv.identity.as_mut() {
        *pick(i) = opt_string(s);
    }
}

fn bank_set(
    cv: &mut CipherView,
    pick: impl Fn(&mut BankAccountView) -> &mut Option<String>,
    s: String,
) {
    if let Some(b) = cv.bank_account.as_mut() {
        *pick(b) = opt_string(s);
    }
}

fn dl_set(
    cv: &mut CipherView,
    pick: impl Fn(&mut DriversLicenseView) -> &mut Option<String>,
    s: String,
) {
    if let Some(d) = cv.drivers_license.as_mut() {
        *pick(d) = opt_string(s);
    }
}

fn pp_set(cv: &mut CipherView, pick: impl Fn(&mut PassportView) -> &mut Option<String>, s: String) {
    if let Some(p) = cv.passport.as_mut() {
        *pick(p) = opt_string(s);
    }
}

fn parse_date(value: &str) -> Result<Option<chrono::NaiveDate>, chrono::ParseError> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse().map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitwarden_collections::collection::CollectionId;
    use bitwarden_core::OrganizationId;
    use bitwarden_vault::CipherType;

    use crate::services::sdk::{Collection, Organization};

    fn org(name: &str) -> Organization {
        Organization {
            id: OrganizationId::new_v4(),
            name: name.to_string(),
            wrapped_key: None,
        }
    }

    fn collection(in_org: OrganizationId, name: &str) -> Collection {
        Collection {
            id: CollectionId::new_v4(),
            organization_id: in_org,
            name: name.to_string(),
        }
    }

    fn form_with_org_setup(
        orgs: Vec<Organization>,
        collections: Vec<Collection>,
        starting_org: Option<OrganizationId>,
    ) -> CipherForm {
        let mut form = CipherForm::new(CipherType::Login, starting_org);
        form.set_organizations(orgs);
        form.collections = collections;
        form
    }

    #[test]
    fn org_selected_none_clears_all_collection_ids() {
        // Personal vault has no collections — switching to it must clear
        // any collection IDs the cipher was carrying from a previous org.
        let org_a = org("Org A");
        let org_a_id = org_a.id;
        let c1 = collection(org_a_id, "Shared");
        let c1_id = c1.id;

        let mut form = form_with_org_setup(vec![org_a], vec![c1], Some(org_a_id));
        form.modified.collection_ids = vec![c1_id];

        form.update(CipherEditMessage::OrgSelected(None));

        assert!(form.modified.organization_id.is_none());
        assert!(
            form.modified.collection_ids.is_empty(),
            "collection_ids cleared when leaving any org"
        );
    }

    #[test]
    fn org_selected_filters_collections_to_new_org() {
        // Switching from Org A to Org B must drop A's collection IDs but
        // KEEP any IDs that happen to belong to B (multi-collection
        // assignments survive the switch). The hash-set semantics matter:
        // a collection from a different org left on the cipher would
        // silently submit invalid IDs to the SDK.
        let org_a = org("Org A");
        let org_b = org("Org B");
        let org_a_id = org_a.id;
        let org_b_id = org_b.id;
        let c_a = collection(org_a_id, "A-shared");
        let c_b = collection(org_b_id, "B-shared");
        let c_a_id = c_a.id;
        let c_b_id = c_b.id;

        // Cipher is currently in Org A with both collection IDs attached
        // (the B id is hypothetically there from a multi-org scenario).
        let mut form = form_with_org_setup(vec![org_a, org_b], vec![c_a, c_b], Some(org_a_id));
        form.modified.collection_ids = vec![c_a_id, c_b_id];

        form.update(CipherEditMessage::OrgSelected(Some(org_b_id)));

        assert_eq!(form.modified.organization_id, Some(org_b_id));
        assert_eq!(
            form.modified.collection_ids,
            vec![c_b_id],
            "A's collection dropped, B's collection retained"
        );
    }

    #[test]
    fn collection_toggled_adds_then_removes_idempotently() {
        // Toggle semantics: present → remove, absent → add. The form
        // doesn't try to dedupe a CollectionToggled message arriving
        // twice; the second call removes the just-added entry. This is
        // what makes a checkbox-style multi-select feel right.
        let org_a = org("Org A");
        let org_a_id = org_a.id;
        let c1 = collection(org_a_id, "Shared");
        let c1_id = c1.id;

        let mut form = form_with_org_setup(vec![org_a], vec![c1], Some(org_a_id));

        form.update(CipherEditMessage::CollectionToggled(c1_id));
        assert_eq!(form.modified.collection_ids, vec![c1_id]);

        form.update(CipherEditMessage::CollectionToggled(c1_id));
        assert!(form.modified.collection_ids.is_empty());
    }
}
