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
