# Experimental Niri pointer capture lease

This recipe extends the pinned `niri-desktop-pointer` source, preserving its
capture-creator, pointer-mapping, keyboard and drag-lifetime changes. The Niri
patch is relative to the same official archive. Both patches live in this recipe;
the original Niri/Smithay licences remain referenced from the desktop-pointer
overlay. No application-local checkout or installed compositor selection is added.

The new private `org.canopa.Niri.InputCapture1` service owns pointer capture
leases. It is a compositor capability, not portal consent or a public XDG backend.
Version 1 supports pointer capability 2 only; keyboard and touchscreen capture
remain open.
The public Rust/Iced InputCapture layer and EI receiver have separate acceptance
records in ADR0060–0063. This private compositor API does not implement those layers.

Keyboard source teardown uses the seat's existing forwarded-press ledger. All
source-owned seat state is released, while an ordinary key-up is sent only for
a press that was forwarded. Changed modifier state is still published. No new
keyboard release API or Niri caller change is required. The retained regression
compares actual ordinary and consumed Wayland delivery, overlapping sources,
mixed keys, modifiers, accessibility grabs and explicit versus teardown release
actions. The initial normal-bind callback candidate failed the accessibility case;
the final refinement and wider-gate status are in the GlobalShortcuts preparation
ledger. No shortcut backend is implied.

`OpenPointer` returns one private Unix socket FD, the current zone-set ID and
logical zones as `(width, height, x, y)` tuples. Only a trusted consent owner may
retain this FD. It must not be passed to an application. Closing the socket,
invalid input or a blocked output retires the lease and restores local handling.
Eight leases can exist, with at most one enabled. Empty barriers are valid.

Control and event messages use a little-endian u32 payload length followed by
JSON, at most 16 KiB each. The main loop handles at most eight commands per
dispatch and never waits for a reader. Commands have a strictly increasing
nonzero `sequence` and a tagged `command`:

```json
{"sequence":1,"command":{"op":"get_zones"}}
{"sequence":2,"command":{"op":"set_barriers","zone_set":1,"barriers":[[7,[1024,0,1024,639]]]}}
{"sequence":3,"command":{"op":"enable"}}
{"sequence":4,"command":{"op":"release","activation":1}}
{"sequence":5,"command":{"op":"disable"}}
```

Replies identify the request sequence. Rejected individual barriers appear in
`failed_barriers`; stale generations, resource contention and unavailable capture
produce explicit errors. Barrier replacement suspends capture. GetZones returns
`zones` and `zone_set`. Topology changes clear barriers and report the invalidated
generation in `zones_changed`. Deactivation precedes disable/topology events.
Release cannot end a newer activation, and repeated Enable preserves an active
capture rather than creating a second activation.

Activation is decided from local pointer movement before output clipping. Only
outside edges of the union are valid, and every barrier must fit one zone.
Inclusive pixel spans also cover their fractional pixel area. The activation
event carries the post-motion global position, which may lie outside the zones;
that triggering motion is not sent a second time. Subsequent motions carry
relative deltas. Button and axis messages retain timestamp and scroll-source,
amount/v120 data. Their timestamps are compositor monotonic microseconds.
Pointer axis sources are wheel=1, finger=2, continuous=3, wheel-tilt=4.

Captured pointer buttons use the existing source-aware suppressed-button path,
so revocation or device loss does not deliver an unmatched release or trigger a
local release binding. Ctrl+Alt+Escape disables capture through the compositor.
Lock, screenshot/exit UI, device loss and incompatible pointer grabs/constraints
also suspend it. RemoteDesktop's private injected-pointer source is excluded
from capture to avoid feeding its events back to an EI receiver. Broader physical,
multi-device and capture/injection coexistence acceptance remains required.

The preparation/build command is explicit and does not install anything:

```sh
python3 scripts/build-niri-screencast.py --overlay niri-input-capture \
  --archives ARCHIVES --work-dir PREPARED_SOURCES --reuse --toolchain nightly
```

This checkpoint uses the installed nightly compiler recorded in its build
receipt, `rustc 1.97.0-nightly (14196dbfa 2026-04-12)`. Verify that identity before
rebuilding; changing compiler selection can create another dependency graph.
Reuse the verified stable source location and existing incremental compositor
target directory. Canonical provenance and the implementation ledger retain exact
inputs, source normalization, native results and outstanding acceptance work.
