# Compositor pointer ownership foundation

Status: prepared Rust compositor implementation; scoped proofs pass; unadopted,
2026-10-31. RemoteDesktop remains unregistered by normal main and unadvertised.
The portal worker and consent currently support keyboard grants only. This work
is a prerequisite for adding pointer transport, not a pointer portal grant.

The [reviewable draft](pointer-ownership-draft/README.md) contains two patches,
seven-file scope/hash metadata, a successful delta replay and private fixture
sources. Its base is the canonical `niri-remote-input` recipe with Niri
`b22eb842ca9506d4e2ff997b116031cdb3f650f8` and Smithay
`8a270232f1bfc416c781273c98a3f52a812a0bba`. No application-local fork, GTK
component, dependency graph change or canonical recipe adoption is introduced.

## Native defects and resulting behavior

The original E5 compositor forwards duplicate presses from overlapping virtual
pointers, and source loss releases a button another source still holds. A
positive receiver marker reproduces this before the survivor releases. It also
runs a departed origin's pending release binding, leaves stale seat bookkeeping
after a consumed picker click, and turns loss during an accepted drag into an
actual drop. The receipts retain positive single-source/ordinary-drop controls.

The prepared model admits transitions before Niri bindings, using actual device
types/equality rather than printable names or hash values. The last owner ends a
combined cycle. A departed first-press origin loses its pending action even if
it released earlier; losing a non-origin leaves the live origin's action intact.
Consumed presses that start gestures get seat bookkeeping without client input.
Retirement cancels an affected explicit grab before clearing the entire ended
batch, preventing unpaired client releases and cleanup-generated drops.

Host blur originally drained a virtual origin's pending mouse binding. The
prepared host path retires only its Winit pointer identity and preserves virtual
pointer state. Full global resets instead end all pointer cycles before clearing
suppression; the existing policy of running global-reset release actions stays.
Tablet suppression now has its own set. Physical tablet and TTY acceptance remain
separate; synthetic host evidence does not establish them.

## Final scoped verification

Final frozen compositor SHA-256:
`7462b49fb74e07a536df86c8a1318f72350f2c7e85376ccdfc124b5b7e99c581`.
The same nightly 1.97, incremental dev profile, one-job compiler and existing
root companion cache are reused. The clean source build passes in 138.93 seconds;
all 5,954 recorded source inputs stay unchanged during the build and native run.
Accepted E5, portal runtime/descriptor and canonical recipe pins remain unchanged.

| Check | Result |
| --- | --- |
| Ownership model, including reset/collisions/origin lifetime/576 orders | 9 passed, 0.04 seconds |
| Two overlapping/recreated virtual sources | 2 passed |
| Release-binding ownership and cancellation | 8 passed |
| PickWindow/PickColor consumption and focus recovery | 6 passed |
| DnD normal drop, loss cancellation and multiple held buttons | 5 passed |
| Synthetic host overlap/focus-loss coexistence | 8 passed |
| Consumed workspace and routed move grabs | 6 passed |
| Full input reset and subsequent fresh cycles | 3 passed |

The seven final native suites pass all 38 checks in 42.61 seconds. Every owned
process is terminal, and fixture/binary/prepared-source hashes stay unchanged.
The persistent evidence archive is `target/remote-input-pointer-ownership-02/`;
`verify-06.json`, `build-05.json` and `preservation.json` identify final results.
Earlier builds, failing controls and intermediate successful hashes are retained.
No full workspace or full native matrix success is inferred from these checks.
The preceding workspace gate remains red with the same ten Files failures.

## Preserved failures and drag setup diagnosis

The first prepared adapter exposes additional native failures: host blur runs a
virtual-origin action, another button's release prematurely ends a consumed
workspace gesture, multi-button DnD cleanup emits an unmatched release, and a
full reset emits a consumed release to a client. Their failing frozen binaries
and positive receiver barriers are retained before each correction.

The first final-source run finishes with six suites passing and a DnD setup
timeout. A controlled half-second receiver flush delay and server protocol trace
prove injector motion arrives before StartDrag. Niri installs DnD with
`Focus::Keep`; target entry requires later motion. Logging the client's request
was therefore an insufficient setup barrier. A same-client `wl_display.sync`
callback now precedes injector motion. The identical delayed-client control
passes all five drag cases in 8.21 seconds; the trace shows StartDrag preceding
motion. No compositor change or rebuild is used to fix this fixture ordering.
The original timeout and both controlled traces remain in the archive.

Other unsuccessful fixture preparations remain recorded: sandbox-denied D-Bus
socket startup; a release-only binding intentionally cancelled by a marker;
short-lived IPC helpers incorrectly treated as persistent services; Xvfb warning
text parsed as its display number; and unsupported IPC/KDL VT reset triggers.
The corrected reset fixture asserts it is on Winit and uses its no-op suspend
path, without suspending the real session. These explanations apply only to their
specific observed failures, not to unrelated matrix or Files failures.

## Continuation

The next prepared slice implements a bounded relative/button/scroll pointer
lease. Six wire tests and 94 native checks pass on its frozen compositor,
including direct FD-leased bindings/DnD and all earlier pointer/keyboard checks.
See [the lease draft](pointer-lease-draft/README.md). Its layered eleven-file delta
replay passes; no canonical adoption or portal pointer grant is introduced.
The subsequent [mapping foundation](POINTER-MAPPING.md) pins live capture sources
and adds an absolute lease bound to active session/stream identities. It passes
scoped native video/input checks and the same 94 input regressions. Portal
negotiation, worker renewal, EI/Notify and Iced approval remain separate; these
prepared transports do not grant pointer control through the portal.

Keep the full 18-interface roadmap. Before pointer advertisement:

1. Integrate the prepared finite compositor pointer lease with approved mapping,
   preserving authenticated owner/PID checks and the existing keyboard interfaces.
   The relative transport and acknowledged cleanup have scoped native proof;
   they do not establish a portal grant or absolute coordinate mapping.
2. Bind absolute input to the exact approved live capture source and authoritative
   geometry/mapping identity. Desktop bounds or recycled output/node names are
   insufficient. Relative pointer input and touch remain distinct capabilities.
3. Add per-device EI state and frame ownership, typed legacy Notify variants,
   independent unchecked Iced pointer approval and common session revocation.
4. Verify screenshot selection cancellation, physical/TTY/tablet behavior,
   combined media/input lifecycle and existing keyboard/FileChooser consumers.
5. Adopt canonical patches/licences/provenance together, replay full archives and
   run the required shared gate once after relevant supported edits. Resolve the
   full native matrix/distribution and remaining acceptance gaps before claiming
   replacement. The current isolated proofs do not clear those gates.
