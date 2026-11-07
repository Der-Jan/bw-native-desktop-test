//! One-shot generator that creates real Bitwarden SDK encrypted data and persists it to
//! one SQLite database per user under `<workspace-root>/data/`, alongside a `mock.json`
//! file holding per-user metadata (email, KDF, encrypted user key, unlock methods).
//!
//! Run with `cargo run -p fake-data -- --output <new-or-empty-directory>`.

mod passkey;

use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use std::collections::HashMap as StdHashMap;

use bitwarden_collections::collection::CollectionId;
use bitwarden_core::{
    ClientBuilder, DeviceType, HostPlatformInfo, OrganizationId, UserId,
    client::persisted_state::BaseUrls,
    init_host_platform_info,
    key_management::{
        LocalUserDataKeyState, MasterPasswordUnlockData, PrivateKeySlotId,
        account_cryptographic_state::WrappedAccountCryptographicState,
        crypto::{InitOrgCryptoRequest, InitUserCryptoMethod, InitUserCryptoRequest},
    },
};
use bitwarden_crypto::{SymmetricCryptoKey, UnsignedSharedKey};
use bitwarden_pm::{PasswordManagerClient, SaveStateData};
use bitwarden_ssh::generator::{KeyAlgorithm, generate_sshkey};
use bitwarden_state::{
    DatabaseConfiguration,
    registry::StateRegistry,
    repository::{Repository, RepositoryError, RepositoryItem},
};
use bitwarden_vault::{
    BankAccountView, CardView, Cipher, CipherRepromptType, CipherType, CipherView,
    DriversLicenseView, Folder, FolderView, IdentityView, LoginUriView, LoginView, PassportView,
    SshKeyView, UriMatchType,
};

use crate::passkey::PasskeySpec;

/// An entry in the output of a `ciphers()` closure: the cipher view plus an
/// optional passkey to register onto it. Logins that have a passkey go
/// through `Fido2Client::register` instead of the normal encrypt path; every
/// other kind carries `None`.
type CipherEntry = (CipherView, Option<PasskeySpec>);
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ── Mock data file schema ──────────────────────────────────────────────────
//
// `mock.json` holds per-user metadata (everything except the encrypted ciphers
// and folders, which now live in per-user SQLite DBs). Mirrored in
// `crates/desktop/src/sdk.rs`. Keep field names + types in sync.

