# 0041: Review changed and unavailable folders in a partial Merge

Status: accepted under delegated implementation authority; verified for the scope below.

A partial directory Merge owns both its incoming folder and the destination
folder into which children have returned. If either location disappears, becomes
unreadable, or contains a different item, Undo now reviews that Merge before
inspecting its remaining children. It does not adopt a replacement folder or
follow a replacement directory link. Invalid or unavailable Trash information
also gets this review rather than a generic failure.

The review offers Finish this Merge, Skip and Cancel. Skip is the Return default.
Skip defers the affected branch once for that invocation; disjoint work may
continue. Cancel stops the invocation. Restoring the original physical folder
or its access makes the same pending children retryable. Already completed
returns are not repeated.

Finish this Merge leaves filesystem data in place. In one transaction, it
protects any saved previous destinations in Recovery and removes only the
selected marker and its recursively pending members from Undo. It reports zero
reversed changes for this decision and identifies the incoming item not returned
and any saved versions not restored. A nested Finish leaves its ancestor marker
pending, with the remaining source contents requiring a later explicit review.
Other siblings can still return during the current invocation.

Finish compares both folder observations and Trash information against what was
shown before updating history. Observations include identities and metadata
versions, or the same lookup/access failure. Reappearance, changed metadata,
changed access and cancellation require a fresh decision. Ancestor ownership is
also rechecked. A noninteractive caller gets WouldBlock before a filesystem
intent is created, leaving its history retryable.

Older history may still own a child returned before the interruption. Finish
updates its captured parent expectations only where the destination and its
Merge ancestry still have their recorded physical identities. It does not adopt
new primary ownership or a replacement destination. Saved destinations remain
protected through history expiry and can be copied out without consuming them.

Trash payloads and metadata are left in place. If metadata is missing or invalid,
the remaining payload stays listed by the Trash catalog with an original-location
problem; Finish does not repair or overwrite that information. Recovery of such
Trash items to a chosen location remains part of the broader Trash work.

The persistent representation is unchanged: history remains schema 10. The new
public conflict sources distinguish the incoming and merged-folder locations.
Native notifications use the general wording "item not returned", since a
blocked Merge can still have readable files at its source.

Nine public tests cover missing/file/directory/link replacements, Skip/Cancel,
restored-root retry, unreadable roots, stale and cancelled Finish, older Copy
ownership, nested branch/sibling continuation, saved versions through expiry and
copy-out, and missing Trash metadata with a still-listed payload. Native
acceptance adds replaced-destination and missing-source workflows, safe defaults,
stale answers, Finish counts and previews to the maintained directory fixture.
Exact final evidence belongs in `merge-roots-verification.json` after validation.

The full product goal remains active. Source-only saved remainders,
cross-filesystem directory children, deeper physical cleanup interruption
checkpoints, broader batch/dependency handling and the other integration work
in the implementation ledger remain unfinished.

Final validation passes 224 core tests with fifteen parent-invoked child helpers,
all-target Clippy with warnings denied, formatting, Python syntax, maintained
inventory and whitespace checks. Native70 dark and71 light at1.25 scale each
pass44 checks on one schema10 binary with clean GTK logs. Eight new dialog and
preview captures were inspected. No native fixture repair was needed.

The first desktop build exhausted disk space writing its generated incremental
query cache. After that compiler terminated, only generated Files incremental
caches were removed. The final desktop build passed with CARGO_INCREMENTAL=0.
The original failure, successful build, exact source/binary hashes and cleanup
records remain in the proof JSON. Source, binaries and earlier evidence were
preserved. This review does not yet handle a saved destination becoming
unavailable during an otherwise healthy active Merge; that remains part of the
source-only/saved-remainder work.
