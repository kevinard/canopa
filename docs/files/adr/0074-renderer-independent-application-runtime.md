# 0074: Renderer-independent application runtime

Status: accepted architectural direction, implementation incomplete, 2026-10-18.
Clarifies ADR0001 and ADR0067 following the user's architecture review. The full
product brief and existing execution-safety requirements remain active.

## Problem

Files already has renderer-independent browsing models, filesystem engines,
search, preview preparation, custom actions, configuration and storage models.
However, the historical GTK adapter also owns application workflows. In
`desktop/jobs.rs`, the queue, dispatch, question handling, retry intent and job
lifetime share types and implementation with GTK rows, windows and buttons.
Moving those workflows to Iced therefore requires extracting application logic
as well as replacing widgets. That coupling is architectural debt.

`operation_jobs` has begun the extraction: accepted mutations, transfers and
Undo run behind renderer-independent requests, snapshots and typed decisions.
This is a partial application runtime, not evidence that every workflow is
already independent. Iced still coordinates directory/search/preview work and
some clipboard, refresh and operation-result behavior.

## Ownership

Keep three explicit responsibilities within the local application:

1. Domain modules own browsing semantics and filesystem behavior: path identity,
   selection and navigation, transfers, Trash, Undo, Recovery, search, preview
   preparation, custom-action execution, configuration and storage requirements.
2. A renderer-independent application runtime owns workflow state and sequencing:
   admission, queues, cancellation, progress, decisions, exact continuation
   ownership, persistence, reconciliation, clipboard intent and the lifetime of
   accepted work. It coordinates asynchronous domain work and rejects obsolete
   results using application identities and generations.
3. The Iced adapter translates input into application commands, renders snapshots
   and decisions, and owns presentation state: editing drafts, widget focus,
   scroll placement, geometry, dialogs and native window interaction. It maps
   application changes to visible refreshes without deciding filesystem policy.

Navigation commands retain the same meaning in columns and narrow views. The
renderer chooses how much navigation context is visible; it does not implement
a second browsing model. Application session state describes which browser or
preview owns work; Iced window IDs and widget handles remain in the adapter.

Platform adapters provide clipboard transport, volume observations, file opening
and desktop integration at their actual seams. Native transport and widget
lifetimes may need toolkit-specific implementation. Ownership, original storage
requirements, source checks and operation decisions belong in the core modules.

## Interface and migration

Use small typed command, snapshot and decision interfaces around complete
workflows. A caller submits user intent and observes its outcome; it must not
assemble a safe operation by sequencing independent retention, journal and
filesystem calls. Keep opaque execution plans interpreted by their owning core
module. Saved presentation payloads and retention leases remain insufficient
authority to execute work.

Continue deepening the existing modules incrementally. Extract each workflow's
application coordination before porting its presentation, rather than copying
GTK policy into Iced. Do not introduce a universal command bus or a facade that
only forwards the same sequencing burden to the caller. No alternative renderer
or generic platform abstraction is needed to justify ordinary internal modules.

The backend consists of Rust core modules. The user's subsequent resident/headless
proposal in [ADR0076](0076-window-independent-and-resident-runtime.md) adds a reason
to host that runtime independently and supersedes the initial dismissal of
separate-process hosting. Keep the core interfaces independent of that deployment
choice. The existing single crate and optional `desktop` feature can retain this
separation; a future crate split is useful if it enforces dependency direction
around coherent modules.

## Verification and limits

Exercise application workflows through the same public core interfaces used by
the GUI, using real private filesystem state and the existing storage seams.
Core behavior must be verifiable without the desktop feature. Native acceptance
continues to cover tiling, focus, input, window relationships, clipboard transport
and rendered presentation. Those checks complement core workflow tests.

Rewriting widgets and native interaction is expected during the toolkit change.
Moving duplicated scheduling and decision policy is application restructuring.
Filesystem safety fixes exposed during this work are core correctness work and
must not be reported as an inherent requirement of Iced.

This decision records the intended seam. It does not claim completion of runtime
extraction, queued-source identity, Retry/Continue, crash reconciliation or any
remaining Iced migration acceptance gate.
