# Pointer Notify and stream pairing foundations

Status: protocol foundations implemented with scoped validation, 2026-10-31.
Production EI/Notify pointer transport and explicit Iced pointer consent remain.
Runtime pointer grants, normal RemoteDesktop registration and advertisement stay
disabled until that integration is ready.

Verified recording metadata now reaches public Start as optional mapping_id and
logical size. A mapped window has size without a monitor position. The public
StreamMapping contains no compositor object paths. Publication rejects invalid
or duplicate mapping IDs, invalid dimensions and disagreement with monitor
geometry, and retires the rejected capture. Legacy providers retain their prior
public stream properties.

All five pointer Notify methods decode into a renderer/transport-independent
PointerInput. One Input type places keyboard and pointer commands in the same
bounded session queue. Admission happens in the object-server receive loop before
reply tasks are spawned, preserving cross-device receive order. The actor checks
session kind, sender, device grant, approved absolute stream and the one-way EIS
gate. Overflow revokes the grant; session/caller/source/backend loss cancels a
pending input and drains replies. Smooth-axis finish is retained as an explicit
semantic value; zero discrete steps remain a valid no-op. Runtime pointer
capability is still disabled, and the default transport callback is NotSupported.

| Final focused check | Result |
| --- | --- |
| New pointer D-Bus protocol groups | 4 passed |
| New mapping publication/cleanup groups | 2 passed |
| Portal library | 151 passed, 5 ignored |
| Complete portal integration executable | 43 passed, 2 ignored |
| Clippy with warnings denied, formatting, test registration/layout | Passed |
| Production public metadata/private binding handoff plus existing pointer/video/retirement | 11 native groups, 64 resize transitions |
| Mapped provider, dark/125%, actual frontend video and Iced controls | 17 native groups |
| Canonical legacy provider, light/100%, view-only compatibility | 17 native groups |
| Mapped provider, light/125%, combined keyboard EIS/media | 6 native groups |

The new private-bus tests cover every pointer method and malformed wire values,
sender/device/stream rejection, post-EIS denial, 1,280 mixed keyboard/pointer
transitions on four runtime threads, pending input retirement and shared queue
overflow. They acknowledge through a controlled transport: they do not establish
live public pointer injection or pointer consent.

The native suites check public mapping properties from actual frontend responses.
The video fixture compares mapped logical size with decoded physical image size
at compositor scale. Legacy captures must omit mapping IDs. A real production
Capture fixture also checks that its public pairing ID/size equals the private
binding used by the separate pointer worker. Existing private pointer injection
and keyboard EIS/media behavior remain covered. All 51 native groups pass.

Normal incremental debug/default-feature validation takes 459.13
seconds; native suites take 274.96 seconds, including startup and
artifact preparation. All 1313 recorded source inputs, helper pins
and frozen binaries stay unchanged. All owned native processes terminate.
The first cargo check's private-accessor visibility error is preserved; the
corrected all-test-target check and final tests/Clippy pass.

[evidence.json](evidence.json) records exact binaries and receipts.
[portal.patch](portal.patch) is the 12-file delta from the preserved
starting state; existing-file baselines and an isolated successful replay are
retained. Native sources, results, logs and images are in native-proof/.
Execution artifacts are at target/portal-pointer-adapters-08/.

The current primary XDG and EI contracts and derived decisions are retained in
[protocol notes](native-proof/protocol-notes.md). Next work is production pointer
transport, EI region/device layout and physical scale, immutable region
replacement with correct held-state ownership, and fresh unchecked Iced pointer
approval. The generic ordered frontend companion is not proof of live public
pointer support. Remaining window/physical input, canonical compositor adoption,
distribution, earlier full workspace/native matrix failures and the full
18-interface goal remain open. See
[ADR0050](../../../docs/adr/0050-pointer-notify-and-stream-pairing.md) and
[private worker evidence](../pointer-workers/README.md).
