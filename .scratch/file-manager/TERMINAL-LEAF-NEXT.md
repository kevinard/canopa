# Continue safe restart recovery

The full user goal remains active. Schema41's terminal copied-leaf slice and
exact scoped checks are in ADR0126 and TERMINAL-LEAF-WIP.md. Preserve source/proof
archives, all red datasets, source27 app, compatible Niri and the earlier required
local-runtime gate. Current no-default required gate may be rebuilt by Cargo;
cache reclamation must keep excluding it. No commits or vendor edits were made.

## Ready/Returning source-task disposition

The late hard-link regression exposed an inode/name distinction. A Claimed
source with an absent private payload has consumed that source entry; a new hard
link of the same inode at the old name is not an authorized return. Recovery now
appends removal metadata without inspecting that new name. Only Ready may infer
that no claim happened; authorized returns retain their explicit Returning marker.

Next add a public-service SIGKILL after the source-to-slot rename while its journal
is still Ready, then reconcile through OperationJobs. The Ready recovery returns
the original safely, but clear_returned changes the journal to Preparing without
retiring the uncommitted SourceRemoved task binding. An advanced cursor then sees
its committed Copy plus an uncommitted removal receipt and refuses continuation.
The equivalent Returning case also needs an exact disposition. This is not yet
verified by a dedicated public-service gate; reproduce it before fixing.

Persist the proved returned/unstarted task disposition atomically with its
Preparing transition. A narrowly validated source-return transition can retire
only its exact uncommitted removal binding; retain the committed Copy proof so
recovery constructs copied-source cleanup instead of recopying. Verify the exact
Forward context, phase, change, operation/ordinal and normalized empty-only stage.
Do not infer a return from a matching inode in Claimed. Consider how legacy loose
uncommitted bindings are refused rather than turned into fresh ownership. Keep
original-source generation/fingerprint checks and NOREPLACE returns. Cover a
second crash during this transition and save-failure rollback. Do not simply
ignore uncommitted receipts to make the existing cursor gate pass.

## Metadata-record failure owner

The final-leaf gate now restores completed metadata and preserves edited/replaced
completed paths, missing parents, links, source-folder reports and repeated crashes.
Application-result failure after its engine end exists has working Retry Save and
activity ownership. Failure while *recording the engine end* instead rolls back
to the intact cursor and currently requires a later reopen/review. Add a real
public-service fault-trigger regression requiring in-process Retry Save here,
then implement a private engine-owned pending metadata witness. No new filesystem
operation should run; corrupt/unproved cursors must never gain such a witness.

Reuse the existing PendingTransfer/TransferConfirmation::Pending machinery if
possible. Mint it only after accepted-request/history, exact suffix, row, count
and journal/access validation. Preserve the validated end when its metadata write
fails, keep the startup/review activity hold, then confirm that end before the
independent application save. Extend the restored-result path to carry the right
confirmation instead of assuming Recorded solely from its saved variant. Do not
serialize replay authority or turn generic request writes into proof. The current
end/retirement refusal tests remain valid rollback evidence; add the in-process
recovery behavior to them. Cover both completed and stopped ends as appropriate.

## Directory publication and child traversal

Next real FS-step seam: transfer.rs reads children from directory::Opened, prepares
an actual directory payload, then commits Change::CreatedFolder through
commit_forward_with_created. Only afterward does it construct DirectoryFinish,
update the directories map and enqueue the already-read child names. A SIGKILL
between these phases still leaves an unbound advanced history step.

Persist the exact expansion before publication: actual opened source/parent,
prepared created payload and parent originals, reviewed destination (including
Keep Both), finish/creation/history scope and the already-read Unix child names.
Use bounded normalized rows, not a giant header or initial-root replay. Bind the
actual CreatedFolder Forward step to that expansion. After proved publication,
advance to those exact children and retained directory facts; do not create the
folder again or accept replacement parents/payloads/devices. Empty final folders
should then use the typed completion path just added.

Merge has no CreatedFolder append: persist the accepted existing folder's actual
opened originals and exact expansion before children run. Its reviewed Merge
choice must not be re-created from current namespace observations. Keep the
existing folder-selection semantics for unprocessed contents. Read ADR0124/0125
and the relevant directory/source observation code before designing the phase.

After expansion, advance completed source-directory removal through its own exact
claim and task proof. Whole-entry Move, reviewed replacement and saved-container
migration need corresponding task phases, rather than broadening the copied-leaf
matcher to arbitrary path-matching changes. Complete Preparing/Cleanup context
binding, queued lineage/admission, legacy data, large reports/paged owners and
sanctioned Undo/relocation/expiry. Trash/Restore and all other goal areas remain.

## Current implementation facts

cursor::Recovered is internal and not deserializable. Store::recover_active_transfer
records CompletedTransfer and returns None so the existing worker completed-end
reader saves its report; stopped results retain their strict nonempty invariant.
The connection-level transfer_completion::record helper atomically records the
end and retires the cursor in either path. Source-folder report rows use role7,
eight counts, scoped digests and existing byte triggers. Normal live reports still
refresh at return; restart uses the last durable descriptive observations. These
rows grant no mutation authority. Six/seven-counter decoding and old task-table
migration remain supported by code; comprehensive legacy acceptance is still open.
