# Native desktop security remediation concept

Status: implementation contract, with runtime hardening and durable local logout implemented
2026-11-07 (details below). Audit baseline: native commit `2c26788`, compared with
the adjacent Electron checkout at `1a11ec6383`.

The immediate objective is to make lock, logout, and protected item access real
security boundaries before connecting this prototype to real accounts. Follow
with [login and sync](login-sync-requirements.md), [mTLS](mtls-requirements.md),
and [sorting parity with iOS](vault-sorting-requirements.md).

## Implemented boundaries and remaining limits

- Revocable session handles fence SDK operations before/after work. Lock/logout
  revoke immediately; per-account leases drain in-flight work before clearing keys.
  Unlock errors, revocation and cancellation clear partially initialized crypto.
- Task results and rendered sensitive UI actions carry account incarnation, unlock
  generation and a presentation generation. Switching away and back cannot revive
  an old completion. Locked views reject secret-bearing actions.
- Lock drops account lists, forms, generator history, Magnify contents and sensitive
  overlays immediately. Clipboard cleanup checks ownership even with no timer.
- Exports use a same-directory 0600 staging file on Unix and a zeroizing final
  buffer; authority is checked before publication. Already-started filesystem/SDK
  side effects can finish while lock drains. This is not rollback or crash-proof
  erasure of exported plaintext; a crash can leave a private staging file.
- Normal startup uses private OS application storage (0700 on Unix), loads no
  mock accounts, and accepts an empty profile. Demo loading requires `--demo
  <directory>`; the generator requires `--output` and refuses nonempty destinations.
- Local logout now commits a retained removal marker, drains SDK work, closes
  the SDK owner, removes demo metadata (including development passwords), and
  deletes the account SQLite database plus WAL/SHM/journal files. Startup retries
  interrupted cleanup before opening account databases and excludes marked IDs.
  Normal shutdown waits for pending logout; persistent UI messages distinguish
  unsaved logout intent from committed logout with incomplete cleanup.

Protected actions now use one-use grants bound to account session generation,
item revision and action, expiring after 60 seconds. Read, copy, edit, write, delete
and Magnify paths enforce reprompt/permissions. Export has a fresh authorization
checked after the native picker and again before final publication; cancellation
and lock invalidate pending validation. Full details fail closed if `view_password`
is denied (selective nonsecret field display remains a later parity task).

Real password/TOTP/email login, installation-local account IDs, protected credential
storage/deletion and account-scoped HTTP mTLS are now implemented for pre-sync
review. See [login requirements](login-sync-requirements.md) and
[mTLS requirements](mtls-requirements.md) for precise coverage and limitations.
Full read sync is implemented with atomic encrypted SDK-SQLite snapshots and
read-only repository adapters. Successful saves automatically upload existing-entry
edits with revision checks, durable encrypted intent and per-item acknowledgments; startup
refresh remains download-only. Conflicts retain separate drafts and require
explicit review/reapplication. Creation/deletion and notifications remain disabled.
Windows ACL/platform validation, remaining Vaultwarden login challenges and
organization enrollment/recovery workflows remain open. Vaultwarden organization
policy enforcement now covers generator requirements, restricted item types,
personal ownership, PIN prohibition and URI defaults, with login-time master-password
checks. The encrypted snapshot carries the last synced policy state offline;
server-managed and unavailable features are identified in Settings. Profile-scoped website
icons are implemented. Targeted session revocation has no supported endpoint in
the audited Vaultwarden versions; global deauthorization is not normal logout.
See the Vaultwarden compatibility audit in [login requirements](login-sync-requirements.md). Dropping UI/SDK strings does not guarantee erasure of
all allocator or widget copies. This remains a prototype under review.

## Verification

