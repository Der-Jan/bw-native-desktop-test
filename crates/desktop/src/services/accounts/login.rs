//! A bounded password/2FA HTTP adapter. Hashing and account-key validation use SDK crypto.
//! The SDK's current password-login API cannot represent 2FA/device credentials.
use super::transport::{Transport, json_body};
use bitwarden_core::{
    UserId,
    key_management::{
        MasterPasswordAuthenticationData, MasterPasswordUnlockData,
        account_cryptographic_state::WrappedAccountCryptographicState,
    },
};
use bitwarden_crypto::Kdf;
use reqwest_account::{Method, Url};
use serde::{Deserialize, Serialize};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Attempt {
    pub id: Uuid,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl Attempt {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(300),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst) || Instant::now() >= self.deadline {
            Err("Login was cancelled or expired".into())
        } else {
            Ok(())
        }
    }
}
impl std::fmt::Debug for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginAttempt")
            .field("id", &self.id)
            .finish()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Factor {
    Authenticator,
    Email,
}
impl Factor {
    pub fn number(self) -> u8 {
        match self {
            Self::Authenticator => 0,
            Self::Email => 1,
        }
    }
}
impl std::fmt::Display for Factor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Authenticator => "Authenticator (TOTP)",
            Self::Email => "Email",
        })
    }
}

pub struct PendingLogin {
    pub attempt: Attempt,
    pub transport: Transport,
    pub email: String,
    pub kdf: Kdf,
    pub api: Url,
    pub identity: Url,
    pub device: Uuid,
    password: Zeroizing<String>,
    hash: Zeroizing<String>,
}
impl std::fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingLogin(<hidden>)")
    }
}
#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub device: Uuid,
}
impl Drop for Credentials {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.access_token.zeroize();
        if let Some(t) = &mut self.refresh_token {
            t.zeroize();
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: UserId,
    pub server_user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub profile: Uuid,
    pub api: String,
    pub identity: String,
    pub master_unlock: MasterPasswordUnlockData,
    pub crypto_state: WrappedAccountCryptographicState,
    pub credentials: Uuid,
}
pub struct LoginSuccess {
    pub transport: Transport,
    pub server_user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub master_unlock: MasterPasswordUnlockData,
    pub crypto_state: WrappedAccountCryptographicState,
    pub credentials: Credentials,
    pub api: Url,
    pub identity: Url,
    pub password: Zeroizing<String>,
}
impl std::fmt::Debug for LoginSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginSuccess(<hidden>)")
    }
}
pub enum Step {
    Challenge(Box<PendingLogin>, Vec<Factor>),
    Authenticated(Box<LoginSuccess>),
}
impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LoginStep(<hidden>)")
    }
}

