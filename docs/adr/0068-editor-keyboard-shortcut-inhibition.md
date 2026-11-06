# Focus-scoped keyboard shortcut inhibition for Iced editors

Status: scoped implementation verified, 2026-11-01. The original GlobalShortcuts
application reproduces F9 activation while its configuration editor has focus.
The current normal application rebuilt by the original workspace graph passes
22 public-frontend native groups in dark/100% and light/125%, including input
suppression, retained reservations and overlapping Bind dialogs. It includes the
native window-retirement memo correction. Three additional native rounds prove
duplicate rejection and native-window retirement with retained handles that do
not revive. Portal 213 library/55 integration tests and all six existing shared
desktop suites pass. The original full workspace finishes with the same ten
Files integration failures; all other Rust targets and four post stages pass.
Keyboard recording is separate work. Exact results are under
`.scratch/xdg-portals/global-shortcuts/inhibition-evidence.json`.

The revised compositor also passes all48 existing native lease/input regression
groups:13 shortcut,10 source teardown,13 raw keyboard and12 typed keyboard.
All 16 owned test processes are terminal and guarded inputs are unchanged. The
full-workspace strict Clippy and all-target compilation pass. The final SDK2
application and dedicated lifetime probe are reconciled with the unchanged
workspace source inventory in `acceptance-01.json` beside the native receipts.

An editor must receive the keys it is configuring without releasing existing
registrations. Clearing the old set while editing would allow another session to
take those triggers and make cancellation unable to preserve the previous set.
Use the standard Wayland keyboard-shortcuts-inhibit-v1 protocol on the existing
window connection, rather than a separate connection or compositor-wide pause.

Winit owns an optional manager and one inhibitor per requested surface and
keyboard seat. It tracks actual keyboard focus as well as Active/Inactive
acknowledgements: the protocol does not send Inactive merely because focus left.
Only acknowledgement from every currently focused keyboard seat reports Active.
Pending, denied and unfocused states remain distinguishable. Seat changes create
or retire the corresponding resources; late events must match both the window's
lease generation and the exact current inhibitor object.

The returned guard shares a live status slot, without owning the native window.
Final guard drop releases its request; native window or event-loop retirement
closes retained clones too. Native-window retirement also clears focus memos for
ordinary windows that never requested an inhibitor, so a recycled native ID
cannot borrow an old focus record. State changes are stored under the hub lock;
notifications run after unlocking and only wake a bounded coalesced stream.

Iced exposes an optional ordinary-window operation and an owned state handle.
The ordinary Winit adapter maps actual permission and focus; unsupported platforms
and layer surfaces explicitly return None. Existing windows do not request
inhibition. The GlobalShortcuts editor requests it for its own lifetime; focus
loss restores normal compositor routing without discarding shortcut reservations.
The compositor may keep emergency shortcuts and the user may disable inhibition.

Niri previously used the same predicate for opening an empty trusted-backend
lease and suppressing activation. That prevented a second consent editor from
opening while another editor inhibited shortcuts. Admission now retains the
lock/compositor-UI checks but permits empty leases during shortcut inhibition.
Activation remains suspended. Existing and new registrations still participate
in conflict checks and keep their reservations. No private lease reaches the
Iced helper, and no interface version changes.

The retained controls are distinct: the old application leaks editor F9 input;
the intermediate client with the old companion fails empty-lease admission.
The revised companion passes both inhibition/reservation and concurrent-dialog
checks. The pure focused-seat policy and actual runtime change-stream tests reuse
stable source paths and existing locked dependencies. The original supervisor
ended unexpectedly while its child Cargo continued; a new observer followed the
same exact PIDs without restarting compilation. Its final failure code is inferred
from Cargo's summary, rather than claimed as an observed parent exit. All
remaining original stages pass. The no-run library lookup for the lifetime probe
reports every artifact fresh; its sole helper executable is archived losslessly
after all native processes exit. No full-workspace-green, physical-device,
sandbox or complete-goal claim is made.

Recording must additionally preserve a source-time protection epoch across
coalesced permission/focus changes, use base-layer XKB symbols and reject
unrepresentable effective modifiers. The existing Niri resolver intentionally
matches base-layer symbols and refuses unknown modifier bits. Recording a rendered
character or assuming that every Unicode/AltGr combination is a base key would
produce a misleading trigger. The local xkbcommon probe accepts Unicode names,
including Latin-1, while named modifier keys retain their XKB identifiers.

References: the [XDG shortcut grammar](https://specifications.freedesktop.org/shortcuts/latest/),
[xkbcommon symbol rules](https://xkbcommon.org/doc/current/group__keysyms.html), and
the installed keyboard-shortcuts-inhibit-v1 protocol XML used by the native stack.
