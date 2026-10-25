# Nested created-folder access during continuation and restart

Status: scoped schema49 verification.
Continues ADR0140 using the existing schema49 records. The full Files goal is active.

A real stopped Copy with two created destination folders changed to 0400 failed
Continue twice before reaching its unfinished partial. The backend tried to open
the immediate parent while its outer created ancestor was still non-searchable.
Borrow temporary owner access from outermost to innermost owned created folder.
Before changing permissions, require the recorded directory and parent generations
through the actual opened objects used for the borrow. Missing original proof or
a replacement folder/device must refuse the step. Completed files are not replayed.

A second real process-death fixture kills the Recovery permission-release worker
after restoring the outer folder and retiring its grant, while the inner grant is
still queued. Two runs reproduced startup leaving the original inner folder at 0500
behind the restored outer 0400 folder. Deepest-first path traversal alone could not
open the descendant to restore it.

For each abandoned grant, reopen only its recorded original ancestors in increasing
depth order. Borrow read/search access when the ancestor retains its original mode,
saving and verifying a separate durable restoration grant before chmod. Existing
external searchable modes are not claimed. Restore the target and retire its exact
grant, then restore all opened borrowed ancestors in reverse order even when the
target is unavailable or a later save fails. Another process death can repeat the
same traversal from the durable records. Validate each grant's stored path key
against its body. Failed restoration leaves its grant for a later attempt; it never
refreshes original-generation evidence from current paths.

These changes use existing schema49 access contexts and grants. They introduce no
new journal representation or transfer execution authority. Recovery retention and
permission access remain separate from the original accepted continuation cursor.

An additional public sequence reproduced two live mode regressions twice. While
an older Recovery selection remained alive, changing both folders to 0701 or 0444
and invoking Continue finished the transfer but replaced those edits with 0500.
Borrowing had reused the older reader's original 0400 unconditionally. Reuse that
original mode only while the actual mode still equals the recorded read/search
grant. Otherwise preserve the observed external mode. Writer restoration extends
read access only for a reader with the same original mode and object/parent proof.
A fresh catalog must acquire the newer recorded context when an older selection
has lost search permission; replacing its registry token prevents an older release
from restoring or retiring the new grant. Both mode edits, fresh inspection and
another restart pass while older selections remain alive.

Eight focused acceptance parents cover nested Copy/Keep/Continue with edited
completed files and
partial data, a second restart, a death between ancestor/descendant releases, a
second death after startup commits a temporary ancestor grant, refused/ignored grant
saves followed by retry, replacement ancestors during restoration and before
Continue, and both live external-mode edits. Final all-target default Clippy with
warnings denied, Files formatting and fixture schema/syntax checks pass. The final
inputs09 run on 882 unchanged build inputs passes 519 backend tests across 20 targets
(47 ignored helpers) and 285 normal isolated desktop-feature library tests
(27 ignored), with zero failures or filters. Both required active/stopped batch
restart gates pass. The earlier inputs07 matrix passed 517 backend tests before
the newly reproduced live-mode regressions required another change and final run.

Exact results, source relations, failed attempts and protected binaries are in
`.scratch/file-manager/nested-access-verification-01.json`. Two failure archives
preserve nine private roots and 195 decoded/live-verified entries. Bytes, modes,
mtime and object identities match; ctime changes from controlled archive-only
permission borrowing are recorded. The application/Niri binaries and required
restart gate remain preserved. No main application binary was rebuilt here.
Complete native/workspace, physical-device and release acceptance remains open.
