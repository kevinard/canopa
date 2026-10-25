# Cross-filesystem saved-container recovery — implementation working notes

This is a proposal informed by inspected code and actual red probes, not a claim
of implemented recovery. Full user goal and earlier gates remain intact.

Observed: context::prepare already maps a forward StageMigration to a synthetic
Forward with stage=None. require_forward ignores StageMigration entirely and
requires next==ordinal for an uncommitted Forward. publish_saved_stage appends
and clears pending bindings, then installs catalogued=true migration in the same
transaction. Its completed Change retains the original context. Cleanup therefore
needs committed context validation, not a new capture of owners after cutover.

Keep serialization and task matching distinct from same-filesystem entry Move.
One possible compact design is Movement's original container proof plus copied
bytes and a digest of an immutable migration recipe (original source stage,
version/fingerprint, target stage/ownership/copy fingerprint, and deterministic
root-claim path). Build it only from actual held/verified preparation. The digest
binds the full static recipe to the independently stored exact task receipt;
it is not execution authority by itself. Recovery still verifies original
objects and parents before mutation. A source/target snapshot only in the journal
without an independent matching receipt is insufficient. Require a schema bump
for new proof-bearing fields; do not reuse ordinary move_facts for a copied root.

Capture strong Movement through Selection before copying (factor the existing
Prepared.saved capture, not current path metadata). Preserve original previous
from the catalog's existing proof. Source stage ownership can then contain that
previous proof. Newly created target stage already has parent/work/payload
originals; after folder->previous and work-root publication, convert that payload
proof to Ownership.previous, granting no generic payload deletion rights.
Store.rebind must preserve this actual target ownership across active/archive
owners and later migrations. Unknown legacy source proofs refuse rather than
freshly adopt; current Undo legacy handling is a separate unfinished audit.

An actual StageMigration starts after complete descriptor verification, before
publication. Bind the exact active head and canonical Change in the same intent
transaction. Context for that static recipe should include the target stage and
its original facts. Before any restart directory access, require context and
receipt. After catalog cutover, validate the committed canonical Change/context,
matching static recipe and existing receipt; never reconstruct owner bindings by
capturing current namespaces. Phase saves need a dedicated verified transition
that preserves committed context, rather than generic intent recapture.

Publication uses the original destination parent/work/created payload handles:
verify source parent/container/previous and version, verify target roots and
payload fingerprint, rename folder->previous NOREPLACE, sync, publish the actual
work directory NOREPLACE, verify the published original root/previous and parents,
then atomic catalog/source-reservation cutover and append. On restart reopen only
those captured generations. An already published original root can reconcile;
a foreign target/parent/work/payload must be preserved. A missing proof is not a
reason to re-capture a generation. Copy fingerprint edits before source cleanup
must preserve the source and stop, not authorize its removal.

Source cleanup needs two claims: its previous file and its now-empty container.
The existing previous->folder quarantine can be descriptor-held inside the
original source root, with Ready/Claimed/Returning roles and exact previous proof.
Claimed with absent payload means that data was removed: never inspect a new
previous entry to infer ownership or return. Unexpected new children stay intact.
Do not remove a source root by its old public name after a crash: that name may
have been reused, even for the same original inode. Once empty, claim the root
itself via a deterministic private sibling name with NOREPLACE. Persist RootReady
before rename, prove the actual original empty root after rename, save RootClaimed,
then remove only that claimed empty directory. RootClaimed with absent payload
ignores the old source namespace completely. A collision refuses. New source
occupants survive. Recovery descriptions/paths must expose the root claim name
when it holds the original container. No recursive deletion of unknown children.

On completed cleanup, clear the journal and advance only the bound canonical
step: pop one task, add copied bytes (regular file size; links contribute zero
like live progress), increment item count/report only if Movement.counted.
Completed metadata must not reopen source or output. Nested saved steps pop with
zero item count. Keep Both binds the actual chosen target parent/name and later
batch work remains independent. Metadata Retry Save and exact end handling stay.

Preparing before the immutable full-copy recipe is available remains an explicit
provenance obligation. The final implementation must connect those partial copy
stages to the original task before accepting full restart completion. Do not
pretend that a completed-result matcher finishes Preparing/Cleanup or legacy
lineage. Keep older stopped/active gates and all prior data-preservation checks.

Required actual-worker crashes: prepared/normalized, installed before cutover,
catalogued before cleanup, file Ready/Claimed/unlinked, root Ready/Claimed/removed,
finished before task update, and a second crash. Reconcile + two opens + exact
Remaining, both part orders, new source names including same inode, changed data,
raw links, Keep Both/batches/nested counts, missing/corrupt/borrowed receipt and
context, original parent/work/payload/device substitution, atomic metadata rollback,
source-reservation/catalog ownership and older Undo. Keep all failure roots.

Simplification to test before adopting the two-claim ordering above: claim the
whole original source container first, after catalog cutover. Ready -> RootClaimed
only after NOREPLACE rename to a deterministic private sibling and verification
of the original root/previous snapshot. Its old public source name is then never
consulted again. RootClaimed can quarantine previous->folder within that held
root; verify the original file and fingerprint before FileClaimed permits unlink.
FileClaimed with absent folder never examines a new previous name. Remove only
that original claimed root when empty, through the original parent. A missing
claimed root after verified RootClaimed means the claim namespace was vacated;
preserve every old-source occupant, even on the same original inode. This can
avoid a separate root-removal claim after file deletion. Exact phase rights,
unknown extra children, missing names before proof, metadata-save failure and
source/copy edits need explicit real-crash tests; do not infer authority from an
unverified Ready rename. Prefer the smaller safe state machine if it meets all
those cases rather than duplicating claims mechanically.
