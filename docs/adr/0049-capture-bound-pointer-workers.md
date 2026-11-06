# 0049: Capture-bound pointer workers

Status: private worker implementation with scoped native acceptance, 2026-10-31.
ScreenCast and RemoteDesktop remain unadvertised. The compositor extensions are
prepared drafts, not the canonical or installed provider.

Absolute input needs the exact approved recording, rather than a monitor name or
a reusable PipeWire node number. After each producer starts, the recording worker
collects its session and stream paths, live source identity, mapping identity and
logical size. It verifies the source identity against the selection and returns
one bounded, complete binding group. The host retains the original unique
compositor owner. Public stream results do not contain these private paths.
The production Capture facade exposes an optional opaque binding group to other
session-owned components. Missing metadata preserves legacy view-only capture;
it cannot imply an absolute-input region.

A separate Rust pointer worker receives that group and an authenticated input
endpoint. It verifies the capture owner, compositor process, live stream metadata
and returned FD peer. OpenMappedPointer atomically supplies the region sizes for
that lease. Commands on the host's private socket name approved PipeWire nodes;
only the worker translates them to the corresponding compositor stream IDs.
Explicit input-only operation can open a relative pointer lease without capture
bindings, but has no absolute region. The private socket is not an EI descriptor.

Keep pointer and keyboard transports distinct while sharing input worker
admission and lifetime observation. Existing EI and legacy keyboard transports
remain keyboard-only. Pointer frames have finite values, bounded scroll, strict
sequence numbers and exact acknowledgements. The worker tracks at most 64 held
buttons and rejects duplicate transitions, unknown regions and out-of-bounds
positions before injection. Only one command is in flight. Cancelling its host
future revokes the worker, so a late acknowledgement cannot answer a later call.

Compositor EOF confirms that the old source has released held buttons and grabs.
A resize may shut down its socket while a command remains unread. On Linux this
can report a pending connection reset before EOF. Require a subsequent bounded
read returning EOF; reset alone is not cleanup confirmation. An interrupted
command is never replayed. Renewal requires the same owner, session, streams,
source identities and mapping identities, plus a verified change in region size.
The replacement lease starts with empty held state. An unexplained same-size EOF,
source destruction, failed revalidation, lock or owner loss ends the worker.

The original recording remains independently owned when pointer input stops.
Backend death leaves both children able to observe revocation, release input and
stop producers. Stop waits for worker retirement. All frames, startup operations,
command replies and cleanup reads have explicit size or time bounds.

Focused validation passes 151 portal library tests, 37 integration tests,
formatting and all-target Clippy with warnings denied. A real Unix-socket
regression first reproduces the reset caused by shutdown with an unread command,
then passes after requiring EOF. This places the platform-specific retirement
behavior inside the pointer module and tests its actual socket boundary.
Earlier startup timeouts and native failures remain recorded separately.

The frozen build passes 62 native groups: two pointer runs with 192 resize
transitions each, 17 new-provider video groups, 17 legacy view-only groups and six
combined keyboard EIS/media groups. All 1,308 recorded source inputs and fixture/
binary pins stay unchanged; all owned native processes terminate. Earlier full
workspace and native matrix failures remain open.

Public pointer support still requires receive-ordered Notify adapters, EI pointer
devices and immutable regions, coherent EI device replacement on geometry change,
and explicit unchecked Iced pointer approval. Window activation/occlusion,
physical input, canonical compositor adoption, distribution and broader workspace
acceptance remain. This slice introduces no portal permission or routing change.
The standard frontend retains sandbox identity and document responsibilities.

The subsequent [ADR0050](0050-pointer-notify-and-stream-pairing.md) adds public
pairing metadata and the typed receive-ordered Notify foundation. Production
pointer transport, EI device replacement and explicit Iced permission remain;
runtime pointer grants are still disabled.

Evidence: [pointer worker ledger](../../.scratch/xdg-portals/pointer-workers/README.md),
[capture mapping ledger](../../.scratch/xdg-portals/POINTER-MAPPING.md),
[live source consent](0048-live-capture-source-consent.md).
