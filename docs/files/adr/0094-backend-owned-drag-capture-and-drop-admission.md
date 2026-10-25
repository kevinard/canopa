# 0094: Backend-owned drag capture and drop admission

Status: scoped backend workflow verified, 2026-10-19. Iced controls now have scoped
native verification under ADR0095 (2026-10-20). Extends ADR0010/0074/0076/0077.
This is part of the full product implementation, not completion of drag and drop.

## Ownership

OperationJobs owns one drag's original source observation, retained-data lease,
storage requirements, cancellation and single-use admission. A frontend prepares
the captured selection with `prepare_drag`, prepares each hover with
`prepare_drop`, chooses a supported action, and submits the resulting opaque
DropChoice. It does not assemble a transfer by sequencing filesystem checks.
Preparation runs on the existing metadata worker. Initial storage capture and
later action negotiation are memory-only on the caller.

PreparedDrag shares one gesture state across clones and windows. Its accepted
activity hold keeps the application alive and prevents safe device removal until
the gesture ends or hands work to the transfer queue. Cancel invalidates every
hover and choice. After admission, the transfer has its own cancellation; a
source-side drag-end callback cannot undo or cancel that accepted work.
Cancelled preparation and stale drop controls retire even while a platform
removal still holds the admission gate.

The private prepared-selection machinery is shared with clipboard transfers;
preparing a drag does not publish or replace the native clipboard. A claim must
belong to the same owning runtime before changing its state or consuming a Cut.
Clones cannot duplicate admission or transfer execution to a different history.
These opaque types are not serializable. Descriptive requests, saved rows and
native file URIs cannot reconstruct their authority.

## Destinations and actions

A prepared hover retains the destination's original storage observation and its
optional browsing-directory binding. A target outside that binding is refused.
Other targets are resolved on the worker. The resolved original directory is
pinned through the existing bounded selected-entry budget and checked on each
destination access during execution. This catches a renamed directory replaced
at the same path, as well as an alias or managed device changing after hover.
Changes to the contents of the original directory remain allowed.

Repeated preparation of the same pathname within one PreparedDrag now reuses
its first successful capture and rechecks that original storage/binding/pin on
the worker. A view refresh cannot acquire authority for a replacement folder.
The bounded per-gesture observations remain after hover cancellation; a fresh
gesture can capture newly reviewed state. Contents arriving in the original
folder stay allowed. See [ADR0097](0097-drag-destination-observations-survive-refresh.md)
for the actual native failure, resource policy and scoped verification.

Each hover keeps its cancellation token. Leaving it invalidates its existing
choices without consuming the drag, so another target remains usable. The
frontend must still validate the actual native offer, window/tab context and
negotiated action; backend handles do not grant Wayland protocol authority.

Automatic selection uses the existing mounted-filesystem policy: Move on the
same mount, otherwise Copy; mixed or unknown source placement defaults to Copy.
The original missing-source failure remains part of the normal per-item report.
Explicit Copy/Move requires the source to support it. Automatic negotiation may
fall back to Copy; a forced unsupported action is refused. Modifier changes can
recompute a choice without filesystem inspection.

Admission consumes the gesture once and retains original source observations,
retention and destination checks through the existing serial transfer worker.
Conflicts, partial outcomes, result saving, attention and shared Undo use their
established interfaces. An unavailable prepared destination produces a stopped
outcome needing attention, with the original source preserved.

## Verification and remaining integration

At the original backend checkpoint, twelve public tests exercised actual private files: default and forced
actions, unsupported capabilities, duplicate admission, gesture/hover
cancellation, cancellation behind removal, source replacement, raw filenames,
an initially missing source, destination alias/device/directory replacement,
foreign-runtime refusal, explicit Keep both, shared Undo, and actual transfers
between the repository filesystem and the temporary filesystem.

Two new-workflow defects were reproduced before correction. A cancelled release
waited behind active device removal; pending cancellation now retires it. A
local destination renamed after hover allowed Move into the replacement folder;
the original directory pin now stops it before mutation. Early test attempts
also used incorrect Finished/question expectations for established Stopped
outcomes. Those oracles were corrected without changing production outcomes.
An existing history-refusal assertion keeps its established wording.

Sixty headless parent tests pass, including existing clipboard capacity, device
coordination, operation and transfer checks. The default build passes 77 library,
12 drop and 27 transfer tests, all-target Clippy with warnings denied, the build
and workspace formatting. Private child fixtures are not counted again. The
same resulting executable passes ten native Iced clipboard checks, including
windowless ownership, conflicts and shared Undo. Its conflict capture was
inspected. This stage received primary review only.

At the backend checkpoint there was no native adapter. Shared transport is now
implemented under root ADR0023 and the real Iced controls under
[ADR0095](0095-iced-drag-gestures-and-transfer-lifetime.md). Eighteen scoped native
checks cover ordinary transfers, shared Undo, raw selected groups, scrolled and
narrow columns, and cancellation, including buffered Escape. Foreign post-Drop
payload confirmation has scoped external native acceptance under ADR0095;
broader protocol/session coverage remains open. ADR0097 adds repeated-preparation
regressions: fourteen drop parents, twenty-seven transfer and twenty-seven
retained-source parents pass headless, including original device identity during
re-preparation. Ten new native checks and sixteen earlier native regression
checks pass on the destination-capture candidate. No shared transport change is
part of this correction.

Original namespace/journal and restart execution authority, general Retry/Continue,
remaining browser/session extraction, accessibility, performance, Downloads and
full workspace/native/release gates remain open. See
[the backend evidence](../../../.scratch/file-manager/drop-workflow-verification.json)
and [the current work log](../../../.scratch/file-manager/ICED-DND-WIP.md).
