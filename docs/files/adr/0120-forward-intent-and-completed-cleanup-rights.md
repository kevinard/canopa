# 0120: Verify the forward step before recovery and retire payload cleanup rights

Status: scoped verification; complete restart ownership remains unfinished, 2026-10-22.
Continues ADR0119. The full Files goal remains active.

Correct file generations do not prove which history step owns a mutation. Real
SIGKILL replacement probes show that changing a pending operation, ordinal or
occupied history slot moves the payload before append rejects the record.
Changing the target or removing context can complete outright. Independent
opened-object facts remain necessary but cannot authorize a different step.

Reuse pending_bindings to save a Forward proof in the same intent transaction.
It records the history identity and a digest over the protocol domain, history,
journal lane and canonical complete Pending::Forward. Before directory-access
preparation or recovery movement, require matching proof/context, an existing
operation and its next unoccupied ordinal. New opened-object claims without
their original proof stay in Recovery. Keep old stored ownership references
readable; do not synthesize a new proof from a decoded pending record.
The digest establishes record coherence. Original opened-object facts remain
independently required; this record does not create a continuation owner.

A separate post-append SIGKILL probe shows cleanup deleting completed output
after that exact object is moved back to its former private payload name.
Committing a change ends the directory's right to delete a payload. Derive and
persist empty-only rights atomically with append, use those same limited rights
for immediate cleanup, and constrain restarted older Cleanup records similarly.
Unknown occupants, including the originally published object, remain intact.
Reject unfinished claim roles rather than silently discarding them.

Schema35 gates this interpretation. Preparing, Cleanup and StageMigration still
need complete ownership binding; legacy pathname-only reconciliation, general
Undo/expiry, saved-container and empty-folder claims remain open. Actual restored
Retry/Continue owners and typed handoff/abandon lineage are not implemented by
this change. Keep their required public crash gate enabled.

Scoped backend verification passes369 parents across19 no-default-feature
targets. Its sole failure remains the unignored restarted Retry/Continue test.
The library passes75 parents, including eight binding probes for Copy and Move
(16 actual killed workers, two opens each) and four returned-output cleanup
cases across both lanes. Default all-target Clippy and formatting pass. The
isolated default desktop library passes156 parents,6 ignored, on unchanged
captured inputs. The earlier undiagnosed video timeout remains open (ADR0119).
No native application rebuild, complete workspace/release or physical-device
acceptance is established by these checks.
