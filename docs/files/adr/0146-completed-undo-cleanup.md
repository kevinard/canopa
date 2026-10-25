# Completed Undo retains cleanup of its original work folders

Status: scoped schema54 verification on 893 unchanged inputs. Continues
ADR0142–0145. The full Files goal remains active.

Two actual public-worker deaths after a cross-filesystem Undo reversal committed
left its empty work folder orphaned. Startup and explicit Reconcile succeeded,
but both pending and undo_work were already absent. The original folder's device
and inode still matched; the remaining destination-copy Undo step and later edits
to the returned source survived. Completion had retired the only cleanup owner
before removing its directory.

Schema54 adds UndoCleanup and a separate per-lane undo_cleanup binding. Derive
empty-only roots from the exact pending Undo and original saved history before
completion. SourceRemoved work must still satisfy its independent Undo binding;
other saved roots must match the original history's root generations. Commit the
reversal, its cleanup journal and its history-generation-bound receipt in one
transaction. Refused or ignored writes roll back that whole metadata transition.
No current pathname supplies new root ownership.

The broader saved-version merge test exposed a parent handoff that previously
updated device/inode while leaving an old generation proof. Directory-return plans
now record the selected or created destination generation before moving children.
Direct saved-container moves propagate that proven parent while retaining the
original container generation. Recreated copies keep their existing created-copy
migration proof. When Skip/resume creates parents through a later Undo entry, the
generation follows the exact surviving identity reference in identity_ref_origins;
its foreign key retires it with that reference. An older Undo can then use the
recorded parent after the later return frame retires, without capturing the current
path as new authority. The existing same/cross-filesystem merge test reproduced the
regression and passes all four Skip/resume combinations in the final full matrix
after diagnostic removal.

Saved-only restoration of a missing cross-filesystem destination uses the retained
folder from its exact paired Copied step, while completion consumes the following
SourceRemoved step. The broader matrix twice reproduced an incorrect comparison
against SourceRemoved, which has no retained folder. Share the existing exact
saved-only pair validation between cleanup derivation and completion; preserve its
path, replacement identity and original root requirements. The full 19-parent
cross-filesystem Undo target passes this correction, including killed reviewed and
saved-only returns, and passes again in the final full matrix.

Immediate cleanup and restart use the same receipt. Open the original parent,
verify its generation and the original root, then remove only an empty directory
and sync the parent. Unexpected entries, replacement roots and unavailable parents
preserve the journal. Cleanup never opens, checks or rewrites the completed return
target, whose later occupants and edits belong to the user. Retire only the exact
receipt after every original work root is gone. A second death after removal can
finish metadata without repeating the Undo step.

Recovery exposes whole original work folders. A fresh private review can archive
those folders and retire their exact cleanup journal in one transaction, preserving
extra children without filesystem mutations. Recovery Copy uses its independent
journal; export Undo and the original operation's remaining Undo work still work.
Serialized Recovery rows do not carry the private review authority.

Focused acceptance covers two restarts, preservation of edits to returned work,
completion of the remaining Undo step, reintroduced output at its former payload
name, additional children, Copy/Keep/export Undo, replacement roots, death after
root removal, refused/ignored receipt and retirement writes, and edited or missing
bindings. Final validation passes 551 backend tests across 20 targets (49 ignored
helpers) and 317 normal isolated desktop-feature library tests (29 ignored), with
zero failures or filters in the parent runs. Both required active/stopped batch
restart gates, default all-target Clippy with warnings denied, Files formatting and
schema54 fixture syntax pass. Temporary diagnostics are removed. Exact attempts,
source relations and the isolated default receipt are in
[the verification record](../../../.scratch/file-manager/undo-cleanup-verification-01.json).
Eighteen failed fixture roots retain 168 decoded and separately live-verified
entries across four archives; originals and application/Niri binaries are preserved.
Exact next work is in
[UNDO-CLEANUP-NEXT.md](../../../.scratch/file-manager/UNDO-CLEANUP-NEXT.md).

Pathname-only legacy stages still need original creator or selected-root proof
before restarted cleanup. Their existing live cleanup remains separate. This does
not finish those paths, other generic Forward/Undo abort handling, Trash/Restore,
broader Recovery/Undo acceptance, backend separation, native/product/device checks,
reliability/performance, workspace/release integration or GTK reference retirement.
