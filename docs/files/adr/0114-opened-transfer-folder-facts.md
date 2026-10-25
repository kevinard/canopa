# 0114: Bind transfer traversal to opened folders and created payloads

Status: implementation in progress, 2026-10-22. Continues ADR0112 and ADR0113.
Restart execution remains unfinished.

A public core regression emulates recycled inode tuples in a decoded directory
continuation while preserving its durable original facts. Before this change,
Copy accepts the replacement and copies its child. A source/destination inode
pair is insufficient to prove the original traversal folder or either parent.

A decomposed transfer now opens a source folder relative to its opened parent
and enumerates names through that actual folder descriptor. It preserves Unix
name bytes and obtains source identity, mode and durable original facts from
that same object. Namespace checks refuse replacement before publication;
subsequent directory checkpoints also verify original source/destination folder
and parent facts when those facts are available.

Merge retains the opened existing destination and parent from before the user
question. It does not manufacture a new observation after the answer. A created
folder retains its private payload descriptor and intended destination parent;
its durable target facts come from those objects before publication. Stage
creation is rooted through the held parent, and publication uses descriptor-
relative rename with no replacement. Only stable namespace labels enter the
journal; process-local descriptor paths do not become saved execution directives.

Checkpoint v3 carries the four durable folder/parent identities. Schema 30 keeps
older executables from interpreting these new bodies. Older checkpoint formats
remain readable without invented facts; versions 1 and 2 cannot smuggle the new
traversal field. Unsupported persistent identities remain unavailable. They
cannot satisfy the scheduler's restart-execution contract.

This closes the identified folder-observation and publication gap. It does not
make a decoded plan an execution owner. Child-file reads/installs, cross-filesystem
source cleanup, retained stages, interrupted journals, exact step association and
typed handoff/abandon lineage still need their mutation/ownership boundaries.
The application continues to restore descriptions without enabling unsafe
Retry/Continue. The unignored required killed-process restart gate remains open,
along with the remaining operation families and full product/release scope.

Publication refusals retain their real journal before and after rename, including
foreign staged files/symlinks and changed target/parent/stage namespaces. Mode
restoration uses the held created-folder descriptor, avoiding a fresh open of a
replacement payload. Generic stage/child-file/source cleanup remains separate.

The codec upgrade also requires receipt checks against original JSON fragments,
not a re-encoded newer checkpoint. After validating the old wire bytes, the
atomic application save binds current part hashes to the saved codec while
retaining the original whole-end hash. Failed upgrades roll back all bindings.
A killed-process schema29/v2 fixture verifies metadata-only save retry and two
starts, preserving newer files/Undo and inventing no traversal facts.
