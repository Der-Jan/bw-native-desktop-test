# Native desktop architecture

The production application uses egui/eframe 0.37 with OpenGL. Iced was replaced
on the `egui-migration` branch; its checkpoint is `0945673`.

## Layers

- `crates/desktop/src/core`: framework-neutral application model. Account state,
  filters, selection, pending protected actions, profile editing, generator and
  guarded asynchronous completions live here. The bounded dispatcher wakes the
  native event loop. It never publishes a result after account/session authority
  or the relevant presentation/request identity has changed.
- `crates/desktop/src/ui`: system typography, menus, login, server/mTLS settings,
  combined navigation, a virtualized sortable table, details/forms, generator,
  settings, transfer dialogs, Send and auxiliary windows.
- `crates/desktop/src/services`: the pinned SDK4, revocable crypto sessions,
  SQLite snapshots, encrypted credentials, bounded TLS transport, clipboard,
  SSH agent, tray, OS session events, preferences, sorting/search and i18n.
- `crates/session-events`: OS lock/suspend/resume notifications.
- `tools/fake-data`: explicitly synthetic SDK-encrypted fixtures.
- `tools/packager`: native packaging. `tools/egui-demo` remains an isolated
  comparison program; production never substitutes its mock authentication.

## Event and window lifecycle

Only Core owns ClientManager. Workers receive SessionClient leases and return
completions through a bounded channel. Rendering uses cached projections and
only builds visible rows; no asynchronous SDK call or vault-wide clone occurs
in row rendering. Search/sort recompute only when their inputs or data change.

The native window loop owns an application model independently of eframe's
surface lifetime. Hiding to tray closes the surface, including on Wayland.
The UI thread then blocks on the dispatcher wake condition, processing timer,
tray, instance-wake and SSH events without an invisible window or a render loop.
Reopening creates a new surface with the same Core. Hidden SSH requests can
open only the approval surface; resolving it returns to the tray daemon.

List focus is independent of row virtualization. Arrow and page keys navigate
only from that focus target. Text fields retain their own keyboard behavior.
Escape cancels a modal first or closes details while preserving the list cursor.
There are no application fade or shadow animations.

Desktop appearance updates reach Core through an asynchronous stream. Linux
reads effective KDE window, view, button, text and interaction color roles through
the host portal, with a local color-scheme fallback. This also works with a
separate application profile/XDG_CONFIG_HOME. In System mode the actual KDE
palette determines light/dark mode; explicit Light/Dark overrides remain available.
Widget cleanup retains appearance/font options while discarding input, undo and
layout state. SVG branding retains its original colors.

## Security boundaries

Lock, logout, account switch and sync authority replacement revoke old results.
A presentation epoch also discards frontend inputs, drafts, widget undo state
and font-layout caches. Password confirmation owns the precise pending action;
its UUID and cipher revision must still match when verification completes.
Native TextEdit copy commands are intercepted and use the same expiring,
owned clipboard as explicit copy buttons. Existing-item editor copy also needs
backend Copy authority. Exports retain private staging and final authority checks.

Server profiles, certificates and private CAs retain their pre-migration storage
format. TLS uses exact allowed origins and checks certificate validity. The SSH
agent retains its fixed private socket, verified host binding, host-scoped
until-lock grants and explicit UUID-bound approval decisions.

See [security concept](security-concept.md), [mTLS](mtls-requirements.md),
[login/sync](login-sync-requirements.md), and [SSH](ssh-agent-requirements.md).

## Local edits before write synchronization

Existing synchronized entries can be edited locally. The SDK encrypts each
change before a single SQLite snapshot-setting transaction commits it. Snapshot
version 2 keeps the original server response plus an encrypted per-item overlay
and its original server revision. Version 1 snapshots remain readable. SDK
repositories still reject arbitrary writes; only the guarded local-edit service
can publish an overlay after durable storage succeeds.

Local saves and discards share the sync coordinator and take an exclusive crypto
lease. Every save rechecks current edit/view permissions, draft revision and any
one-shot master-password Write grant. Ownership, collections, keys, attachments,
type, permissions and lifecycle timestamps are immutable in this milestone;
folder changes must target a cached folder. A new local revision invalidates
stale drafts and approval grants. Organization entries retain their ownership and
use the organization's existing key.

No HTTP writes are made. Read-sync is paused in both Core and the backend while
any local edits exist, avoiding accidental replacement by a fresh download.
Per-item discard restores the last downloaded encrypted version and permits
read-sync again once the last change is discarded. Lock/restart preserve local
changes; logout deliberately removes the local account and its unsent changes.
The UI states these limits. Server creation/deletion and ownership changes remain
disabled. A future write-sync must add server revision/conflict checks and server
permission revalidation before releasing this download pause.
