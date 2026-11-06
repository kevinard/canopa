# Print startup recovery discovery

Status: accepted for the discovery foundation; recovery review/export remains
in progress and Print remains private and unregistered.

## Context

Publication journals preserve uncertain staged and displaced bytes, but a new
backend previously had no bounded way to discover their original locations.
Ready journals also exist while the document is still being copied. A PDF
header or a nonzero length cannot establish that a retained copy finished.

## Decision

Production requires an explicitly configured private recovery store. Stateless
printer metadata clients do not create user state. The store records an intent
before staging starts and records the exact publication identities before a
publisher can change destination entries. Workers perform all catalog, process,
filesystem and synchronization operations; the owning backend retains only its
existing descriptor bundles and non-I/O configuration.

The versioned catalog is bounded to sixteen entries and two MiB. Normal work
admits fourteen entries, leaving two positions for a subsequent recovery export
path. New work fails before destination changes if discovery cannot be persisted.
Updates use a private nonblocking flock, retrying contention for at most250ms
inside the worker, a single bounded scratch file, fsync, atomic rename and directory
synchronization. Existing malformed, insecure,
symlinked or hardlinked catalog objects are rejected. A missing store is an
empty read-only discovery result and does not create directories.

Each entry records owner and current worker PID, process start time and boot ID.
Registration and discovery's process checks use the same lock. Registration
rechecks owner liveness while locked. Discovery releases the lock before reading
the recorded work location. A live worker, or a live owner of active work, makes
the entry busy. An error checking liveness never proves retirement. A worker
records retained state only after settlement has decided to preserve the work;
it must itself exit before discovery offers that entry. This does not establish
cross-restart global resource accounting or supervision of converter descendants.

Before claiming an existing destination, commit hashes the complete synced
document with SHA-256 and persists a private completion record. The publication
journal records that marker's identity. Marker creation or cancellation failures
cannot proceed to claiming. Settlement recognizes only its own recorded marker;
a foreign marker is preserved. Older journals remain readable and cannot offer
an unproven document as complete.

Discovery reads at most the bounded catalog and its exact recorded locations.
It classifies busy, missing, incomplete, interrupted, uncertain, published and
unavailable work without replaying any transaction. It checks held identities,
private object types, bounded journals, completion metadata and displaced-file
availability. Missing or moved locations remain visible. It never scans the
filesystem or automatically restores, retries, overwrites or deletes retained
data. An entry and its original pathname are evidence, not fresh write authority.

## Remaining work at this decision

[ADR0082](0082-print-recovery-review-and-export.md) implements the subsequent Iced
review, verified export, dismissal and asynchronous startup flow. The constraints
below remain the basis for that implementation.

An Iced review surface must select a retained body, create an immutable snapshot,
verify its completion digest, and require a new Files picker destination and
overwrite review before exporting. A displaced file is an arbitrary byte body,
not necessarily the originally reviewed occupant or a supported print format.
Explicit dismissal must remove only the discovery entry, leaving retained data
intact. The reserved export positions are not enabled by this change.

Startup integration, abrupt backend/power-loss and actual remote/kernel-wait
acceptance remain. This discovery layer does not promote Print or complete its
broader device, layout, Flatpak, ambiguous CUPS or runtime packaging gates.

## Validation

`target/portal-print-40/acceptance-01.json` and
`.scratch/xdg-portals/print/recovery-evidence.json` seal the final inputs and pins.
Formatting, strict workspace/all-target Clippy and the original whole-workspace
all-target incremental build pass. All116 Print and12 Lockdown tests pass; complete
Portal regressions pass334 library and55 integration tests (19/2 ignored).

Seventeen new regular cases cover partial copies, completion hashes, PID reuse,
reboot identity, busy owners, uncertainty, retained bytes, missing/moved work,
foreign markers, private catalog objects, bounds, contention and cleanup. The
cancellation test now observes the actual claimed phase rather than counting
callbacks, since hashing itself checks cancellation.

A native fixture reuses the compiled harness for an owned producer stopped and
SIGKILLed at Publishing. A fresh normal worker first reports Busy, then discovers
the uncertain document and displaced bytes after confirmed death. No destination
is replayed. Ordinary successful publication removes only its own entry, and two
simultaneous publications share the catalog successfully. This proves the scoped
producer-loss case, not complete backend death or physical power loss.

Six filesystem/publication, three retirement and twenty-four production renderer
groups pass. Eighteen public frontend/controller/Production/Iced groups pass:
files in dark/100% and private CUPS in light/125%. All272 observed native processes
are terminal, with no emergency fixture cleanup kills. The four-segment public
matrix from phase39 remains historical evidence for its own inputs.

No new dependency, native Rust driver or binary copy was introduced. Existing
root target/profile/graph and stored native tools were reused. The phase's measured
net storage growth is negative184,418,304bytes, with7,334,580,224bytes free at seal,
within the6GB reserve and3GB growth cap. Native verifiers reuse hashes only while
file identity, size, modification time and change time stay unchanged.

The final validation used host access for owned private bus/socket fixtures;
the default sandbox's earlier32 fixture failures are retained as failed evidence.
A production fixture's0755 temporary root was corrected to use a private child
store. A parallel WouldBlock result led to bounded worker-side lock retries.
Earlier receipts are not presented as final acceptance.

No new full-workspace test gate ran; the ten previously recorded Files integration
failures remain unresolved. Normal startup links no GTK/CUPS/qpdf, and installed
services, queues and routing were not changed.
