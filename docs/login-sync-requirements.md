# Native desktop login and sync requirements

Status: login and full read-sync implemented for review; writes/notifications deferred, 2026-11-07. Depends on the lifecycle and persistence contracts
in [security concept](security-concept.md). Networking must obey the
[mTLS transport policy](mtls-requirements.md), including when no identity is set.

## Current implementation and SDK proof

The pre-sync implementation now covers HTTPS US/EU/self-hosted profiles, anonymous
configuration discovery with explicit origin authorization, bounded prelogin KDF
parameters, SDK password hashing, password authentication, TOTP/email verification,
profile identity verification and SDK account-key unlock. Local account UUIDs are
independent of server user IDs and email addresses. Full read sync now runs after
login/unlock and on explicit refresh. Real vaults remain read-only: server CRUD,
import/export and Send are disabled in this slice.

Credentials are AES-256-GCM ciphertext in private files; random encryption keys
live in the OS credential store (Secret Service on Linux). Account metadata and SDK
SQLite contain encrypted account keys and credential references, never OAuth tokens
or the master password. The master password is retained in memory only during the
bounded login/2FA attempt and SDK unlock. Generic UI/SDK allocations can contain
transient copies; this is not a promise of complete process-memory erasure.

Prelogin selection is persisted as an exact profile UUID in `login-profile.json`,
restored on startup/account addition/logout, and used by both email and password
pages. Changing server settings on the password page updates the actual login
binding and clears an already-entered password when the endpoint changes. The
settings picker distinguishes saved profiles for the same URL by their client
certificate fingerprint. Older previews without a selection receipt recover an
unambiguous custom endpoint and its sole certificate binding; multiple endpoints
or certificate identities require explicit selection. Profiles and certificates
are retained during this migration. Save waits for a selected PKCS#12 import and
keeps import failures visible in the open dialog.

The authenticated status page has an explicit profile probe. Refresh is serialized
per local account, reloads the latest protected credentials under that gate, commits
rotated credentials before using the new access token, and uses the account's mTLS
transport. Logout revokes work, drains leases and deletes the current credential key
and ciphertext before removing metadata/SQLite. Pending account publication has a
startup recovery receipt; orphan ciphertext/keys and unused account profile snapshots
are collected before accounts load. A prelogin certificate remains configured for
future sign-ins. Local logout does not revoke all server sessions.

SDK 4.0.0 at `a13b4c4e1fa3685280241f417920fd19d7ef24a6` has no public modern
password-login request representing 2FA and desktop device credentials. The app uses
a small HTTP protocol adapter for these fields; all password hashing, wrapped key
parsing and key initialization use SDK components. The isolated `vendor/bitwarden-core`
patch adds account transport injection for API and Identity; see its UPSTREAM.md.
Do not enable SDK `test-fixtures` in production or persist SDK auth tokens in SQLite.

Not implemented in this slice: CAPTCHA/new-device challenge completion, SSO,
WebAuthn/security-key/Duo/recovery-code login, password-change workflows,
notifications, periodic background polling, online CRUD, import/export and Send. Errors are explicit; no downgrade is attempted.
The remaining sections describe the acceptance contract for the later sync work,
not a claim that those features are already available.

## Implemented read-sync slice

The account coordinator fetches `/sync?excludeDomains=true` through the same scoped
mTLS transport as login/refresh. A 401 allows one refresh/retry; a definitively
invalid refresh stops further authenticated networking for that runtime session,
while existing cached data remains locally unlockable. Manual requests coalesce
with an in-flight successful sync. Sync remains limited to locally unlocked accounts.

The SDK's `PasswordManagerClient::new_with_sync` currently registers crypto,
folders, Send and policies, but lacks a complete cipher handler and cross-handler
atomic transaction. Its repository `replace_all` is remove-then-set. Calling it
directly would not meet our snapshot contract. The app instead persists one typed
`native_encrypted_vault_snapshot_v1` setting through **SDK-managed SQLite**. The
single-row SQLite transaction contains the complete encrypted response plus account
identity, schema version and successful download-start timestamp. There is no
separate JSON cache, plaintext vault database or custom crypto implementation.

SDK conversions validate all cipher/folder/collection IDs and encrypted formats;
missing lists, duplicate IDs, unsupported types and wrong-account responses fail
without replacing the previous snapshot. Legacy organization collections without
`type` map to the SDK SharedCollection type. SDK crypto initializes wrapped org
keys and validates the derived list. SDK field decryption retains its upstream
lenient behavior for some legacy cipher fields; this is not a strict cryptographic
audit of every field. Full-data mutations stay disabled.

