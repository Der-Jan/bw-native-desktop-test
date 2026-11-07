# bitwarden-desktop-next

A vibe-coded project investigating how the [Bitwarden Rust SDK](https://github.com/bitwarden/sdk-internal) can be used to build a native desktop password manager — a slim alternative to the official Electron client, written entirely in Rust on top of [iced 0.15](https://iced.rs/).

The UI is hand-rolled; cryptography and local vault operations use the real `bitwarden-*` SDK crates. Password login, TOTP/email verification, account-scoped mTLS and full read-only vault sync are available for review. Server writes and notifications are deferred. Treat this as an exploration, not a production app.

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

## Running it

```bash
cargo run                                     # Empty profile in the OS application data directory
cargo run -p fake-data -- --output ./demo-data # Generate synthetic accounts in a new/empty directory
cargo run -- --demo ./demo-data                # Explicit demo profile

cargo run --bin packager                      # Package .app/.dmg/.msi/.deb/.pacman
cargo run -p i18n-unused                      # List Fluent keys with no `fl!`/`E()` callers
```

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
read-only vault. If initial sync fails, an authenticated status page offers retry;
a previous committed cache stays usable offline. File → Sync now refreshes the full snapshot. “Verbindung prüfen (ohne Sync)” remains
a profile-only check and can renew expired tokens without fetching vault entries.
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
- Cipher list / detail / edit / delete
- Send list / detail / create (in-memory)
- Password generator
- Magnify quick launcher (`Ctrl+Shift+Space` / `Cmd+Shift+Space`)
- System tray with a monochrome Linux symbolic icon, Wayland hide/restore + single-instance wake-up
- Custom title-bar menus (Windows/Linux) and native `muda` menus (macOS)
- Light / dark theme, persisted settings (`settings.json` in the selected profile)
- Favicons in the vault list
- Packaging to `.app` / `.dmg` / `.msi` / `.deb` / `.pacman`

### Linux notes

- The system tray reaches the OS via the `StatusNotifierItem` (SNI) D-Bus protocol. On distros that don't advertise an SNI host (vanilla GNOME, sway without `waybar`'s tray module, etc.) `services::tray::build()` returns `None`; the window remains reachable through normal minimize/close behavior. Linux uses `ksni` directly, without a GTK event loop. Add the AppIndicator GNOME extension or a tray-aware status bar to get tray support back.
- The `.deb` / `.pacman` produced by `cargo run --bin packager` declares `libayatana-appindicator3-1` (Debian/Ubuntu) / `libayatana-appindicator` (Arch) as a runtime dependency so package-manager installs pull it in automatically. If you build the binary directly without packaging, install that library yourself before launching.

## TODO (highlights)

Full list lives in [docs/todo.md](docs/todo.md). The big ones:

- **Real login command** — current flow only unlocks an existing local DB seeded by `fake-data`
- **2FA / Registration / SSO / master-password-hint** screens
- **PIN unlock** and **biometrics** (Touch ID / Windows Hello / polkit)
- **SSH agent** — opt-in Unix socket agent, active unlocked account, per-signature approvals and verified OpenSSH session binding; see [setup and limits](docs/ssh-agent-requirements.md).
- **Browser integration**, **autotype**, native-messaging host
- **Send**: swap in-memory map for the SDK's `Repository<Send>`, add file sends
- **Right-click context menus** on text inputs
- Many settings tabs are UI-only stubs that emit a "not supported yet" toast
