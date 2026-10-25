# 0054: Journal retained-replacement expiry and preserve parent authority

Status: accepted under delegated implementation authority, 2026-10-17. Verified
for the recorded cleanup and parent-authority scope; the full product brief
remains active.

## Context

Preserving copied-folder permissions exposed an expiry failure. Replacing an
independently arriving file inside a newly copied read-only folder retained its
previous bytes correctly. After history expired, cleanup unlinked those bytes,
then failed to remove the empty private container from its read-only parent.
Recovery subsequently showed an unavailable item. Making only that parent
writable before expiry cleared the failure; adding more history did not.

A cross-filesystem move also recreated the saved container under a different
application-created parent without carrying its permission authority. A direct
same-filesystem container move relocated its path but left the captured parent
identity behind. In the latter case even explicit permission repair could not
finish cleanup. Public Copy/Move/Undo and actual filesystem checks reproduce
these cases at umasks 022 and 077.

## Decision

Expiry is a journaled filesystem mutation. The primary journal records the
archive entry, immutable Recovery record and physical parent before unlink.
Normal expiry checks saved-version eligibility, identified known stage contents,
remaining owners and actual parent write/search access first. Permission refusal
on an existing parent keeps the saved bytes available. An application-created
captured parent may use the existing durable temporary-access mechanism; its
final mode and distinct external edits retain the policy from ADR0053.

Reconciliation permits the payload or container to be absent only for this
recorded cleanup intent. It checks the recorded parent, original stage, current
catalog record, saved version, known contents and other owners before continuing.
Replacement parents and stages are refused until their originals return. Final
catalog retirement and intent removal share one transaction. A failed sweep
stops when any journal or permission-restoration record remains, so another
cleanup cannot overwrite unresolved work. Recovery Copy may trigger history
expiry, but expiry always uses the primary lane and restores its caller's lane.

Parent access belongs to the physical Stage. An optional marker distinguishes
legacy absence, an explicitly existing parent, and a captured created parent.
New copies and recreated saved containers record that policy. A return into a
user-created Merge root explicitly records Existing, preventing an older copy's
parent authority from being applied to that root. Legacy fallback requires the
explicit copied-parent identity to equal the stage's captured original parent;
private-looking filenames and the current path do not establish authority.

Saved-container moves capture their destination parent and access policy before
commit. The same history transaction updates every indexed physical container
owner's parent metadata. Reverse moves use the directory-return plan's captured
root and policy, or the saved step's scoped original-parent expectation. Parent
metadata updates stream stable owner indexes; namespace paths keep their existing
separate handling. Recreated containers carry the new captured parent through
forward migration and Undo.

The history format is 19. Older readers must refuse it. Existing optional stage
and movement fields default to absent for old inputs. Protected, edited, shared,
unavailable or additional saved work retains the existing preservation rules.

## Verification and remaining work

The implementation uses public Engine/Recovery/Copy/Move/Undo interfaces and real
private files, including two actual filesystems and separate child umasks.
Closed-engine SQL prepares legacy and interrupted-cleanup inputs only. Restart
states cover before unlink, after unlink, after container removal, replaced
parents/stages and explicit permission repair. These are prepared journal states,
not claims of an observed process kill at each expiry syscall. Existing real
Copy/Undo kill tests continue to exercise durable permission restoration.

The settled candidate passes 41 focused permission/expiry tests and all 345 core
tests (21 invoked helper definitions), all-target Clippy, the default desktop
build and workspace formatting. Its 103 compiled inputs remain unchanged through
eight final native runs: 162 checks across Downloads, Recovery, legacy context
and ordinary Retry, in dark and light at scale 1.25. All 80 captures were
inspected. Initial Downloads acceptance is recorded separately. See the
[verification record](../../../.scratch/file-manager/retained-cleanup-verification.json)
and [implementation ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).

This does not establish
atomic descriptor authority for every child mutation, a persistent unfinished-job
queue, general protected-data disposal, every orphan/catalog bound or the full
file-manager acceptance matrix. Those remain part of the active product goal.
