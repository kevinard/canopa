# Preparation disposal namespaces

Status: scoped schema46 verification. Continues ADR0136.
The complete five-area Files goal remains active.

A real public-worker probe kills Copy after its first partial write, retains a
hardlink to the partial inode, then kills a second worker immediately after its
cleanup unlink. Reintroducing folder as a hardlink to that original inode causes
another unlink on the following Reconcile. This reproduces twice. A targeted
snapshot confirms the old Preparing journal and independent binding are unchanged
and the actual held payload still satisfies its original generation proof.
Object identity proves which object was selected; it does not prove that a reused
name is still the original directory entry or that unlink may be repeated.

Reserve a separate private holding Stage through actual created parent/root handles.
Pending::Disposing retains the original preparation, holding Stage and explicit
Reserved, Claimed and Consumed phases. The original independent task binding advances
atomically with each phase, keeping its accepted task, history-file generation,
source and ordinal. Raw generic journal writes cannot mint or replace disposal.
Use held verified roots and NOREPLACE to move folder into holding/folder. A Reserved
restart can verify an already moved original payload; contradictory occupants stay
for review. Claimed no longer grants rights to the original folder namespace.

Save Consumed before unlink. Only that live transition retains an unlink capability;
a restarted Consumed record may retire original empty roots only. If the process
dies before unlink, the still-present partial copy requires review. It cannot be
distinguished safely from a new entry created after an unlink. Preserve either
case, including a new link to the same original inode. Unknown or substituted roots
and payloads stay intact. Recovery exposes both namespaces for inspection and
explicit copy. Failed phase saves must roll back journal and binding together and
must not delete payloads; failed retirement must leave the consumed state durable.

Scoped validation on 877 unchanged inputs passes 495 selected backend tests across
20 targets (42 ignored helpers), 261 normal isolated desktop-feature library tests
(22 ignored), both unignored restart gates rechecked after a cache change, all-target
Clippy with warnings denied, formatting and fixture schema/syntax. Two desktop
compiles failed on storage space/quota before tests; the third normal compile passed.
Failure archives retain decoded evidence even though temporary live roots disappeared.

Review is not yet complete as a product flow. Public Recovery copies are verified,
but tests move retained leaves aside externally before retrying cleanup. Keep in
Recovery still accepts Rename only. Add a stale-safe public retention disposition
before claiming that ambiguous disposal can be resolved entirely through Files.
Arbitrary extra work-root children, partial edits/source disappearance and restricted
created-parent access also need direct acceptance. Generic Forward/Undo abort cleanup,
legacy/reverse Undo, expiry, queued lineage, large reports, Trash/Restore and remaining
backend/native/workspace/device/performance/release work stay open. These scoped
checks do not prove the full goal.
