# Reviewed replacement restart tasks

Status: scoped backend and isolated default-library verification.
The full five-area goal and complete Copy/Move restart recovery remain open.

## Problem

Two actual public-worker Copy replacement crashes restore an Interrupted row
despite a completed output and intact saved old destination. The committed
Retained version has refreshed change-time fields after the backup rename, while
the task receipt still contains its pre-mutation version. Ordinary receipt
equality correctly refuses the difference. Same-filesystem replacement Move also
needs its own exact task binding; ordinary Move deliberately excludes retained
destinations. Basic return can conceal missing proof because an unchanged history
watermark does not consume a committed suffix.

## Decision

A reviewed replacement has a distinct matcher for its exact source and target,
Copy or Move facts, reviewed old destination and retained stage. Keep Both and
ordinary copied/Move matching are not broadened to infer replacement authority.
Its actual initial Forward intent binds this canonical change before mutation.

Actual reviewed/saved-entry verifiers produce a private, non-serializable
Refreshed value with exact before and after changes. Only the retained entry's
verified metadata and recovery flag change. Store requires the independently bound
Forward context, unchanged original intent, exact active head and existing
receipt. One SQLite transaction updates that receipt's canonical body/digest,
appends the history row and records empty-only cleanup. It maintains the bounded
cursor byte count. Missing or changed receipts refuse; none is recreated. A
failed receipt update or history append rolls back the entire transition.

Restart consumes only that exact independently bound committed suffix. Completed
Copy and Move restore their actual result without opening source or completed
output again. A cross-filesystem Move retains its already published copy and
original source cleanup; Continue verifies and removes only that original source,
without copying again or revisiting replacement review. Saved old data stays in
its original retained stage and history.

Forward phase updates for reviewed Copies, as for Moves, must find the original
receipt and unchanged canonical change. Ready/Returning source claims and
Unclaimed/Returning old-destination claims use the dedicated returned-source
transaction. It validates their exact roles, facts and normalized stage, retires
the uncommitted receipt and saves empty-only Preparing together. A second crash
retains no payload deletion rights and refuses foreign private contents. Once
reconciled, the unconsumed replacement task requires a fresh conflict answer;
the previous approval cannot authorize replacement of a current entry.

A Ready old-destination claim may precede the backup syscall. The restore verifier
checks the still-present original target and records its exact Returning subject
before cleanup; it does not relax the return transition to accept an unproved
Ready state. Unclaimed cleanup still avoids inspecting a changed public target.

Live descriptor-held Copy without source-generation facts retains its prior
path; the refresh does not invent a missing source/task proof or grant new restart
authority. Existing original object, parent, device and publication checks stay
required. No schema or continuation encoding changes are introduced.

## Verification and limits

Nine public OperationJobs parents cover real SIGKILL through publication and
return for Copy, same-filesystem Move and real cross-filesystem Move. They cover
raw-name links, edited completed output/new source contents, later batch work,
saved data, repeated opens/Reconcile/Continue, fresh conflict answers, missing or
changed committed/uncommitted proofs, atomic refresh refusal/rollback and a second
crash with foreign private payloads. Substituted saved entries and destination
parents remain untouched; returning the originals allows recovery to complete.
The original failures, probes and actual fixture roots remain preserved in the
archives referenced by REPLACEMENT-TASK-WIP.md.

The selected backend checks pass 455 parents across 20 Cargo targets, 37 ignored
helpers; the isolated default library passes 223 parents, 17 ignored. Clippy with
warnings denied, formatting and schema42 fixture syntax/hash pass on the same 869
final formatted inputs. Both required stopped/active restart gates remain
unignored. A full-library Ready regression was reproduced, fixed and rechecked;
the exact source relations and formatting capture correction are preserved in
[REPLACEMENT-TASK-WIP.md](../../../.scratch/file-manager/REPLACEMENT-TASK-WIP.md),
alongside failure/source archives, receipts and protected binary hashes.

Saved-container task phases remain next. Broader Preparing/Cleanup provenance,
weak legacy bindings and equivalent phase-update reinsertion in other families,
queued admission/handoff lineage, large restored reports/paged owners, sanctioned
Undo/relocation/expiry and Trash/Restore remain required. Navigation/search/preview
separation, product/device/accessibility/physical acceptance, reliability and
optimized performance, complete native/workspace/release/Shell/Notify/Niri
integration and GTK reference retirement remain open. Focused checks cannot
establish those wider acceptance requirements.
