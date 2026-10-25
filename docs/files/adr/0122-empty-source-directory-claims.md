# 0122: Claim an empty source directory before removing it

Status: scoped verification; complete restart ownership remains unfinished, 2026-10-22.
Continues ADR0121. The full Files goal remains active.

After a decomposed Move transfers its children, pathname-only remove_dir can
delete a replacement folder or a folder under a replacement parent. Public-engine
boundary tests reproduce both after the final checkpoint. A real process killed
before that syscall also shows startup treating a different directory at the
source name as proof that the intended directory was removed.

Use the original source/destination directory facts captured during traversal.
Open and validate both directories, keep their actual descriptors, and claim the
source into an owned sibling name through its held parent. Verify the actual
claimed original, emptiness, destination and namespace before saving Claimed.
Only that verified claim permits removal. A Ready claim can be returned but
cannot authorize deletion; a changed or unavailable source cannot be inferred
to have been removed.

Save Returning with the actual subject before moving it back without replacement.
Existing names, unexpected objects and missing original facts preserve files and
journal. Restart returns unverified claims, finishes proved empty removal, or
records an already-completed unlink without touching newer source occupants or
replaying destination data. A verified source that gained entries is returned.
Cleanup keeps only empty-directory rights. A changed namespace retains return
direction until that namespace can be verified.

Schema37 reserves the claim and retained removal facts. Forward context binds
the exact role and history slot before reconciliation. Full claim/step coherence
and original object generations remain independently required. A missing role
cannot fall into older pathname-only inference. Unsupported or legacy evidence
does not create a new execution owner.

The scoped library passes82 parents,7 ignored; the new matrix exercises15 killed
workers and two opens each, plus the original live parent/entry races and one
pre-syscall restart regression. Selected backend coverage passes376 parents
across19 no-default-feature targets, with the required restarted Retry/Continue
test the sole failure. Default all-target Clippy, formatting and schema37 fixture
syntax pass. The isolated default desktop library passes163 parents,8 ignored,
on unchanged captured inputs. No native application rebuild, complete workspace/
release or physical-device acceptance is claimed. General Undo/relocation must
consume the new original facts, and actual Retry/Continue owners plus typed
handoff/abandon lineage remain unfinished. Keep the required public gate enabled.
