# Session-owned clipboard transport

Status: Rust transport passes scoped acceptance, 2026-11-01. Seven new pipe/ledger
unit tests and eight real-compositor groups pass. The complete Portal library
passes 170 tests and integration passes 43, with formatting and Clippy clean.
Clipboard consent, backend methods, session integration and normal advertisement
remain pending. This does not claim a working Clipboard portal.

This is the transport-stage checkpoint. [ADR0058](0058-clipboard-session-consent-and-ordering.md)
records the later private RemoteDesktop API, Iced consent and ordering work;
its current validation status is tracked separately in the Clipboard ledger.

The Clipboard backend extends consented RemoteDesktop and InputCapture sessions;
it cannot grant a ScreenCast-only session or create its own session. RequestClipboard
records a request before Start. The owning session must obtain an independent Iced
choice, return clipboard_enabled, and deny all access after revocation. InputCapture
also gates access on active capture. The standard frontend retains application
attribution and public policy; backend methods must still validate their authenticated
frontend owner and session kind/phase. The pinned contract and complete state/FD
requirements are in the InputCapture/Clipboard primary-source research report.

Niri already exposes ext-data-control to ordinary unsandboxed clients. This is
broad privilege, not portal consent. A private Rust Bridge will belong to one
approved session and retain its own named Wayland connection, pinned by Unix
peer credentials to the compositor PID already verified by the owning portal.
It never consumes WAYLAND_SOCKET or returns the privileged connection to an app.
It binds one unambiguous seat, exposes only the regular clipboard, and terminates
on disconnect or removal of its original seat/manager.

One worker owns all protocol objects, current source/offer generations, transfer
serials and stream pumps. A watch snapshot contains bounded MIME metadata and
correlated ownership. An unpredictable private source marker plus source
cancellation identifies the bridge's own offer; the marker is filtered from
session-visible formats. Sources are single-use and stale read generations fail.
Source replacement cancels both reads and writes, including pending serials.

SelectionRead returns an app-readable pipe. SelectionWrite consumes one issued
serial and returns an app-writable pipe; repeated/foreign/expired serials fail.
The worker retains a pump between each handed-out FD and the actual source/peer,
so closing the session also revokes app-held duplicates. This cannot recall bytes
already delivered. Failure completion cancels the pump. Success completion
requires a claimed writer and EOF before the destination receives EOF; it does
not turn an already streaming payload into an atomic transaction.

Buffers are fixed at 16 KiB per pump, with at most 16 total transfers and a 16-command
queue. MIME metadata is bounded to 128 unique types of 256 bytes each. Unanswered
requests expire after 30 seconds; active streams have 30 seconds idle and 300 seconds
absolute limits. Each loop performs bounded work, polls actual readiness and
services shutdown before queued commands. A shared close latch prevents a full
queue from delaying revocation. Pipes and Unix streams are supported transfer
descriptors; regular files/devices are refused because NONBLOCK does not bound
their I/O. Payloads are streamed and never cached on disk.

The transport has a private real-compositor regression using independent
wl-copy/wl-paste peers, binary payloads, write completion/replay, source replacement,
primary-selection isolation and closure with application FDs still open. Unit
checks exercise actual pipes and the transfer ledger. These transport checks are
separate from the still-required public frontend, consent, session owner/loss and
real Flatpak acceptance. No Clipboard descriptor or live routing is added by this
transport work. The full 18-interface roadmap remains open.

[The Clipboard ledger](../../.scratch/xdg-portals/clipboard/README.md) records exact
source/binary pins, initial failures, corrected native controls and remaining
integration. The first native test stalled because its producer ran before its
wl-paste stdout consumer; concurrent draining removes the fixture deadlock with
unchanged transport source. The final native case passes on the exact final
library harness, with all tested inputs unchanged and private processes terminal.

Initial clipboard signals need a verified Start-response ordering barrier: the
pinned standard frontend ignores them before clipboard_enabled. The locked zbus
ResponseDispatchNotifier is a candidate for that boundary; actual public frontend
verification remains required. See [the integration plan](../../.scratch/xdg-portals/clipboard/PLAN.md).
