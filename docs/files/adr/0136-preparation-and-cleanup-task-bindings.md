# Preparation and cleanup task bindings

Status: scoped schema45 verification; filesystem disposal claims and complete
restart recovery remain required. Continues ADR0135.

Generic preparation previously used original stage identities without an
independent operation/task record. Removing an active cursor allowed startup to
clean its partial copy before request restoration refused. The killed-worker
repro observed that deletion twice. A damaged cursor digest already refused via
the directory-expansion validator; it was not the missing-cursor failure.

Schema45 reserves separate stage_intents records for both lanes. A nonserializable
Registration borrows the actual source and created parent/work/payload handles.
Only that creator may record ordinary preparation. Validate its captured object
proof, original task source and chosen destination before saving an immutable
history/lane/step/task/pending-body binding. Payload registration updates only the
same original empty preparation. Unknown or unregistered payloads stay for review.
A directory expansion retains its own independent validated graph. Recovery checks
these records before preparing directory access or touching staged files.

Forward v2 records bind the original history-file generation as well as its inode.
Cleanup and verified return normalization inherit that proof; they cannot adopt the
file currently at a history pathname. Older job records can use their independent
original execution receipt, while unproved direct work stays in Recovery. Phase
updates first verify the old context. Completion is a private value derived from
the exact Forward, normalized original stage, original task and existing receipt.
Appending canonical history and its empty-only cleanup binding is one transaction,
including descriptor-verified replacement refresh. Cleanup retires binding, context
and journal atomically. It has no disposable payload rights after publication.

A generic descriptive checkpoint must not replace the accepted running transfer
record while its filesystem journal and active cursor are pending. Doing so
advanced the application revision and stranded the original execution after a
payload-save refusal. Preserve that accepted Plan. A separate bounded
request_transfer_interruptions row records descriptive results against its exact
revision and body digest, without replacing or advancing either. Journal failures
remain Interrupted; failure to save the report is SaveFailed, and Retry Save only
retries metadata. Count the report in request budgets and retire it atomically
with a later actual result checkpoint. It carries no execution ownership.

Startup displays the interrupted report and requires explicit Recovery review,
even when an active cursor can already describe a stopped end. Otherwise a trim
failure silently becomes resumable after restart. Following successful review,
restoration reads the durable accepted Plan and independently verified cursor/end
to obtain a real continuation owner. Descriptive errors never supply that owner.
Public worker tests verify failed report saves, two restarts, exact accepted-record
preservation, confirmed folder history, no actions before review, and continuation
after review without replaying completed work.

Verification uses the same 875 recorded inputs: 488 selected backend tests across 20
targets, 254 isolated default-feature library tests, both unignored active/stopped
batch restart gates, default all-target Clippy with warnings denied, formatting and
fixture schema/syntax checks. Failure and decoded source archives retain original
red/green attempts. This does not constitute native, workspace or release acceptance.

These metadata bindings do not finish generic preparation disposal. The existing
staging cleanup still needs durable claim/consumption phases so a second crash
cannot consume a newly reused name, including a link to the same original inode.
Full legacy/reverse Undo, expiry, queued handoff lineage, Trash/Restore, native,
workspace, physical-device, performance and release acceptance remain open.
