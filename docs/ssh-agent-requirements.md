# SSH agent

The production frontend now uses egui. The SSH backend and verified host-binding
policy are retained; see [current native acceptance checks](egui-migration.md).
Earlier Iced acceptance notes below describe the preserved checkpoint.

Implemented for Unix sockets, validated on Linux. Windows named pipes are not
implemented; that platform shows an explicit unavailable status. This is an
application service, not a facility supplied by the Bitwarden SDK. The SDK provides
key import/normalization and the revocable access to encrypted vault items.

## Enable and connect

Enable **Settings → Integrations → SSH agent**. The section shows the live status
and buttons to copy the socket path or a ready-to-use `SSH_AUTH_SOCK` export.
The Linux default is fixed across launches, account switches and profiles:

```sh
export SSH_AUTH_SOCK="$XDG_RUNTIME_DIR/bitwarden-desktop-next/ssh-agent.sock"
ssh-add -L
ssh user@host
```

Put the export in your shell startup file (for example `~/.zshrc`) to use it in
future shells. The desktop app cannot change an already-running shell's environment.
Alternatively, restrict the agent to selected hosts in `~/.ssh/config`, using the
literal socket path shown by the app:

```sshconfig
Host example-server
    IdentityAgent /run/user/1000/bitwarden-desktop-next/ssh-agent.sock
```

Replace the UID/path with the displayed value. `IdentityAgent` avoids changing the
agent used by other programs. Without `XDG_RUNTIME_DIR`, Unix uses a private
`bw-native-ssh-<uid>/ssh-agent.sock` directory under the system temporary directory.
No profile hash or process/random ID is appended to the socket name.

For a separate concurrent profile, launch it with an explicit absolute
`BITWARDEN_SSH_AUTH_SOCK` path. Its parent must satisfy the same ownership/0700
checks. The variable controls where this app listens; it does not set the caller's
`SSH_AUTH_SOCK`. Existing files/listeners are never replaced. Only one profile can
own the default endpoint at a time; a conflicting instance reports the occupied
path rather than taking over another profile's agent.

No shell configuration, SSH configuration or existing agent socket is changed
automatically. The listener remains opt-in and available while the app is in the tray.
Only the **active unlocked account** supplies keys, independently
of the current sidebar filter or search. Locked accounts list no identities.

The default asks for approval with **Allow once** as the initial focused action.
A verified host-bound request also offers **Allow until vault locks** directly.
The prompt-behavior preference chooses the initial action, rather than hiding the
explicit session grant: **Ask (default: allow until lock)** focuses that action only
when the request has both proofs; otherwise it still defaults to **Allow once**.
The dialog shows the vault item, public-key
fingerprint, SSH username, local process information and the verified server-key
fingerprints if available. Process names/PIDs are informational, not trusted
application identity. Unbound requests are explicitly labelled and never remembered.
The native dialog opens as a separate window above other windows, retaining the
vault's previous visibility. Enter/Space activate its focused button; Tab and
Shift+Tab stay within its actions; Escape and native/custom close deny. A visible
focus ring and return glyph identify the focused/default action. Key-repeat and
modified Enter cannot approve, and keyboard messages retain the request UUID.
The dialog expires after 45 seconds. Dismiss, Escape, hiding the window, disconnect,
lock, logout, account switch, sync generation change and disabling the service
prevent a pending signature from being returned.

**Allow until vault locks** is offered only with cryptographically verified OpenSSH
session bindings **and** a host-bound authentication payload covering the terminal
host key, and is scoped to item ID, revision, public key, SSH username and
entire verified host-key forwarding path. Authority/mode changes clear grants.
**Never ask** deliberately allows local same-user processes to authenticate with
eligible keys while the active account is unlocked; the settings section explains
this effect. It does not bypass vault permissions or reprompt exclusions.

## Key and protocol boundary

- Ed25519, ECDSA P-256/P-384/P-521 and RSA 2048–8192-bit public keys. The SDK imports
  unencrypted OpenSSH/PKCS8 material; PKCS1 RSA PEM is normalized in memory first.
  RSA signatures require SHA-256 or SHA-512; SHA-1 is rejected.
- Only active SSH items with `view_password` and no master-password reprompt are
  exposed. Archived/deleted, protected and hidden-password items are excluded.
  Supporting protected items requires an explicit additional reprompt flow.
- Private key parsing and signing stay inside the SDK session lease. No private-key
  cache, exported key file or secret-bearing UI message is used. Temporary string
  buffers under application control are zeroized; third-party importer allocations
  and operating-system memory are outside that guarantee.
- Signing accepts the exact SSH user-authentication payload, with consistent key,
  algorithm, username and session ID. Plain `publickey` requests never reuse or
  create remembered grants, even on a connection with a verified binding. Generic
  signing, Git/SSHSIG signing,
  ssh-add mutation, key removal and agent control requests fail closed.
- `session-bind@openssh.com` verifies the server's signature over the session ID,
  bounds forwarding chains, rejects conflicting/reordered bindings and requires
  the terminal binding to authenticate rather than forward. A failed binding
  poisons subsequent sign requests on that connection. Server hostnames are not
  inferred from an untrusted process argument or client-supplied label.
