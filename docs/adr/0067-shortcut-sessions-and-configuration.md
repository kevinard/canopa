# Session-owned global shortcuts and Iced configuration

Status: experimental implementation with scoped verification, 2026-11-01. The backend
exports GlobalShortcuts v2, but the packaged descriptor does not advertise it.
Portal checks pass 213 library tests (eight ignored), 55 integration (two ignored),
formatting and strict all-target Clippy. Thirty-four shortcut checks and eighteen
public native groups pass in dark/100% and light/125%, with all ten Iced helpers
terminal and no inherited backend/compositor sockets. The exact same native
fixture reproduces an Escape handling failure on the archived initial binary and
passes after the correction. Exact results and the one nonsemantic post-build
formatting change are in `.scratch/xdg-portals/global-shortcuts/session-evidence.json`.

The standard frontend authenticates applications and owns public requests and
sessions. The Rust backend stores its original unique caller and application ID
when creating each session. Bind, List and Configure have no application-ID
argument; they use that stored identity. A foreign caller cannot operate or close
the session. Original caller loss and compositor loss revoke the owned socket;
reacquiring either service name never revives the old session.

GlobalShortcuts uses the compositor lease from ADR0066 through a bounded Rust
transport. Its sole lease owner is the session actor. UI processes and retained
command handles never own the privileged descriptor. The actor observes closure
and compositor events while awaiting consent, mutation replies and storage.
There are at most sixteen sessions and one editor per session, with bounded
request, command, feedback and event queues.

Bind is a once-only attempt, including cancellation and invalid action definitions.
Malformed D-Bus signatures and unexpected descriptors are rejected before
admission. Application action IDs remain stable; the backend assigns private
numeric IDs within an immutable action set. Original descriptions and readable
trigger descriptions accompany only the actual admitted set. List before Bind
reads earlier successful choices; after an attempt it reports the current live
session set, including empty after cancellation or failure. Successful empty
approval permits subsequent Configure, just like any other successful Bind.

Iced presents every Bind action unchecked, with saved or preferred triggers as
editable suggestions. Configure begins with the current bindings checked.
Apply is the explicit decision authorizing an atomic, generation-checked complete
replacement. A refused mutation leaves the old live set and the same editor
available for correction. Feedback refreshes the observed generation without
discarding the draft. Cancel before Apply preserves an existing set. Escape must
reach the dialog even when its text field has captured that event.

After Apply, an ambiguous transport result or a persistence failure revokes the
session instead of leaving an unaccounted registration. Dropped Bind replies
also revoke provisional authority. Configure is a v2 method with an empty reply,
without a Request handle; duplicate concurrent configuration is rejected.
Closing a helper after a confirmed Apply does not undo the authorized mutation.

Only explicit successful choices are persisted, in the private bounded SQLite
file `$XDG_STATE_HOME/canopa/portal-shortcuts.db` (default
`$HOME/.local/state/canopa/portal-shortcuts.db`). It stores application/action IDs,
descriptions and selected triggers, never live leases or private generations.
Missing history is read without creating files. Transactions replace one app's
choices; the file retains at most 128 apps and 64 actions per app. Interrupted
first initialization can recover without deleting the database. File ownership,
link identity, size, permissions and schema are checked. Historical choices never
install authority without fresh consent.

Activated and Deactivated carry the original compositor's u64 millisecond event
time and are directed only to the original frontend owner. ShortcutsChanged
follows ordered compositor changes. No activation token is fabricated; Configure
can consume the frontend's optional token to activate its editor. There is no
promise that every signal waits for the public frontend's Request.Response.

The initial editor uses explicit text entry for XDG trigger names. Keyboard
recording, focused-editor shortcut inhibition and broader physical/layout,
sandbox, failure and distribution acceptance remain open. Normal routing stays
on the shipped descriptor until those acceptance decisions are made.

Contract: [backend v2 XML](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/data/org.freedesktop.impl.portal.GlobalShortcuts.xml),
[public v2 XML](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/data/org.freedesktop.portal.GlobalShortcuts.xml).
