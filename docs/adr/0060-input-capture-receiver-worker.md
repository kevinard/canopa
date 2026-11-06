# Isolated InputCapture receiver worker

Status: private pointer transport passes nine native groups, 2026-11-01.
Final Portal checks and regressions after the receiver negotiation change are
recorded in the InputCapture ledger. At this checkpoint the public InputCapture
v2 backend and Iced consent/controls remain open; the later
[ADR0061](0061-input-capture-sessions-and-consent.md) adds them for pointer capture.
Keyboard/touch and active-phase Clipboard integration remain open. Normal provider
descriptors and installed programs are unchanged.

[ADR0059](0059-compositor-owned-input-capture.md) establishes real compositor
interception. The privileged Niri control socket must stay behind the trusted
backend boundary. An application receives a separate EI Receiver connection;
RemoteDesktop's EI Sender injection worker cannot serve this purpose.

The Rust parent authenticates the original compositor bus owner, process and Niri
IPC peer before starting an isolated worker. The worker subscribes to owner loss
before revalidating the compositor, original backend and original caller. The
backend's bus PID must match the control socket's parent PID. Only then does it
open a private pointer lease. Grants currently support pointer capability 2 only.
Read-only discovery never opens a capture lease or substitutes for consent.

The child creates an EI socket pair and retains its server end. The checked
single-FD handoff validates marker, ancillary cardinality, Unix stream type,
close-on-exec and the child peer PID. Its implementation is shared with the
existing RemoteDesktop handoff. The application-facing socket therefore belongs
to the worker, rather than exposing Niri's privileged control channel. One
Connect handoff is allowed, and Enable requires the connection and a configured
receiving device. Delayed Connect starts its handshake deadline at handoff.

The parent serializes commands through finite queues and bounded framed JSON.
An independent revoker can stop the worker without waiting for an operation's
mutex or reply. Input data goes straight from Niri to EI; parent notices contain
only phase/zone metadata. Independent framed readers preserve partial messages
across select cancellation. Frame, queue, label, handshake, device-history,
barrier, zone and held-button limits bound retained data. Output WouldBlock is
terminal; source acknowledgements, parent writes and cleanup have deadlines.

Receiver-role handshakes expose relative pointer, button and scroll only.
Sender roles and input-producing requests are rejected before the Reis request
converter can buffer unframed input. The receiver binds a device and receives
configuration/resumed before start_emulating. Every activation uses Niri's exact
activation ID as its EI sequence. Motion, buttons and scroll carry frames; held
buttons and finger scroll are neutralized before stop_emulating on deactivation.
Release preserves enablement; Disable/Enable reuse the same EI connection.
Device replacement suspends compositor interception before explicit enablement
can activate the replacement. Seat release ends the grant.

The installed libei 1.6 receiver sends ei_device.ready when device v3 is
negotiated. The [protocol's ready request](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html)
is documented as sender-only. This worker negotiates device v2, which supports
the receiving events it implements, and continues to reject input-producing
requests. Readiness is established by receiver configuration, not by waiting for
the sender-only request. A real libei receiver verifies interoperability.

On original-owner loss, parent EOF or stop, the worker half-closes its Niri
control write direction, ends EI input, and drains to compositor EOF under time
and byte limits. The parent retains its admission permit until the child exits.
There is no parent-death signal: backend process loss must leave the child time
to clean up. SIGKILL and blocked EI output cannot guarantee synthetic releases
reach a client; those tests prove terminal connection/interception retirement and
clean subsequent local input. A client-requested device close also discards
further input in libei; device removal retires that logical device's local state.
Graceful owner-loss tests separately require explicit releases and stop events.

The ignored test harness exposes a private authenticated fixture, never an
InputCapture backend. Native acceptance combines the actual Rust worker, pinned
Niri, passive libei receiver, persistent Wayland pointer and an independent local
client. It verifies one FD, owner isolation, real diverted events, activation IDs,
stale Release, repeated cycles, ordered absence of local duplicates, topology,
device replacement, repeated sender rejection, original caller loss, child and
backend death, and isolated EI output backpressure. Worker pidfds and owned
process handles establish terminal cleanup; binary/source pins remain fixed.

[The ledger](../../.scratch/xdg-portals/input-capture/README.md) records exact
receipts, failures, final checks and remaining full-goal work. These virtual-device
transport checks do not establish physical-input, public frontend, Iced consent,
Clipboard phase or production distribution acceptance.
