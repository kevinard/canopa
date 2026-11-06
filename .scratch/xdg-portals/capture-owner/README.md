# Capture creator lifetime

Status: canonical compositor source, Rust admission and scoped production native
acceptance verified, 2026-11-01. Full workspace remains red on ten Files tests.
ScreenCast/RemoteDesktop are unadvertised; companion distribution and the broader
portal goal remain open. No installed service or routing changed.

A killed capture worker previously released held input and closed its public
session while leaving a real PipeWire producer alive. Independent two-client
native tests reproduce the same leak: the first creator disappears, but its
Session/producer survive and the second recording stays live. Explicit Stop
removes each producer. The creator's NameOwnerChanged event arrives, identifying
the missing compositor ownership rule. Original failures and exact fixture pins
remain in evidence/ and target/portal-pointer-lifecycle-14/.

The correction subscribes to a session creator's unique-name disappearance
before publishing the session, rechecks liveness after registration and stops
only that recording on creator/notification-stream loss. Normal Stop cancels the
watcher and prunes the registry; weak references avoid a retention cycle.
CanopaCaptureOwnerVersion=1 identifies this contract. Both Rust owner resolution
and the recording worker require version 1 before CreateSession; initial admission
runs before Iced consent.

## Verified evidence

- The maintained compositor regression covers independent live owners and queued
  CreateSession/RecordMonitor calls whose callers disappear before dispatch. A
  private bus monitor observes the actual reply before checking the object tree,
  preventing a pass before the queued work runs. All three cases fail on Niri 84ac
  and pass repeated runs on Niri 925c. The other owner's real producer stays live.
- The maintained production admission test fails on portal 4a95 by exposing consent
  on the unproven compositor, then passes on portal ec73 with response 2, no windows
  and no recording objects. Fixture preconditions verify an otherwise supported
  capture API and missing cleanup contract. Unit tests cover absent/unknown
  versions and a changed contract between owner resolution and worker creation.
- The rebuilt portal/harness pass all six public mixed-source lifecycle cases:
  capture-worker death in both themes, selected source closure, caller death,
  backend death and the Iced stop control. These total 55 groups and 66 video captures.
- Standalone ScreenCast passes 17 groups per theme on those same production bytes.
  Across both themes it verifies 14 ordinary video captures plus 12 cursor captures,
  restricted frontend FDs, source pixels, static resume and producer/control
  retirement. Combined scoped production acceptance totals 89 groups and 92 captures.
  All pins remain unchanged.
- The explicit niri-desktop-pointer recipe reproduces all 5961 compiled input files,
  normalizing only two local Smithay paths. Both upstream archives, original
  licenses, changed-file inventory and reviewable patches are retained. Prepared
  source reuse keeps the stable Cargo paths/cache and refuses changed inputs.

Niri 925c is SHA-256
`925c49c8745972e609d014886b3af251ee1d53d05677d95c6eaf6404c5bb32ed`;
portal ec73 is `ec73f98433853eb761a5f32dc11f28b9af748a17bff86a0c39ca3e82fd552dd7`;
frontend cb35 is `cb35d770e84578dec58682cc2deeb0b8e2a93b1458e2371b3c0ce2036bf2a4d7`.
The final-validation receipt records full paths, harness identity and every run.
The earlier 89 groups on pre-admission portal 4a95 remain historical evidence and
are not substituted for the rebuilt Rust results. After the final failing/passing
admission pair, the inactive pre-admission portal/harness raw copies are retired
from /tmp: their existing gzip archives reproduce both exact checksums. This
reclaims 292,405,176 bytes of tmpfs; current executables and Cargo caches remain.
The retirement receipt is linked from evidence.json.

## Shared validation and storage

The full original workspace/default-feature/all-target gate runs once with
incremental compilation, one job and no application exclusions. Tooling, fmt,
Clippy and attribution pass. All Rust executables complete using `--no-fail-fast`:
Portal 163 library/43 integration pass; Files integration 596 pass/10 fail/41 ignored.
The ten failure names match the previous gate: two timing failures and eight
saved-history/child-crash failures. This comparison does not establish causality
or classify them as harmless. All remaining original Settings, CLI, configuration
and fork stages pass, including 4 library, 3 event-batch and 6 pipe tests.

The 4332-input guard records only an expected capture-owner/draft.json diagnostic
status change; compiled sources remain unchanged. The new maintained admission
script was added while the gate ran and independently verified by its red/green
native pair. Documentation updates follow gate completion. Storage growth is
about 2.58 GB under the 3 GB guard, with about 20 GB free. Root target/cache and stable
fork paths are reused; no additional portal or compositor binary snapshots were
copied for these cases.

[evidence.json](evidence.json) links the canonical source proof, build, failing
controls, maintained regressions and final gate/native receipts. See
[ADR0056](../../../docs/adr/0056-capture-creator-lifetime.md) for the contract.
The initial hardware-GL startup timeout and later successful controls remain
recorded separately; successful reruns do not establish the timeout's cause.
Broader projection/loss/race, physical/touch, distribution and real Flatpak
acceptance remain open, alongside InputCapture, Clipboard, GlobalShortcuts,
Print and Usb implementation.

Later Clipboard development rebuilds root debug outputs. The exact Portal14
production portal/harness are preserved in verified gzip archives linked from
evidence.json. Replay them from a separate private extraction path when needed;
do not overwrite current Cargo outputs. The archive receipt retains both full
checksums and verifies lossless restoration.