- The item revision, permissions, deletion/archive status, reprompt policy and
  private/public correspondence are rechecked after approval. Session generation
  is checked again before publishing a signature.

## Local transport and resource limits

Use an owned private runtime directory (0700 or stricter), preferring
`XDG_RUNTIME_DIR`, otherwise a per-user private directory under the system temporary
directory. The socket is 0600 and accepts only the effective user's peer UID.
Existing files, sockets and symlinks are never overwritten, including stale sockets.
If a previous crash left the displayed socket path, verify no app instance owns it
before removing it manually. Normal shutdown removes only the exact socket inode
created by this instance; replacement files are preserved.

Limits: 4 connections, 64 KiB packets, 4 queued packets per connection, 256 packets
per connection, 64 advertised keys, 16 bindings, 128 remembered grants, one pending
approval. Header and body share an absolute 60-second read deadline; writes have a
5-second deadline. A request that disconnects while approval is pending is cancelled.

Same-user malware can talk to a Unix agent socket: socket permissions are not a
sandbox against processes running as the same user. Approvals and host binding
reduce accidental or unintended use; **Never ask** explicitly relaxes approvals.
The pinned winit Wayland backend does not implement focusing an existing surface.
Opening a dedicated native prompt is used instead; KDE/Wayland activation was
verified with a synthetic native-window harness. Other compositor policies may
still limit activation; requesting attention alone is not evidence of focus.

## References

The security/lifecycle model follows the recent
[Prizm SSH-agent branch, pinned at 980eafdb](https://github.com/Der-Jan/prizm/tree/980eafdbf50765cede118639ad01179cb83ae915/Prizm/Data/SSH)
and its [security review](https://github.com/Der-Jan/prizm/blob/980eafdbf50765cede118639ad01179cb83ae915/openspec/changes/ssh-agent/security-review.md).
The stronger reusable-approval rule also follows
[WeTTY's forwarding implementation, pinned at f1ace07f](https://github.com/Der-Jan/wetty/blob/f1ace07f0c80f8d13f2d8377ba948ab08c4ab98a/docs/browser-agent-forwarding.md).
The protocol extension is documented by
[OpenSSH PROTOCOL.agent](https://github.com/openssh/openssh-portable/blob/master/PROTOCOL.agent).
This implementation does not copy Prizm's Swift runtime or rely on the unfinished
Electron V2 agent. Cross-platform parity, generic/Git signing and protected-key
reprompt remain separate work.


## Validation on 2026-11-07

The workspace suite passes (274 desktop tests and one fixture-generator test).
Two environment-dependent checks were run separately: registration with the live
Linux StatusNotifier host and a complete OpenSSH client → local `sshd` login using
the agent, verified session binding and a one-shot approval. Unit/integration checks
also cover signature verification across supported key families, RSA SHA-1 rejection,
public/private mismatch, malformed/hidden/protected/archived/deleted items, stale
revision and lock rejection, grant scoping, timeout/disconnect cancellation, packet
bounds/deadlines and socket permissions/conflicts/owned-inode cleanup.

A disposable synthetic-account Iced GUI profile was used to verify restored filters
and collapsed groups after restart, settings/status, key listing, the approval dialog
and a returned signature, closing to the tray and restoration through SNI activation.
The production preview profile was not used for these tests. macOS and Windows were
not exercised. Clippy introduces no additional warnings beyond the three existing
menu/opacity warnings.

The fixed endpoint was also exercised with `ssh-add -L` in a disposable GUI profile.
The copied shell command was checked against the actual clipboard, including a
separate shell-quoting test for spaces, apostrophes and metacharacters. Normal Quit
removed the socket; conflicting listeners and relative override paths fail closed.

If both the SSH item list and `ssh-add -L` are empty despite keys on Vaultwarden,
verify a fresh sync with the explicit `Bitwarden-Client-Version` header. Older
versions of this app omitted it, so Vaultwarden excluded SSH items before they
ever reached the agent. See [protocol negotiation](login-sync-requirements.md#protocol-version-and-ssh-item-visibility).


## Approval focus and reusable-grant validation

Native KDE/Wayland window tests distinguished a focus/attention request on an
existing surface from creation of a new always-on-top prompt: the existing surface
did not gain focus, while the new prompt did. A second test used the production app
ID and a separate foreground test-terminal process; the new prompt took focus
from that process without changing compositor settings. The Xvfb full-app journey explicitly
focused the native prompt (Xvfb has no window manager), then verified Enter → one
signature, Tab/Enter → an explicit until-lock grant reused by a fresh independently
bound socket, and Tab/Enter → denial. Simulator checks cover the request UUID,
repeated/modified Enter and scoped Tab navigation. Lifecycle checks cover native
close, lock, late open after cancellation, stale window close and stale dismiss.

The opt-in real OpenSSH/sshd test forces host-bound authentication, grants until
lock, and succeeds again on a fresh SSH connection without another approval. A
verified session binding with a plain publickey payload cannot reuse that grant.
