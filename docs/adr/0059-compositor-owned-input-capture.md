# Compositor-owned input capture

Status: private pointer foundation passes scoped compositor acceptance,
2026-11-01. Seven geometry tests and eight native groups pass on one pinned
candidate. The existing portal/input/media regressions pass 54 more native groups
in both appearances. The required workspace gate completes and repeats the same
ten Files integration failures; all other Rust targets, formatting, workspace
Clippy and later Settings/CLI/configuration/vendor stages pass.
At this checkpoint InputCapture's public backend, EI receiver and Iced consent
remain unimplemented. [ADR0060](0060-input-capture-receiver-worker.md) subsequently
adds the private EI receiver transport. No normal portal advertisement or
installation is added by either checkpoint.

InputCapture diverts local input to an application acting as an EI receiver. The
existing RemoteDesktop worker accepts an EI sender and injects input. Allowing a
receiver handshake on that worker would not implement capture: Niri must decide
when a barrier is crossed and prevent captured events reaching ordinary clients.
The exact v2 backend/session contract is retained in
[the primary-source investigation](../../.scratch/xdg-portals/INPUT-CAPTURE-CLIPBOARD.md).

An experimental `niri-input-capture` recipe extends the verified desktop-pointer
source. It retains the original pinned Niri and Smithay archives, licences and
all existing capture/drag/input patches. Its complete Niri patch is canonical in
vendor/, with the unchanged Smithay patch referenced from the previous overlay.
The same prepared source paths and incremental compositor cache are reused.
Full source comparison proves equality to the new archive materialization after
the two validated stable Smithay bindings, rather than trusting edited build
outputs. The prior compositor remains independently frozen by checksum.

Niri now exports a private `org.canopa.Niri.InputCapture1` control service.
Version 1 supports pointer capability 2 only. OpenPointer returns a private
Unix socket, logical zone-set ID and zones. This is a privileged compositor
capability held by a trusted consent owner; it must never be returned to an
application instead of its EI socket. Eight leases can exist, with one enabled.
Closing the control FD revokes interception. It does not grant portal consent.

The geometry module owns bounded logical zones, generation and validated barrier
replacement. Barriers fit one zone and lie on the outside boundary of the union,
so adjoining outputs do not acquire an internal capture edge. Pixel endpoints
are inclusive, including fractional positions within their last pixel. Crossing
uses the unconfined destination before ordinary output clipping. Topology changes
erase barriers, increment with wrapping and report the invalidated generation.
Stale generation requests fail; invalid individual IDs/positions are reported
without discarding valid barriers from the same replacement.

The compositor owns disabled, enabled and active state, activation IDs and
exclusive arbitration. Enable alone cannot activate capture. A local crossing
produces Activated with the unconfined post-motion position; the triggering
motion is represented there once. Later motion/button/axis events go to the
lease rather than normal pointer handling. The pointer is hidden during capture
and restored afterward. Release validates the current activation ID, preserving
the enabled lease; barrier replacement suspends it. Repeated Enable preserves an
already active capture. Capture is not armed through an existing pointer grab.

Captured button presses use Niri's existing source-aware suppressed-button path.
Closure and device loss do not deliver unmatched local releases or execute local
release bindings. Ctrl+Alt+Escape disables capture in the compositor. Lock,
screenshot/exit UI, source loss and incompatible pointer constraints also suspend
it. The existing private RemoteDesktop pointer source is excluded from capture;
broader capture/injection coexistence still needs explicit acceptance.

The control protocol uses bounded length-prefixed JSON, finite command admission
and nonblocking event writes. Malformed/oversized messages, EOF and output
backpressure retire the lease. Main-loop work handles at most eight commands per
dispatch and retains no pending event queue. The private protocol and limits are
documented beside its canonical overlay. A future EI worker must translate these
facts with bounded acknowledgements and compositor identity/owner supervision.
It must release held EI input and stop emulation on deactivation, and never
substitute the private control socket for an application-facing EI connection.

Native acceptance uses a persistent Wayland pointer source and independent local
receiver. An ordered keyboard marker on the receiver's same Wayland connection
checks that captured pointer data was not duplicated locally. Tests cover actual
crossing overshoot, motion/buttons/scroll, FD closure with a held button, clean
local input afterward, exclusive enablement, stale activation release, empty
barriers, live scale changes, compositor escape before device destruction,
backpressure, admission limits, malformed input and source loss while held.
These are virtual-device compositor checks, not physical hardware, touch, EI
receiver, public frontend or production distribution acceptance.

[The InputCapture ledger](../../.scratch/xdg-portals/input-capture/README.md)
records exact source/binary pins, the missing-capability baseline, initial build
and fixture failures, successful checks and remaining work. The full 18-interface
goal stays open. Keyboard capture, EI delivery, v2 session ownership/Clipboard
active-phase integration and truthful Iced consent/controls are required next.
