# Remote touch ownership and mapped compositor transport

Status: experimental public implementation; scoped native verification passes.

## Decision

Extend the complete pinned Niri companion with a separate FD-owned touch lease.
Keep the existing keyboard, pointer, shortcut, input-capture, source-lifetime and
ScreenCast behavior in the same recipe. The portal continues to own consent;
the compositor owns contact identities and validates live capture mappings.
Touch is not pointer emulation, and closing one source must not cancel the seat.

`org.canopa.Niri.RemoteInput1.TouchVersion=1` negotiates the private extension.
`OpenMappedTouch(o session, ao streams)` returns a socket and the same immutable
stream/mapping/size/physical-scale descriptors as mapped pointer input. Unstarted
streams, stale capture identities, changed projections, lock and source loss
prevent continued input. There is no unmapped whole-desktop fallback.

The socket accepts 40-byte little-endian commands: sequence, kind, contact and
reserved words (`u32`), stream (`u64`), then coordinates (`f64`, `f64`). Kinds are
0 Frame, 1 Down, 2 Motion, 3 Up. Frame has no payload; Up has only a contact ID.
Sequences start at one and cannot skip or wrap. Reserved words, invalid payloads,
nonfinite/out-of-range coordinates and invalid contact transitions close the
source. Acknowledgments contain the sequence and three zero `u32` words.
Reads retain at most one command, process at most 32 commands per dispatch and
never accumulate an output queue for slow clients.

## Contact and device lifetime

Client contact IDs are namespaced by monotonic compositor source identities.
The registry admits at most 16 sources, 32 contacts per source and 64 shared seat
slots; existing transport admission can impose a lower source limit. A client
cannot select a seat slot or release another source's contact. Socket loss lifts
only that source's contacts and emits a frame before reuse. Retirement clears
screenshot touch state without confirming a screenshot. It never issues a
seat-wide cancel to withdraw one remote owner.

Physical input takes priority. A physical down reserving an occupied slot retires
the remote owner before forwarding the down. Reservations count overlapping
device-local physical slot IDs. Per-device down/up bookkeeping prevents stale
tails after cancel from releasing newly allocated remote contacts. Device removal
releases its reservations; another physical device's hold keeps a slot reserved.
Physical seat cancel retires remote sources and clears physical tracking before
the existing seat-wide cancel. IDs outside the remote pool retain their existing
physical path. Counter uncertainty pins a reservation until physical cancel.

Touch capability follows both physical devices and remote sources, including
the nested Winit virtual input device. The adapter records device addition and
removal independently of Niri's libinput-only touch-device collection. Opening
a remote source can create touch support without a physical touchscreen; closing
the last remote source must preserve any existing physical touch capability.

## Verification and limits

The canonical recipe is in `vendor/provenance.json`; the complete patch and
original-license references remain under `vendor/source-overlays/`. Ownership
and wire tests extract the exact declared new Rust files and use a stable
standard-library harness, without another Cargo graph. Archive comparison must
show exactly six additional/modified Niri files and unchanged Smithay relative
to `niri-input-capture-keyboard`. The compositor build reuses the original source
paths, toolchain, feature selection and incremental target.

Private native checks cover actual Wayland coordinates, independent contacts,
owner-only release/frame, invalid-transition withdrawal and capture retirement.
The initial fixture incorrectly accepted removal of Winit's existing touchscreen;
its source correction and failing capability control are retained in phase93.
The next candidate preserves that device but misses a frame if the final Up
precedes socket loss; the native regression records the unframed release. Source
retirement now flushes a frame even when its contact registry is already empty.
Native acceptance requires both corrected boundaries.

## Public integration

The normal Rust/Iced backend offers device bit 4 only after live TouchVersion
negotiation. Touch requires both its own fresh unchecked approval and a selected
ScreenCast source. Keyboard/pointer approval does not imply touch. A separate
bounded worker validates the approved PipeWire-node-to-private-stream binding,
contact transitions and capture geometry before writing compositor commands.
Each accepted Notify call includes a compositor Frame; the public API has no
NotifyTouchFrame method. The ordered dispatcher must decode and route all three
Touch methods as well as advertise their signatures. A fast real-backend probe
and an actor-level wire regression cover this previously missed boundary.

EI has a separate touch device and budget. Its regions share the capture mapping,
logical size and physical scale. Touchscreen protocol v1 is negotiated; v2 Cancel
is not advertised. Connecting to EI releases legacy contacts and disables Notify
input. Stop-emulating releases held contacts and replaces the device so old touch
IDs cannot control a new generation. EI client loss releases contacts and closes
the public session. Capture renewal never replays unacknowledged input.

The compositor now routes typed remote touch events through process_input_event,
including normal animation, idle/wake and overlay handling. Only the owned typed
remote device bypasses physical coordinate transforms and physical-contact guards.
Source retirement emits owned Up/Frame events without simulating user activity
or screenshot confirmation. The actual idle-notifier test fails on the old direct
helper path and passes on this pipeline.

Phase94 records seven private native groups and six public Iced/Notify/libei
groups in each appearance (dark 100%, light 125%). Public checks cover fresh
default-off consent, exact coordinates/two contacts, handoff, EI mapping/input,
stop-emulating and client-loss cleanup. They do not establish physical-device
coexistence or deployment. InputCapture touch reception remains unavailable.
Optional companion packaging accepts this complete recipe; live session
selection stays an explicit deployment operation.

The public RemoteDesktop specification defines the public layer:
[API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)
and [backend](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html).

Update, 2026-11-10: the `vendor/` source-overlay recipes and their verifier were removed. Remote touch is now the `canopa/remote-input-touch` branch of [kevinard/niri](https://github.com/kevinard/niri), with a tracking PR in the fork, and is merged into the pinned `canopa/main` commit that `scripts/build-niri.py` builds for the packaged compositor (`packaging/forks.json`). `--ref canopa/remote-input-touch` rebuilds the topic for comparison. See ../vendor-maintenance.md#niri-compositor.
