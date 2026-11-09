//! Vaultwarden policy enforcement over the same immutable account snapshot.
//! No Bitwarden-only timeout, export or SSO policy is applied.
use super::{ClientManager, SessionToken};
use bitwarden_api_api::models::SyncResponseModel;
use bitwarden_core::OrganizationId;
use bitwarden_generators::{PassphraseGeneratorRequest, PasswordGeneratorRequest};
use bitwarden_vault::{CipherType, UriMatchType};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub const DENIED: &str = "This action is restricted by an organization policy";
const INVALID: &str =
    "Invalid or unsupported organization policy data; the previous vault snapshot is retained";

#[derive(Clone, Debug, Default)]
pub struct GeneratorRules {
    pub enabled: bool,
    pub mode: Option<usize>,
    pub min_length: u8,
    pub upper: bool,
    pub lower: bool,
    pub numbers: bool,
    pub special: bool,
    pub min_numbers: u8,
    pub min_special: u8,
    pub min_words: u8,
    pub capitalize: bool,
    pub include_number: bool,
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GeneratorData {
    override_password_type: Option<String>,
    min_length: Option<i64>,
    #[serde(alias = "useUpper")]
    use_uppercase: Option<bool>,
    #[serde(alias = "useLower")]
    use_lowercase: Option<bool>,
    use_numbers: Option<bool>,
    use_special: Option<bool>,
    #[serde(alias = "minNumbers")]
    number_count: Option<i64>,
    #[serde(alias = "minSpecial")]
    special_count: Option<i64>,
    min_number_words: Option<i64>,
    capitalize: Option<bool>,
    include_number: Option<bool>,
}
fn bounded(value: i64, max: u8) -> Result<u8, String> {
    u8::try_from(value.max(0))
        .ok()
        .filter(|v| *v <= max)
        .ok_or_else(|| INVALID.into())
}
impl GeneratorRules {
    fn merge(&mut self, data: &Value) -> Result<(), String> {
        let d: GeneratorData = serde_json::from_value(if data.is_null() {
            serde_json::json!({})
        } else {
            data.clone()
        })
        .map_err(|_| INVALID)?;
        self.enabled = true;
        let mode = match d.override_password_type.as_deref().unwrap_or("") {
            "" => None,
            "password" => Some(0),
            "passphrase" => Some(1),
            _ => return Err(INVALID.into()),
        };
        if self.mode != Some(0) && mode.is_some() {
            self.mode = mode;
        }
        self.min_length = self
            .min_length
            .max(bounded(d.min_length.unwrap_or(0), 128)?);
        self.upper |= d.use_uppercase.unwrap_or(false);
        self.lower |= d.use_lowercase.unwrap_or(false);
        self.numbers |= d.use_numbers.unwrap_or(false);
        self.special |= d.use_special.unwrap_or(false);
        self.min_numbers = self
            .min_numbers
            .max(bounded(d.number_count.unwrap_or(0), 9)?);
        self.min_special = self
            .min_special
            .max(bounded(d.special_count.unwrap_or(0), 9)?);
        self.min_words = self
            .min_words
            .max(bounded(d.min_number_words.unwrap_or(0), 20)?);
        self.capitalize |= d.capitalize.unwrap_or(false);
        self.include_number |= d.include_number.unwrap_or(false);
        self.numbers |= self.min_numbers > 0;
        self.special |= self.min_special > 0;
        Ok(())
    }
    pub fn password(
        &self,
        mut request: PasswordGeneratorRequest,
    ) -> Result<PasswordGeneratorRequest, String> {
        if self.mode == Some(1) {
            return Err(DENIED.into());
        }
        request.lowercase |= self.lower;
        request.uppercase |= self.upper;
        request.numbers |= self.numbers;
        request.special |= self.special;
        if self.lower {
            request.min_lowercase = Some(request.min_lowercase.unwrap_or(1).max(1));
        }
        if self.upper {
            request.min_uppercase = Some(request.min_uppercase.unwrap_or(1).max(1));
        }
        if request.numbers {
            request.min_number = Some(request.min_number.unwrap_or(1).max(self.min_numbers).max(1));
        }
        if request.special {
            request.min_special = Some(
                request
                    .min_special
                    .unwrap_or(1)
                    .max(self.min_special)
                    .max(1),
            );
        }
        let required = [
            (request.lowercase, request.min_lowercase),
            (request.uppercase, request.min_uppercase),
            (request.numbers, request.min_number),
            (request.special, request.min_special),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, min)| u16::from(min.unwrap_or(1).max(1)))
        .sum::<u16>()
            + u16::from(
                request
                    .custom_required_chars
                    .as_ref()
                    .is_some_and(|s| !s.is_empty()),
            );
        let length = u16::from(request.length)
            .max(u16::from(self.min_length))
            .max(required);
        if length > 128 {
            return Err(DENIED.into());
        }
        request.length = length as u8;
        Ok(request)
    }
    pub fn passphrase(
        &self,
        mut request: PassphraseGeneratorRequest,
    ) -> Result<PassphraseGeneratorRequest, String> {
        if self.mode == Some(0) {
            return Err(DENIED.into());
        }
        request.num_words = request.num_words.max(self.min_words);
        request.capitalize |= self.capitalize;
        request.include_number |= self.include_number;
        if request.num_words > 20 {
            return Err(DENIED.into());
        }
        Ok(request)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub generator: GeneratorRules,
    pub personal_ownership: bool,
    pub pin_disabled: bool,
    pub uri_default: Option<UriMatchType>,
    pub summaries: Vec<(String, String)>,
    restricted: HashMap<OrganizationId, HashSet<u8>>,
}
impl Rules {
    pub(super) fn from_response(response: &SyncResponseModel) -> Result<Arc<Self>, String> {
        let profile = response.profile.as_ref().ok_or(INVALID)?;
        let orgs = profile
            .organizations_new
            .as_ref()
            .or(profile.organizations.as_ref())
            .ok_or(INVALID)?;
        let organizations: HashMap<_, _> =
            orgs.iter().filter_map(|o| o.id.map(|id| (id, o))).collect();
        let mut rules = Self::default();
        let Some(policies) = response.policies.as_ref() else {
            return if orgs.is_empty() {
                Ok(Arc::new(rules))
            } else {
                Err(INVALID.into())
            };
        };
        let mut ids = HashSet::new();
        let mut types = HashSet::new();
        for policy in policies {
            let id = policy.id.ok_or(INVALID)?;
            let org = policy.organization_id.ok_or(INVALID)?;
            let kind = policy.r#type.ok_or(INVALID)?.as_i64();
            if !ids.insert(id) || !types.insert((org, kind)) {
                return Err(INVALID.into());
            }
            let member = organizations.get(&org).ok_or(INVALID)?;
            if !policy.enabled.ok_or(INVALID)? {
                continue;
            }
            // Ignore explicitly unsupported Bitwarden-only policies. Unknown
            // future policies are rejected instead of silently weakening them.
            if matches!(kind, 4 | 9 | 10 | 11 | 12 | 13 | 18 | 19 | 21 | 23) {
                continue;
            }
            if !matches!(
                kind,
                0 | 1 | 2 | 3 | 5 | 6 | 7 | 8 | 14 | 15 | 16 | 17 | 20 | 22
            ) {
                return Err(INVALID.into());
            }
            let status = member.status.map(|s| s.as_i64()).unwrap_or(2);
            if !matches!(status, 1 | 2)
                || member.enabled == Some(false)
                || member.use_policies == Some(false)
            {
                continue;
            }
            let admin = member
                .r#type
                .is_some_and(|role| matches!(role.as_i64(), 0 | 1));
            if admin && !matches!(kind, 2 | 15) {
                continue;
            }
            let data = policy.data.as_ref().unwrap_or(&Value::Null);
            let label = match kind {
                0 => "Two-factor authentication (checked by server)",
                1 => "Master-password requirements (checked at login)",
                2 => {
                    rules.generator.merge(data)?;
                    "Password/passphrase generator requirements"
                }
                3 => "Single organization (membership checked by server)",
                5 => {
                    rules.personal_ownership = true;
                    "Personal vault writes disabled"
                }
                6 => "Send disabled",
                7 => "Send options (Send is unavailable in this client)",
                8 => "Account recovery (manage enrollment in the web vault)",
                14 => {
                    rules.pin_disabled = true;
                    "PIN unlock disabled"
                }
                15 => {
                    let values = if data.is_null() {
                        vec![Value::from(3)]
                    } else {
                        data.as_array().ok_or(INVALID)?.clone()
                    };
                    let set = rules
                        .restricted
                        .entry(OrganizationId::new(org))
                        .or_default();
                    for value in values {
                        let t = value
                            .as_u64()
                            .filter(|t| (1..=8).contains(t))
                            .ok_or(INVALID)?;
                        set.insert(t as u8);
                    }
                    "Restricted entry types (including read and copy)"
                }
                16 => {
                    let value = data
                        .get("uriMatchDetection")
                        .and_then(Value::as_u64)
                        .filter(|n| *n <= 5)
                        .ok_or(INVALID)?;
                    let match_type: UriMatchType =
                        serde_json::from_value(Value::from(value)).map_err(|_| INVALID)?;
                    // Match the client's first applicable default; this is a
                    // default for new URIs, not a rewrite of existing matches.
                    if rules.uri_default.is_none() {
                        rules.uri_default = Some(match_type);
                    }
                    "Default URI matching for new website addresses"
                }
                17 => "Autotype default (autotype is unavailable in this client)",
                20 => "Organization notifications (handled by server)",
                22 => "Fill Assist default (autofill is handled by the browser extension)",
                _ => unreachable!(),
            };
            rules.summaries.push((
                member.name.clone().unwrap_or_else(|| "Organization".into()),
                label.into(),
            ));
        }
        Ok(Arc::new(rules))
    }
    pub fn allows_read(&self, kind: CipherType, organization: Option<OrganizationId>) -> bool {
        let t = kind as u8;
        if !self.restricted.values().any(|types| types.contains(&t)) {
            return true;
        }
        organization.is_some_and(|org| {
            !self
                .restricted
                .get(&org)
                .is_some_and(|types| types.contains(&t))
        })
    }
    pub fn allows_write(&self, kind: CipherType, organization: Option<OrganizationId>) -> bool {
        self.allows_read(kind, organization) && !(self.personal_ownership && organization.is_none())
    }
}
impl SessionToken {
    pub(super) fn generator_rules(&self) -> Result<GeneratorRules, String> {
        if let Some(snapshot) = self.state.snapshot.current() {
            return Ok(snapshot.policies.generator.clone());
        }
        if !self
            .state
            .writable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(
                "Synchronize once to load organization policies before generating passwords".into(),
            );
        }
        Ok(GeneratorRules::default())
    }
    pub(super) fn policy_rules(&self) -> Arc<Rules> {
        self.state
            .snapshot
            .current()
            .map(|s| s.policies.clone())
            .unwrap_or_default()
    }
}
impl ClientManager {
    pub fn policy_rules_for(&self, uid: &bitwarden_core::UserId) -> Arc<Rules> {
        self.users
            .get(uid)
            .map(|e| e.session.token().policy_rules())
            .unwrap_or_default()
    }
}

