# Browser integration on Linux

The first target is Helium. Settings → Browser integration provides opt-in
registration for Helium, Chrome, Chromium and Firefox. No registration is made
on application startup or during a build. Vivaldi and Brave are not registration
targets in this milestone.

## Trying it

1. Unlock the desired account in the desktop application.
2. Enable **Allow browser unlock from this desktop app**, select **Helium**,
   and press **Register selected browser**.
3. Sign into the official Bitwarden extension using the same account and server.
   Enable its **Unlock with biometrics** setting. The extension's terminology
   is inherited from its native messaging protocol: this application asks for
   desktop approval and performs no biometric or Polkit verification.
4. Lock the extension and request an unlock. The desktop opens a focused native
   approval window. **Always allow** is selected by default; Enter confirms it.
   Escape or closing denies the request. The duration selector also offers
   **Just this once** and **Until desktop locks**.

Desktop unlock does not perform the extension's initial login, synchronize its
cache or configure its server/mTLS transport. The extension still manages those
independently. The desktop must already be unlocked; a locked desktop is not
unlocked by a browser request. This is shared key access, not server SSO.

Use **Restore previous browser registration** to disable integration and restore
the previous native host manifest. Existing official desktop registrations are
backed up before replacement. If another application has subsequently changed
the manifest, restoration leaves it alone and retains the backup.

## Installation and transport

The package includes `bitwarden-browser-proxy` beside the GUI executable. A
private launcher in the application data directory selects this executable,
the exact GUI executable and a deterministic profile-specific Unix socket.
For Helium the manifest is normally installed in
`~/.config/net.imput.helium/NativeMessagingHosts/com.8bit.bitwarden.json`.
Chromium configuration follows `XDG_CONFIG_HOME`; a separate desktop preview
can set `BITWARDEN_BROWSER_CONFIG_HOME` to the real browser configuration root.
Firefox uses `~/.mozilla/native-messaging-hosts`.

The manifest allows only official Bitwarden extension IDs. The proxy checks
the browser-supplied origin, the parent browser executable and the GUI socket
peer. The GUI checks the proxy executable, browser parent and peer UID. Browser
executables must be root-owned and not group/world writable; portable user-owned
builds and sandboxed browser packages are not supported by this check. The
socket directory is private (0700), its socket is 0600, and a live foreign
listener is never replaced. Native frames, peers, concurrent requests, request
rates and approval durations are bounded.

Both native protocols are supported: the extension's legacy RSA-OAEP/SHA1 plus
AES-CBC/HMAC channel and the pinned Bitwarden SDK's Noise/CBOR IPC. Legacy
messages require authenticated encryption, fresh timestamps and unique request
IDs. SDK Noise is upstream code, exposed through the small native export patch
documented in `vendor/bitwarden-ipc/NATIVE-EXPORTS.md`; no custom Noise protocol
is implemented.

Legacy replies use the native messaging `EncString` object representation
(`encryptedString`, `encryptionType`, `iv`, `data`, `mac`). Sending only the
`toJSON()` string breaks the official browser's decrypt service and leaves its
status/unlock callback waiting until timeout. The interoperability test requires
the object and checks its fields before SDK decryption.

## Key release and limits

Only the selected account's user key is released, after an explicit current
approval or a saved permanent pairing, plus backend session checks. The master password and OAuth tokens are
never sent. The request must use the server account UUID, which can differ from
the local account ID. Lock, logout, account switch, session replacement, disable
and disconnect revoke the connection and its until-lock permission. Approval is
bound to a connection, session and UUID and expires after 50 seconds.

**Always allow** saves a pairing bound to the canonical browser executable path,
exact extension origin, local account ID, server account UUID and desktop server
URL. It survives reconnect, lock/unlock and application restart; the desktop must
still be unlocked, and all live session fences remain enforced. It applies to
that browser's extension across profiles using the same account. The private
`browser-host/trusted-browsers.json` stores identity digests only, never keys or
passwords. A failed durable save denies key release. Invalid/oversized storage
loads without permissions. **Forget saved approvals** in Settings clears all
saved pairings and disconnects live peers so even until-lock grants stop working.
A storage failure during revocation is reported and must be resolved to make the
removal durable across restart.


The approval identifies the browser, extension, desktop email and server. The
biometric RPC identifies an account UUID but does not attest the browser's
configured server URL: the user must check that both accounts use the same
server. Process/origin checks do not protect against an attacker who already
controls the user's desktop session or the approved extension. Approval grants
the extension the ability to decrypt that account's entire cached vault.

Revocation prevents further key releases; it cannot retract a key already
received by the extension. Browser auto-lock remains the extension's policy.
The newer `SharedUnlockSync` state propagation and OS user-verification RPCs
are not implemented. An extension rollout that requires those exclusively needs
additional integration; no successful biometric verification is fabricated.

## Validation

Workspace tests cover actual SDK Noise interoperation, legacy SDK encryption
interoperation, approval/denial, incorrect accounts, expired/replayed/tampered
requests, lock/session/disconnect revocation, socket permissions, framing limits,
and registration backup/restoration. A disposable TLS account verifies that
local and server account IDs remain distinct.

A native test used the installed Helium executable with a separate disposable
profile and a synthetic extension exercising the official native protocol. It
verified Enter approval, Escape denial, until-lock reuse and browser disconnect
on desktop lock. No real vault or browser storage was used. This validates the
transport and approval flow; the installed official extension's settings flow
still needs an interactive acceptance check.

A follow-up test also loaded the public WASM SDK and JavaScript bindings from
the installed official extension version 2026.10.2 into the disposable test
extension. Discover, status, approval, denial, remembered approval and lock
revocation pass against this actual browser SDK as well.

The native test passed with EGL under Xvfb (GLX disabled). Raw Xvfb with GLX
failed when repeatedly recreating child windows with `GLXBadWindow`; this is an
unresolved X11/GLX renderer limitation, not a passed acceptance check. The actual
KDE preview uses Wayland/EGL.
