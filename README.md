# bitwarden-desktop-next

A vibe-coded project investigating how the [Bitwarden Rust SDK](https://github.com/bitwarden/sdk-internal) can be used to build a native desktop password manager — a slim alternative to the official Electron client, written entirely in Rust on top of [egui/eframe 0.37](https://github.com/emilk/egui).

The production UI uses egui; cryptography and local vault operations use the real `bitwarden-*` SDK crates. Password login, TOTP/email verification, account-scoped mTLS and full read-only vault sync are available for review. Server writes and notifications are deferred. Treat this as an exploration, not a production app.

The Bitwarden SDK is pinned to **4.0.0**, commit
[`a13b4c4e`](https://github.com/bitwarden/sdk-internal/commit/a13b4c4e1fa3685280241f417920fd19d7ef24a6)
(upstream `main` checked on 2026-11-07). `Cargo.lock` pins its dependency graph;
builds do not follow a moving branch.

See [docs/architecture.md](docs/architecture.md) for the project layout and [docs/decisions.md](docs/decisions.md) for the rationale behind the major choices.

Proposed hardening and server integration requirements:

- [Security remediation concept](docs/security-concept.md)
- [Login and sync requirements](docs/login-sync-requirements.md)
- [mTLS requirements and iOS references](docs/mtls-requirements.md)
- [Vault sorting parity with iOS](docs/vault-sorting-requirements.md)

The first hardening slice implements revocable SDK sessions, plaintext UI cleanup on lock, guarded task results, private staged exports, and explicit demo profiles. Local demo logout now survives restart and recovers interrupted deletion. The next slice adds single-use protected-action grants, encrypted credentials backed by the OS credential store, password login and scoped mTLS. See the login/mTLS documents for the implemented subset and remaining gates.

## egui frontend

The complete production frontend now uses egui. See [migration status and review
questions](docs/egui-migration.md). The former Iced UI is preserved in commit
`0945673`. Supported backend features and profile formats are retained.

### Independent comparison demo

A separate [egui demo](tools/egui-demo/README.md) compares virtualized lists,
scrolling, combined filters and keyboard navigation using synthetic data:

```bash
cargo run --manifest-path tools/egui-demo/Cargo.toml --release
```

## Running it

```bash
cargo run                                     # Empty profile in the OS application data directory
cargo run -p fake-data -- --output ./demo-data # Generate synthetic accounts in a new/empty directory
cargo run -- --demo ./demo-data                # Explicit demo profile

cargo run --bin packager                      # Package .app/.dmg/.msi/.deb/.pacman
cargo run -p i18n-unused                      # List Fluent keys with no `fl!`/`E()` callers
```

For Ubuntu 24.04 `.deb` builds, use `python3 tools/build-ubuntu-deb.py`; see
[Ubuntu packaging](tools/packaging/README.md) for requirements and installation.

Useful dev modes:

```bash
DEV_BOTH_MENUS=1 cargo run                                            # Native + custom menus side-by-side
RUST_LOG=bitwarden_desktop_next=debug,bitwarden_core=debug cargo run  # Full SDK tracing
```

First-run review: choose Self-hosted, enter your final HTTPS server URL, and optionally
import a PKCS#12 identity under Server / mTLS settings. Import a private server CA
separately if needed. Additional API/Identity origins require explicit permission.
Enter account credentials only in the app. Linux requires a working Secret Service
(e.g. KWallet/gnome-keyring); there is no plaintext fallback. After login/unlock the app downloads a complete encrypted snapshot and opens the
vault. If initial sync fails, the vault displays the error and File → Sync now retries;
a previous committed cache stays usable offline. File → Sync now refreshes the full snapshot. The server-settings connection test checks the selected TLS/mTLS configuration without downloading vault data.
CAPTCHA, SSO, WebAuthn/security-key 2FA and new-device verification are not implemented.
For synthetic vault interaction, use the demo commands above. The generator refuses
existing nonempty directories.

### Seeded accounts

`fake-data` writes three synthetic users to the explicitly selected directory. Their passwords are hardcoded in the generator and written to demo metadata; never reuse them. Normal startup never loads this metadata. Logging out removes the selected demo account and its local database permanently; generate a fresh demo directory to reset the synthetic accounts.

| Email                   | Password   | Notes                                                              |
| ----------------------- | ---------- | ------------------------------------------------------------------ |
| `alice@example.com`     | `password` | Personal vault. Master password + biometrics unlock methods.       |
| `alice@acmecorp.com`    | `123456`   | Work vault with orgs + collections. Master password + PIN unlock.  |
| `loadtest@example.com`  | `loadtest` | ~20k mixed ciphers for layout / scroll / filter perf testing.      |

## Supported

- Multi-user vault unlock against a local SDK-backed SQLite store
- Optional per-account OS credential-store unlock at startup, with explicit unlock after later locks; master passwords are not stored ([security details](docs/security-concept.md#optional-os-credential-store-unlock))
- Merge two or more selected duplicate logins into a chosen primary, preserving differing fields and trashing secondaries only after the primary save is confirmed ([merge rules](docs/vault-sorting-requirements.md#merge-selected-duplicate-logins))
- Cipher list / detail / edit / delete
- Send list / detail / create (in-memory)
- Password generator
- Magnify quick launcher (`Ctrl+Shift+Space` / `Cmd+Shift+Space`)
- System tray with a monochrome Linux symbolic icon, Wayland hide/restore + single-instance wake-up
- Custom title-bar menus (Windows/Linux) and native `muda` menus (macOS)
- Light / dark theme, persisted settings (`settings.json` in the selected profile)
- Favicons for local/demo vaults; remote icon fetching stays disabled until account-scoped transport is wired
- Packaging to `.app` / `.dmg` / `.msi` / `.deb` / `.pacman`

### Linux notes

- The native `cargo run --bin packager` workflow also requires the target Debian/Ubuntu distribution and `dpkg-dev`; use the Ubuntu container recipe above for Ubuntu 24.04 packages. The packaging hook runs `dpkg-shlibdeps` against both release binaries and includes dynamically loaded graphics libraries and supplies versioned library dependencies to cargo-packager. Missing library metadata fails packaging. Inspect the resulting package with `dpkg-deb -f path/to/package.deb Depends`. Cross-distribution builds need a matching build environment; the host package database is not a substitute for the target distribution.
- A reachable D-Bus **user session** and a working Secret Service provider are required for saved credentials and client certificates. Use your desktop's existing KWallet or GNOME Keyring provider. Installing a package alone does not ensure that its service is running or that unlock prompts work; do not start competing providers.
- On **Ubuntu 24.04 with KWallet 5**, install `libqca-qt5-2-plugins` for the encrypted Secret Service session. For a GPG-protected wallet, install a graphical pinentry such as `pinentry-qt`. Restart KWallet or sign out and back in after installing the QCA plugin. These are backend requirements that ELF dependency scanning cannot detect. Qt6/newer distributions need separately verified package names. GNOME Keyring does not require these KWallet packages; wallet management GUIs are optional.
- Keys are stored as versioned ASCII Base64 inside the OS credential store (encoding, not additional encryption). Existing raw 32-byte keys remain readable and are not rewritten on read. Their encrypted local objects still authenticate the key. Already-corrupted values cannot be reconstructed: sign in again for affected account credentials; use Server / mTLS settings to import/replace the affected certificate. Do not delete the whole wallet. Credential errors identify missing keys, access failures, service failures and the known QCA session failure without logging secret values.
- Before releasing a Linux package, run the [credential-store smoke tests](docs/testing.md#real-credential-store-smoke-test) in fresh KWallet 5 and GNOME Keyring sessions, including unlock prompts and installation without Recommends.

- The system tray reaches the OS via the `StatusNotifierItem` (SNI) D-Bus protocol. On distros that don't advertise an SNI host (vanilla GNOME, sway without `waybar`'s tray module, etc.) `services::tray::build()` returns `None`; the window remains reachable through normal minimize/close behavior. Linux uses `ksni` directly, without a GTK event loop. Add the AppIndicator GNOME extension or a tray-aware status bar to get tray support back.

## TODO (highlights)

Full list lives in [docs/todo.md](docs/todo.md). The big ones:

- Registration, SSO, security-key 2FA and master-password-hint screens
- **PIN unlock** and **biometrics** (Touch ID / Windows Hello / polkit)
- **SSH agent** — opt-in Unix socket agent, active unlocked account, per-signature approvals and verified OpenSSH session binding; see [setup and limits](docs/ssh-agent-requirements.md).
- **Browser integration**, **autotype**, native-messaging host
- **Send**: swap in-memory map for the SDK's `Repository<Send>`, add file sends
- Broader OS integration and accessibility review
- Many settings tabs are UI-only stubs that emit a "not supported yet" toast
