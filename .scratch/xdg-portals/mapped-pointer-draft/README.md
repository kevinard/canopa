# Draft capture-bound absolute pointer lease

Status: unadopted Rust compositor transport, 2026-10-31. This ten-file Niri delta
extends the live capture metadata and pointer ownership/lease drafts. It does
not yet grant pointer control through the public portal.

OpenMappedPointer and MappedPointerVersion=1 add an FD-owned binding to 1–16
unique, active streams from one capture session. The returned capability mask 31
and stream/mapping-ID/logical-size descriptors are specific to that binding.
Selected pins alone, streams from another session, missing producers and reused
node numbers cannot create a mapping. OpenPointer still returns mask 15.

Absolute commands use kind 5 in the existing bounded, acknowledged transport.
Each command names a bound stream and a point within its immutable logical
region. Monitor points use the current output origin without repeating hardware
transforms. Window points follow rendered surface origins and the same rounded
pixel bounding box used by capture. Occluded points, overview and points outside
the selected window's input region cannot fall through to another surface.

Source loss, session stop, changed region size, lock and FD/I/O loss retire this
source and its held buttons/grabs. Idle refresh observes invalidated regions even
without another input command. Geometry changes require a new lease; the future
EI adapter must replace its immutable regions accordingly. See protocol.md.

Seven wire tests and one canonical-path test pass. One frozen compositor passes
16 native mapping groups in 5.65 seconds, plus the same 16 with a nonzero window
geometry offset in 6.20 seconds. Actual video colors and Wayland pointer delivery
agree at 125% scale. Checks include movement, negative output origin, rotation,
resize/scale renewal, cross-session/bounds rejection, held-input Stop cleanup,
occlusion and idle source destruction. The existing 94 pointer/drag/keyboard
regressions pass in 80.43 seconds. All owned jobs/processes terminated; fixture,
binary and 5,961 build-source pins remained unchanged.

The first native run passed 13 groups and then used numeric PipeWire IDs alone
for removal. The corrected check also matches object serials; the registry proves
that a freed node ID was reused by a PipeWire Client object while both original
producer identities were gone. That failed run and both earlier compile errors
remain preserved in target/remote-input-mapped-pointer-05/.

The 189.10-second incremental dev build reuses the established cache/profile.
Formatting passes for all ten changed/added files. Layered replay checks 20 files
against the accepted canonical source plus all four draft deltas. This is not a
new full archive verification or canonical recipe adoption. Niri's GPL and the
derived Smithay adapter's MIT attribution remain intact.

Portal source-ID negotiation, worker lease ownership/renewal, EI and legacy Notify
pointer adapters, Iced explicit permission, remaining window activation behavior,
full workspace/matrix, physical input and distribution acceptance remain. The
full 18-interface roadmap stays active; canonical recipes, normal registration,
shipped descriptors and installed services remain unchanged by this slice.