Current verification (organization-policy implementation): `cargo test --workspace`
passes 213 desktop tests plus one generator test (three desktop tests ignored).
`cargo clippy --workspace --all-targets` passes with existing warnings; the release
workspace builds. The native Wayland settings-fit test also passes explicitly.
Write-sync regressions cover revision conflicts, lost responses, partial batches,
revocation during dispatch, encrypted offline recovery, fresh policies/permissions,
strict field decryption and one-use conflict review. Policy regressions use real
SDK-encrypted disposable accounts over mandatory mTLS
and cover offline restart, backend generator enforcement, list/read/copy/SSH
restrictions, personal ownership, policy removal and login rejection. Added regression checks cover one-use grants,
stored-policy enforcement when a caller removes reprompt, cancelled authorization,
wrong password/expired PKCS#12 rejection, mandatory real TLS client authentication,
SDK Identity transport injection, TOTP/email login, credential rotation under
concurrent probes, locked account reload/unlock, token-key removal, redirects,
exact port scope, independent identities at the same host, explicit server CA
trust and typed TLS expiry diagnostics for both app and SDK requests. All accounts and certificates in these checks are disposable test fixtures.
The GUI review uses an isolated profile; no user server or credentials were used.
The synthetic conflict GUI check exercises retained draft review and reapplication.
Automatic-save integration tests prove that successful saves trigger uploads,
failures do not retry indefinitely, switching accounts cannot misroute a save,
search/selection survive refresh and protected details remain gated.
Read-sync regressions cover encrypted full-snapshot fetch over mandatory mTLS,
coalescing, replacement/deletions, failed HTTP/incomplete payload preservation,
lock during fetch, organization-key rollback/removal, namespace isolation and
locked reload/offline unlock. SQLite/WAL scans reject fixture vault plaintext,
master passwords and OAuth tokens. The offline GUI check opens the stored item
with read-only controls while an intentionally unavailable credential store makes
auto-refresh fail; the committed cache stays visible.

Earlier lifecycle/SDK baseline checks:

- `cargo test -p bitwarden-desktop-next -p fake-data`: 219 app tests and one
  generator test passed. Regression coverage includes real SDK key cleanup,
  cancellation before polling/while waiting, stale results across lock/account
  replacement, UI purge, clipboard ownership and private export publication.
  Durable-removal tests cover recovery after intent commit, failed deletion and
  retry, restored old data, concurrent accounts, private markers, invalid journal
  links, unpolled handles/unlocks, persistence failure and deferred shutdown.
- `cargo clippy -p bitwarden-desktop-next -p fake-data --all-targets` completed
  with the pre-existing desktop warnings; no new warnings remain.
- Release build completed. An isolated X11 preview successfully unlocked a demo
  account, locked with Ctrl+L, and unlocked again, loading 50 ciphers both times.
  Normal and explicit demo startup were checked separately. The durable-logout
  preview verified 3 → 2 accounts after logout and restart, interrupted-removal
  recovery to one remaining account, removal of the last account with an empty
  profile after restart, and blocked loading for an invalid journal. The final
  runtime also unlocked the real SDK demo vault and loaded its 50 ciphers before logout.
- SDK 4.0.0 at `a13b4c4e1fa3685280241f417920fd19d7ef24a6` passed the full
  workspace tests, Clippy and release build. An additional offline test covers
  real password verification (correct/incorrect), folder/cipher writes, export,
  lock/re-unlock and soft deletion. Separate GUI runs used copied SDK-3 databases
  and freshly generated SDK-4 databases; both loaded all 50 work-vault ciphers
  and passed durable logout/restart recovery, including removing the last account.
  SDK memory-hardening remains enabled; dangerous crypto debug is not enabled.

## Scope and threat model

Protect against unintended secret persistence, stale asynchronous work, account
mix-ups, accidental network disclosure, hostile server responses, and another OS
user reading application files. A compromised OS, root, or malware running as the
same user can inspect an unlocked process; this design does not promise to defeat
that attacker. Memory cleanup reduces exposure and does not guarantee erasure of
every copy in GUI widgets, allocators, swap, crash dumps, or system clipboards.

