# bitwarden-desktop-next

Native Rust desktop client using egui/eframe 0.37 and the pinned Bitwarden SDK4.
Read [architecture](docs/architecture.md), [testing](docs/testing.md), and
[egui migration](docs/egui-migration.md) before architecture changes.

## Commands

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p fake-data -- --output ./demo-data
cargo run -- --demo ./demo-data
cargo run --bin packager
```

Use workspace checks for consistent SDK feature unification. Normal startup
loads only the private OS application profile. Synthetic data requires an
explicit `--demo` directory. Never use a real profile in tests or screenshots.

## Architecture and security

- `core/` owns application state and dispatches asynchronous work. It has no
  GUI types. `ClientManager` is owned by the UI thread; never borrow it across
  await. Use revocable SessionClient handles and `Core::spawn_scoped`.
- `ui/` draws native egui panels, virtualized tables and immediate dialogs.
  Use stable widget IDs and one focus target for the virtualized list.
- `services/` owns SDK, mTLS, encrypted storage, clipboard, SSH, tray, menus,
  preferences, and OS integration. Services must not import `core/` or `ui/`.
- Preserve generation, account, session, request and cipher-revision fences.
  Security decisions belong to the backend, never just disabled UI controls.
- Protected operations need single-use grants scoped to the precise action,
  cipher and revision. Passive repaint/scroll must not cancel password prompts.
- Use `ui::secret` for secret TextEdits; clear undo state for every vault editor.
  On `clear_epoch` reset secret UI state and egui memory/font layout caches.
- Route explicit and native widget copy through ClipboardManager; preserve TTL,
  ownership checks, clearing on lock, and protected-copy authorization.
- Server writes, import, Send publication, PIN/biometric unlock and autostart
  remain unsupported. Do not show a stub as a working feature.
- Server entries support encrypted local edits through `sdk/local_edits.rs`.
  Keep original revisions, permission/reprompt checks and exclusive leases.
  Read-sync must pause while overlays exist; do not bypass this gate before
  implementing server conflict handling. Logout removes unsent local changes.
- mTLS secrets remain in OS credential storage. Keep exact origin binding,
  certificate expiry errors, no redirects or permissive TLS fallbacks.
- Preserve the fixed SSH socket, verified host binding and UUID-bound approvals.
- On Wayland, tray hiding destroys the native surface and retains Core. The
  daemon waits for worker/timer/tray events; no invisible helper window.
- System fonts only in production, original outlined SVG wordmark; immediate
  transitions, no shadows. Keep OpenEXR and TIFF disabled.
- Use existing Fluent strings and stable translation keys where available.
- Do not log secrets. Do not weaken tests or broaden permissions to pass checks.

The former Iced implementation is preserved in commit `0945673`, not in the
production tree. The independent egui comparison is `tools/egui-demo`.
