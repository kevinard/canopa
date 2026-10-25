# 0133: Saved-container whole-Move restart tasks

Status: scoped backend and isolated default-library verification. Continues ADR0132;
cross-filesystem saved-container migration and full restart recovery remain open.

A catalogued saved-data container moves through the same held source, source
parent, destination parent and private claim as an ordinary whole-entry Move.
Capture the actual catalog owner before mutation. Its saved previous entry must
match an existing original generation proof, either retained replacement facts
or a previously transferred container proof. Missing legacy or recreated-data
proof cannot be replaced by observing its current path. Catalog identity scalars
remain useful metadata and grant no generation authority.

The saved Movement records original source parent, container, previous entry,
destination parent and whether this task contributes to item counts. Its matcher
stays separate from ordinary and reviewed replacement Moves. The exact task
receipt includes those facts before mutation. Schema43 prevents older binaries
from accepting these proof-bearing records.

Ready/Returning restores the original container, retires the uncommitted receipt
and leaves only empty-work cleanup. Publishing verifies original folders,
container and previous generation before publication or catalog cutover. An
absent Publishing payload cannot infer a return from a new source occupant.
History append rebases retained catalog ownership to the already captured target
parent in the same transaction. Completed metadata consumes only its proved
suffix; it does not reopen either completed data or a reused source name.
Nested saved steps remove their task without counting a user item or destination.

Recovery checks the existing exact active-task receipt before directory access
preparation and filesystem mutation for supported modern Forward steps. Journal
context alone is insufficient. Initial binding and this read-only validation use
the same task matcher; validation cannot recreate a missing receipt.

Physical generation, private-work and parent checks remain strict. Legacy
bindings, Preparing/Cleanup provenance, cross-filesystem saved migrations, Undo
parent lineage and broader release/native/hardware acceptance remain separate
unfinished work. Nine public-worker parents cover actual intent/Ready/Publishing/published,
Returning/returned and cleanup crashes; raw previous symlinks; Keep Both and
later batch/nested work; previous, parent and private-work substitution refusal;
missing/corrupt receipts; and catalog/task rollback. Completed metadata preserves
edited saved data and new source occupants over two restarts. A deleted-receipt
probe reproduced publication before metadata refusal; the early common check
now refuses that step before mutation.

The selected backend set passes 464 parents/20 targets/38 ignored on inputs02.
The final isolated default library passes 232 parents/18 ignored on inputs03;
Clippy with warnings denied, formatting and schema43 fixture syntax/hash pass.
The sole inputs02-to03 change collapses two equivalent parent-rebasing guards for
Clippy; its before file and exact diff are preserved. Both required active/stopped
restart gates pass unignored. Protected application/Niri/earlier restart binaries
are unchanged. See [the WIP record](../../../.scratch/file-manager/SAVED-TASK-WIP.md)
and source/verification records for exact relationships and retained failures.