#[derive(Serialize, Deserialize, Debug)]
struct MockVaultMeta {
    users: Vec<MockUserMeta>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MockUserMeta {
    user_id: UserId,
    email: String,
    display_name: String,
    server_url: String,
    /// Hardcoded master password for dev-only unlock. Never ship this in a real client.
    master_password_dev_only: String,
    unlock_methods: UnlockMethodsCfg,
    kdf: bitwarden_crypto::Kdf,
    encrypted_user_key: bitwarden_crypto::EncString,
    private_key: bitwarden_crypto::EncString,
    /// Orgs the user belongs to. Stored here (not in SQLite) because the app
    /// is a UI-only stub with no sync flow that would normally populate these.
    #[serde(default)]
    organizations: Vec<MockOrganization>,
    /// Collections within the user's orgs. Each entry is scoped to one org.
    #[serde(default)]
    collections: Vec<MockCollection>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MockOrganization {
    id: OrganizationId,
    name: String,
    /// Org's symmetric key, wrapped with the user's public key. Replayed on
    /// unlock via `initialize_org_crypto` so the app can encrypt/decrypt
    /// org-owned ciphers. Generated fresh per `cargo run -p fake-data`.
    #[serde(default)]
    wrapped_key: Option<UnsignedSharedKey>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MockCollection {
    id: CollectionId,
    organization_id: OrganizationId,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct UnlockMethodsCfg {
    master_password: bool,
    pin: bool,
    biometrics: bool,
}

// ── User specs ─────────────────────────────────────────────────────────────

struct UserSpec {
    email: &'static str,
    display_name: &'static str,
    server_url: &'static str,
    password: &'static str,
    unlock_methods: UnlockMethodsCfg,
    ciphers: fn() -> Vec<CipherEntry>,
    folders: fn() -> Vec<FolderView>,
    /// Organizations + collections the user belongs to. Produces
    /// `(orgs, collections)` at build time so names can share closures.
    orgs_and_collections: fn() -> (Vec<MockOrganization>, Vec<MockCollection>),
    /// Optionally reassigns ciphers to an org + collections after they're
    /// generated. Passed (ciphers, orgs, collections) and mutates in place.
    /// Lets the work user have some org-owned items without baking the IDs
    /// into every cipher builder.
    assign_ownership: fn(&mut [CipherEntry], &[MockOrganization], &[MockCollection]),
}

// User IDs are generated as fresh UUIDv4s per `build_user` call, matching how
// production IDs come from the server rather than being baked into the binary.
const USER_SPECS: &[UserSpec] = &[
    UserSpec {
        email: "alice@example.com",
        display_name: "Alice Johnson",
        server_url: "bitwarden.com",
        password: "password",
        unlock_methods: UnlockMethodsCfg {
            master_password: true,
            pin: false,
            biometrics: true,
        },
        ciphers: personal_ciphers,
        folders: personal_folders,
        orgs_and_collections: no_orgs,
        assign_ownership: no_ownership,
    },
    UserSpec {
        email: "alice@acmecorp.com",
        display_name: "Alice (Work)",
        server_url: "vault.acmecorp.com",
        password: "123456",
        unlock_methods: UnlockMethodsCfg {
            master_password: true,
            pin: true,
            biometrics: false,
        },
        ciphers: work_ciphers,
        folders: work_folders,
        orgs_and_collections: work_orgs_and_collections,
        assign_ownership: assign_work_ownership,
    },
    // Load-test account: ~20k ciphers mixing logins/cards/notes/identities/ssh
    // keys with deterministic pseudo-random names. Exercises layout, scroll,
    // decrypt-list, and filter perf against a realistic-size vault. Unlock
    // password is "loadtest".
    UserSpec {
        email: "loadtest@example.com",
        display_name: "Load Test",
        server_url: "bitwarden.com",
        password: "loadtest",
        unlock_methods: UnlockMethodsCfg {
            master_password: true,
            pin: false,
            biometrics: false,
        },
        ciphers: load_test_ciphers,
        folders: load_test_folders,
        orgs_and_collections: no_orgs,
        assign_ownership: no_ownership,
    },
];

// ── Main ───────────────────────────────────────────────────────────────────

/// Never infer a destination or delete an existing vault directory.
fn data_dir() -> Result<PathBuf, Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--output")) {
        return Err("Usage: fake-data --output <new-or-empty-directory>".into());
    }
    let path = args
        .next()
        .filter(|p| !p.is_empty())
        .ok_or("Missing output directory")?;
    if args.next().is_some() {
        return Err("Unexpected extra arguments".into());
    }
    Ok(PathBuf::from(path))
}

fn prepare_output(path: &Path) -> Result<(), Box<dyn Error>> {
    if let Ok(meta) = path.symlink_metadata()
        && (!meta.is_dir()
            || meta.file_type().is_symlink()
            || std::fs::read_dir(path)?.next().is_some())
    {
        return Err("Refusing to overwrite a nonempty directory or a symlink; choose a fresh demo destination".into());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Required by `Client::load_from_state`, which reads `get_host_platform_info()`
    // when constructing `ClientSettings`. Must be set once before any SDK client
    // construction; subsequent calls are no-ops.
    init_host_platform_info(HostPlatformInfo {
        user_agent: "Bitwarden Rust-SDK".to_string(),
        device_type: DeviceType::SDK,
        device_identifier: None,
        bitwarden_client_version: None,
        bitwarden_package_type: None,
    });

    let data_dir = data_dir()?;
    prepare_output(&data_dir)?;

    let mut users = Vec::with_capacity(USER_SPECS.len());
    for spec in USER_SPECS {
        users.push(build_user(spec, &data_dir).await?);
    }

    let meta_path = data_dir.join("mock.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&meta_path)?;
    serde_json::to_writer_pretty(file, &MockVaultMeta { users })?;

    #[cfg(unix)]
    for entry in std::fs::read_dir(&data_dir)? {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(entry?.path(), std::fs::Permissions::from_mode(0o600))?;
    }

    println!(
        "Wrote {} users to {}",
        USER_SPECS.len(),
        data_dir.canonicalize()?.display()
    );
    Ok(())
}

async fn build_user(spec: &UserSpec, data_dir: &Path) -> Result<MockUserMeta, Box<dyn Error>> {
    // Fresh random UserId per run — matches how production IDs come from the server.
    // One Client = one user, and this ID is what subsequent SDK calls bind to.
    let sdk_user_id = UserId::new(uuid::Uuid::new_v4());
    let kdf = bitwarden_crypto::Kdf::default_pbkdf2();

    // `make_register_keys` is exposed only as a method on `AuthClient`, but
    // its body doesn't touch `self.client` — it's pure crypto over (email,
    // password, kdf). We use a throwaway memory-only client just to access it,
    // so we have the wrapped private key before building the real registry.
    let scratch = ClientBuilder::new().build();
    let reg = scratch.auth().make_register_keys(
        spec.email.to_string(),
        spec.password.to_string(),
        kdf.clone(),
    )?;

    // Build the disk-backed registry and pre-populate it. `save_to_state` needs
    // a `&StateRegistry`, so it has to run before we hand the registry to
    // `with_state` (which consumes it). The desktop loader reads these three
    // settings back via `load_from_state` to rebuild a locked client.
    let registry = StateRegistry::new_with_db(
        DatabaseConfiguration::Sqlite {
            db_name: sdk_user_id.to_string(),
            folder_path: data_dir.to_path_buf(),
        },
        bitwarden_pm::migrations::get_sdk_managed_migrations(),
    )
    .await?;

    PasswordManagerClient::save_to_state(
        SaveStateData {
            user_id: sdk_user_id,
            email: spec.email.to_string(),
            urls: BaseUrls {
                identity_url: "http://localhost:8080/identity".to_string(),
                api_url: "http://localhost:8080/api".to_string(),
            },
            crypto_state: WrappedAccountCryptographicState::V1 {
                private_key: reg.keys.private.clone(),
            },
        },
        &registry,
    )
    .await?;

    // `LocalUserDataKeyState` isn't in `get_sdk_managed_migrations()`, but
    // `initialize_user_crypto` does write to it. Register an empty in-memory
    // repo on the bare registry so the crypto init path doesn't fail.
    register_empty_repo::<LocalUserDataKeyState>(&registry);

    // Reuse the same entry point the desktop loader uses — the URLs come from
    // `BASE_URLS` (just written) and `user_id` is auto-bound on the client.
    let token_handler =
        Arc::new(bitwarden_auth::token_management::PasswordManagerTokenHandler::default());
    let client = PasswordManagerClient::load_from_state(token_handler, registry).await?;

    client
        .crypto()
        .initialize_user_crypto(InitUserCryptoRequest {
            user_id: Some(sdk_user_id),
            kdf_params: kdf.clone(),
            email: spec.email.to_string(),
            account_cryptographic_state: WrappedAccountCryptographicState::V1 {
                private_key: reg.keys.private.clone(),
            },
            method: InitUserCryptoMethod::MasterPasswordUnlock {
                password: spec.password.to_string(),
                master_password_unlock: MasterPasswordUnlockData {
                    contained_key_id: None, // Existing demo accounts use the legacy V1 user key.
                    kdf: kdf.clone(),
                    master_key_wrapped_user_key: reg.encrypted_user_key.clone(),
                    salt: spec.email.to_string(),
                },
            },
            upgrade_token: None,
        })
        .await?;

    let (mut organizations, collections) = (spec.orgs_and_collections)();

    // For each org: generate a fresh symmetric key, wrap it with the user's
    // public key into an `UnsignedSharedKey`, stash it in the mock meta, and
    // register it in the client's keystore via `initialize_org_crypto` so the
    // subsequent `encrypt(view)` calls can handle org-owned ciphers.
    if !organizations.is_empty() {
        let mut org_init: StdHashMap<OrganizationId, UnsignedSharedKey> = StdHashMap::new();
        let public_key = {
            let ctx = client.0.internal.get_key_store().context();
            ctx.get_public_key(PrivateKeySlotId::UserPrivateKey)?
        };
        for org in organizations.iter_mut() {
            let sym_key =
                SymmetricCryptoKey::make(bitwarden_crypto::SymmetricKeyAlgorithm::Aes256CbcHmac);
            #[allow(deprecated)]
            let wrapped = UnsignedSharedKey::encapsulate_key_unsigned(&sym_key, &public_key)?;
            org.wrapped_key = Some(wrapped.clone());
            org_init.insert(org.id, wrapped);
        }
        client
            .crypto()
            .initialize_org_crypto(InitOrgCryptoRequest {
                organization_keys: org_init,
            })
            .await?;
    }

    let mut cipher_entries = (spec.ciphers)();
    (spec.assign_ownership)(&mut cipher_entries, &organizations, &collections);

    let total = cipher_entries.len();
    if total > 1_000 {
        println!("  encrypting {} ciphers for {}…", total, spec.email);
    }
    let mut ciphers = Vec::with_capacity(total);
    for (i, (view, passkey_spec)) in cipher_entries.into_iter().enumerate() {
        let cipher = match passkey_spec {
            // `register_passkey` drives `Fido2Client::register`, which
            // generates a P-256 keypair, attaches it to the cipher view,
            // and encrypts the full cipher via the user's key store — so
            // we skip the normal `encrypt(view)` path.
            Some(spec) => passkey::register_passkey(&client.0, view, &spec)
                .await
                .map_err(|e| -> Box<dyn Error> { e })?,
            None => client.vault().ciphers().encrypt(view).await?.cipher,
        };
        ciphers.push(cipher);
        if total > 1_000 && (i + 1) % 5_000 == 0 {
            println!("    {} / {}", i + 1, total);
        }
    }

    let folder_views = (spec.folders)();
    let mut folders = Vec::with_capacity(folder_views.len());
    for view in folder_views {
        // `encrypt(view)` is deprecated in favour of `create()`/`edit()`, but
        // those go through the API. We're generating offline fixtures, so we
        // need the raw key-store path here.
        #[expect(deprecated)]
        let folder = client.vault().folders().encrypt(view)?;
        folders.push(folder);
    }

    // Persist ciphers and folders to SQLite via the SDK-managed repo. Encrypt
    // → set_bulk writes into the per-user `{sdk_user_id}.sqlite` file.
    let cipher_repo = client.platform().state().get::<Cipher>()?;
    let cipher_entries: Vec<(bitwarden_vault::CipherId, Cipher)> = ciphers
        .into_iter()
        .map(|c| (c.id.expect("generated ciphers always have an id"), c))
        .collect();
    cipher_repo.set_bulk(cipher_entries).await?;

    let folder_repo = client.platform().state().get::<Folder>()?;
    let folder_entries: Vec<(bitwarden_vault::FolderId, Folder)> = folders
        .into_iter()
        .map(|f| (f.id.expect("generated folders always have an id"), f))
        .collect();
    folder_repo.set_bulk(folder_entries).await?;

    Ok(MockUserMeta {
        user_id: sdk_user_id,
        email: spec.email.to_string(),
        display_name: spec.display_name.to_string(),
        server_url: spec.server_url.to_string(),
        master_password_dev_only: spec.password.to_string(),
        unlock_methods: UnlockMethodsCfg {
            master_password: spec.unlock_methods.master_password,
            pin: spec.unlock_methods.pin,
            biometrics: spec.unlock_methods.biometrics,
        },
        kdf,
        encrypted_user_key: reg.encrypted_user_key,
        private_key: reg.keys.private,
        organizations,
        collections,
    })
}

fn register_empty_repo<T: RepositoryItem + Clone>(registry: &StateRegistry) {
    let repo = Arc::new(MemoryRepo::<T> {
        data: Mutex::new(HashMap::new()),
    });
    registry.register_client_managed(repo);
}

// ── Cipher view builders ───────────────────────────────────────────────────

fn login(name: &str, username: Option<&str>, uri: Option<&str>) -> CipherEntry {
    (build_login(name, username, uri), None)
}

/// Build a login that will also get a real passkey registered on it via
/// `Fido2Client::register` later in the pipeline. `rp_id` should be the
/// registrable domain (matches the `uri` here so autofill lines up).
fn login_with_passkey(
    name: &str,
    username: &str,
    rp_id: &str,
    rp_name: &str,
    user_display_name: &str,
) -> CipherEntry {
    let view = build_login(name, Some(username), Some(rp_id));
    let spec = PasskeySpec {
        rp_id: rp_id.to_string(),
        rp_name: rp_name.to_string(),
        user_name: username.to_string(),
        user_display_name: user_display_name.to_string(),
    };
    (view, Some(spec))
}

fn build_login(name: &str, username: Option<&str>, uri: Option<&str>) -> CipherView {
    cipher_with(
        name,
        None,
        CipherKind::Login(Box::new(LoginView {
            username: username.map(str::to_string),
            password: Some("fake-password-123".to_string()),
            password_revision_date: None,
            uris: uri.map(|u| {
                vec![LoginUriView {
                    uri: Some(u.to_string()),
                    r#match: Some(UriMatchType::Domain),
                    uri_checksum: None,
                }]
            }),
            totp: None,
            autofill_on_page_load: None,
            http_authentication_enabled: None,
            fido2_credentials: None,
        })),
    )
}

/// Build a `CipherView` of the given type, with cipher-level metadata pre-filled.
/// Starts with everything `None` for the type-specific sub-views, then the
/// `match` below flips both the discriminant and the matching slot.
fn cipher_with(name: &str, notes: Option<String>, kind: CipherKind) -> CipherView {
    let now = Utc::now();
    let mut view = CipherView {
        id: Some(bitwarden_vault::CipherId::new(uuid::Uuid::new_v4())),
        organization_id: None,
        folder_id: None,
        collection_ids: vec![],
        key: None,
        partial: false,
        name: name.to_string(),
        notes,
        r#type: CipherType::Login,
        login: None,
        identity: None,
        card: None,
        secure_note: None,
        ssh_key: None,
        bank_account: None,
        drivers_license: None,
        passport: None,
        favorite: false,
        reprompt: CipherRepromptType::None,
        organization_use_totp: false,
        edit: true,
        permissions: None,
        view_password: true,
        local_data: None,
        attachments: None,
        attachment_decryption_failures: None,
        fields: None,
        password_history: None,
        creation_date: now,
        deleted_date: None,
        revision_date: now,
        archived_date: None,
    };
    match kind {
        CipherKind::Login(l) => {
            view.r#type = CipherType::Login;
            view.login = Some(*l);
        }
        CipherKind::Card(c) => {
            view.r#type = CipherType::Card;
            view.card = Some(*c);
        }
        CipherKind::Identity(i) => {
            view.r#type = CipherType::Identity;
            view.identity = Some(*i);
        }
        CipherKind::SecureNote => {
            view.r#type = CipherType::SecureNote;
            view.secure_note = Some(bitwarden_vault::SecureNoteView {
                r#type: bitwarden_vault::SecureNoteType::Generic,
            });
        }
        CipherKind::SshKey(k) => {
            view.r#type = CipherType::SshKey;
            view.ssh_key = Some(*k);
        }
        CipherKind::BankAccount(b) => {
            view.r#type = CipherType::BankAccount;
            view.bank_account = Some(*b);
        }
        CipherKind::DriversLicense(d) => {
            view.r#type = CipherType::DriversLicense;
            view.drivers_license = Some(*d);
        }
        CipherKind::Passport(p) => {
            view.r#type = CipherType::Passport;
            view.passport = Some(*p);
        }
    }
    view
}

enum CipherKind {
    Login(Box<LoginView>),
    Card(Box<CardView>),
    Identity(Box<IdentityView>),
    SecureNote,
    SshKey(Box<SshKeyView>),
    BankAccount(Box<BankAccountView>),
    DriversLicense(Box<DriversLicenseView>),
    Passport(Box<PassportView>),
}

fn note(name: &str) -> CipherEntry {
    (
        cipher_with(
            name,
            Some(format!("Notes for {name}")),
            CipherKind::SecureNote,
        ),
        None,
    )
}

fn card(
    name: &str,
    cardholder: &str,
    brand: &str,
    number: &str,
    exp_month: &str,
    exp_year: &str,
    code: &str,
) -> CipherEntry {
    (
        cipher_with(
            name,
            None,
            CipherKind::Card(Box::new(CardView {
                cardholder_name: Some(cardholder.to_string()),
                exp_month: Some(exp_month.to_string()),
                exp_year: Some(exp_year.to_string()),
                code: Some(code.to_string()),
                brand: Some(brand.to_string()),
                number: Some(number.to_string()),
            })),
        ),
        None,
    )
}

fn identity(
    name: &str,
    title: &str,
    first: &str,
    last: &str,
    email: &str,
    phone: &str,
) -> CipherEntry {
    (
        cipher_with(
            name,
            None,
            CipherKind::Identity(Box::new(IdentityView {
                title: Some(title.to_string()),
                first_name: Some(first.to_string()),
                middle_name: None,
                last_name: Some(last.to_string()),
                address1: Some("742 Evergreen Terrace".to_string()),
                address2: None,
                address3: None,
                city: Some("Springfield".to_string()),
                state: Some("IL".to_string()),
                postal_code: Some("62704".to_string()),
                country: Some("USA".to_string()),
                company: None,
                email: Some(email.to_string()),
                phone: Some(phone.to_string()),
                ssn: None,
                username: None,
                passport_number: None,
                license_number: None,
            })),
        ),
        None,
    )
}

/// Generate a real Ed25519 SSH key pair via the SDK so the desktop app's
/// detail pane shows parseable OpenSSH data (public key + SHA-256
/// fingerprint). Ed25519 is the fastest algorithm in `bitwarden_ssh`
/// (~sub-millisecond), which keeps load-test generation snappy even at
/// several hundred keys. `comment` is appended to the public key so the
/// trailing `user@host` field looks realistic, but isn't otherwise used.
fn ssh_key(name: &str, comment: &str) -> CipherEntry {
    let mut view = generate_sshkey(KeyAlgorithm::Ed25519)
        .expect("Ed25519 key generation is infallible on a working RNG");
    // `ssh-ed25519 <base64>` — append a comment so the public key renders
    // as `ssh-ed25519 <base64> <comment>`, matching real `ssh-keygen -C`
    // output.
    view.public_key = format!("{} {comment}", view.public_key);
    (
        cipher_with(name, None, CipherKind::SshKey(Box::new(view.into()))),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn bank_account(
    name: &str,
    bank_name: &str,
    name_on_account: &str,
    account_type: &str,
    account_number: &str,
    routing_number: &str,
    pin: Option<&str>,
    iban: Option<&str>,
) -> CipherEntry {
    (
        cipher_with(
            name,
            None,
            CipherKind::BankAccount(Box::new(BankAccountView {
                bank_name: Some(bank_name.to_string()),
                name_on_account: Some(name_on_account.to_string()),
                account_type: Some(account_type.to_string()),
                account_number: Some(account_number.to_string()),
                routing_number: Some(routing_number.to_string()),
                branch_number: None,
                pin: pin.map(|p| p.to_string()),
                swift_code: None,
                iban: iban.map(|i| i.to_string()),
                bank_contact_phone: None,
            })),
        ),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn drivers_license(
    name: &str,
    first: &str,
    last: &str,
    license_number: &str,
    issuing_country: &str,
    issuing_state: &str,
    license_class: Option<&str>,
) -> CipherEntry {
    (
        cipher_with(
            name,
            None,
            CipherKind::DriversLicense(Box::new(DriversLicenseView {
                first_name: Some(first.to_string()),
                middle_name: None,
                last_name: Some(last.to_string()),
                date_of_birth: Some("1990-05-12".parse().expect("valid demo date")),
                license_number: Some(license_number.to_string()),
                issuing_country: Some(issuing_country.to_string()),
                issuing_state: Some(issuing_state.to_string()),
                issue_date: Some("2022-08-01".parse().expect("valid demo date")),
                expiration_date: Some("2032-08-01".parse().expect("valid demo date")),
                issuing_authority: None,
                license_class: license_class.map(|c| c.to_string()),
            })),
        ),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn passport(
    name: &str,
    given: &str,
    surname: &str,
    passport_number: &str,
    nationality: &str,
    issuing_country: &str,
    issuing_authority: Option<&str>,
) -> CipherEntry {
    (
        cipher_with(
            name,
            None,
            CipherKind::Passport(Box::new(PassportView {
                surname: Some(surname.to_string()),
                given_name: Some(given.to_string()),
                date_of_birth: Some("1990-05-12".parse().expect("valid demo date")),
                sex: Some("F".to_string()),
                birth_place: Some("Springfield, IL".to_string()),
                nationality: Some(nationality.to_string()),
                issuing_country: Some(issuing_country.to_string()),
                passport_number: Some(passport_number.to_string()),
                passport_type: Some("P".to_string()),
                national_identification_number: None,
                issuing_authority: issuing_authority.map(|a| a.to_string()),
                issue_date: Some("2021-03-15".parse().expect("valid demo date")),
                expiration_date: Some("2031-03-15".parse().expect("valid demo date")),
            })),
        ),
        None,
    )
}

fn folder(name: &str) -> FolderView {
    FolderView {
        id: Some(bitwarden_vault::FolderId::new(uuid::Uuid::new_v4())),
        name: name.to_string(),
        revision_date: Utc::now(),
    }
}

fn personal_ciphers() -> Vec<CipherEntry> {
    vec![
        // Email / comms
        login("Gmail", Some("alice@example.com"), Some("mail.google.com")),
        login(
            "Outlook",
            Some("alice.johnson@outlook.com"),
            Some("outlook.live.com"),
        ),
        login("iCloud Mail", Some("alice@icloud.com"), Some("icloud.com")),
        login("ProtonMail", Some("alice.j"), Some("mail.proton.me")),
        login("Discord", Some("alice#1234"), Some("discord.com")),
        login(
            "Slack (Community)",
            Some("alice_j"),
            Some("rustlang.slack.com"),
        ),
        login("WhatsApp Web", Some("+15550100"), Some("web.whatsapp.com")),
        login("Signal", Some("+15550100"), Some("signal.org")),
        login("Zoom", Some("alice@example.com"), Some("zoom.us")),
        // Social
        login("Reddit", Some("alice_online"), Some("reddit.com")),
        login("Twitter / X", Some("@alice_j"), Some("x.com")),
        login("LinkedIn", Some("alice@example.com"), Some("linkedin.com")),
        login("Instagram", Some("alice.j.photos"), Some("instagram.com")),
        login("TikTok", Some("@alicej"), Some("tiktok.com")),
        login(
            "Mastodon (mastodon.social)",
            Some("@alice"),
            Some("mastodon.social"),
        ),
        login("Bluesky", Some("alice.bsky.social"), Some("bsky.app")),
        login("Facebook", Some("alice.johnson.94"), Some("facebook.com")),
        // Dev
        login_with_passkey(
            "GitHub",
            "alice-dev",
            "github.com",
            "GitHub",
            "Alice Johnson",
        ),
        login("GitLab", Some("alice-dev"), Some("gitlab.com")),
        login(
            "Stack Overflow",
            Some("alice-dev"),
            Some("stackoverflow.com"),
        ),
        login("npm", Some("alice-dev"), Some("npmjs.com")),
        login("crates.io", Some("alice_dev"), Some("crates.io")),
        login("Docker Hub", Some("alicej"), Some("hub.docker.com")),
        // Streaming / entertainment
        login("Netflix", Some("alice@example.com"), Some("netflix.com")),
        login("Spotify", Some("alice@example.com"), Some("spotify.com")),
        login(
            "YouTube Premium",
            Some("alice@example.com"),
            Some("youtube.com"),
        ),
        login("Disney+", Some("alice@example.com"), Some("disneyplus.com")),
        login("HBO Max", Some("alice@example.com"), Some("max.com")),
        login("Twitch", Some("alice_streams"), Some("twitch.tv")),
        login("Steam", Some("alice_gamer"), Some("store.steampowered.com")),
        login("GOG", Some("alice_gamer"), Some("gog.com")),
        // Shopping
        login("Amazon", Some("alice@example.com"), Some("amazon.com")),
        login("eBay", Some("alicej_buys"), Some("ebay.com")),
        login("Etsy", Some("alice_j"), Some("etsy.com")),
        login("PayPal", Some("alice@example.com"), Some("paypal.com")),
        // Finance
        login(
            "Bank of Example",
            Some("alice.johnson"),
            Some("bankofexample.com"),
        ),
        login("Chase", Some("alice_johnson"), Some("chase.com")),
        login("Venmo", Some("@alice-j"), Some("venmo.com")),
        login(
            "Robinhood",
            Some("alice@example.com"),
            Some("robinhood.com"),
        ),
        // Productivity / misc
        login("Dropbox", Some("alice@example.com"), Some("dropbox.com")),
        login("Notion", Some("alice@example.com"), Some("notion.so")),
        login(
            "1Password (legacy)",
            Some("alice@example.com"),
            Some("1password.com"),
        ),
        login("Pinboard", Some("alice_j"), Some("pinboard.in")),
        login("Duolingo", Some("alice_j"), Some("duolingo.com")),
        // Cards
        card(
            "Personal Visa",
            "Alice Johnson",
            "Visa",
            "4111 1111 1111 1111",
            "08",
            "2029",
            "123",
        ),
        card(
            "Travel Mastercard",
            "Alice Johnson",
            "Mastercard",
            "5555 5555 5555 4444",
            "12",
            "2027",
            "456",
        ),
        card(
            "Debit",
            "Alice Johnson",
            "Visa",
            "4242 4242 4242 4242",
            "03",
            "2028",
            "789",
        ),
        // Identity
        identity(
            "Alice Johnson",
            "Ms",
            "Alice",
            "Johnson",
            "alice@example.com",
            "+1 555 0100",
        ),
        // Secure notes
        note("Recovery Codes Backup"),
        note("WiFi Passwords"),
        note("Passport info"),
        note("Emergency contacts"),
        // SSH
        ssh_key("GitHub SSH Key", "alice@desktop"),
        ssh_key("Home Server", "alice@homelab"),
        // Bank accounts
        bank_account(
            "Primary Checking",
            "My Bank",
            "Alice Johnson",
            "Checking",
            "000123456789",
            "021000021",
            Some("1234"),
            None,
        ),
        bank_account(
            "Travel Savings",
            "Example Credit Union",
            "Alice Johnson",
            "Savings",
            "987654321",
            "021000089",
            None,
            Some("GB29NWBK60161331926819"),
        ),
        // Drivers license + passport
        drivers_license(
            "Illinois Driver's License",
            "Alice",
            "Johnson",
            "J123-4567-8901",
            "USA",
            "IL",
            Some("D"),
        ),
        passport(
            "US Passport",
            "Alice",
            "Johnson",
            "X12345678",
            "USA",
            "USA",
            Some("U.S. Department of State"),
        ),
    ]
}

fn personal_folders() -> Vec<FolderView> {
    vec![folder("Personal"), folder("Finance")]
}

fn work_ciphers() -> Vec<CipherEntry> {
    vec![
        // Core work tools
        login(
            "Company Jira",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.atlassian.net"),
        ),
        login(
            "Confluence",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.atlassian.net"),
        ),
        login("Company GitHub", Some("alice-acme"), Some("github.com")),
        login(
            "Company GitLab",
            Some("alice-acme"),
            Some("gitlab.acmecorp.com"),
        ),
        login("Bitbucket", Some("alice-acme"), Some("bitbucket.org")),
        login("Slack", Some("ajohnson"), Some("acmecorp.slack.com")),
        login(
            "Microsoft Teams",
            Some("ajohnson@acmecorp.com"),
            Some("teams.microsoft.com"),
        ),
        login(
            "Zoom (Work)",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.zoom.us"),
        ),
        login_with_passkey(
            "Google Workspace",
            "ajohnson@acmecorp.com",
            "google.com",
            "Google",
            "Alice (Work)",
        ),
        login(
            "Office 365",
            Some("ajohnson@acmecorp.com"),
            Some("office.com"),
        ),
        login(
            "Dropbox Business",
            Some("ajohnson@acmecorp.com"),
            Some("business.dropbox.com"),
        ),
        // Cloud providers
        login(
            "AWS Console",
            Some("ajohnson@acmecorp.com"),
            Some("aws.amazon.com"),
        ),
        login(
            "AWS (dev account)",
            Some("alice-dev"),
            Some("aws.amazon.com"),
        ),
        login(
            "GCP Console",
            Some("ajohnson@acmecorp.com"),
            Some("console.cloud.google.com"),
        ),
        login(
            "Azure Portal",
            Some("ajohnson@acmecorp.com"),
            Some("portal.azure.com"),
        ),
        login(
            "Cloudflare",
            Some("ajohnson@acmecorp.com"),
            Some("dash.cloudflare.com"),
        ),
        login("Vercel", Some("alice-acme"), Some("vercel.com")),
        login("Netlify", Some("alice-acme"), Some("app.netlify.com")),
        login(
            "Heroku",
            Some("ajohnson@acmecorp.com"),
            Some("dashboard.heroku.com"),
        ),
        login("Fastly", Some("alice-acme"), Some("manage.fastly.com")),
        // Ops / observability
        login(
            "Datadog",
            Some("ajohnson@acmecorp.com"),
            Some("app.datadoghq.com"),
        ),
        login(
            "PagerDuty",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.pagerduty.com"),
        ),
        login(
            "Grafana Cloud",
            Some("ajohnson@acmecorp.com"),
            Some("grafana.net"),
        ),
        login("Sentry", Some("alice-acme"), Some("sentry.io")),
        login("Linear", Some("ajohnson@acmecorp.com"), Some("linear.app")),
        login(
            "Notion (Team)",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.notion.site"),
        ),
        // HR / admin
        login("Workday", Some("ajohnson"), Some("acmecorp.workday.com")),
        login(
            "Expensify",
            Some("ajohnson@acmecorp.com"),
            Some("expensify.com"),
        ),
        login(
            "Greenhouse",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.greenhouse.io"),
        ),
        login(
            "DocuSign",
            Some("ajohnson@acmecorp.com"),
            Some("docusign.net"),
        ),
        login(
            "Gusto (Payroll)",
            Some("ajohnson@acmecorp.com"),
            Some("gusto.com"),
        ),
        // SaaS customers care about
        login(
            "Stripe",
            Some("ajohnson@acmecorp.com"),
            Some("dashboard.stripe.com"),
        ),
        login("Segment", Some("alice-acme"), Some("app.segment.com")),
        login(
            "Mixpanel",
            Some("ajohnson@acmecorp.com"),
            Some("mixpanel.com"),
        ),
        login(
            "Intercom",
            Some("ajohnson@acmecorp.com"),
            Some("app.intercom.com"),
        ),
        login(
            "Zendesk",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.zendesk.com"),
        ),
        login(
            "Salesforce",
            Some("ajohnson@acmecorp.com"),
            Some("acmecorp.my.salesforce.com"),
        ),
        login(
            "HubSpot",
            Some("ajohnson@acmecorp.com"),
            Some("app.hubspot.com"),
        ),
        login("Figma", Some("ajohnson@acmecorp.com"), Some("figma.com")),
        // Domain / DNS / infra
        login("Namecheap", Some("alice-acme"), Some("namecheap.com")),
        login(
            "Terraform Cloud",
            Some("alice-acme"),
            Some("app.terraform.io"),
        ),
        // Cards
        card(
            "Corporate Card",
            "Acme Corp",
            "Amex",
            "3782 822463 10005",
            "06",
            "2028",
            "7890",
        ),
        card(
            "Travel Expenses Card",
            "Alice Johnson",
            "Visa",
            "4000 1234 5678 9010",
            "04",
            "2027",
            "321",
        ),
        // Notes
        note("Production DB Credentials"),
        note("On-call Runbook"),
        note("Release Process Checklist"),
        note("VPN Config"),
        // SSH
        ssh_key("Deploy SSH Key", "deploy@acmecorp"),
        ssh_key("Staging Bastion", "alice@staging"),
        // Bank accounts
        bank_account(
            "Operating Account",
            "Acme Business Bank",
            "Acme Corp",
            "Checking",
            "111222333444",
            "026009593",
            None,
            None,
        ),
    ]
}

fn work_folders() -> Vec<FolderView> {
    vec![folder("Work"), folder("Infrastructure")]
}

// ── Org / collection generators ────────────────────────────────────────────

fn no_orgs() -> (Vec<MockOrganization>, Vec<MockCollection>) {
    (vec![], vec![])
}

fn no_ownership(_: &mut [CipherEntry], _: &[MockOrganization], _: &[MockCollection]) {}

fn work_orgs_and_collections() -> (Vec<MockOrganization>, Vec<MockCollection>) {
    let org = MockOrganization {
        id: OrganizationId::new(uuid::Uuid::new_v4()),
        name: "Acme Corp".to_string(),
        wrapped_key: None, // filled in by `build_user` after user crypto init
    };
    let collections = vec![
        MockCollection {
            id: CollectionId::new(uuid::Uuid::new_v4()),
            organization_id: org.id,
            name: "Engineering".to_string(),
        },
        MockCollection {
            id: CollectionId::new(uuid::Uuid::new_v4()),
            organization_id: org.id,
            name: "Marketing".to_string(),
        },
        MockCollection {
            id: CollectionId::new(uuid::Uuid::new_v4()),
            organization_id: org.id,
            name: "HR".to_string(),
        },
    ];
    (vec![org], collections)
}

/// Assign the first two work ciphers to the Engineering collection so the
/// edit form has a realistic preselected org + collection to render. Relies
/// on `build_user` having called `initialize_org_crypto` first so the org
/// key is in the keystore by the time we encrypt these views.
fn assign_work_ownership(
    entries: &mut [CipherEntry],
    orgs: &[MockOrganization],
    collections: &[MockCollection],
) {
    let Some(org) = orgs.first() else { return };
    let Some(eng) = collections.iter().find(|c| c.name == "Engineering") else {
        return;
    };
    for (cipher, _) in entries.iter_mut().take(2) {
        cipher.organization_id = Some(org.id);
        cipher.collection_ids = vec![eng.id];
    }
}

// ── Load-test generators ───────────────────────────────────────────────────
//
// Produces ~20k ciphers with deterministic pseudo-random names so the JSON
// output is stable across regenerations. The mix (≈80/10/5/3/2) approximates
// what a real heavy user would have in their vault.

/// Total cipher count for the load-test account. Tune here if 20k turns out
/// to be too large (or too small).
const LOAD_TEST_CIPHER_COUNT: usize = 20_000;

/// Fixed seed for the load-test PRNG. Changing this reshuffles all generated
/// names/usernames, so only touch it if you deliberately want to invalidate
/// the checked-in mock vault.
const LOAD_TEST_SEED: u64 = 0xC0FFEE_DEADBEEF;

/// Minimal xorshift64 PRNG. Deterministic, no dependencies, good enough for
/// picking words out of a bank — we are not generating crypto randomness here.
struct Prng {
    state: u64,
}

impl Prng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        let idx = (self.next_u64() as usize) % slice.len();
        &slice[idx]
    }
    fn range(&mut self, max: usize) -> usize {
        (self.next_u64() as usize) % max
    }
}

const LOAD_TEST_COMPANIES: &[&str] = &[
    "Acme",
    "Globex",
    "Initech",
    "Umbrella",
    "Wayne",
    "Stark",
    "Wonka",
    "Tyrell",
    "Cyberdyne",
    "Hooli",
    "Pied Piper",
    "Soylent",
    "Dunder Mifflin",
    "Vandelay",
    "Massive Dynamic",
    "Oscorp",
    "LexCorp",
    "Aperture",
    "Black Mesa",
    "Rekall",
    "Weyland",
    "Nakatomi",
    "InGen",
    "Parabellum",
    "Genco",
    "Virtucon",
    "Buy n Large",
    "Omni Consumer",
    "Prestige Worldwide",
    "Scruffy",
];

const LOAD_TEST_KINDS: &[&str] = &[
    "Portal",
    "Dashboard",
    "Admin",
    "VPN",
    "Support",
    "Intranet",
    "Cloud",
    "Staging",
    "Prod",
    "CI",
    "Git",
    "Wiki",
    "Billing",
    "Analytics",
    "Monitoring",
    "Metrics",
    "Reports",
    "Ops",
    "Mail",
    "Chat",
    "Forum",
    "Bug Tracker",
    "CDN",
    "DNS",
    "Registrar",
    "Backup",
    "Archive",
    "API",
    "Console",
    "Hub",
];

const LOAD_TEST_USERNAMES: &[&str] = &[
    "alice.johnson",
    "ajohnson",
    "alice_j",
    "a.johnson",
    "alice@loadtest.dev",
    "loadtest-admin",
    "service-account",
    "alice.j@example.com",
    "alice2024",
];

const LOAD_TEST_TLDS: &[&str] = &[
    "com", "io", "dev", "net", "org", "co", "app", "cloud", "tech", "systems",
];

const LOAD_TEST_CARD_BRANDS: &[&str] = &["Visa", "Mastercard", "Amex", "Discover"];

const LOAD_TEST_FIRST_NAMES: &[&str] = &[
    "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Heidi", "Ivan", "Judy", "Mallory",
    "Oscar", "Peggy", "Trent", "Victor", "Walter",
];

const LOAD_TEST_LAST_NAMES: &[&str] = &[
    "Smith", "Jones", "Brown", "Taylor", "Wilson", "Davies", "Evans", "Thomas", "Roberts",
    "Walker", "Wright", "Robinson",
];

fn load_test_ciphers() -> Vec<CipherEntry> {
    let mut prng = Prng::new(LOAD_TEST_SEED);
    let mut out: Vec<CipherEntry> = Vec::with_capacity(LOAD_TEST_CIPHER_COUNT);

    // Type distribution: 80% logins, 10% notes, 5% cards, 3% identities, 2% ssh keys.
    // We derive each cipher's type from its index rather than rolling PRNG for
    // the distribution, so tuning LOAD_TEST_CIPHER_COUNT keeps the ratios stable.
    for i in 0..LOAD_TEST_CIPHER_COUNT {
        let bucket = i % 100;
        let cipher = if bucket < 80 {
            // Login
            let company = prng.pick(LOAD_TEST_COMPANIES);
            let kind = prng.pick(LOAD_TEST_KINDS);
            let tld = prng.pick(LOAD_TEST_TLDS);
            let username = prng.pick(LOAD_TEST_USERNAMES);
            let name = format!("{company} {kind} #{i}");
            let slug = company.to_ascii_lowercase().replace(' ', "-");
            let uri = format!("{slug}.{tld}");
            login(&name, Some(username), Some(&uri))
        } else if bucket < 90 {
            // Secure note
            let company = prng.pick(LOAD_TEST_COMPANIES);
            note(&format!("{company} runbook #{i}"))
        } else if bucket < 95 {
            // Card
            let brand = prng.pick(LOAD_TEST_CARD_BRANDS);
            let first = prng.pick(LOAD_TEST_FIRST_NAMES);
            let last = prng.pick(LOAD_TEST_LAST_NAMES);
            let cardholder = format!("{first} {last}");
            let number = format!(
                "{:04} {:04} {:04} {:04}",
                prng.range(10000),
                prng.range(10000),
                prng.range(10000),
                prng.range(10000)
            );
            let exp_month = format!("{:02}", 1 + prng.range(12));
            let exp_year = format!("{}", 2026 + prng.range(6));
            let code = format!("{:03}", prng.range(1000));
            card(
                &format!("{brand} #{i}"),
                &cardholder,
                brand,
                &number,
                &exp_month,
                &exp_year,
                &code,
            )
        } else if bucket < 98 {
            // Identity
            let first = prng.pick(LOAD_TEST_FIRST_NAMES);
            let last = prng.pick(LOAD_TEST_LAST_NAMES);
            let first_lower = first.to_ascii_lowercase();
            let last_lower = last.to_ascii_lowercase();
            let name = format!("{first} {last} #{i}");
            let email = format!("{first_lower}.{last_lower}@example.com");
            let phone = format!("+1 555 {:04}", prng.range(10000));
            identity(&name, "Ms", first, last, &email, &phone)
        } else {
            // SSH key — real Ed25519 pair via the SDK. Ed25519 is fast
            // enough that generating a few hundred in a batch is fine.
            let company = prng.pick(LOAD_TEST_COMPANIES);
            let slug = company.to_ascii_lowercase().replace(' ', "-");
            let name = format!("{company} Deploy Key #{i}");
            let comment = format!("deploy@{slug}");
            ssh_key(&name, &comment)
        };
        out.push(cipher);
    }

    out
}

fn load_test_folders() -> Vec<FolderView> {
    vec![
        folder("Personal"),
        folder("Work"),
        folder("Infrastructure"),
        folder("Archived"),
        folder("One-Time"),
    ]
}

// ── In-memory repository ───────────────────────────────────────────────────

struct MemoryRepo<T: RepositoryItem + Clone> {
    data: Mutex<HashMap<String, T>>,
}

#[async_trait::async_trait]
impl<T: RepositoryItem + Clone> Repository<T> for MemoryRepo<T> {
    async fn get(&self, key: T::Key) -> Result<Option<T>, RepositoryError> {
        Ok(self.data.lock().unwrap().get(&key.to_string()).cloned())
    }
    async fn list(&self) -> Result<Vec<T>, RepositoryError> {
        Ok(self.data.lock().unwrap().values().cloned().collect())
    }
    async fn set(&self, key: T::Key, value: T) -> Result<(), RepositoryError> {
        self.data.lock().unwrap().insert(key.to_string(), value);
        Ok(())
    }
    async fn set_bulk(&self, values: Vec<(T::Key, T)>) -> Result<(), RepositoryError> {
        let mut map = self.data.lock().unwrap();
        for (k, v) in values {
            map.insert(k.to_string(), v);
        }
        Ok(())
    }
    async fn remove(&self, key: T::Key) -> Result<(), RepositoryError> {
        self.data.lock().unwrap().remove(&key.to_string());
        Ok(())
    }
    async fn remove_bulk(&self, keys: Vec<T::Key>) -> Result<(), RepositoryError> {
        let mut map = self.data.lock().unwrap();
        for k in keys {
            map.remove(&k.to_string());
        }
        Ok(())
    }
    async fn remove_all(&self) -> Result<(), RepositoryError> {
        self.data.lock().unwrap().clear();
        Ok(())
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn generation_refuses_existing_data_without_deleting_it() {
        let root = std::env::temp_dir().join(format!("bw-seed-test-{}", uuid::Uuid::new_v4()));
        prepare_output(&root).unwrap();
        let sentinel = root.join("existing-vault");
        std::fs::write(&sentinel, b"keep me").unwrap();
        assert!(prepare_output(&root).is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep me");
        std::fs::remove_dir_all(root).unwrap();
    }
}
