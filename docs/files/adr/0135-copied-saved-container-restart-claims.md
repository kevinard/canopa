# 0135: Copied saved-container restart claims

Status: scoped verification complete. Continues ADR0134;
complete restart recovery and the five-area product goal remain open.

A cross-filesystem saved-container Move captures its original source parent,
container and previous entry from the catalog's existing proof and the actual
pre-review selection. The destination parent is the same held selection. Require
the actual created work directory and payload generations, verify its descriptor
fingerprint and bind the immutable recipe before publication. No current-path
observation can replace missing original proof. Schema44 reserves copied recipe
bindings, source claims and their independent phase records.

The canonical saved namespace Change records a digest of the immutable operation,
source/target stages, source version/fingerprint, created copy and context recipe.
Its exact durable task receipt is saved in the same intent transaction. Matching
and metadata advancement stay separate from ordinary and same-filesystem Moves.
Completed metadata pops only that proved task, restores copied bytes (links zero),
and increments item/report counts only for non-nested saved roots. Keep Both
records the actual chosen target. Completed output and reused source names need
no reopening when restoring this result.

Publication opens only the captured original generations through verified parents.
Rename the created payload to previous and publish the actual work directory with
NOREPLACE; sync and verify it before the existing atomic catalog/source-reservation
cutover. Retain original source data until that transaction commits. The created
saved previous proof remains in all retained owners, per ADR0134.

After cutover, Ready claims the whole original source container to a bounded
private sibling through its original parent. Verify the actual original root and
previous snapshot before saving RootClaimed. Inside that held root, claim previous
to folder and verify it before FileClaimed permits unlink. Claimed absent names
never inspect the old public source namespace, even when the same original inode
reappears there. Unknown children, payloads, parents, devices or copy edits refuse
and remain intact. Only the original empty claimed root can be removed; no
recursive cleanup is permitted.

A verified vacated root, or FileClaimed with its payload already absent, needs
no further destination verification. Finish only metadata or the original empty
root cleanup through the original source parent. Completed output edits and
replacement output roots/parents remain untouched. Data still present requires
the original copy check before deletion. Unknown private children still refuse.

New Undo Move-to-Recovery archives retain the reviewed source entry and create
their saved directory through the held original parent. Record the selected
entry's Original before displacement and verify it after descriptor-relative
rename. Snapshotting these archives verifies their already-recorded original
roots/previous; it never adopts a later occupant. Legacy archives without proof
remain unable to authorize forward continuation. Other Undo creation and restart
claims still need their own audit.

Before any recovery directory access or filesystem mutation, verify the original
intent context and existing exact task receipt. After cutover, validate the
original canonical committed Change/bindings rather than capturing current owners.
Each mutable cleanup phase has its own independent digest in pending_bindings,
derived from that original committed owner list and the full current journal.
Catalog commit installs the first phase atomically. A private non-serializable
verified transition updates later phase/journal records atomically; failed saves
roll both back. Changing a valid phase enum or deleting its record cannot grant
cleanup rights. Older unproved forward migrations stay inspectable in Recovery.

The original completed-cross restart failure now passes unignored. Fourteen public
worker parents pass actual process kills around publication, catalog cutover,
source claims/unlinks, task completion and a second reconciliation crash. They
cover two further opens after output edits, new and same-inode source namespaces,
missing/corrupt receipt/context/phase refusal, original parent/work/payload
substitution, unknown private work, metadata/phase rollback, raw links, Keep Both,
nested counts and remaining batch work. The full no-default library passes
165 tests with 19 ignored helpers and no filtered tests.
The selected backend passes 479 tests across 20 targets (40 ignored helpers).
Default all-target Clippy with warnings denied, formatting and fixture syntax pass
on the same 872 captured inputs. The isolated default-feature library passes
246 tests with 20 ignored helpers and zero filters; see
SAVED-CLAIMS-WIP.md for current exact source and outcomes.

Generic Preparing/Cleanup provenance, weak legacy and reverse/Undo claims, queued
handoff lineage, large reports/paged owners, Trash/Restore and the remainder of the
user goal remain required. This decision does not accept native, physical-device,
full workspace or release behavior.