Client-managed read-only repository adapters expose ciphers and folders from one
shared immutable snapshot Arc. This is an index over the SDK-SQLite snapshot,
not another persistent store. A per-account exclusive session lease protects
key/repository transitions. Failure or cancellation restores prior organization
keys; removing even the last organization clears its old runtime key. Successful
publication invalidates old grants and queued UI results by advancing the session
generation. An encrypted SQLite transaction already authorized before lock may
finish while lock/logout drains the lease; it cannot republish plaintext or revive
an unlocked session, and logout removes its database after draining.

Restart restores encrypted repositories before local unlock. Collections are
materialized only after SDK key unlock and are cleared on lock. Successful sync
purges old plaintext views before reload, preserves selection by cipher ID when
possible, and displays the committed timestamp. Failure keeps the old cache and
shows retry/status instead of reporting an empty vault. Public favicon lookup is
disabled for real vaults until profile-scoped icon transport is implemented.

Key/password/KDF/security-stamp changes require a new login; automatic key migration
is deferred. Policies/domains/Send payloads are retained in the typed encrypted
snapshot but have no full feature UI or complete client-policy enforcement yet.
There is no background polling, WSS, attachment transfer or online write queue.

## Server profiles and account identity

A saved profile contains a local profile ID, US/EU/self-hosted selection, base URL,
resolved API/Identity/notifications/icons/web-vault endpoints, configuration
revision, and transport policy reference. Preserve reverse-proxy base paths.
Require HTTPS/WSS in production; permit insecure loopback only in an explicit
development mode. Reject URL credentials and inappropriate query/fragment data.

Resolve server configuration anonymously before login. Server-discovered URLs
must pass the endpoint policy; discovery cannot silently authorize a new host to
receive a password-derived credential, bearer token, or client certificate.
Additional origins require an explicit trusted profile decision. Key accounts by
local account ID plus profile and authenticated user ID; emails are display data.
Persist a stable device identifier per installation using SDK-supported desktop
platform metadata and version information.

## Login state machine

The target flow is `No account -> Profile ready -> Prelogin -> Password submitted
-> Challenge pending (if required) -> Session established -> Initial sync`.
Every branch also has cancellation and recoverable error states. Network login,
local master-password unlock, and remote sync are separate states.

1. Select/create a profile, configure its certificate before the first request
   if required, and validate discovered endpoints.
2. Obtain KDF configuration through SDK prelogin. Validate parameters through the
   SDK; bound expensive work and keep it off the UI thread. Never downgrade KDF
   settings silently or send the master password to the server.
3. Submit SDK-generated authentication with device metadata. Clear password
   buffers after consumption. Pending login state is isolated from live accounts.
4. Handle server-requested device verification, CAPTCHA and 2FA as typed outcomes.
   Initial supported methods must be explicit (for example TOTP/email); unsupported
   Duo/WebAuthn/SSO methods show a usable explanation and retain no authenticated
   session. Full Electron parity requires the remaining methods, not bypasses.
5. Publish an authenticated account only after required challenges and local
   persistence succeed. A persistence failure may offer explicit session-only
   operation, never fake a remembered login. Respect server-directed password/KDF
   changes and account restrictions before enabling vault actions.
6. Run initial sync, then decrypt only when locally authorized. If sync fails,
   show an authenticated but unsynced account with retry; do not report an empty
   remote vault as a successful sync.

Bind each completion to its login attempt, profile revision, account incarnation,
and transport generation. Cancelled login and account switching cannot attach
tokens or crypto state to a new target. Pending certificates are referenced by
profile ID, never by a mutable global active-account pointer.

SSO is a later explicit milestone: browser launch, cryptographically bound
state/nonce and PKCE where the protocol uses it, validated callback routing,
timeouts, and SDK account-key acquisition. An external browser needs its own
certificate configuration; the Rust identity is not automatically available there.

## Session persistence and token refresh

Inventory access tokens, refresh tokens, remembered-2FA credentials, device
metadata, KDF parameters, wrapped user/org keys, private-key envelopes, and all
SDK repositories. Classify sensitive values before selecting durable storage.
Verify SQLite, WAL, journals, diagnostics and migration outputs contain no
plaintext credentials or vault values. Protect tokens using the OS credential
store or an encrypted store with an OS-protected key. Wrapped SDK keys remain
wrapped; do not persist a decrypted account key to simplify offline unlock.

Serialize refresh per account. Persist rotated refresh credentials atomically
before depending on them for restart, and reject responses from an old account
incarnation. Handle temporary network failure separately from a definitively
invalid session. On the latter, stop authenticated networking and request login;
encrypted offline access may remain according to local policy. Credential
invalidation must not be mislabeled as mTLS expiry, or vice versa.

Restart restores a locked account from protected metadata and encrypted cache.
Offline unlock uses SDK-supported local key envelopes and cannot authenticate a
new account. Logout follows the tombstone/cleanup contract, including offline
logout and credential-store errors.

