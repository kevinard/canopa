# Capture source identity and absolute pointer mapping

Status: prepared Rust compositor changes with scoped native evidence, unadopted,
2026-10-31. RemoteDesktop remains unregistered by normal main and unadvertised.
The public worker and Iced consent still support keyboard grants only.

The [metadata draft](capture-mapping-draft/README.md) pins actual live Output
objects and unique window IDs at Record time. MonitorInfo and an optional
canopa-source-id precondition support future consent-time identity checks.
Stream Parameters returns live logical geometry and a stable per-stream mapping
ID. A monitor name, cached IPC projection or recycled PipeWire node is insufficient
to identify a source. The existing dynamic picker remains compatible, but supplies
no absolute mapping before selecting a fixed source.

The preserved baseline reproduces 1×1 window metadata and monitor size that stays
stale after scaling. Four tests using real Smithay Output objects pass after
correcting thread-local insertion to shared userdata. The prepared metadata
candidate passes 15 native checks in 4.28 seconds and 17 existing Rust/Iced video
groups at 125% scale in 88.58 seconds. Its incremental build preserves 5,959 source
inputs and takes 190.84 seconds. Evidence: target/capture-source-metadata-04/.

The [mapped pointer draft](mapped-pointer-draft/README.md) adds OpenMappedPointer,
which binds an FD lease to 1–16 active streams belonging to one capture session.
It returns the selected stream IDs, mapping IDs and immutable logical sizes.
Absolute commands name a bound stream and use that stream's coordinates. Current
output origins are applied once; hardware transforms are not repeated. Window
projection follows rendered surface origins and capture's rounded pixel bbox.
An obscured point cannot redirect input into another window. Source loss,
geometry change, Stop, lock and FD/I/O failure retire the source and held input.

| Check on the mapped candidate | Result |
| --- | --- |
| Wire validation and canonical stream/session paths | 8 passed |
| Actual capture pixels and pointer delivery at 125% scale | 16 groups, 5.65 seconds |
| Same cases with nonzero client window-geometry offsets | 16 groups, 6.20 seconds |
| Existing relative pointer, bindings, drag, cleanup and keyboard regressions | 94 checks, 80.43 seconds |
| Formatting of ten changed/added files | Passed |
| Incremental dev build, same compiler/cache/profile | 189.10 seconds, 5,961 unchanged inputs |

Mapped candidate SHA-256:
`c56e0779dfa85234813bb654a378e30b15206434c95e808a8673f5ca5302552b`.
All owned jobs and private process groups terminated. The native results retain
binary/fixture pins. The first native run's numeric-ID-only removal check is
preserved; the corrected serial-aware check proves a retired node ID was reused
by a PipeWire Client object while both original producer identities were gone.
Both earlier compile errors are retained too. Evidence:
target/remote-input-mapped-pointer-05/.

Layered replay checks the exact 20 changed/added files against the accepted
canonical compositor source and four successive draft deltas. This is not full
archive verification, recipe adoption or a full workspace/native matrix pass.
The preceding shared gate remains red with ten unresolved Files failures; earlier
native matrix failures also remain. No installed service or routing was changed.

## Next integration

1. Portal source-ID negotiation through consent and recording is now implemented
   on identity-capable providers; see [the evidence](portal-source-identities/README.md)
   and ADR0048. The code passes 145 library, 37 integration and 40 native groups.
   Canonical adoption/distribution of the compositor extension still precedes
   pointer authorization; legacy view-only capture retains connector semantics.
2. Capture-bound private pointer worker ownership and lease renewal are now
   implemented; see [the evidence](pointer-workers/README.md) and ADR0049.
   The slice passes 151 library, 37 integration and 62 native groups, including
   two 192-transition resize runs. Renewal requires confirmed cleanup and the
   same source identities with changed size; interrupted commands are not replayed.
   EI regions are immutable, so public device replacement and held-state
   coordination on geometry change still need integration.
3. Add EI relative/absolute/buttons/scroll with per-device frame ownership, typed
   legacy Notify pointer requests, and fresh, unchecked Iced pointer approval.
   [ADR0050](../../docs/adr/0050-pointer-notify-and-stream-pairing.md) now supplies
   public mapping-ID/logical-size publication and the typed receive-ordered
   pointer Notify foundation, with 151 library, 43 integration and 51 native
   groups passing. Production transport and runtime grants remain disabled.
   Verify the public mapping-ID handoff and EI region/device layout against the
   primary protocol before implementing that adapter. Keep the private command
   socket distinct from an EI descriptor.
   [Atomic scale integration](ei-scale/README.md) now resolves the window-only
   scale prerequisite: descriptor version 2 pins physical scale, retires the old
   held source on scale-only change, and permits verified worker renewal. It
   passes 153 library, 43 integration and 198 native groups with v1 compatibility.
   Actual immutable EI device replacement still needs implementation.
4. Complete window behavior for inactive workspaces, overview and remaining popup/
   animation cases. Current rejection of unrepresentable points is an interim
   behavior, not complete window-only remote control.
5. Adopt canonical patches, licences and provenance together; verify full source
   archives and run the required shared gate once after supported edits. Complete
   combined media/input, physical/TTY/tablet, full matrix and distribution
   acceptance before advertisement.

The full 18-interface roadmap remains active. Touch is an independent capability;
none of these pointer results establish touch or the remaining portal interfaces.
