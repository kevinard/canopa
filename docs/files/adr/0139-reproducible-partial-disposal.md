# Reproducible partial-copy disposal

Status: scoped schema48 verification. Continues
ADR0137 and ADR0138. The complete five-area Files goal remains active.

Restart cleanup must preserve a partial copy when its original source has gone
or its bytes contain edits. A private public-worker crash fixture reproduced
actual loss twice after removing the source: startup/Reconcile removed the partial,
retired the journal and reported success. Inspection found neither retained data
nor a published target. Separate public Recovery tests also reproduced deletion
of edited partial bytes while the source still existed.

Original staging ownership proves which object was created. It does not prove
that its current contents are disposable. Capture a reproduction descriptor from
the actual opened copy reader when registering preparation. Bind the original
source parent/entry generations and version into the independent stage intent.
Updating the payload registration must preserve those same source facts. Ordinary
decoded paths cannot supply a descriptor. Schema48 prevents older executables
from applying their earlier disposal policy to the new journal.

Verification reads the retained prefix off the UI thread. Pending-data startup
cost still needs measurement in the performance work.

Live cancellation still completes verified compensation with a separate token;
the cancelled copy token cannot strand the stopped-result checkpoint. Explicit
Recovery comparison remains cancellable. An existing live-copy test reproduced
that regression twice, and terminal-state diagnostics distinguished cancellation
from source validation failure.

Before consuming a claimed partial, reopen the exact original source generation
and compare its bytes with the entire partial prefix through pinned readers.
Compare raw symlink targets without following them. Refuse changed types, versions,
parents, devices, unavailable original proofs and differing bytes. Check cancellation
between bounded reads. A returned original can have a different ctime, so initial
matching allows that namespace change while the actual comparison retains a fresh,
strict snapshot through the live reader.

Keep the verified source/payload handles alive across the consumed-phase save.
Before unlink, repeat current-name, generation and strict live-version checks.
Late edits, replacement sources and cancellation preserve the partial; a consumed
restart still grants only empty-root retirement. Directory preparations retain
empty-only rmdir behavior and never recursively dispose children. Failed source
verification leaves original journal ownership available to Recovery Copy and Keep.

A live source failure must still allow unrelated batch items to continue. If verified
compensation fails specifically because the partial cannot be reproduced, retain
that exact live task's proven work roots through the same atomic retention transaction.
Mark the item failed and explain that partial data is in Recovery, then advance the
batch. A private typed error distinguishes this case from unknown journals, metadata
save failures or replaced work roots, which must still stop for review. Restart
reconciliation never uses this live failure path to reconstruct execution ownership.

Legacy bindings without a captured source descriptor are not upgraded by observing
the current path. Their partials remain protected until explicit Keep. Retention
still requires original work roots and the exact independently bound task; subsequent
Continue derives from the original accepted request/cursor, never the retained data.

Public acceptance exercises missing sources, edited data, two restarts, Recovery
Copy/Keep, returning originals and refusing identical-byte replacements. Additional
cases exercise cancellation during comparison and partial/source changes after the
consumed save. Cross-filesystem Move, old-binding coverage and a queued-operation/cursor case
also pass. Missing-source Keep exposes safe abandonment without deleting retained
data. Final checks on 879 unchanged inputs pass 507 backend tests across 20 targets
(42 ignored helpers) and 273 normal isolated desktop-feature library tests
(22 ignored), with zero failures/filters. Both required batch restart gates,
default all-target Clippy with warnings denied, workspace formatting and fixture
schema/syntax checks pass. Exact attempt relations and preserved artifacts are
recorded in `.scratch/file-manager/partial-disposal-verification-01.json`. Four
private data-loss roots retain 35 decoded verified archive entries; transient
integration-test roots were not retained. The existing Files/Niri binaries and
the rebuilt restart gate are preserved.

This fixes preparation disposal data loss. Restricted created-parent access,
generic Forward/Undo abort cleanup, broader Undo/expiry, queued lineage, large reports,
Trash/Restore and the remaining backend, product/device/accessibility, reliability,
performance, complete native/workspace and release requirements remain open.
