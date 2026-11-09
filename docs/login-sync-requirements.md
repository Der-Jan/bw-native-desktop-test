# Native desktop login and sync requirements

Status: login, full read sync and revision-checked existing-entry updates implemented for review, 2026-11-09. Creation/deletion and notifications remain deferred. Depends on the lifecycle and persistence contracts
in [security concept](security-concept.md). Networking must obey the
[mTLS transport policy](mtls-requirements.md), including when no identity is set.

## Vaultwarden scope and audited compatibility

The implementation target is Vaultwarden, not full Bitwarden Enterprise/Electron
feature parity. Enable only functionality supported by the configured Vaultwarden
server; presence in the SDK is not sufficient. Optional server features may be
configured off, and a supported server feature is not necessarily implemented in
this client yet.

Audit: 2026-11-09, stable [Vaultwarden 1.37.5](https://github.com/dani-garcia/vaultwarden/releases/tag/1.37.5),
commit `6ed6921cc2b0e03b4d74189574d2a5e71c512e97`. The same relevant route/policy
boundaries were checked on main `17b159beff4b892f07f61f38ce2fe452e55251c1`.
This is a source audit, not discovery of the user's installed server version.

| Method | Vaultwarden 1.37.5 | Native client |
| --- | --- | --- |
| Master password, TOTP, email 2FA | Supported; email depends on server setup | Implemented |
| FIDO2/WebAuthn as second factor | Supported/configurable | Open |
| YubiKey OTP, Duo | Supported/configurable | Open |
| Login approved on another device (`authRequest`) | Supported | Open |
| OIDC SSO | Supported when enabled by the server | Open |
| Passwordless passkey login to the vault | Explicitly unsupported | Outside scope |
| Direct SAML login flow | Not the server's OIDC SSO protocol | Outside scope |

Sources: [identity grant/challenge handling](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/api/identity.rs),
[2FA routes](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/api/core/two_factor/mod.rs),
[passkey-login placeholder](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/api/core/mod.rs).
WebAuthn 2FA is distinct from passwordless vault login and from passkeys stored for
other websites. API-key grants also exist server-side but are not a commitment to
add another desktop login flow.

Organization-policy scope follows Vaultwarden's actual `OrgPolicyType` and its
server behavior. Applicable examples include mandatory 2FA, master-password and
generator rules, personal ownership, disabling Send, PIN restrictions, restricted
item types and URI matching defaults. Do not implement Bitwarden-only maximum
vault-timeout, personal-vault export prohibition or RequireSso policies: these
are explicitly commented out as unsupported in
[Vaultwarden's policy enum](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/db/models/org_policy.rs).
OIDC can still be required by server configuration independently of an org policy.
Collection/item permissions remain mandatory and are distinct from these policies.

Logout API audit: no registered endpoint for revoking only the current app's
access/refresh token or device session was found in the audited versions. In
particular, `/devices/identifier/<device_id>/clear-token` removes only a push
notification token, not login tokens. `/accounts/security-stamp` invalidates all
sessions and deletes all devices. Neither is a substitute for targeted session
revocation. Do not probe guessed OAuth revocation endpoints or use the global
reset during normal logout. See
[registered account routes and implementations](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/api/core/accounts.rs).
Local logout continues to revoke in-flight authority and remove protected tokens
and the local cache, including offline. If a future supported Vaultwarden API
provides targeted revocation, add a bounded best-effort request through the
account's TLS/mTLS transport before deleting the credential, without making
local logout depend on network availability. Global deauthorization would need
a separate explicit action and is not part of this task.

## Current implementation and SDK proof

The pre-sync implementation now covers HTTPS US/EU/self-hosted profiles, anonymous
configuration discovery with explicit origin authorization, bounded prelogin KDF
parameters, SDK password hashing, password authentication, TOTP/email verification,
profile identity verification and SDK account-key unlock. Local account UUIDs are
independent of server user IDs and email addresses. Full downloads run after
login/unlock and preserve local drafts. Every successful edit save automatically
synchronizes existing-entry changes. File → Synchronize remains available for
manual refresh/retry. Creation/deletion, folder mutations, import/export and
Send remain disabled for real accounts.

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
shows retry/status instead of reporting an empty vault. Website icons now use
`<account server base>/icons/<hostname>/icon.png`, preserving a configured subpath
and the account's mTLS identity/private CA. Requests send no OAuth credentials,
follow no redirects and have no public-service fallback. The existing appearance
setting controls lookup; icons stay in memory only. Lock, logout, sync/session
replacement, profile changes and disabling icons invalidate the source/cache.
Late requests cannot populate a recreated cache. Responses are capped at 2 MiB;
decoding is capped at 4096×4096 and 32 MiB before resizing to 32px. Missing or
rejected icons retain the existing item marker. Separate cloud/custom icon-service
URLs are not configured automatically.

Key/password/KDF/security-stamp changes require a new login; automatic key migration
is deferred. Domains/Send payloads are retained in the typed encrypted snapshot
but have no full feature UI yet. Organization-policy coverage is described below.
There is no background polling, WSS or attachment transfer. Existing-entry drafts
are synchronized after each successful save as described below.

## Implemented existing-entry write sync

Save stages an SDK-encrypted draft in the same durable SQLite setting as the
server snapshot, retaining its original server revision. A successful Save queues
an immediate account-scoped upload, including if the user closes the editor or
switches accounts while saving. If another editor is open for that account, the
upload waits until it closes. Failed uploads retain the draft without a retry
loop. File → Synchronize offers explicit refresh/retry. Startup/unlock downloads
remain download-only; old drafts are not uploaded just by reopening the app.
No plaintext edit queue is persisted. Logout still removes unsent drafts with the local account.

Before writing, fetch and validate a full current snapshot, account keys, security
stamp, organization keys/membership, policies and item permissions. A changed
revision, ownership, collection membership, key, attachments, removed folder or
write restriction makes the draft a conflict. Conflicts remain encrypted and
separate from current server rows; there is no automatic merge or rebase.

Eligible existing-entry updates use SDK `CipherRequestModel` conversion and
`PUT /ciphers/<id>` with the original full-precision `lastKnownRevisionDate`, the
server user ID in `encryptedFor`, and the user key ID when provided. Every request
uses the account's bounded, origin-scoped TLS/mTLS transport and bearer token.
No mutation is retried automatically after token errors, rejection or timeout.

Server limitation: Vaultwarden 1.37.5's revision rejection compares whole-second
age and rejects only values greater than one second. Exact preflight revision
checks reduce the race window, but cannot make this API a strict atomic
compare-and-swap: sufficiently close concurrent writes can still be accepted by
the server. Stronger guarantees require server support. See the
[revision check](https://github.com/dani-garcia/vaultwarden/blob/1.37.5/src/api/core/ciphers.rs).

An uncertain intent is committed before dispatch. An acknowledged response must
match the submitted encrypted content and identity before its server revision and
removal of the draft commit together. Batches commit each acknowledgment separately.
After dispatch, a read-only refresh reconciles replies lost after a successful
write by exact encrypted-content comparison (normalizing empty optional arrays).
Otherwise the draft needs explicit review; it is never silently resent, including
after a crash or lock. Lock/logout revokes authority before later batch requests.

The vault lists conflicts with Server version, Review local draft and confirmed
Discard actions. Reviewing requires current read/edit permissions and any current
or draft password reprompt. It opens the draft's editable content against fresh
server metadata; an explicit Save stages a new base revision and automatically
synchronizes the reapplied draft. Deleted/inaccessible entries retain encrypted drafts, but recovery to
a new item is not yet available. Draft discard only deletes local data.

Remote clients enable the SDK's strict field decryption so a corrupt field cannot
silently become empty on save. Password and changed hidden-field history retain
up to five entries, following SDK edit behavior. Partial items or attachment
failures cannot be saved. Keyless entries with v1 attachments cannot be edited: the
SDK would introduce an item key, requiring file-content re-encryption that this
client does not yet perform. Attachment upload/download and ownership transfers are
not supported by this write slice.

Verification uses disposable SDK-encrypted accounts behind mandatory client TLS:
normal upload/offline restart, unchanged original revisions, fresh permissions and
policies, preflight conflicts, a change between GET and PUT, explicit reapplication,
lost replies with/without server application, partial batches, lock during dispatch,
one-use draft reprompt, strict corrupt-field rejection and download-only refresh.
Automatic-save integration tests also cover successful upload, failures without
retry loops, retained conflicts, switching away during save, preserved search and
selection/details, and protected entries remaining hidden after revision changes.
No real account is used by this harness.

## Implemented Vaultwarden organization policies

Policies are compiled from the same immutable encrypted snapshot as items and
memberships. They survive offline restart; changes take effect after a successful
sync. Invalid, duplicate, foreign-organization or unknown enabled policies reject
the new snapshot instead of weakening the previous rules. Applicable roles and
memberships follow the official client's exemptions; generator and restricted-item
rules also apply to owners/admins.

- Generator requirements merge across applicable organizations and are enforced
  both in the UI and at the SDK service boundary. Password/passphrase generation
  for remote accounts waits for the first policy-bearing sync.
- Restricted item types filter the list and guard direct read/copy/edit and SSH
  identities/signing. Personal ownership blocks local edits/saves of personal
  items while retaining permitted read/copy access.
- URI matching defaults appear in the editor; inherited defaults remain unset in
  stored items, so later policy changes still apply. Explicit item overrides remain.
- PIN prohibition disables the PIN preference (PIN unlock is not implemented).
- Identity master-password requirements marked enforce-on-login are checked before
  publishing an authenticated account. An inadequate password requires a change in
  the web vault; no password-change workflow is claimed here.

Settings show active policies and distinguish server-enforced 2FA/membership rules
from Send, recovery, autotype and FillAssist workflows unavailable in this app.
No unsupported Bitwarden-only timeout, export or RequireSso policy is invented.
Browser pairing releases an account key to the official extension; that extension
handles its own policies. Native item guards do not restrict that key's capabilities.

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
   session. Only Vaultwarden-supported remaining methods are in scope; never bypass a challenge.
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

Vaultwarden OIDC SSO is a later explicit milestone: browser launch, cryptographically bound
state/nonce and PKCE, validated callback routing,
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

Existing-entry write sync is implemented above. Creation, deletion and folder
mutations remain disabled until their own server ID, concurrency and ambiguous
outcome handling is proven. Preserve the current separation between validation
failure, revision conflict and unknown server outcome. Do not blindly replay
creates/deletes or extend edit retry semantics to those operations.

Conflict behavior follows Vaultwarden's optimistic concurrency: send the original
`lastKnownRevisionDate`; on rejection, fetch the current version and keep the
local draft for explicit review/reapplication or discard. Do not automatically
merge fields, pick a timestamp winner or retry with a refreshed revision. Retaining
a draft does not authorize a write after organization/permission changes.

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
| Y3 | Existing-entry edit/conflict/ambiguous-result tests pass; create/delete and folder-write gates remain open |
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
