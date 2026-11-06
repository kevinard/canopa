# Capture-bound remote touch

This experimental recipe extends the complete `niri-input-capture-keyboard`
companion. It retains the same pinned Niri/Smithay archives, original GPL and MIT
notices, dependency graph, and Smithay patch. Its additional Niri changes provide
an FD-owned `OpenMappedTouch` transport and compositor-owned contact identities.

The Rust/Iced RemoteDesktop backend negotiates this extension before offering
device bit 4. A separate, initially unchecked touch choice and an approved capture
source are required. Public Notify and EI touchscreen input use the same owned
contact transport. Normal compositor input processing includes idle activity,
animation advancement and overlays. InputCapture touch remains unavailable.

Build explicitly with `scripts/build-niri-screencast.py --overlay
niri-remote-touch`, preserving the prepared source paths and incremental cache.
The standard-library ownership/wire checks run in the workspace gate. Optional
`scripts/test-remote-touch-core.py --archives DIR --output RESULT` verifies the
complete six-file delta from the preceding recipe against cached official
archives. `scripts/test-remote-touch-lease.py` checks actual touch delivery on a
private nested compositor, including actual idle notification resumption.
`scripts/test-remote-desktop-touch.py` checks real Iced consent, public Notify,
libei input, legacy handoff, stop-emulating and client-loss release. Both dark
100% and light 125% runs pass on the phase94 candidate. These checks do not
establish physical-device coexistence, installation or distribution acceptance.
Optional companion packaging accepts this recipe and preserves explicit selection.

See [ADR0117](../../../docs/adr/0117-remote-touch-ownership-and-transport.md).
