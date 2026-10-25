# 0055: Preserve clipboard source placement from selection to Retry

Status: accepted under delegated implementation authority, 2026-10-17. Verified
for the recorded clipboard placement scope; the full product goal remains active.

## Context

Copy/Cut published native URI paths and an intent counter. Paste captured storage
placement later, using its destination Browser as the origin. Replacing source
media between Cut and Paste therefore made the replacement device appear to be
the requested source. Two isolated native runs moved its different bytes while
the original selected file remained on the held original device. The observed
replacement UUID and exact native Cut payload rule out provider and intent errors.

## Decision

An application-owned file clipboard retains the immutable storage snapshot taken
at Copy/Cut. Browser selections bind their displayed source folder to the resolved
folder from the worker's Directory snapshot. The native URI formats remain
interoperable. The UI performs no filesystem reads to create this capture.

Paste captures the destination independently at request time. A Job keeps its
source and destination placement snapshots separately: engine SourceEntry checks
use the captured source; DestinationFolder checks and normal preparation use the
destination snapshot. A device the user deliberately chooses as a destination
after copying a local source therefore remains available for that new request.

Retry/Continue refresh availability for both snapshots while retaining their
original media requirements and source/destination browsing bindings. An original
stable UUID can return; replacement media cannot authorize the old selection.
The existing conservative policy for media without stable identity still applies.
Repeated retries retain bounded requirement maps, not chains of snapshots.

Partial Cut publication keeps the source capture and exact intent. External
clipboard ownership changes, empty completion and failed publication release the
capture. A newer selection retains its own capture and intent when older work
finishes. The generic clipboard publisher used by Trash captures known placement
without claiming a Browser binding.

## Verification and limits

The native regression uses the real Browser, native clipboard, normal Operations
Retry/Undo and the private GIO provider with distinct UUIDs and separate real
temporary trees. It also tests Copy via an alias, alias retargeting and a later
independently reviewed destination. Final native141-146 pass 76 checks with all
41 captures inspected: dark and light1.25 clipboard acceptance across two actual
filesystems, existing destination safety, mixed-source failure isolation,
ordinary directory Retry with newer clipboard selections, and Downloads. The
secondary filesystems are removed after shutdown. Additional same-filesystem
acceptance is recorded separately.

The final 103 application/build inputs remain unchanged through those runs.
All-target Clippy, default build, formatting, fixture inventory/syntax and packaged
documentation pass. Only three desktop inputs differ from the settled 345-test
core candidate, so its unchanged core/test evidence is reused without claiming
a new core run. See the
[verification record](../../../.scratch/file-manager/clipboard-placement-verification.json)
and [implementation ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).

External file clipboard owners provide paths without Canopa's earlier source
observation; their requests continue to capture known placement at Paste. This
does not establish drag-origin binding, an inode lease on queued ordinary saved
files, persistent unfinished jobs or atomic descriptor authority between checks
and filesystem mutation. Those remain in the active full product goal.
