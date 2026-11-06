# Clipboard session consent and delivery ordering

This records the phase-16 RemoteDesktop checkpoint.
[ADR0062](0062-input-capture-clipboard-activation.md) extends Clipboard to actual
InputCapture activation and records newer scoped validation.

Status: private RemoteDesktop integration passes scoped acceptance, 2026-11-01.
Portal 172 library/48 integration tests, formatting, Clippy and 62 native groups
pass. Public Clipboard consent/transfers, independent revocation and existing
input/media regressions pass in both appearances on the final candidate. Normal
registration and advertisement remain disabled. This continues
[ADR0057](0057-session-clipboard-transport.md).

Clipboard extends the existing RemoteDesktop session registry and actor. Each
method resolves the stored session handle and authenticated original frontend
unique name. Unknown, closed, foreign and ScreenCast-only sessions fail.
RequestClipboard is admitted once while configuring; subsequent data operations
require the actual active clipboard grant. Application attribution comes from
session creation rather than a new caller-supplied identity.

RemoteDesktop Start passes the request into an independently unchecked Iced
choice. Alt+C toggles that choice only when requested; ordinary typing and initial
Enter/Ctrl+Enter cannot grant access. Clipboard-only consent needs no keyboard,
pointer or recording grant. The privileged Wayland bridge opens only after
approval and compositor identity revalidation. The result reports
clipboard_enabled only when that resource exists. Ongoing controls describe the
actual granted resources and stop them together.

The standard frontend sends RequestClipboard without waiting for a backend reply.
The Clipboard interface therefore admits work synchronously on the object-server
receive loop, then awaits resource acknowledgements in separate tasks. This
preserves request-before-Start ordering without blocking unrelated dispatch on a
clipboard operation. The existing bounded session queue uses finite admission;
saturation closes only the authenticated owning session. Metadata and FD/serial
validation retain the transport limits from ADR0057.

The frontend discards initial clipboard signals before it has processed a
successful Start result. RemoteDesktop dispatch queues a ClipboardReady message
after the typed Start response has been dispatched. It carries the original
message serial, which the session actor matches against its successful Start.
Failed concurrent starts cannot release an earlier session's initial metadata.
The actor relays the current snapshot and later events only after this barrier.
Signals target the original frontend unique owner; clipboard metadata is not
broadcast. Wire tests check reply/signal order, and the real public frontend must
receive the clipboard contents already present before Start.

Closure, frontend loss and resource loss interrupt pending clipboard operations
and signal sends. Signal delivery also has a finite timeout. A separate grant
monitor can revoke the raw bridge without awaiting actor progress, so transferred
reader/writer pipes lose authority even while another operation is pending.
The raw native test retains both application FD duplicates and the Bridge owner
while invoking this revoker, then checks EOF and EPIPE before dropping the owner.
Cleanup stops resources before reporting the implementation session closed.

Five focused integration cases exercise authenticated ownership, kind/phase and
consent checks, actual FD direction/replay, directed signal ordering, no-reply
RequestClipboard ordering, cancellation during blocked operations and bounded
queue saturation. The maintained public fixture uses real Iced consent and
independent wl-copy/wl-paste peers on private compositor and frontend instances.
Exact outcomes, binary/source pins and earlier fixture failures belong to
[the Clipboard ledger](../../.scratch/xdg-portals/clipboard/README.md).

InputCapture has no implementation yet. It must use real capture/barrier/EI
receiver capabilities, its own session lifecycle and active-phase transfer
policy. Registering a synthetic session kind or routing another backend's
sessions to this registry would not implement that contract. Regular-file
Wayland receiver compatibility, a broader failure matrix, real Flatpak use and
companion packaging/distribution also remain open. No installed routing changes
are part of this decision, and the full 18-interface goal remains active.
