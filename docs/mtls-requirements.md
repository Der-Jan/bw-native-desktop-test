# Native desktop mTLS requirements

Status: HTTP login/probe, read sync, incoming WSS/polling, item creation/updates, archive/unarchive, trash/restore, attachments and folder management implemented for review, 2026-11-09.
The egui frontend preserves these contracts (2026-11-08); see
[migration validation](egui-migration.md). Permanent item deletion remains deferred.
Implement this with the [security lifecycle](security-concept.md) and
[login and sync flow](login-sync-requirements.md).

## References and scope decisions

The recent iOS work is in two separate branches, both ahead of `main` at review
time; these references do not imply that either change is merged or shipped:

- [mTLS commit f55dc0d](https://github.com/Der-Jan/bitwarden-ios/commit/f55dc0d5f88c14d93d81963290746aff9aa49e87),
  branch `codex/mtls-certificate-settings`: account-scoped certificate management,
  validity checks, replacement and expiry recovery.
- [Sorting commit 3e5e9e6](https://github.com/Der-Jan/bitwarden-ios/commit/3e5e9e6afb7b447848153673ca9e56559274db1f),
  branch `codex/ios-login-sort-options`: persistent detailed vault sorting,
  described separately in [sorting requirements](vault-sorting-requirements.md).

The adjacent Electron implementation at `1a11ec6383` uses a Flatpak NSS store,
fingerprint selection and endpoint-global bindings in
`apps/desktop/src/platform/main/mtls/`. Its changes require restart. The adjacent
`apps/desktop/native-linux-client-concept.md` proposes Rust transport injection
and endpoint isolation for a different GTK project; it is architectural input,
not an implementation in this desktop app.

Adopt the iOS account-scoped replacement behavior for this native app, with an
explicit endpoint allowlist per saved profile. Before login, the saved profile
owns the certificate reference; accounts inherit a profile snapshot with their
own configuration revision. Replacing one account's certificate must not silently
change another account, even on the same host. A profile may be explicitly cloned
or linked during onboarding, but editing one account detaches its configuration.

This deliberately differs from Electron's endpoint-global selection model. Rust
can use separate pools per account, allowing two accounts on the same endpoint
to use different identities. Deduplicate identity secret storage by fingerprint,
but keep account bindings independent and reference-count deletion. Keep an
explicit prelogin profile reference after logout so a new login can reach an
mTLS-protected server; show that reference in identity management.

SDK integration is now evaluated against 4.0.0 at
`a13b4c4e1fa3685280241f417920fd19d7ef24a6`. Its `load_from_state` can fetch `/config`
while reconstructing a locked client. Cover that request with account transport
policy too. The builder's `with_server_communication_config` currently installs
SSO cookie middleware; that name alone is not proof of a client-certificate API.
The local SDK patch now injects the account client and origin policy into both API and Identity. External SDK HTTP remains anonymous.

## Implemented review scope

Server settings are available before login and on the locked/authenticated account
screens. PKCS#12 import requires exactly one private key and a matching leaf, orders
its chain by issuer/signature, rejects unrelated or ambiguous certificates and checks
validity, client-auth EKU and signing key usage when those extensions exist.
Issuer signatures, validity and signing usage are checked. The certificate chain is not installed as server trust. Optional private
server CA trust is an explicit separate PEM import. Settings show subject, SHA-256
fingerprint and expiry. Import passwords are never saved. Keys/identity PEM are
stored as AEAD ciphertext protected by the OS credential store.

Each login inherits a separate account profile snapshot and client pool, even for
accounts sharing a host. Allowed HTTPS origins include the port; base paths survive
endpoint resolution. Discovery cannot expand permission. All redirects are blocked.
The same client identity covers config, prelogin, token/2FA, profile and explicit
probe/refresh, authenticated notification upgrades, revision polling, full read-sync, item POST/PUT (including archive/unarchive and soft delete/restore), attachment metadata/
multipart transfers, item organization sharing/collection PUTs and folder POST/PUT/DELETE requests. Signed attachment file
GETs use the same TLS/mTLS identity without an OAuth bearer token. Only the
account base origin and exact attachment path are accepted, including subpaths;
external storage and redirects are rejected. Downloads recheck the transport
revision after receiving bytes and before publishing authenticated plaintext. SDK API and Identity receive the same scoped policy through
the small vendored builder patch. An expired identity fails before a request, even
on a reusable connection. Replacing a profile invalidates its old transports by
revision. The explicit probe builds the replacement transport immediately; existing
SDK clients remain denied until restart. The encrypted icon cache is bound to the
account profile UUID/revision and never bypasses source revocation or initiates
a fallback request to a different icon server. Offline key unlock remains possible when
keyring/certificate recovery is needed; no anonymous network fallback is allowed.

Notification connections use HTTP/1.1 upgrades through the same transport builder,
including client identity, private CA, no redirects and allowed-origin checks.
The access token is only in the authorization header. The upgrade response is
validated before handing the existing TLS stream to the WebSocket codec; no
second TLS stack or default WebSocket trust store is used. Live sockets recheck
certificate expiry and profile epoch every 100 ms and before accepting data, and
cancel on lock/logout. Replacement reconnects load the new protected identity.
Synthetic mandatory-mTLS tests verify the actual peer fingerprint, replacement by
a different identity, base-path retention, header auth, reconnect, rejected upgrades
and redirects, and focus/manual-check fallback. Windows/macOS still require platform testing.

Known remaining acceptance work: broader online CRUD coverage, chain policy beyond expiry/signature/key-usage and TLS backend validation, multi-identity selection (currently
rejected), reference storage deduplication by fingerprint, comprehensive Windows ACL
and macOS/Windows platform validation. Old unreferenced identity ciphertext/keys are
removed by startup orphan collection. This scope does not claim full Electron parity.

TLS diagnostics inspect typed rustls errors through the reqwest/io error chain.
Server certificate expiry (including an expired chain certificate), future validity,
unknown issuer, hostname mismatch and revocation get specific messages. Peer TLS
alerts are described as rejection of the client identity, keeping that distinct
from local validation of the server. Ambiguous failures remain explicitly unknown;
raw error strings, URLs and headers are never copied into the UI or used to guess
expiry. Login errors offer a direct button to server/mTLS settings. Regression
coverage includes an HTTPS server with an already-expired certificate: app HTTP
and SDK Identity both report expiry, and no HTTP request reaches the server.

## Behaviors carried over from iOS

| iOS source at f55dc0d | Required native behavior |
| --- | --- |
| [Certificate service](https://github.com/Der-Jan/bitwarden-ios/blob/f55dc0d5f88c14d93d81963290746aff9aa49e87/BitwardenShared/Core/Platform/Services/ClientCertificateService.swift) | Import PKCS#12; identify the leaf by SHA-256 fingerprint; reject expired and not-yet-valid imports; replace only the captured target account; keep an identity referenced elsewhere. |
| [Account security processor](https://github.com/Der-Jan/bitwarden-ios/blob/f55dc0d5f88c14d93d81963290746aff9aa49e87/BitwardenShared/UI/Platform/Settings/Settings/AccountSecurity/AccountSecurityProcessor+ClientCertificate.swift) | Show certificate metadata and replacement while offline; clear pending import data on dismissal; never retarget an import to whichever account is active when it finishes. |
| [Vault refresh](https://github.com/Der-Jan/bitwarden-ios/blob/f55dc0d5f88c14d93d81963290746aff9aa49e87/BitwardenShared/UI/Vault/Vault/VaultList/VaultListProcessor.swift) | Check expiry on explicit refresh even if sync is throttled or an existing TLS connection still works; preserve cached vault rows on failure. |
| [Recovery navigation](https://github.com/Der-Jan/bitwarden-ios/blob/f55dc0d5f88c14d93d81963290746aff9aa49e87/BitwardenShared/UI/Vault/Vault/VaultCoordinator.swift) | Offer a direct route to certificate management from expiry errors instead of a generic sync failure. |

Carry over the service tests for wrong password, future/expired certificates,
replacement rollback, inactive-account replacement, deleted-account races and
shared-reference cleanup. Use an injectable clock and treat `now >= notAfter`
as expired. Server acceptance of a client identity is distinct from validating
the server certificate; do not add a client-bundle CA to server trust.

There is an important boundary to strengthen rather than copy: the inspected
[iOS HTTP delegate](https://github.com/Der-Jan/bitwarden-ios/blob/f55dc0d5f88c14d93d81963290746aff9aa49e87/BitwardenShared/Core/Platform/Services/CertificateHTTPClient.swift)
obtains an identity from the active environment without checking the challenged
host/port in that delegate. It also blocks automatic 302 redirects specifically,
while other redirect statuses return the new request. That file does not
establish endpoint isolation or complete WSS coverage. Native transport must
enforce the following policy for every path and redirect status.

## Transport architecture and SDK gate

Introduce a transport policy/factory service and an identity store behind the SDK
coordinator. No view or generic URL helper receives a private key or an unrestricted
authenticated HTTP client. Public UI messages carry identity IDs and metadata.
Secret material stays in the import/store/transport layers.

The logical factory input is `(account or pending profile, service role, canonical
endpoint, configuration revision, transport generation)`. Roles distinguish
configuration, Identity, API, notifications, attachments, and anonymous icons or
external downloads. Credential authorization depends on both role and endpoint:
permission to send a client certificate does not grant permission to send a vault
bearer token. Separate pools/configurations also apply across accounts using the
same certificate, because tokens and cookies remain account-specific.

Prove at the SDK pin that this factory controls every internal HTTP client,
including clients created during load/rehydration, refresh and retries. If the
builder does not expose it, implement a small pinned SDK extension with tests.
Middleware operating after connection setup cannot supply a missing TLS identity.
An app-created reqwest client does not establish coverage of SDK-created clients.

Use a compatible Rust TLS stack for HTTP and WSS and choose crypto provider,
root loading and PKCS#12 importer together with the SDK dependency graph. PKCS#12
parsing is a separate import concern; do not presume rustls parses it. Select a
maintained parser after testing real synthetic bundle variants and supported key
algorithms. Use a standard X.509 parser for dates, rather than porting iOS's
older-platform DER fallback.

Keep server verification and hostname validation enabled. Do not implement
accept-all verification, downgrade to HTTP, or use a local TLS-terminating proxy
as a production substitute. TLS session resumption state must not cross identity
or server-trust configurations; rustls documents this restriction in
[ClientConfig](https://docs.rs/rustls/0.23.34/rustls/client/struct.ClientConfig.html#sharing-resumption-between-clientconfigs).

## Endpoint and credential routing

- Canonicalize DNS/IDNA names and effective ports; reject credentials, wildcards
  and malformed endpoints. Match the exact TLS host and port, not a suffix, path,
  base domain, resolved IP address, or DNS CNAME. `wss://host:443` and
  `https://host:443` identify the same TLS endpoint but retain separate service roles.
- Allow identity presentation only to explicitly approved endpoints for the
  captured account/profile. API, Identity, notifications and file services may
  each need a different binding. A discovery response cannot add one silently.
- Disable automatic redirects on credential-bearing clients for all 3xx statuses.
  If a supported flow follows a redirect, validate its role and destination,
  select a new transport and independently authorize any headers/body. Never
  forward password-derived credentials, cookies or bearer tokens automatically.
- Anonymous icons, vault item URLs, SSO browser launches and presigned external
  downloads receive no vault token or mTLS identity by default. An explicitly
  configured attachment endpoint may have its own scoped policy.
- Use immutable account context, not the current active account, for background
  sync, refresh and TLS challenges. Avoid cross-account cookie jars and TLS pools.
- Honor normal proxy configuration through the same policy and retain end-server
  TLS verification through CONNECT. Proxy authentication and proxy trust do not
  authorize the proxy as a vault endpoint. Test system/custom roots explicitly.

The base-domain helper used for sorting must never be used for TLS authorization.
Two subdomains that sort together remain distinct credential destinations.

## Import and secret storage

Allow import before login from the self-hosted server screen, and replacement
from account security or the connection error screen. Display label, public
fingerprint, validity dates, target account/profile and approved endpoints.
The user selects a file and supplies its import password; never persist that
password. Bound input size (initial proposal: 10 MiB), parser work and temporary
buffer lifetime. Avoid private-key temporary files and command-line passwords.

Validate bundle decryption, presence of a matching private key, supported signing
algorithm, coherent chain and validity dates. If multiple keyed identities exist,
require explicit selection or reject with an explanation; do not choose an
arbitrary first bag. Check key usage/EKU compatibility where present, but do not
require a locally trusted issuer for the client leaf: server policy decides
whether to accept it, including a self-signed identity.

Keep public metadata/bindings in the private application directory and private
keys in the OS secret store, or authenticated encrypted storage under a random
OS-protected key. Validate binary storage and size limits on each platform. A
locked/unavailable keyring produces a retryable pre-network error. An explicit
session-only import is permissible; plaintext durable fallback is not. No NSS
database or host browser certificate store is required by this design.

Importing a certificate chain must not change server trust. Private server CAs
are configured separately and scoped to the intended profiles; do not globally
trust roots embedded in a PKCS#12 file. Do not infer client revocation status from
dates: issuer/server revocation policy remains a separate concern.

## Atomic replacement and recovery

1. Capture target account incarnation/profile revision. Parse and validate the
   candidate without altering live bindings. Account deletion cancels the draft.
2. Store the candidate secret, prepare all required clients, and validate local
   usability. An optional server probe can help but must not make offline
   replacement impossible or claim the server will accept the identity.
3. Commit the binding with a revision check and a recovery journal. Publish the
   new transport generation only when the durable configuration and clients are
   ready. On preparation/commit failure keep the previous binding active.
4. Stop new work on old clients, cancel/drain requests, close WSS, and retire old
   pools and TLS sessions. If SDK rehydration requires it, lock the affected
   account and rebuild its client; do not require a whole-app restart.
5. Reconcile mutations with uncertain remote outcomes before retrying. Retry
   eligible reads under the new generation. Old results cannot commit state.
6. Remove the old secret only after all account/profile/draft references are
   checked. If reference lookup fails, retain it and schedule cleanup. A cleanup
   failure after successful activation is reported separately, not as a failed
   replacement. On restart reconcile journaled partial operations safely.

Apply the same reference and generation rules to unbinding/removal. Explicit
deletion reports keyring failures and closes in-memory transports holding that
identity. Distinguish local removal from revocation at the issuing authority.

Check validity at import, before new authenticated operations and reconnects,
on resume, and on explicit refresh before throttling. Use a timer to retire
connections when the active certificate expires. Manual refresh must therefore
expose expiry even if a pooled connection or TLS ticket could otherwise hide it.
Background checks must not repeatedly open modals; publish one actionable status.

Report missing identity/key, keyring locked, expired/future certificate, import
password failure, server certificate trust failure and generic remote rejection
separately where the transport provides evidence. Never infer expiry merely from
an arbitrary network error. Offer replacement offline without deleting the vault
or logging the user out. Cached vault access remains governed by local unlock.

By default, vault lock retains protected transport credentials for encrypted
background sync. The mTLS key may therefore remain in memory while vault keys
are cleared. Explain this distinction in settings. An optional disconnect-on-lock
mode must close pools/WSS and release identity handles as well as stop requests.

## Acceptance matrix

| Gate | Required test |
| --- | --- |
| T1 | Config discovery, prelogin, password/2FA token exchange, refresh, API sync and online writes all pass a proxy requiring a client certificate |
| T2 | Notifications negotiate WSS and reconnect with the same scoped identity; authenticated attachment endpoints use their own policy |
| T3 | A second controlled host/port requiring a certificate receives none unless bound; cover 301/302/303/307/308, external downloads and icon requests |
| T4 | Two accounts on one host use different identities/tokens correctly during concurrent background work and rapid active-account switching |
| T5 | Wrong bundle password, key mismatch, unsupported key, malformed/oversized bundle, multiple identities, future/expired dates, missing key and locked keyring fail safely |
| T6 | Manual refresh reports expiry despite throttling or pool reuse; expiry during a request/reconnect offers recovery and preserves cached rows |
| T7 | Offline replacement, account deletion during import, revision conflict and failed store/commit retain the correct old binding; shared identities are preserved |
| T8 | Rotation retires old WSS/pools/session tickets; ambiguous writes are reconciled; crashes at each journal step recover consistently |
| T9 | Bundled CA never elevates server trust; unknown CA, wrong hostname and invalid server certificate remain rejected; private CA configuration is scoped |
| T10 | Key deletion/restart leaves no live identity use or silent plaintext fallback; secret-store failure is visible and recoverable |

Use disposable synthetic identities and an HTTPS/WSS proxy that records public
certificate fingerprints and sanitized request roles. A successful curl probe
only proves the fixture: the native SDK paths themselves must pass T1/T2. Repeat
the platform storage/proxy matrix on Linux/Flatpak, Windows and macOS before
claiming support there. Tests and device runs in the iOS branches are references,
not evidence that this native app passes these gates.