impl PendingLogin {
    pub async fn prepare(
        attempt: Attempt,
        transport: Transport,
        email: String,
        password: Zeroizing<String>,
        device: Uuid,
    ) -> Result<Self, String> {
        attempt.check()?;
        let email = email.trim().to_lowercase();
        if !email.contains('@') || email.len() > 320 || password.is_empty() {
            return Err("Enter a valid email address and master password".into());
        }
        let mut api = transport.profile.default_api()?;
        let mut identity = transport.profile.default_identity()?;
        // Discovery never adds trust: every advertised service must match an explicitly allowed origin.
        let config_url = api
            .join("config")
            .map_err(|_| "Invalid configuration endpoint")?;
        let request = transport
            .request(Method::GET, config_url)?
            .build()
            .map_err(|_| "Could not build configuration request")?;
        let response = transport.execute(request).await?;
        if response.status().is_success() {
            let config = json_body(response).await?;
            if let Some(env) = get(&config, "environment") {
                for (field, target) in [("api", &mut api), ("identity", &mut identity)] {
                    if let Some(value) = get(env, field)
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        let mut parsed = Url::parse(value)
                            .map_err(|_| "Server advertised an invalid endpoint")?;
                        if parsed.query().is_some() {
                            return Err("Server service endpoints must not contain a query".into());
                        }
                        transport.check(&parsed)?;
                        if !parsed.path().ends_with('/') {
                            parsed.set_path(&format!("{}/", parsed.path()));
                        }
                        *target = parsed;
                    }
                }
            }
        } else if response.status().as_u16() != 404 {
            return Err("Could not load server configuration".into());
        }
        attempt.check()?;
        let endpoint = identity
            .join("accounts/prelogin")
            .map_err(|_| "Invalid prelogin endpoint")?;
        let request = transport
            .request(Method::POST, endpoint)?
            .json(&serde_json::json!({"email":email}))
            .build()
            .map_err(|_| "Could not build prelogin request")?;
        let response = transport.execute(request).await?;
        if !response.status().is_success() {
            return Err("Server prelogin failed".into());
        }
        let kdf = parse_kdf(&json_body(response).await?)?;
        let derivation_password = Zeroizing::new(password.to_string());
        let derivation_email = email.clone();
        let derivation_kdf = kdf.clone();
        let hash = tokio::task::spawn_blocking(move || {
            MasterPasswordAuthenticationData::derive(
                &derivation_password,
                &derivation_kdf,
                &derivation_email,
            )
            .map(|d| Zeroizing::new(d.master_password_authentication_hash.to_string()))
            .map_err(|_| "Unsupported or invalid password derivation settings".to_owned())
        })
        .await
        .map_err(|_| "Password derivation failed")??;
        attempt.check()?;
        Ok(Self {
            attempt,
            transport,
            email,
            kdf,
            api,
            identity,
            device,
            password,
            hash,
        })
    }
    pub async fn submit(self, factor: Option<(Factor, Zeroizing<String>)>) -> Result<Step, String> {
        self.attempt.check()?;
        let mut form = vec![
            ("grant_type", "password".to_owned()),
            ("username", self.email.clone()),
            ("password", self.hash.to_string()),
            ("scope", "api offline_access".to_owned()),
            ("client_id", "desktop".to_owned()),
            ("deviceIdentifier", self.device.to_string()),
            ("deviceName", "Native Desktop".into()),
            ("deviceType", device_type().to_string()),
        ];
        if let Some((provider, code)) = factor {
            form.push(("twoFactorProvider", provider.number().to_string()));
            form.push(("twoFactorToken", code.to_string()));
            form.push(("twoFactorRemember", "0".into()));
        }
        let endpoint = self
            .identity
            .join("connect/token")
            .map_err(|_| "Invalid identity endpoint")?;
        let request = self
            .transport
            .request(Method::POST, endpoint)?
            .form(&form)
            .build()
            .map_err(|_| "Could not build login request")?;
        use zeroize::Zeroize;
        for (_, value) in &mut form {
            value.zeroize();
        }
        let response = self.transport.execute(request).await?;
        let success = response.status().is_success();
        let data = json_body(response).await?;
        self.attempt.check()?;
        if !success {
            if let Some(providers) = get(&data, "twoFactorProviders2").and_then(|v| v.as_object()) {
                let mut supported = Vec::new();
                if providers.contains_key("0") {
                    supported.push(Factor::Authenticator);
                }
                if providers.contains_key("1") {
                    supported.push(Factor::Email);
                }
                if supported.is_empty() {
                    return Err("This account requires a two-factor method not yet supported by this preview".into());
                }
                return Ok(Step::Challenge(Box::new(self), supported));
            }
            if get(&data, "hCaptcha_SiteKey").is_some() {
                return Err(
                    "The server requires CAPTCHA; this preview cannot complete that challenge"
                        .into(),
                );
            }
            return Err("Login failed. Check your credentials or verification code. Device-verification and CAPTCHA challenges are not supported yet".into());
        }
        if get(&data, "forcePasswordReset")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || get(&data, "resetMasterPassword")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            return Err(
                "The server requires a master-password change; complete it in the web vault first"
                    .into(),
            );
        }
        if let Some(policy) = get(&data, "masterPasswordPolicy") {
            crate::services::sdk::policies::validate_login(policy, &self.password, &self.email)
                .await?;
            self.attempt.check()?;
        }
        let access = get(&data, "access_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .ok_or("Server returned no access token")?
            .to_owned();
        let refresh = get(&data, "refresh_token")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let expiry = get(&data, "expires_in")
            .and_then(|v| v.as_u64())
            .filter(|v| *v <= 365 * 86400)
            .ok_or("Server returned an invalid token lifetime")?;
        let master_unlock = if let Some(value) =
            get(&data, "userDecryptionOptions").and_then(|d| get(d, "masterPasswordUnlock"))
        {
            let model: bitwarden_api_api::models::MasterPasswordUnlockResponseModel =
                serde_json::from_value(value.clone())
                    .map_err(|_| "Invalid master-password unlock data")?;
            MasterPasswordUnlockData::try_from(&model)
                .map_err(|_| "Invalid master-password unlock data")?
        } else {
            MasterPasswordUnlockData {
                kdf: self.kdf.clone(),
                salt: self.email.clone(),
                contained_key_id: None,
                master_key_wrapped_user_key: get(&data, "key")
                    .and_then(|v| v.as_str())
                    .ok_or("No password unlock key returned")?
                    .parse()
                    .map_err(|_| "Invalid password unlock key")?,
            }
        };
        bound_kdf(&master_unlock.kdf)?;
        let crypto_state = if let Some(value) = get(&data, "accountKeys") {
            let model: bitwarden_api_api::models::PrivateKeysResponseModel =
                serde_json::from_value(value.clone()).map_err(|_| "Invalid account keys")?;
            WrappedAccountCryptographicState::try_from(&model)
                .map_err(|_| "Invalid account keys")?
        } else {
            WrappedAccountCryptographicState::V1 {
                private_key: get(&data, "privateKey")
                    .and_then(|v| v.as_str())
                    .ok_or("No wrapped account private key returned")?
                    .parse()
                    .map_err(|_| "Invalid wrapped account private key")?,
            }
        };
        let endpoint = self
            .api
            .join("accounts/profile")
            .map_err(|_| "Invalid profile endpoint")?;
        let request = self
            .transport
            .request(Method::GET, endpoint)?
            .bearer_auth(&access)
            .build()
            .map_err(|_| "Could not build account profile request")?;
        let response = self.transport.execute(request).await?;
        if !response.status().is_success() {
            return Err("Could not verify the authenticated account profile".into());
        }
        let profile = json_body(response).await?;
        self.attempt.check()?;
        let server_user_id = get(&profile, "id")
            .and_then(|v| v.as_str())
            .ok_or("Account profile is missing its ID")?
            .parse()
            .map_err(|_| "Invalid server account ID")?;
        let email = get(&profile, "email")
            .and_then(|v| v.as_str())
            .ok_or("Account profile is missing its email")?
            .to_owned();
        if !email.eq_ignore_ascii_case(&self.email) {
            return Err("The authenticated account does not match the requested account".into());
        }
        let display_name = get(&profile, "name")
            .and_then(|v| v.as_str())
            .unwrap_or(&email)
            .to_owned();
        let credentials = Credentials {
            access_token: access,
            refresh_token: refresh,
            expires_at: chrono::Utc::now().timestamp() + expiry as i64,
            device: self.device,
        };
        Ok(Step::Authenticated(Box::new(LoginSuccess {
            transport: self.transport,
            server_user_id,
            email,
            display_name,
            master_unlock,
            crypto_state,
            credentials,
            api: self.api,
            identity: self.identity,
            password: self.password,
        })))
    }
    pub async fn send_email(&self) -> Result<(), String> {
        self.attempt.check()?;
        let url = self
            .api
            .join("two-factor/send-email-login")
            .map_err(|_| "Invalid email verification endpoint")?;
        let request=self.transport.request(Method::POST,url)?.json(&serde_json::json!({"email":self.email,"masterPasswordHash":&*self.hash,"deviceIdentifier":self.device})).build().map_err(|_|"Could not build email verification request")?;
        let response = self.transport.execute(request).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err("Could not request an email verification code".into())
        }
    }
}
fn get<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    v.as_object()?
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}
fn parse_kdf(value: &serde_json::Value) -> Result<Kdf, String> {
    let number = |key: &str| -> Result<NonZeroU32, String> {
        let n = get(value, key)
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .and_then(NonZeroU32::new)
            .ok_or("Invalid password derivation parameters")?;
        Ok(n)
    };
    let kdf = match get(value, "kdf").and_then(|v| v.as_u64()) {
        Some(0) => Kdf::PBKDF2 {
            iterations: number("kdfIterations")?,
        },
        Some(1) => Kdf::Argon2id {
            iterations: number("kdfIterations")?,
            memory: number("kdfMemory")?,
            parallelism: number("kdfParallelism")?,
        },
        _ => return Err("The server requires an unsupported password derivation algorithm".into()),
    };
    bound_kdf(&kdf)?;
    Ok(kdf)
}
fn bound_kdf(kdf: &Kdf) -> Result<(), String> {
    match kdf {
        Kdf::PBKDF2 { iterations } if iterations.get() <= 5_000_000 => Ok(()),
        Kdf::Argon2id {
            iterations,
            memory,
            parallelism,
        } if iterations.get() <= 20 && memory.get() <= 256 && parallelism.get() <= 8 => Ok(()),
        _ => Err(
            "The server's password derivation settings exceed the preview's resource limits".into(),
        ),
    }
}
pub fn device_type() -> u8 {
    if cfg!(target_os = "windows") {
        6
    } else if cfg!(target_os = "macos") {
        7
    } else {
        8
    }
}
