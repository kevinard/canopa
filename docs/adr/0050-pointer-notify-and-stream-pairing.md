# 0050: Pointer Notify ordering and public stream pairing

Status: protocol foundations implemented with scoped native acceptance, 2026-10-31.
Runtime pointer grants remain disabled pending production EI and Iced integration.

The public ScreenCast contract pairs streams with EI regions through mapping_id.
Expose an optional StreamMapping containing only that identifier and logical size.
Copy it from the recording worker's verified mapping metadata. Keep compositor
owner/session/stream paths in the private capture binding. Before publishing
Start, reject empty, control-containing, oversized or duplicate mapping IDs,
invalid dimensions and disagreement with monitor geometry. A mapped window can
publish size without position; monitor position retains its existing meaning.
Legacy recordings with no mapping retain their prior public properties.

Represent backend input as one typed Input with separate keyboard and pointer
variants. Pointer input preserves all five Notify operations, including smooth
axis finish and discrete axis/step semantics. It does not expose private worker
frames or treat that command socket as an EI FD. Validate wire types, finite
coordinate/scroll ranges, button state, stream selector and bounded step counts
before queueing. A zero-step discrete notification remains a valid no-op.

Admit keyboard and pointer commands synchronously in the existing object-server
receive loop before spawning reply tasks. One bounded session queue preserves
their relative order across runtime worker threads. Awaiting a command reply in
that receive loop would stall every interface; spawning before admission could
reorder a button/key transition. Overflow revokes the grant because a discarded
release cannot leave active input ownership behind.

The session actor verifies the original caller and remote-session kind, current
device grant and one-way legacy-to-EIS gate. Absolute motion must name one of its
approved streams. Current coordinate bounds belong to the live pointer worker,
not stale Start geometry. Session/caller/source/backend loss cancels pending input
and drains queued replies before stopping the owned resources. EIS connection
for either modeled input bit disables subsequent legacy Notify operations.

This host foundation can model pointer grants supplied by a controlled presenter.
Production still selects only keyboard capability and the Iced dialog still has
no pointer grant. The default pointer callback returns NotSupported. Enabling
runtime pointer capability requires the real production transport, EI pointer
devices and their immutable region replacement, and fresh unchecked Iced consent.
No normal RemoteDesktop registration or descriptor advertisement is introduced.

Focused tests cover all five pointer methods, malformed input, sender/grant/stream
checks, post-EIS denial, cancellation, shared queue overflow, 1,280 mixed keyboard/
pointer transitions on four runtime threads, public mapping metadata and failed
publication cleanup. All 151 library and 43 integration tests pass, along with Clippy, formatting and
test-layout checks. Native
metadata checks must use actual frontend results and decoded video, retain legacy
capture behavior, and compare the production facade's public ID/size with the
private binding passed to the independently owned pointer worker. The frozen build passes
51 native groups, including 64 resize transitions, mapped/legacy public video
and combined keyboard EIS/media. All 1,313 source inputs and fixture/binary pins
stay unchanged; all owned native processes terminate.

The [ScreenCast contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html),
[RemoteDesktop backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html)
and [EI device protocol](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html)
were checked before this implementation. Next EI work must verify region layout/
physical scale, capability negotiation, device replacement/tombstones and held
state. The prepared frontend's generic ordered forwarding is separate from proof
of a live public pointer session. Compositor adoption, distribution, broader
workspace/native failures and the full eighteen-interface goal remain open.

[ADR0051](0051-atomic-input-region-scale.md) now resolves the window-only scale
prerequisite with atomic version-2 descriptors and worker renewal. It retains
version-1 compatibility. Actual EI device lifecycle and Iced pointer grants still
need implementation.

Evidence: [pointer adapter ledger](../../.scratch/xdg-portals/pointer-adapters/README.md),
[private worker ownership](0049-capture-bound-pointer-workers.md),
[ordered frontend forwarding](0047-ordered-portal-input-forwarding.md).