Vault encryption, password derivation, key wrapping, and protocol authentication
remain SDK responsibilities. App lifecycle, storage policy, transport routing,
and access authorization are application responsibilities.

## Audit gaps and required outcomes

The table records the original audit baseline, not a list of current unresolved
issues. See the implemented boundaries above and [egui migration](egui-migration.md)
for current validation.

| Gap and source | Required outcome |
| --- | --- |
| [Seed tool](../tools/fake-data/src/main.rs) writes development passwords to `mock.json`; [loader](../crates/desktop/src/services/sdk/loader.rs) requires mock metadata | Production startup accepts an empty profile and never reads development secrets. Test data has a separate explicit mode and directory. |
| [Lock](https://github.com/Der-Jan/bw-native-desktop-test/blob/2c26788/crates/desktop/src/app/helpers.rs) clears SDK keys but preserves decrypted view state | Lock removes all account plaintext, invalidates outstanding work, and prevents secret-bearing actions. |
| [Logout](../crates/desktop/src/services/sdk/mod.rs) removes only runtime membership | Logout remains effective after restart, removes local account credentials and cache, and cannot be reversed by late tasks. |
| [Reprompt checkbox](https://github.com/Der-Jan/bw-native-desktop-test/blob/2c26788/crates/desktop/src/views/vault/widgets/cipher_edit/sections/shared.rs) has no corresponding checks in access handlers | Protected field reveal/copy, edit, and other protected operations require fresh authorization. |
| [Favicon setup](https://github.com/Der-Jan/bw-native-desktop-test/blob/2c26788/crates/desktop/src/app/lifecycle.rs) hardcodes the US icon service | Resolve each account's configured icon service, honor disablement, and never fall back from a private server to public infrastructure silently. |
| [Paths](../crates/desktop/src/paths.rs) depend on cwd; exports use ordinary file writes | Use stable private OS directories and explicit private file creation. |

## Account runtime and operation authority

Introduce an account runtime coordinator under `services/sdk/`; App owns its
handle and routes events. Views request operations through the coordinator rather
than retaining freely cloned SDK clients. The coordinator owns SDK access, task
registration, cancellation, lifecycle transitions, and commit authorization.
The first slice implements revocable `SessionClient` handles and leases; its
current contract is described in `architecture.md`. Network cancellation and
transactional storage commits must extend this boundary during server integration. Keep view rendering and modal presentation
in the existing App/view layers.

Use an installation-local account ID plus the server profile ID and authenticated
server user ID. Email and server UUID alone are insufficient account namespaces:
two servers may return the same user ID.

Track separate generations:

| Generation | Changes when | Invalidates |
| --- | --- | --- |
| Account incarnation | Login replacement, logout, account removal | Every operation belonging to the previous account session |
| Unlock generation | Lock, unlock attempt replacement, successful unlock | Decrypt results, secret-bearing UI messages, reprompt grants, exports |
| Transport generation | Certificate, endpoint, or server trust change | HTTP pools, TLS sessions, WebSockets, transport-bound SDK clients |

Each operation captures the relevant account identity and generations. Check them
before executing, before committing SDK/storage side effects, and when accepting
a UI result. A user-ID-only completion check is insufficient. Switching the
selected account changes the presentation target; it need not lock other accounts.

Cancellation alone is insufficient: a future may already be mutating shared SDK
state. Serialize crypto lifecycle changes through the coordinator, revoke operation
authority immediately on lock, and either drain an outstanding crypto operation
before clearing keys or dispose of its isolated candidate client. Never publish an
unlocked candidate after its generation expires. A network request already sent
may still reach the server; prevent its response from reviving local state.

## Linux wallet and biometric unlock concept (deferred)

Recorded 2026-11-08. Concept only: do not enable biometric unlock or change system
PAM configuration until the key-release design and hardware can be tested.

### Shared wallet interface

Use the freedesktop Secret Service D-Bus API (`org.freedesktop.secrets`) through
our existing Rust `keyring` adapter, rather than separate KDE/GNOME integrations.
GNOME Keyring and KDE's KWallet/Secret Service infrastructure are providers; the
exact installed provider depends on desktop and distribution versions. There is
a common protocol, not a single mandatory Linux wallet implementation.

The current `accounts/storage.rs` already uses this interface on Linux: a random
wrapping key is stored in the OS credential store and protects local AEAD credential
envelopes, including persisted login and mTLS secrets. That does not currently
provide vault unlock. The master password must never be saved for this feature.

The protocol supports locked collections/items and provider prompts, but does not
guarantee a fresh password/biometric check for every read, hardware-backed storage,
or uniform application access control. A desktop wallet may already be unlocked
by login. An application label/UUID is a lookup namespace, not proof that another
process running as the same user cannot retrieve the secret. Collection lock state
also differs from our per-account vault lock state. Do not lock the shared desktop
wallet as a side effect of locking this app.

### Two independent future options

1. **Unlock using the desktop wallet:** a simpler, explicitly opt-in convenience
   mode, testable without biometric hardware. After master-password confirmation,
   create an account/installation-bound wrapping secret for SDK-supported local
   unlock material. Retrieve it from Secret Service on an explicit unlock action;
   preserve all existing SDK session, generation and stale-result checks. Explain
   that trust extends to the desktop session and wallet access policy; an already
   unlocked wallet may release the secret without another prompt. Name this mode
   “Desktop wallet unlock”, never “Biometric unlock”. This is a proposed option,
   not an approved security boundary or an implemented feature.
2. **Fresh biometric authentication:** remain deferred. Fingerprints can use
   fprintd/PAM; IR face recognition can potentially use Howdy/PAM if the camera,
   IR stream and emitter work under Linux. A successful PAM/fprintd/polkit check
   alone supplies no vault decryption secret. A stronger design needs a trusted
   key-release component enforcing fresh authentication before releasing the
   wrapping secret; an app-side check followed by an unrestricted wallet read
   does not establish that boundary. TPM-backed protection is a separate design
   decision, not something supplied automatically by Secret Service or PAM.

Howdy explicitly warns about lookalike/photo spoofing. Windows Hello branding on
a camera does not establish equivalent Linux authentication guarantees. Camera
compatibility and IR-emitter support must be verified for the actual model before
any face-unlock implementation or security claim.

### Acceptance gates before implementation

- Default off; activation requires a fresh master-password check and explicit
  acknowledgement of the chosen mode's protection and persistence across restarts.
- Validate the pinned SDK's local key wrapping/unlock path; use a dedicated secret
  per installation/account and separate it from token and certificate keys.
- Missing/locked/unavailable wallet, cancelled prompts and corrupt envelopes fail
  closed with a usable master-password fallback; never silently store plaintext.
- Lock clears runtime unlock material. Account switch, cancellation and lock revoke
  pending unlock results. Logout, disabling the feature and account removal delete
  the dedicated secret and envelope with recoverable deletion on failure.
- Test wallet availability, login-unlocked versus locked wallets, restart, removal,
  stale results and account separation on KDE and GNOME. For biometric mode, also
  test failure/cancellation, daemon loss and the real sensor/IR camera. Tests using
  fake authentication cannot validate spoof resistance or actual key-release policy.

References (reviewed 2026-11-08):
[Secret Service specification](https://specifications.freedesktop.org/secret-service/latest-single/),
[KDE wallet/provider architecture](https://github.com/KDE/kwallet/blob/master/README.md),
[libsecret and GNOME Keyring](https://gitlab.gnome.org/GNOME/libsecret/-/blob/main/docs/reference/libsecret/migrating-libgnome-keyring.md),
[fprintd](https://fprint.freedesktop.org/),
[Howdy security limitations](https://github.com/boltgolt/howdy#a-note-on-security),
[IR emitter support](https://github.com/EmixamPP/linux-enable-ir-emitter).

## Lock and logout contracts

Lock begins by denying new plaintext operations and advancing the unlock
generation. Immediately replace secret-bearing windows with the unlock surface;
close sensitive overlays without fade-out frames containing old values. Cancel
and drain affected work, clear SDK keys, then release all account plaintext:

- Vault lists, selected details, edit buffers, search indexes, and cached sort keys.
- Send contents and forms, generator output/history, import buffers, TOTP seeds,
  SSH material, export buffers, and reprompt password fields.
- Magnify results and clipboard worker copies. Clear the OS clipboard if the
  current value is still app-owned; do not overwrite unrelated clipboard data.

Prefer zeroizing secret containers where compatible with the SDK. Audit ordinary
`String`, `Arc`, message payloads, `Debug` implementations, tracing, GUI copies, and
allocator behavior. Do not disable SDK memory hardening to simplify integration.
Release plaintext on account switch where the view is shared; any background
account caches must still obey that account's lock transition.

Lock retains encrypted vault records and, in the target design, the protected
network session so encrypted sync can continue. It does not imply logout. If the
SDK cannot sync without unlocking vault keys, pause sync while locked instead.
PIN and biometrics stay unavailable until their actual key-release paths exist.

Logout first persists an account tombstone and denies all operations for that
incarnation. Stop account workers, clear keys/plaintext, delete credential-store
entries, remove the encrypted cache including SQLite WAL/SHM files after handles
close, and remove account metadata. Use a recovery journal for partial deletion;
startup must honor the tombstone and retry cleanup instead of loading the account.
If secret-store deletion fails, expose pending cleanup; do not claim complete
credential deletion. Attempt server revocation where supported, but local logout
must work offline. File deletion is not guaranteed physical erasure on SSDs or
backups; encrypted storage and destruction of local wrapping keys reduce exposure.

mTLS profile identities survive logout by default because they are needed before
login. Explain this separately and offer explicit identity/profile deletion as
specified in the mTLS requirements. Never delete a shared identity while another
profile references it.

### Current local removal protocol

`services/sdk/removal.rs` implements the existing demo backend:

1. Revoke runtime authority and purge plaintext immediately. Show pending logout.
2. Create `removed-accounts/<user UUID>` privately, sync the file, then sync its
   directory on Unix. The marker is retained after successful cleanup.
3. Drain operation leases, clear keys and drop the SDK owner. Queued operations
   and native dialogs retain session tokens, not database-owning SDK clients.
4. Atomically rewrite demo metadata without the account, using a private temporary
   file and a synced replacement. Remove only UUID-derived database filenames;
   never recurse into arbitrary caller-supplied paths.
5. Complete the UI operation. Failed intent commit leaves a persistent warning
   that the account may return after restart and cancels a deferred normal exit.
   Failed cleanup after commit keeps the account signed out and retries on restart.

Startup reads and validates the removal journal before opening databases. An
unreadable or invalid journal blocks account loading and shows a persistent
storage warning. Individual cleanup failures keep the affected IDs excluded while
other accounts can load. An abrupt termination before intent commit is not a
completed durable logout; normal exit waits for pending operations. The Unix
file/directory sync sequence is covered here; Windows directory-entry durability
and ACL validation still require platform-specific verification. File removal
cannot guarantee physical erasure, and copies outside this profile are unaffected.

The journal currently uses the demo server user UUID. Real login must introduce
installation-local account incarnations and explicit re-enrollment; it must never
remove an old tombstone merely because a late request returned. The current backend
has no OS credential-store entries or live server tokens to delete/revoke. mTLS
profile identities will remain separate from account logout as specified above.

## Protected item actions

Put reprompt checks in a common authorization service reached by detail, edit,
Magnify, menu, keyboard, and export paths. UI visibility is not authorization.
Match Electron's protected field/action behavior and enforce current account and
organization permissions as well. Reprompt is an additional UI authorization
boundary, not separate cryptographic encryption of that item.

After SDK password verification, issue an in-memory grant scoped to account,
unlock generation, item ID and revision, and requested action. For the first
implementation, grants authorize one action. Recheck at execution; lock, account
switch, item change, and cancellation invalidate the grant. The existing export
password gate remains, but its authorization must also expire during a native
save dialog or intervening lock. Never log verification inputs or raw SDK errors
that may contain secrets.

## Storage and privacy

Use the OS application data/config/cache locations, including Flatpak's assigned
directories. On Unix create private directories as `0700` and sensitive files as
`0600`; on Windows restrict ACLs to the intended user. Check ownership and reject
symlink substitutions at sensitive writes. Use atomic replacement and crash
recovery for configuration. Apply the policy to DB sidecars, backups and journals.

Store only SDK-encrypted vault records and wrapped vault keys on disk. Inventory
every SDK state repository before enabling persistence: token state must use the
OS credential store or authenticated encryption under an OS-protected random key,
not a plaintext SQLite table or JSON settings. If the credential store is
unavailable, allow an explicit session-only mode; never silently save unprotected
tokens or private keys. See the login document for persistence gates.

Seed generation must require an explicit disposable target, refuse a production
directory, and never recursively erase normal user data. Existing mock accounts
must not be auto-migrated into production profiles.

Exports use private temporary files in the destination directory, a checked
overwrite flow, and atomic finalization where supported. Check authorization
before plaintext generation and before final publication. Discard temporary data
on cancellation/lock; warn about a filesystem that cannot enforce permissions.
Do not describe a plaintext export chosen by the user as encrypted storage.

Icons use a separate anonymous transport and no account token/client identity.
Default icon fetching off for new self-hosted profiles until their configured
service is resolved and enabled. Disablement cancels queued fetches. Logs and
support bundles redact credentials, certificate bundles, vault values, and URLs
containing secret query parameters; even hostname logs can reveal vault contents.

## Delivery and acceptance gates

| Stage | Work | Required evidence |
| --- | --- | --- |
| S0 | Separate mock mode; stable private paths; truthful unavailable controls | Fresh production startup needs no `mock.json`; seeded data cannot overwrite it |
| S1 | Coordinator, generations, full plaintext purge, persistent logout | Deterministic lock/logout race tests and restart/deletion recovery tests |
| S2 | Reprompt and permission enforcement; export/clipboard/log hardening | Every entry point denies unauthorized access; sanitized diagnostic tests |
| S3 | Login/token persistence and encrypted read sync | Login/sync requirements gates pass with synthetic accounts |
| S4 | mTLS transport and recovery; online writes | Every transport path and identity isolation test passes |

Use pure tests for authority/grant rules, App/view update tests for stale messages,
and integration tests with explicit barriers around crypto and storage commits.
Exercise lock during unlock, decrypt, export dialog, save, certificate import,
refresh and sync; logout followed by login to the same user ID; account switches;
two servers sharing a UUID; and restart during each deletion step. Assert both
state cleanup and inability to execute an old action. Dropping a visible row alone
does not prove cleanup. Inspect storage/log fixtures for recognizable synthetic
secrets. A dependency advisory scan and platform security review remain release
gates; this concept is not an independent security audit.


## SSH signing service

The Unix SSH-agent implementation is opt-in and defaults to per-signature approval.
Its revocable SDK authority, private socket, bounded protocol, verified OpenSSH host
bindings, exclusion of reprompt/hidden/archived/deleted keys and post-approval checks
are documented in [SSH agent requirements](ssh-agent-requirements.md). Sidebar
filters do not grant signing authority. Cached grants are memory-only and invalidated
with the active session generation; Windows and protected-item reprompt are pending.
