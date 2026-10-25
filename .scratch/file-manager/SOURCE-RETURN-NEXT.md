# Continue restart recovery

The complete five-area user goal remains active. Returned-source Ready/Returning
continuation now has a dedicated atomic task disposition (ADR0127). Preserve the
source/proof archives, red roots, protected app/Niri/earlier restart gate, and all
previous checkpoints. The current no-default gate may be rebuilt by Cargo, but
cache reclamation must continue excluding it. No commits or vendor edits.

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


## Returned-source implementation worth retaining

Store.returned_source_intent is distinct from generic intent writes. It verifies
the exact normalized source_removal::returned_transition and original Forward
context, then retires the uncommitted removal receipt and writes Preparing in one
transaction. transfer_cursor::active_head is shared by binding and retirement;
retirement compares exact key/task/body/digest and checks actual next ordinal.
The committed Copy proof remains. Kernel return operations still use NOREPLACE;
Claimed-with-absent-payload never infers a return from a matching inode. A second
crash leaves only empty-stage cleanup rights; unrelated private payloads refuse.

The existing public gates cover corrupted Forward/step/head proofs, deletion and
Preparing-save refusal triggers, later successful review, repeated opens and
identical-byte source/output replacements. Keep these guards when implementing
metadata ownership and new task phases. Legacy loose uncommitted bindings and
complete Preparing/Cleanup provenance still need their own acceptance; do not
relax history suffix checks or mint fresh execution owners to bypass them.
