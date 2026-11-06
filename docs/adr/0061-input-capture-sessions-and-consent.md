# InputCapture sessions and Iced consent

This records the phase-19 pointer-only checkpoint.
[ADR0062](0062-input-capture-clipboard-activation.md) adds separately consented,
active-only Clipboard with newer scoped validation; the earlier results below
remain historical evidence.

Status: experimental pointer implementation, 2026-11-01. Eight public native
groups pass in each appearance; complete Portal checks pass 173 library and
52 integration tests, formatting, Clippy and the main build. With the private
receiver and existing Clipboard/RemoteDesktop/ScreenCast regressions, the final
candidate passes 113 scoped native groups. Exact receipts are recorded in the
InputCapture ledger. Keyboard/touch,
InputCapture Clipboard integration, broader acceptance and distribution remain
open. No normal provider advertisement or installation changes are made.

[ADR0060](0060-input-capture-receiver-worker.md) establishes the isolated receiver
transport. InputCapture now has its own finite session registry and actor, sharing
the existing Requests cancellation/admission mechanism. It does not reuse a
RemoteDesktop session kind or reinterpret injection as capture. Session lookups
match the original backend caller's unique bus name and application ID. The
standard frontend authenticates the original public application; its session
ownership and original-backend supervision remain part of the boundary.

The exact backend v2 interface includes CreateSession2, Start, GetZones,
SetPointerBarriers, Enable, Disable, Release, ConnectToEIS, the phase/zone signals,
SupportedCapabilities and lowercase version. The deprecated backend CreateSession
uses the same fresh startup path. An invalid deprecated create retires its
provisional session before returning an error. The standard frontend's legacy
public CreateSession uses backend CreateSession2 plus Start; a method-call-only
trace verifies these calls without observing transferred descriptors.

CreateSession2 opens no compositor lease and shows no dialog. Start is allowed
once, never restores authority from supplied persistence data, and presents an
Iced dialog with pointer permission initially unchecked. Only an explicit checked
grant can open the private compositor lease. It uses the shared theme, font,
checkbox/button styles and transient-parent/activation APIs. The compatible Niri
endpoint is resolved and observed before review, then independently validated by
the worker before admission. Current capabilities are pointer only (2), with zero
reported if the compatible compositor is unavailable. Clipboard is not offered;
Start truthfully returns clipboard_enabled=false.

An ongoing Iced control window says "Input capture is allowed". This describes
the permission while capture is waiting for an edge or has been paused. Its Stop,
Escape or window close ends the session. Ctrl+Alt+Escape suspends capture in Niri.
A monitor independent of an outstanding operation revokes the worker when the
control window disappears. Both the controls and worker retire before a live
session's backend Closed signal is emitted.

Started consent, receiver connection, compositor enablement and active capture
remain separate facts. One EI descriptor is handed off; Connect must precede
Enable. Disable/Enable and Release reuse that connection. Niri validates barriers,
zone generations and activation IDs; captured data never goes through the actor.
Phase/zone metadata is translated into directed signals for the original backend
caller. Suggested release coordinates are validated but need not be honored.
Omitting an activation ID releases the currently observed activation; an explicit
stale ID remains subject to compositor validation.

Enable/Disable/Release/Connect are admitted synchronously in D-Bus receive order,
then awaited off the object-server receive loop. Queues, resource counts, signal
writes and operation deadlines are finite. Closure and owner loss interrupt an
operation without waiting for its ordinary acknowledgement. The actor retains a
canceled startup future until its helpers retire; the canceled Start response also
waits for session retirement, including cancellation before command admission.
The transport's startup error path and sharing-control readiness cancellation now
wait for their owned child retirement rather than merely requesting it.

A failed or canceled Start retires through its response. Emitting backend Closed
first makes the standard frontend try to export an already closed session and
turn an explicit cancellation into a generic failure. Closed remains the live
session teardown notification after resources have stopped. The startup and
deprecated-create regression tests preserve these distinctions.

Native acceptance uses the actual main executable, a private descriptor, the
pinned standard frontend, checksum-addressed Niri, real Iced windows, a passive
libei receiver and persistent pointer source. It checks exact introspection,
both public client generations, unchecked denial, real zone/barrier handling,
one FD, activation sequence/position, motion and held-button cleanup, live topology, repeated
capture, Iced Stop and original application/backend loss. Process handles and
worker pidfds establish termination. Screenshots retain consent and controls
in each tested appearance. Existing private receiver and Clipboard/RemoteDesktop
regressions remain required after changes to the shared helper lifecycle.

[The ledger](../../.scratch/xdg-portals/input-capture/README.md) records exact
pins, failures, final checks and remaining work. Mixed-output/transform, startup/phase
races, physical input, constraints/lock, capture/injection coexistence and complete
owner/UI/frontend failure matrices still need wider acceptance. The full
eighteen-interface goal remains open; this checkpoint does not enable routing.
