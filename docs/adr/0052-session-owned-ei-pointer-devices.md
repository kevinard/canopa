# Session-owned EI pointer devices

Status: private implementation with scoped acceptance, 2026-10-31.
Production pointer approval and normal RemoteDesktop registration remain disabled.

Extend the existing Rust sender worker to own independent keyboard and pointer
devices on one anonymous EI connection. Each device has its own resume serial,
emulation sequence, active state, pending frame budget and compositor input lease.
Keyboard events retain their sealed XKB map and authoritative modifier updates.
A pointer-only grant neither opens nor announces a keyboard. Unsupported device
classes remain rejected. The parent keeps the worker lease separately from the
transferred EI FD and awaits both compositor cleanup acknowledgements on shutdown.

Build absolute regions from the descriptors atomically returned by the pinned
compositor. Require known physical scale; descriptor version 1 remains available
to the private legacy pointer path but cannot invent an EI scale. Place approved
regions next to each other in a private nonoverlapping logical layout, preserving
their mapping IDs, logical dimensions and physical scale. Translate absolute
positions back to the selected PipeWire node and source-local coordinates before
the existing pointer actor checks and forwards them. Require EI device version 2
for absolute input, because region mapping IDs are necessary to pair video and
input. Input-only sessions provide relative motion, buttons and scrolling.

EI region configuration is immutable. On confirmed compositor EOF, discard old
pending input and held state; renew only a verified size/scale change with the
same source/mapping identity. Destroy the old EI pointer device and announce a
fresh one, preserving the keyboard. Retired object IDs are finite tombstones and
cannot control a replacement. Interrupted commands are never replayed. Explicit
device/interface release also retires its compositor source, while a later
request-device can create a fresh device. Stop-emulating closes and reopens the
source before accepting a new sequence.

Keep raw unframed input bounded before Reis buffers it and retain a separate
frame budget for each device. A pointer frame has a bounded total send deadline.
Smooth and discrete scrolling are mutually exclusive within one frame. Zero
deltas remain no-ops, and stop/cancel end only selected axes. The compositor does
not synthesize kinetic events. Invalid transitions, unapproved coordinates and
unsupported capabilities terminate the connection and release its sources.

Closing the EI socket can race a keyboard modifier update. Broken-pipe/reset/EOF
on that client transport means ordinary retirement; compositor cleanup still has
to complete independently. Malformed input, output backpressure and compositor
errors remain errors. A real libei client fixture retains the pre-fix broken-pipe
reproduction and checks the corrected exit with held keyboard and pointer input.

The private fixture exercises actual captured colors and dimensions, client
coordinates, region replacement, keyboard continuity, source cleanup and bounded
input. This is not public-session acceptance. Remaining work includes the
production legacy pointer adapter, legacy-to-EI retirement, fresh unchecked Iced
pointer consent, runtime capability checks, the broader matrix and companion
distribution/adoption. Earlier shared-workspace failures remain open.

Final acceptance is 154 library and 43 integration tests, Clippy and formatting,
18 pointer/media native groups repeated twice and 12 existing keyboard groups.
The identical pointer fixture reproduces the pre-fix client-EOF error and passes
with the corrected worker. All 1,320 final source inputs stay unchanged. The
12-file delta replays against recorded baselines. Owned native jobs terminate;
these results do not establish a passing full-workspace or public pointer matrix.

The [EI device protocol](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html)
defines fixed regions and mapping IDs; the
[scroll protocol](https://libinput.pages.freedesktop.org/libei/interfaces/ei_scroll/index.html)
defines delta/discrete exclusivity and stop/cancel. Implementation evidence belongs
in the [EI pointer ledger](../../.scratch/xdg-portals/ei-pointer/README.md).
See also [atomic scale](0051-atomic-input-region-scale.md) and
[public stream pairing](0050-pointer-notify-and-stream-pairing.md).
