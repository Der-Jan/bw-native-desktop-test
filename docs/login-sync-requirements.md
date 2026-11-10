# Native desktop login and sync requirements

Status: login, incoming WebSocket push with focus/reconnect reconciliation, full read sync, item creation, revision-checked updates, archive/unarchive, trash/restore, attachments and folder management implemented for review, 2026-11-09. Permanent item deletion remains deferred. Depends on the lifecycle and persistence contracts
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
manual refresh/retry. Folder create/rename/delete are synchronized immediately.
New items synchronize automatically after Save. Attachment upload/download/deletion is available in item details. Trash and Restore synchronize immediately. Permanent deletion, import/export and Send remain disabled for real accounts.

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
push notifications, permanent item deletion, import/export and Send. Errors are explicit; no downgrade is attempted.
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
audit of every field. Remote cipher editing enables strict field decryption;
type changes stay disabled; ownership/collection changes use a separate online flow below. Attachment mutations use their own fresh-permission checks and encrypted operation record, described below.

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
setting controls lookup. Lock, logout, sync/session replacement, profile changes
and disabling icons invalidate the in-memory source/cache. Validated 32×32 icons
also persist as normalized PNGs inside a separate SDK SQLite setting encrypted
with the account user key. The encrypted catalog includes account/profile UUIDs,
profile revision, hostnames and fetch times: copying a cache across those bindings
cannot reuse it. Neither hostnames nor PNG bytes are stored in plaintext. Sync,
unlock and restart reuse valid cached icons before requesting the server.

Persistent cache entries expire after seven days; at most 512 icons and 2 MiB of
encoded image payload are retained, with oldest entries evicted first. Decoding
cache PNGs is limited to 32×32 and 8 KiB; the corner mask is applied only once.
Corruption, expiry or failed cache storage falls back to the usual scoped fetch.
Failed network/image results are not persisted as successful icons. Cache
read/modify/write uses a short revocable account lease, outside HTTP/image decode,
so concurrent sources cannot lose updates. Disabling icons cancels pending cache
writes; an already-authorized SQLite commit may finish while cleanup drains its
lease, but cannot republish UI results. Logout removes the catalog with the
existing account database cleanup. Profile changes require a fresh catalog.
Late requests cannot populate a recreated cache. Responses are capped at 2 MiB;
decoding is capped at 4096×4096 and 32 MiB before resizing to 32px. Missing or
rejected icons retain the existing item marker. Separate cloud/custom icon-service
URLs are not configured automatically.

Key/password/KDF/security-stamp changes require a new login; automatic key migration
is deferred. Domains/Send payloads are retained in the typed encrypted snapshot
but have no full feature UI yet. Organization-policy coverage is described below.
Incoming WebSocket notifications, focus/reconnect checks and attachment transfers are implemented below. New and existing entry drafts synchronize after each successful save.

## Implemented URI rule display and duplicate view

Every website in read-only item details displays its saved matching mode. A null
mode shows the resolved Default (Base domain unless an organization policy supplies
another default). Editing and sync preserve explicit modes in the URI metadata;
the URI itself stays SDK-encrypted. These are autofill matching settings, not
server-maintained duplicate flags.

View → Duplicates in the sidebar lists active login entries whose saved website
rules overlap another entry's website. It evaluates every saved URI, rather than
only the first URL column, using Base domain (PSL registrable domains, including
private suffixes), Host (including non-default ports), Exact, Starts with and
case-insensitive regex modes. Never is excluded, and repeated URLs within the same
entry do not make that entry a duplicate. A match in either direction includes
both entries; different usernames, passwords, names and notes can still be listed.
Base-domain matching retains the regular client's script.google.com exception.

Vault, folder, collection and type facets constrain the comparison set. Search
filters the completed results so finding one entry by name does not invalidate
its matching partner. Comparison runs on a cancellable worker, is refreshed after
cache reload/sync, and publishes only to the current account/presentation. It uses
already-available list URI metadata; protected-item read/copy gates remain intact.
Nothing is deleted or changed, and URL/regex processing never fetches a website.

