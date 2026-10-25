# Bound Undo return work and explicit retention

Status: schema50 retention foundation verified on undo-abort-inputs-09.json.
Continues ADR0139–0141. The complete five-area Files goal remains active.

A real public worker first completes a cross-filesystem Move, then dies during the
first64KiB of Undo's return copy. Editing the partial and invoking public Reconcile
reproduced actual loss twice: Reconcile reported success, removed the payload and
work folder, cleared its journal and left no Recovery record. Matching a created
object's identity did not prove its current contents disposable.

Bind this Undo preparation independently before data writes. The private creator
registers its actual stage/root/source facts against the original history-file
generation, operation/ordinal, canonical stored history step and exact journal.
Payload registration preserves the same source descriptor and empty preparation
scope. Generic journal writes cannot mint or replace the binding. A live Keep-both
relocation normalizes only the recorded return name for comparison; it never
rewrites the canonical history owner. A live explicit review may also select newer
destination contents: comparison normalizes only the reviewed version and matching
return fingerprints, while registration verifies that version against its actual
opened source. The canonical hash still names the original stored Move. The source
proof and exact journal prevent a later decoded record from minting approval.
Reconciliation verifies the binding before
borrowing directory access or mutating paths. Older unbound Undo work is not
upgraded from current paths and remains copyable without cleanup authority.

An interrupted return with a payload stays journalled. No identity-only generic
cleanup consumes it. A payload moved back to the source cannot complete Undo unless
its original object and full source fingerprint match; an incomplete/edited return
remains protected alongside the original moved destination. Recovery also exposes
the whole work folder, including an empty folder when the partial was moved away.

A nonserialized fresh review permits Keep for bound return work without displaced
Undo data. It captures exact journal/binding/access records and current root/payload
versions. One verified transaction archives both the available leaf and entire work
folder as protected, nonexpiring Recovery data, indexes them, and retires only the
captured journal and binding. Unexpected additional children stay in the retained
folder. Refused/ignored writes roll back. Replacement roots, changed journals or
changed review versions refuse Keep. No file name is moved or unlinked by retention.
The original Undo history remains available. Recovery copies retain their ordinary
Undo entries, so retry follows the existing newest-first Undo ordering.

Schema50 prevents older executables from applying identity-only cleanup to the new
Undo work records. This is a safety/retention foundation for interrupted return
preparation, not completion of all Undo or generic cleanup recovery. Automatic
verified disposal of reproducible Undo partials, displaced Undo work, empty-root
cleanup after completed reversals, and other generic Forward/Undo abort paths still
need their own durable task-bound transitions and complete acceptance.

Broader acceptance caught a regression in reviewed cross-filesystem returns and
reproduced it independently: approved newer bytes differed from the original Move's
fingerprint. The normalization above preserves that existing workflow. A separate
preparation preflight rejects original-location edits made during the conflict
question before creating a return payload, after validating the original parent.
Installation repeats its existing conflict check for changes during the copy.

Seven focused public acceptance parents pass: two restarts, edited partial Copy,
whole-folder Copy with extra children, Keep, explicit Undo retry; incomplete files
returned to the source; changed journal/owner refusal; failed retention transactions;
replacement work roots; legacy unbound copies; and Keep-both relocation. All-target
default Clippy with warnings denied, Files formatting and fixture schema/syntax pass.
Final verification passes 526 backend tests across 20 targets (48 ignored helpers)
and 292 normal isolated desktop-feature library tests (28 ignored), without failures
or filters. Both required active/stopped batch restart gates pass. All 884 captured
build inputs stayed unchanged. Exact attempts and source relations are recorded in
`.scratch/file-manager/undo-abort-verification-01.json`; the final source archive is
tracked by `undo-abort-source-01-verification.json` beside it. Complete native/
workspace, physical-device and release acceptance remains open.
