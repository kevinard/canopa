# 0077: Core-owned prepared Copy/Cut workflow

Status: scoped workflow implemented and verified, 2026-10-18. Continues
ADR0069/0072/0074/0075; broader execution/runtime extraction remains incomplete.
The full product brief and resident/headless direction in ADR0076 remain active.

## Evidence and interface

Native Copy and Cut regressions reproduced Paste using an unrelated replacement
placed at the selected pathname after publication. Admission-time observation
was too late. Prepare the owned selection through OperationJobs before publishing
it. The opaque PreparedSelection combines selected-entry observations, original
Storage binding and retained-data protection. Paste accepts that prepared handle
and an independently captured destination. It never observes the owned sources
again. Preparation and durable admission run on the metadata worker, including
while another filesystem operation waits for a decision.

ObservedSources shares the existing process-wide descriptor budget. Binding it
to a destination creates an immutable ObservedTransfer; only paths in the
original observation may be bound. Missing or changed selected entries remain
isolated failures. Clones share observations and their reservations.

## Cut state and durable completion

The core owns the current selection and a revision. It permits one unsettled
Paste per Cut; Copy can be pasted independently. Admission failure releases the
claim. A saved result removes only confirmed consumed selections, preserving
failed, skipped, unstarted and original-folder remainders. A result requiring
reconciliation preserves the intent. A failed result save keeps the Cut claimed;
Retry Save checkpoints that retained result and then settles the selection.
Retrying a failed pre-execution checkpoint retains the claim for actual dispatch.
Neither path reconstructs or executes a saved transfer plan.

Independent prepared handles cannot clear one another. The native adapter
mirrors a newer core revision only to its matching still-owned transport token.
It retains publication generations, asynchronous transport bookkeeping, window
origins and visible error/refresh handling. It no longer computes Cut remainders
or sequences retained-source acquisition and transfer admission itself. A failed
native update is reported without an automatic event-loop retry.

## Retention through physical migration

A public partial-Cut regression moved a directory containing retained versions
across filesystems, skipped an unrelated file, then reproduced refusal when
pasting that original remainder. The first implementation reused an immutable
RetainedSources capture whose stage identity had become obsolete.

The prepared selection instead keeps its original live lease. Admission copies
that lease's current reservations in the same short transaction that accepts the
request. Catalog migration already relocates those reservations atomically.
Validate the lease's original history and live source-lease record, then apply
the existing catalog-owner, pending-journal and storage-budget checks. Do not
recapture catalog dependencies from current source pathnames. Conservative
sharing of the original lease's dependencies is allowed, as in ADR0068, until
intent and stopped-request owners release them. No unrelated stages are adopted.

This remains retention authority. Ordinary selected-entry identity and original
Storage checks remain separately required; a lease does not permit consuming a
protected payload, descriptor-relative mutation, saved replay or restart.

## Verification and limits

Verification uses the established public runtime/engine and native clipboard
seams with real private files. It covers repeated refusal, deliberate reselection,
partial Cut, concurrent Cut admission, independent intents, foreign-history refusal
and physical catalog migration. The full core suite passes450 parent tests plus
one isolated descriptor-budget helper; the default Iced library passes51 tests.
All-target Clippy, formatting and the normal default build pass. Both scoped
review axes have no unresolved findings.

The native matrix passes20 checks across Copy/Move in dark100 and light125, then
30 existing clipboard and retained-data lifetime checks across both themes.
Those include closing every Browser while retaining native file ownership and
100 real history operations before Paste of protected retained bytes. All eight
application logs are empty. Evidence and exact source/binary linkage are in
`.scratch/file-manager/clipboard-source-verification.json`. These scoped checks
are separate from the full product and resident/headless acceptance gates.

Observations do not freeze contents or pin every descendant. External clipboard
selections have no owned Copy/Cut observation and are observed at Paste. Source
request/journal association, original-media/restart authority, safe Retry/Continue,
complete application-runtime extraction and a resident/headless host remain
separate implementation and acceptance gates. No daemon mode is added here.