/// Identity's PascalCase policy is authoritative for enforce-on-login. Password
/// changes remain a web-vault operation; no weak login is published locally.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MasterPasswordRules {
    #[serde(alias = "MinComplexity")]
    min_complexity: Option<u8>,
    #[serde(alias = "MinLength")]
    min_length: Option<u32>,
    #[serde(alias = "RequireUpper")]
    require_upper: bool,
    #[serde(alias = "RequireLower")]
    require_lower: bool,
    #[serde(alias = "RequireNumbers")]
    require_numbers: bool,
    #[serde(alias = "RequireSpecial")]
    require_special: bool,
    #[serde(alias = "EnforceOnLogin")]
    enforce_on_login: bool,
}
pub(crate) async fn validate_login(
    value: &Value,
    password: &str,
    email: &str,
) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let policy: MasterPasswordRules = serde_json::from_value(value.clone())
        .map_err(|_| "Invalid master-password policy from server")?;
    if !policy.enforce_on_login {
        return Ok(());
    }
    if policy.min_complexity.unwrap_or(0) > 4 {
        return Err("Unsupported master-password strength requirement".into());
    }
    let sufficient = password.encode_utf16().count() >= policy.min_length.unwrap_or(0) as usize
        && (!policy.require_upper || password.chars().any(char::is_uppercase))
        && (!policy.require_lower || password.chars().any(char::is_lowercase))
        && (!policy.require_numbers || password.chars().any(|c| c.is_ascii_digit()))
        && (!policy.require_special || password.chars().any(|c| "!@#$%^&*".contains(c)));
    if !sufficient {
        return Err("Your organization requires a stronger master password. Change it in the web vault before signing in.".into());
    }
    if policy.min_complexity.unwrap_or(0) > 0 {
        let password = zeroize::Zeroizing::new(password.to_owned());
        let email = email.to_owned();
        let score = tokio::task::spawn_blocking(move || {
            bitwarden_pm::PasswordManagerClient::new(None)
                .0
                .auth()
                .password_strength(password.to_string(), email, Vec::new())
        })
        .await
        .map_err(|_| "Could not check master-password strength")?;
        if score < policy.min_complexity.unwrap_or(0) {
            return Err("Your organization requires a stronger master password. Change it in the web vault before signing in.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitwarden_api_api::models::SyncResponseModel;
    use serde_json::json;
    fn rules(orgs: Value, policies: Value) -> Result<Arc<Rules>, String> {
        let data: SyncResponseModel =
            serde_json::from_value(json!({"profile":{"organizations":orgs},"policies":policies}))
                .unwrap();
        Rules::from_response(&data)
    }
    fn member(id: uuid::Uuid, role: i64, status: i64) -> Value {
        json!({"id":id,"name":"Test organization","type":role,"status":status,"enabled":true,"usePolicies":true})
    }
    fn policy(org: uuid::Uuid, kind: i64, data: Value) -> Value {
        json!({"id":uuid::Uuid::new_v4(),"organizationId":org,"type":kind,"enabled":true,"data":data})
    }
    #[test]
    fn merges_generator_rules_and_roles_without_weakening_or_applying_enterprise_only_rules() {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let rules = rules(
            json!([member(a, 0, 2), member(b, 2, 2)]),
            json!([
                policy(
                    a,
                    2,
                    json!({"minLength":22,"useUppercase":true,"numberCount":4})
                ),
                policy(
                    b,
                    2,
                    json!({"minLength":16,"useLowercase":true,"specialCount":3})
                ),
                policy(a, 5, json!({})),
                policy(b, 9, json!({"minutes":1})),
                policy(b, 10, json!({}))
            ]),
        )
        .unwrap();
        assert!(
            !rules.personal_ownership,
            "owners are exempt from personal-ownership policy"
        );
        let req = rules
            .generator
            .password(PasswordGeneratorRequest {
                length: 5,
                lowercase: false,
                uppercase: false,
                numbers: false,
                special: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(req.length, 22);
        assert!(req.lowercase && req.uppercase && req.numbers && req.special);
        assert_eq!(req.min_number, Some(4));
        assert_eq!(req.min_special, Some(3));
        assert_eq!(
            rules.summaries.len(),
            2,
            "unsupported enterprise policies are not enforced"
        );
    }
    #[test]
    fn generator_type_passphrase_and_strict_limits_are_enforced() {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let r=rules(json!([member(a,2,2)]),json!([policy(a,2,json!({"overridePasswordType":"passphrase","minNumberWords":8,"capitalize":true,"includeNumber":true}))])).unwrap();
        assert!(
            r.generator
                .password(PasswordGeneratorRequest::default())
                .is_err()
        );
        let req = r
            .generator
            .passphrase(PassphraseGeneratorRequest::default())
            .unwrap();
        assert_eq!(req.num_words, 8);
        assert!(req.capitalize && req.include_number);
        let r = rules(
            json!([member(a, 2, 2), member(b, 2, 2)]),
            json!([
                policy(a, 2, json!({"overridePasswordType":"passphrase"})),
                policy(b, 2, json!({"overridePasswordType":"password"}))
            ]),
        )
        .unwrap();
        assert_eq!(r.generator.mode, Some(0));
        assert!(
            r.generator
                .passphrase(PassphraseGeneratorRequest::default())
                .is_err()
        );
        let nullable = rules(
            json!([member(a, 2, 2)]),
            json!([policy(
                a,
                2,
                json!({"overridePasswordType":null,"minLength":null,"useUppercase":null})
            )]),
        )
        .unwrap();
        assert!(
            nullable
                .generator
                .password(PasswordGeneratorRequest::default())
                .is_ok()
        );
        for data in [
            json!({"minLength":129}),
            json!({"numberCount":10}),
            json!({"minNumberWords":21}),
            json!({"useUppercase":"false"}),
        ] {
            assert!(rules(json!([member(a, 2, 2)]), json!([policy(a, 2, data)])).is_err());
        }
    }
    #[test]
    fn restrictions_have_union_for_personal_items_and_org_specific_exceptions() {
        let a = uuid::Uuid::new_v4();
        let b = uuid::Uuid::new_v4();
        let r = rules(
            json!([member(a, 0, 2), member(b, 2, 2)]),
            json!([policy(a, 15, json!([1, 5]))]),
        )
        .unwrap();
        assert!(!r.allows_read(CipherType::Login, None));
        assert!(!r.allows_read(CipherType::SshKey, Some(OrganizationId::new(a))));
        assert!(r.allows_read(CipherType::Login, Some(OrganizationId::new(b))));
        assert!(r.allows_read(CipherType::Card, None));
        let r = rules(
            json!([member(a, 2, 2)]),
            json!([
                policy(a, 15, Value::Null),
                policy(a, 5, json!({})),
                policy(a, 14, json!({})),
                policy(a, 16, json!({"uriMatchDetection":3}))
            ]),
        )
        .unwrap();
        assert!(!r.allows_read(CipherType::Card, None));
        assert!(r.allows_read(CipherType::Login, None));
        assert!(!r.allows_write(CipherType::Login, None));
        assert!(r.pin_disabled);
        assert_eq!(r.uri_default, Some(UriMatchType::Exact));
    }
    #[test]
    fn malformed_duplicate_foreign_or_unknown_policies_do_not_replace_rules() {
        let a = uuid::Uuid::new_v4();
        let p = policy(a, 2, json!({}));
        for policies in [
            json!([p, p]),
            json!([policy(uuid::Uuid::new_v4(), 2, json!({}))]),
            json!([policy(a, 999, json!({}))]),
            json!([policy(a, 15, json!([99]))]),
        ] {
            assert!(rules(json!([member(a, 2, 2)]), policies).is_err());
        }
        assert!(rules(json!([member(a, 2, 2)]), Value::Null).is_err());
        for status in [-1, 0, 3] {
            let r = rules(
                json!([member(a, 2, status)]),
                json!([policy(a, 5, json!({}))]),
            )
            .unwrap();
            assert!(!r.personal_ownership);
        }
    }
    #[tokio::test]
    async fn login_policy_handles_pascal_case_nulls_and_weak_passwords() {
        let strict = json!({"EnforceOnLogin":true,"MinLength":16,"MinComplexity":0,"RequireUpper":true,"RequireLower":true,"RequireNumbers":true,"RequireSpecial":true});
        assert!(
            validate_login(&strict, "short", "test@example.invalid")
                .await
                .is_err()
        );
        assert!(
            validate_login(&strict, "Long-Synthetic!Pass123", "test@example.invalid")
                .await
                .is_ok()
        );
        assert!(
            validate_login(
                &json!({"EnforceOnLogin":true,"MinLength":null,"MinComplexity":null}),
                "test",
                "test@example.invalid"
            )
            .await
            .is_ok()
        );
        assert!(
            validate_login(
                &json!({"enforceOnLogin":false,"minLength":100}),
                "test",
                "test@example.invalid"
            )
            .await
            .is_ok()
        );
        assert!(
            validate_login(
                &json!({"enforceOnLogin":true,"minComplexity":4}),
                "password",
                "test@example.invalid"
            )
            .await
            .is_err()
        );
        assert!(
            validate_login(
                &json!({"EnforceOnLogin":"false"}),
                "test",
                "test@example.invalid"
            )
            .await
            .is_err()
        );
    }
}
