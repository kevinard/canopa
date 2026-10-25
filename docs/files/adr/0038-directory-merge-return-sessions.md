# 0038: Persist directory Merge as part of the original Undo entry

Status: accepted under delegated implementation authority; verified for the scope below.

Undo now offers Merge when an incoming directory returns to another directory.
Keep both remains the Return-key default. Merge reuses the shown destination
root and returns its children individually. Child file collisions retain
Replace, Keep both, Skip and Cancel; replacing newer work protects it in
Recovery. A child that has returned stays returned across partial Undo and
reopen. Retry reviews unfinished children without copying completed work again.

History schema 9 adds a DirectoryMerge marker wrapping the original Rename or
Trash change. Normal child return rows remain in that same operation. Persistent
membership links them to their parent marker; nested Merge adds another marker
under the first. No hidden operations enter chronological Undo history. The
existing filesystem reversal and intent journal handle each child. This avoids
calling the public transfer interface while holding Undo's mutation mutex.
The directory-return module owns expansion, root authority and completion;
the store owns its atomic history and namespace transactions.

Child rows are committed together with their parent marker before any child
returns. A completed child's row is removed atomically with its normal ownership,
Recovery and intent updates. Root completion is a separate durable Undo intent:
remove only the owned empty incoming root, handle its previous destination or
Trash metadata, then consume the marker. A newly arrived child prevents empty
root removal and preserves the session. Unfinished Merge operations are exempt
from ordinary history expiry; the normal limit applies to other entries.

Every child checks both physical root identities through all ancestor sessions.
After a conflict answer, this check precedes staging or displacement, and runs
again before mutation. A swapped source/destination root preserves pending work
and refuses the answer. This supplements the existing item identity/version
checks; pinned directory-descriptor and original-media authority remain broader
outstanding requirements.

The reused destination root never becomes owned by older primary root history.
Completion updates only captured parent references that need this container for
older child returns. Older CreateFolder history still reviews the newer root.
Normal child returns retain their own causal identity bindings.

Pending child rows replace the original whole-folder movement for active
Recovery path resolution. Returned stages stop following their consumed child
row; pending stages still resolve to the incoming root. Archived incoming
versions have a separate session association: their displayed logical names can
follow child Keep both while their physical stages follow actual movements.
Newer destination archives retain their own locations and names (ADR0037).
Reserved `.canopa-files-stage-` containers cannot Merge; Keep both moves their
complete containers intact, preserving the identities required by Recovery.

This is the first working Merge slice, using existing same-filesystem return
primitives. The full feature still requires cross-filesystem children, source
cleanup checkpoints, new-arrival review/rescan, changed/unavailable root reviews,
kept-child disposition, history-expiry and killed-process acceptance, saved-only
remainders and Trash-specific coverage. Source-only Merge with an unavailable
saved destination is refused before expansion so its backup reference is never
lost; Keep both retains its existing source-only behavior. Merge can retry once
the saved version returns and is reviewed. Those remain required by the full
product goal. Passing this slice does not establish complete directory Merge.

Public tests cover disjoint returns and destination identity, partial
Skip/reopen/Keep both, active and archived Recovery with older history, stale
root refusal before stage creation, nested Cancel/reopen/Replace/raw links, and
a reproduced reserved-stage collision. Native acceptance extends the original
Keep both workflow with Merge, a child conflict, Skip/retry and Quick Look.
Final exact evidence belongs in `directory-merge-verification.json`.

Final core passes 200 tests with thirteen parent-invoked child helpers. The
directory suite passes ten, including seven new Merge parents. Versions 4
through 8 migrate from inputs without the new session tables, preserving their
public Undo and ownership behavior. Unknown future history remains refused
without rewriting its file. All-target Clippy with warnings denied, desktop
build, formatting, Python syntax, maintained inventory and whitespace checks
pass. Native64 dark and65 light at1.25 scale each pass seventeen checks, totaling
34 on one schema-9 binary. All ten rendered captures were inspected; both runs
have no GTK warnings, criticals or panics. No native fixture repair was needed.
Initial missing-capability/visitor compile failures, the reserved-stage red
regression, the updated future-schema input and the corrected invalid-choice
error category remain separate from final green evidence.