Regex evaluation has bounded pattern/input size, backtracking, compiled size and
a five-second comparison budget. Invalid/unsupported rules and limit exhaustion
are reported in the view. This is a local website-overlap view, not proof of
identical credentials, nor a replacement for the browser extension's autofill
implementation. Non-web application URIs and server equivalent-domain sets are
not incorporated. Regex rules are tested against concrete saved website URLs;
arbitrary regex-versus-regex intersection is not inferred. The matcher follows the adjacent regular-client URI strategy
implementation; registrable-domain extraction uses the static
[PSL library](https://docs.rs/psl/latest/psl/), with no runtime domain lookup.

## Implemented automatic incoming sync

Unlocked Vaultwarden accounts connect directly to `notifications/hub` beneath the
configured server base path using HTTP/1.1 WebSocket upgrade and SignalR
MessagePack v1. The authenticated GET uses a bearer header (never a token in the
URL), the same account TLS/mTLS identity, private CA and exact allowed origins.
No redirect, anonymous fallback or separately discovered host is trusted. Incoming
invocations are hints for full read-only synchronization; no event payload is
applied directly. Bursts coalesce with a two-second minimum between push-triggered
refreshes. Authenticated `LogOut` notifications lock the affected account, including
while an editor is open. Other updates remain pending until editing/dialogs finish.

The connection survives snapshot generation changes but is cancelled on lock,
logout or subscription disposal. Transport revision/certificate validity and
network authorization are rechecked every 100 ms and before processing frames.
Connection and SignalR handshakes have ten-second deadlines; frames, messages and
buffered SignalR data are bounded to 64 KiB, with a bounded MessagePack nesting
depth. Ping/pong and 15-second client heartbeats detect dead connections. Reconnect
backs off from two seconds up to 120 seconds with jitter; each successful SignalR
handshake requests a revision check to reconcile disconnected intervals. The normal
serialized credential refresh is reused before expiry and once after a 401.
The HTTP/1.1 client is built by the same transport constructor as SDK HTTP, so
servers offering HTTP/2 cannot accidentally suppress the RFC 6455 upgrade.

Revision checks run at startup/unlock, when focus returns, and after a successful
WebSocket reconnect; there is no periodic server polling. Checks use
`/accounts/revision-date` through the account transport. An unchanged revision
avoids a full download unless the snapshot is at least five minutes old, in which
case that triggered check also refreshes policy and other metadata. This age test
does not schedule network requests. The revision checkpoint precedes `/sync`, so
later notifications or triggered checks can detect changes racing the download.
A revision endpoint returning 404 makes triggered checks use full reads. If push
is unavailable, updates arrive on focus, reconnect or manual Synchronize.
Vaultwarden must enable WebSockets and its proxy must forward upgrades; see the
[Vaultwarden configuration](https://github.com/dani-garcia/vaultwarden/wiki/Enabling-WebSocket-notifications).

The dispatcher wakes even while the main window is hidden in the tray; restoring
focus requests a check. Per-account probes coalesce, fail quietly and back off to
30/60/120 seconds. Lock/logout stop authenticated incoming checks. Open editors, password
prompts, settings and file pickers defer background replacement; after they close,
pending notifications and checks resume. Search, list position, selected entry and an open generator are retained; changed
protected entries require authorization again. Incoming refresh never submits
local drafts or replays uncertain outgoing requests. Manual Synchronize remains
available to retry eligible pending edits.

## Implemented new-entry creation

New enables the existing native editor for server accounts after their first
successful full sync. Save uses SDK encryption and stages a durable encrypted
creation draft in SQLite, then automatically sends a personal `POST /ciphers` or
an organization `POST /ciphers/create` with collection IDs. Personal-ownership and
item-type policies apply. Organization creation requires confirmed membership,
a wrapped organization key and writable collections in that organization.

The server may assign a different UUID. The encrypted acknowledgment is verified,
the local ID is mapped to the server ID, and selection stays on the new entry.
Exact ciphertext matching during read-after-write also resolves a lost response
without another POST. An uncertain create is retained for explicit review/save or
discard, never blindly repeated on a timer or restart. Drafts remain encrypted
and visible offline. Existing-item type changes remain deferred; personal-to-organization transfers use the dedicated flow below.

## Implemented archive and unarchive

Item details uses the item name as its sole compact 18-point heading and shows
its actual vault (personal or organization), assigned collection names for
organization entries, current folder (including No folder), and Created / Last
changed timestamps in local time. Ownership and collection labels come from the
entry, independently of the selected navigation filters. Archive or Unarchive is
available alongside Edit. Archive hides the entry from
the active/favorites lists; the Archive filter exposes it for reading or unarchiving.
These actions synchronize immediately via Vaultwarden's account-scoped
`PUT /ciphers/<id>/archive` and `/unarchive` endpoints. The audited 1.37.5 and main
sources support both. A server rejecting the route produces an explicit error.
This is per-user metadata, so readable read-only collection entries can be archived
without changing the shared entry's content or requiring collection edit rights.

The backend requires a current unlocked session, fresh item access/policies and
unchanged item revision/key/ownership/reprompt. Protected entries use a distinct
one-use Archive authorization. First synchronize the item's pending content edits.
A durable intent precedes dispatch; only an authenticated full read confirming the
requested archive state clears it. Lost replies, restart and incoming sync never
repeat the PUT automatically. Pending outcomes expose Check server and confirmed
Dismiss local record; editing that item is blocked until its outcome is resolved
or explicitly dismissed. Lock/logout revoke in-flight publication. The usual
account TLS/mTLS transport and redirect restrictions apply. Permanent deletion remains
separate and disabled.

## Implemented Trash and Restore

Item details offers Move to trash for an editable server entry with delete
permission, and Restore for a trashed entry with restore permission. Both honor
the existing per-action master-password reprompt gate and fresh server access,
policy, revision/key/ownership and lifecycle-state checks. Pending local edits
must be synchronized first. Archive is per-user visibility metadata; Trash
changes the shared cipher's deleted state, affecting all users of an organization
entry. An archived entry stays archived if it is trashed and then restored.

Only `PUT /ciphers/<id>/delete` is used for soft deletion, and
`PUT /ciphers/<id>/restore` for restoration. In Vaultwarden, `POST .../delete` and
`DELETE /ciphers/<id>` permanently delete data; they are never substituted. A
successful empty deletion reply is supported, but only an authenticated full read
confirming the desired deleted state acknowledges the operation. Attachments and
cipher contents are preserved until a server-side purge. The configured server
retention policy determines when Trash is purged; the client does not schedule a
purge or offer permanent deletion in this slice.

Trash/restore use the existing durable lifecycle metadata intent alongside the
encrypted vault snapshot, extended with
an optional deleted-state target while retaining compatibility with old archive
records. Lost replies and restart reconcile with reads and never automatically
repeat a write. An unresolved operation blocks other lifecycle mutations and
editing/attachment management for that entry. SSH keys with an unresolved
lifecycle request are neither advertised nor used for signing. Check server and
confirmed local-record dismissal remain available; dismissal does not undo the
server operation. Lock/logout revoke publication and preserve only recovery
metadata alongside the encrypted cache. The server endpoints have no revision
CAS; fresh preflight
checks narrow but cannot eliminate a race with another device after validation.

Same-account refresh keeps the list's numeric vertical offset after archive,
trash, restore and ordinary updates. It holds that offset while the replacement
cache is empty or duplicate analysis is pending, then clamps it only to the new
scrollable range. The plaintext UI/font caches and grants are still cleared.
Search, filter and sorting changes intentionally reset the list, as do lock,
logout and account changes; a separate navigation epoch prevents cross-account
scroll restoration. Demo entries support local Trash/Restore without network.

## Implemented attachment transfers

Details offers Attach file, Download and confirmed Delete for server-backed items.
Save/synchronize a new entry or pending item edits first. Every operation fetches
fresh permissions and policies, checks the item's revision/key/ownership and uses
the existing copy/write and master-password reprompt gates. Attachment deletion
requires item edit permission. It does not require permission to delete the item.

Uploads use Vaultwarden's v2 Direct metadata API and an encrypted multipart body.
The SDK encrypts names, per-file keys and contents; plaintext source paths and
contents are never persisted in the operation record. The source is a regular
file (Unix symlinks are rejected), is left unchanged, and uploads are limited to
10 MB (10,000,000 bytes). The limit is enforced before SDK encryption or upload-slot
creation and shown in the file picker and upload tooltip. Existing larger server
attachments remain downloadable under the separate 500 MiB download safety cap.
Encryption/decryption currently buffers the file in memory; streaming very large
files is future work. Binary transfers have a 15-minute timeout and observe lock
cancellation at 100 ms intervals. The per-account mutation coordinator serializes
writes while other vault reads stay responsive during long transfers.

Only the account's exact same-origin `/attachments/<cipher>/<file>` endpoint is
accepted for signed file downloads, including a configured base path. The normal
TLS/mTLS identity and CA apply, redirects and external storage are rejected, and
signed file requests carry no OAuth token. Metadata and multipart requests use
the authenticated API. Response sizes are bounded. SDK authentication/decryption
must succeed before plaintext is published to the user's selected path, using a
private sibling temporary file, fsync and atomic rename (0600 on Unix). Failure,
lock or a changed connection profile preserves an existing destination. Legacy
v1 downloads use the SDK user-key path; editing keyless items with v1 files still
requires a future attachment migration.

Encrypted durable intent precedes upload-slot creation, data dispatch and delete.
A lost data response is reconciled by downloading and hashing the exact encrypted
file; a lost delete response by confirmed metadata absence. A lost slot response
can discover its encrypted name/key but does not imply its contents were uploaded.
An incomplete upload is shown for explicit deletion/retry; no source file, bytes
or local path are retained for automatic resumption. Check server is read-only.
Dismiss removes only the local operation record and cannot undo a server change.

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

## Implemented organizations and item collection assignments

Item details → **Vault / collections…** opens a native dialog. Personal items can
move into an eligible confirmed organization with a loaded key and a writable,
visible-password collection. The user selects collections and explicitly acknowledges
ownership transfer. Cancel initially has focus; no transfer is submitted merely by
opening the dialog or pressing Enter. Organization-owned entries stay in their
organization and can change collection assignments. Existing read-only/hidden-password
assignments are preserved; the user cannot add/remove those assignments. At least
one writable collection must remain selected. Organization creation, membership,
collection CRUD/access administration and moving between organizations/back to a
personal vault remain web-vault tasks.

These are separate online operations, not ordinary content drafts: personal transfer
uses `PUT /ciphers/<id>/share` with the SDK-reencrypted cipher and collection IDs;
collection membership uses `PUT /ciphers/<id>/collections` with IDs only. Both routes
exist in audited Vaultwarden 1.37.5 and main. The account TLS/mTLS transport and
protected-action gate apply. Fresh preflight checks the source revision, ciphertext,
ownership, existing collections and reprompt; target membership, keys, collection
permissions and current policies are checked again immediately before sending.
A policy forbidding personal ownership does not prevent moving an existing personal
item into an eligible organization. Ordinary edits remain subject to that policy.

The SDK moves/decrypts/encrypts the item and rewraps per-item/attachment metadata keys;
file bytes and private keys never leave the SDK boundary as unencrypted transport
data. Incomplete decryption and legacy attachments missing keys fail before dispatch
and require upgrading in the web vault. Modern attachments remain downloadable after
transfer without reuploading file contents.

A durable encrypted sharing/membership intent is committed before HTTP. Only a full
server read confirming ownership, collection membership, and for transfers the exact
encrypted content clears it. No response or mutation retry is trusted blindly, even
on a 4xx: Vaultwarden's collection changes are not transactional. Manual Synchronize
reconciles but never replays a sharing request. Pending records survive lock/restart,
block other mutations/signing of that item, and expose Check server / explicitly
confirmed Dismiss local record actions. Dismiss does not undo server changes. Normal
cache refresh preserves list position and applies current access permissions. Open
placement dialogs defer incoming replacement; lock/account switching clears them.

The collection endpoint has no conditional revision parameter. Preflight cannot
eliminate a change racing its update; fresh read reconciliation and conservative
uncertain-operation handling limit the consequences without claiming atomic CAS.

## Implemented folder management

The item-details Folder selector changes an entry's folder directly, including
No folder, without opening the editor. It uses the same encrypted item save,
revision checks and automatic outgoing sync as other edits. Read-only/trashed
items, pending drafts and busy operations disable the selector. Protected items
require a one-use write confirmation. Closing/navigating away from details while
the save completes does not reopen or replace the newly selected entry.

File → New folder and the sidebar's New folder action work for real accounts.
Right-click an existing folder to rename, create a nested folder, or delete it.
Native windows focus the name input; Enter submits creation/rename and Escape
closes the dialog. Deletion has a separate confirmation with Cancel initially
focused. Names are trimmed and limited to 1000 characters. Slash-separated names
provide nesting, matching the existing folder filter. Rename/delete affects only
the selected folder; other folder names, including nested folders, are preserved.

Folder changes use SDK user-key encryption and the account's scoped TLS/mTLS
transport: POST `/folders`, PUT `/folders/<id>`, DELETE `/folders/<id>`. A full
read-only preflight validates account keys/membership and compares the target's
cached revision/encrypted name with the current snapshot. Stale/foreign targets
are rejected. Vaultwarden has no conditional revision parameter for these folder
writes, so preflight cannot eliminate a change racing the subsequent request.

A create/rename/delete intent is durably stored with encrypted names before the
request. Successful create/rename replies must return the expected ciphertext
and a valid server ID; the response and intent removal commit together. Full
read-after-write sync reconciles missing replies by exact encrypted name/ID or
confirmed deletion. Requests are never automatically replayed; a pending record
survives restart and prevents another folder mutation until reconciled or explicitly
dismissed. Check server performs a read-only refresh. Dismissing a record does not
undo a server change and warns against accidentally creating another copy.
Known validation/authentication rejections keep the form available for correction;
an uncertain create closes its form and displays the pending-operation notice.

Deleting a folder keeps all entries and lets the server remove their folder
assignments. Nested folders are independent and remain. A deleted selected folder
changes the navigation filter to No folder. Local cipher drafts are retained;
those referring to a removed folder become conflicts for explicit review. Folder
operations do not upload unrelated item drafts. An existing editor or active sync
is preserved rather than replaced by a folder operation.

Disposable mandatory-mTLS tests cover lifecycle, encryption, server IDs, preserved
entries/nested folders/drafts, stale and foreign targets, lost/unapplied requests,
restart, lock during dispatch, authentication rejection, redirects and application
sidebar/form updates. The same actions are available in explicit local demo mode.

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
focus/reconnect checks, and notification events; do not run concurrent state-replacing
syncs. Notifications trigger reconciliation rather than directly trusting event
payloads as vault state. Reconnect with bounded exponential backoff and jitter;
use focus/reconnect checks and manual Synchronize if push is unavailable.

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

Existing-entry edits, item creation, attachments and folder management are implemented above. Trash/restore also have durable intent and read-only confirmation. Permanent deletion remains disabled pending its own concurrency and ambiguous-outcome handling. Preserve the current separation between validation
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
| Y3 | Existing-entry edits and folder management pass conflict/ambiguous-result tests; item creation and attachment reconciliation pass; trash/restore and scroll retention pass; permanent deletion gates remain open |
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