## Sync and mutation semantics

Use one sync coordinator per account. Coalesce manual refresh, startup refresh,
periodic polling, and notification events; do not run concurrent state-replacing
syncs. Notifications trigger reconciliation rather than directly trusting event
payloads as vault state. Reconnect with bounded exponential backoff and jitter;
poll as a documented fallback if push support is unavailable.

Fetch and validate encrypted state into staging, then apply an internally
consistent snapshot. Include ciphers, folders, organization keys, memberships,
collections, policies, profile/KDF changes and deletions supported by the server.
If the SDK cannot transact the entire apply, use a staged snapshot and atomic
active-snapshot switch; never report completion midway through partial updates.
Preserve unknown fields/types or prohibit editing them instead of losing data.
Commit the server revision/checkpoint with the snapshot. Update the successful
sync timestamp only after commit; retain the previous usable snapshot on failure.

Locked accounts may receive encrypted updates without plaintext materialization
only if the SDK proves that behavior. Unlock reconstructs derived views from
the latest committed snapshot. Apply results only to the correct account and
generation; invalidation prevents both stale UI updates and storage commits.
Membership removal and key changes invalidate permissions and decrypted caches.

Deliver read sync first. Until online writes land, disable mutations in real
profiles and label the vault read-only. Later writes encrypt via the SDK, send to
the server, accept its assigned IDs/revisions, and persist the confirmed result.
Treat conflict, validation failure and an unknown server outcome separately.
After timeout following a sent mutation, reconcile before retrying; do not blindly
replay creates or deletes. Initial release has no offline mutation queue. A later
queue requires encrypted persistence, conflict semantics and idempotency design.

Send requires its own SDK repository, encryption/upload, real access ID and
profile web-vault link handling. It must not appear operational while backed by
the current plaintext memory map. File Sends and attachments are separate
milestones with download-origin and encryption tests.

## User feedback and acceptance

Show last successful sync, progress, offline/cached status and retry. Errors must
distinguish credential challenges, session expiry, server trust failure, client
certificate expiry/missing key, timeout, and storage failure. Preserve existing
cached data on sync failure. Certificate recovery is reachable without a working
server and checks expiry even when manual refresh would otherwise be throttled.

| Gate | Required integration evidence |
| --- | --- |
| L1 | Password login against US/EU configuration and a disposable self-hosted server; correct KDF and stable device identity |
| L2 | Wrong password, TOTP/email challenge, device verification, CAPTCHA, unsupported provider, cancellation and account-switch paths |
| L3 | Restart locked, offline unlock, rotated refresh token, credential-store lock/failure and permanent session invalidation |
| Y1 | Initial/update/deletion sync; org membership/key changes; failure during apply preserves the old snapshot |
| Y2 | Overlapping refreshes coalesce; WSS reconnect/poll fallback; lock/logout during fetch/apply cannot revive state |
| Y3 | Online create/edit/delete uses server IDs and revisions; conflicts and ambiguous timeouts reconcile without duplicates |
| Y4 | Two profiles with equal server UUID/email remain isolated; sort preference survives refresh and selection stays on the same cipher ID |

Use synthetic fixtures and an instrumented server/proxy to observe every HTTP and
WSS path. Run the same login/refresh/sync matrix with and without required mTLS.
Maintain the proven SDK symbol/transport inventory alongside the harness whenever
the SDK pin changes.


## Protocol version and SSH-item visibility

Every scoped account transport sends `Bitwarden-Client-Version`; the SDK host
metadata uses the same centralized `BITWARDEN_PROTOCOL_VERSION`. This is a wire
compatibility baseline, not the Rust SDK crate version or the app's 0.x package
version. The current baseline is `2024.12.0`, the minimum for SSH-key sync in
[Vaultwarden's sync handler](https://github.com/dani-garcia/vaultwarden/blob/main/src/api/core/ciphers.rs).
[Vaultwarden's ClientVersion guard](https://github.com/dani-garcia/vaultwarden/blob/main/src/auth.rs)
reads the explicit header. Without it, `/sync` succeeds but excludes type-5 SSH
items. A successful HTTP response therefore did not guarantee a complete supported
type set in the old cache. Perform a fresh sync after upgrading this transport;
retain existing cache contents and permissions until the atomic refresh succeeds.

The regression fixture reproduces the server's version gate over mTLS: an explicit
`0.1.0` request omits the SSH item, whereas the default request includes it. Real
SDK encryption and API `sshKey.keyFingerprint` conversion are exercised through
snapshot installation, vault list decryption and agent identity enumeration. Private
keys do not enter diagnostic logs or test request snapshots as plaintext.
