# Directory expansion recovery — in progress

The full five-area goal remains active. Previous turn: metadata Retry Save progress,
verified engine-end-retry-source-02.tar.gz (6ac657296ee3f3f62d6eeba966242806632b78739a6e00c40a7d92b9e4066150),
1058 decoded members, all 864 inputs02 unchanged before new edits. No commits,
worktrees, subagents or vendor changes. Root/Files AGENTS, both context maps and
ADR0124/0125 read. Diagnose discipline reused. Protected binary hashes and all
prior archives/proofs remain. Last default library ELF was safely reclaimed via
directory-expansion-cache-01.json from the completed engine-end-retry log.

New private directory_boundary module exercises OperationJobs with actual SIGKILL
at directory creation append, raw folder/link names, held published inode, two
opens, Continue Copy/cross-filesystem Move, late unselected source data and newer
unrelated destination contents. Repeated red-01/-02 logs show Interrupted because
there is no exact bound CreatedFolder history step. Original roots are retained
and must be archived/verified before finalizing this checkpoint.

Schema42 WIP adds roles8/9 to normalized cursor tasks (expansion descriptor and
children keyed by head position plus child ordinal), ten progress counters with
legacy6/7/8 padded, and preserves table rows during migration. The descriptor
stores exact source/destination, DirectoryFinish originals, actual prepared Stage,
canonical expected Change, operation/ordinal and digest of sorted raw child names.
Tracker preparation writes descriptor/children with Preparing atomically through
its live scope, without consuming the durable head. It allows current live
permission-access records but refuses existing journals. Ordinary next checkpoints
remove only that descriptor/children and clear step receipts with normal deltas.

CreatedFolder Forward binding now combines the original queue-row hash and the
expansion-row hash. Exact suffix verification checks that receipt and canonical
change before expansion advancement. Advancement consumes one creation and expands
to precisely those children plus Move postorder finish, preserves directory facts,
counts and root destination report, and leaves later queue tasks untouched.
The first green-01 attempt had private method visibility errors; fixed green-02
passes the initial public parent for both Copy and actual cross-filesystem Move.

directory-expansion-red-03.log is terminal101 and acknowledged. Its first Keep
Both case passes; Merge exposes another real gap: startup trim deletes the empty
operation before the first child, so the accepted expansion loses its history ID.
New public parent covers Keep Both, Merge, postpublication/preappend and Preparing
crashes. Current source changes retain empty operations while an active cursor
still references them, extract read_active proof validation, and advance accepted
Merge at an unchanged watermark. Not compiled or verified yet. Missing behavior:
Merge no-history expansion at cursor recovery, Preparing promotion from proved
active expansion, and safe original-only CreatedFolder Forward recovery before
append. New production source currently compiles and the first published-folder
parent passes, but these extra phases are not implemented or verified yet.

Implementation direction: extract shared cursor read/proof validation from
Store.recover_active_transfer so Preparing promotion can validate accepted request,
original history, spec/progress/rows/counts and exact watermark before binding its
expected Forward. Never promote generic/legacy Preparing without that independent
proof. Use the actual original prepared payload; no new observation/adoption.
Directory Forward reconciliation should verify original parent/stage/payload and
publication before restoring mode or publishing NOREPLACE, using held descriptors;
generic recovery currently uses weak inode-path committed() for created folders.
Merge restores its accepted descriptor/children without a history append. Keep
validation failures Interrupted and preserve pending data; reuse ADR0128 metadata
write retry after a proved end. Complete new corruption/replacement/empty/large/
second-crash and transaction-refusal gates, relevant library/integration/default/
Clippy/format/fixture checks, then archive final exact source and update ledger.

Current changed paths include transfer.rs, new transfer/directory_boundary.rs,
new transfer/cursor/expansion.rs, transfer/cursor.rs, transfer/directory.rs,
store/transfer_cursor.rs, store.rs VERSION42, rename_boundary.rs/request_execution
schema assertions and the claim-review Python schema fixture. Capture fresh full
inputs after implementation; include newly added Rust files, not just the old864
manifest keys. No other goal area is complete. See ENGINE-END-RETRY-NEXT.md for
remaining directory-removal, whole-entry Move, saved-container, Preparing/Cleanup,
legacy, lineage, large-report and all later user-goal requirements.


Latest progress: core-01.log showed Copy Keep Both, Merge and public postpublish
review passing, then Preparing failed because generic cleanup disposed its original
empty stage. core-02.log had visibility/import compile errors, fixed in core-03.log.
core-03.log is terminal0 and acknowledged: both directory public parents pass,
one ignored private worker, covering all eight kind/phase/choice combinations.
New scoped Preparing promotion validates the complete independent cursor graph,
original accepted request/history and zero advanced/borrowed steps, projects the
exact child graph for validation, verifies original source, then writes its exact
Forward/context/receipt atomically. Original-only CreatedFolder recovery uses
actual parent/work/payload pins and NOREPLACE before mode changes or append.
Untracked legacy Preparing keeps existing cleanup; missing facts cannot promote.
Shared read_active validates original history, scope, hashes and bounded rows.
Merge's zero-change operation is retained while a cursor references it.

Red data now archived: directory-expansion-red-fixtures-01.tar.gz,
SHA b78c824f88e0aa46b8ecb678a621f44f74feb9ba71477bd375fc98b86eda2bd5,
50 decoded verified entries across four failure pairs; verification JSON includes
raw symlinks, original hashes and read-only durable-state counts. Original roots
remain untouched. No test sessions currently live. Next: corruption/replacement/
parent-generation, empty-folder count and metadata rollback/second-crash gates,
then complete relevant validation and final source capture (with all new Rust).


core-04.log (terminal101, acknowledged) passes replacement and reviewed-choice
gates but exposes two additional regressions: an empty Merge Move loses its one
completion count after resume; missing Preparing expansion descriptor leaves CHILD
rows reaching an unreachable parser branch, panicking the worker. core-05.log
fixes both and passes five public parents. Folder enumeration now records a
was_empty fact (only true values serialized); continuation checkpoint5 is emitted
only when such facts exist, retaining3/4 otherwise. A successful removal of an
originally empty Merge source counts once even across restart. Unconsumed expansion
rows refuse decoding, never panic; Preparing's declared counters also prevent a
missing descriptor from falling into generic cleanup.

Private fixtures now use separate source-parent/destination-parent beneath their
isolated roots, so tests can move the same original folder into a replacement
parent without replacing history. The eight final directory parents in core-06.log
pass, two ignored helpers. Additional gates refuse missing/borrowed/changed rows,
keep replacement folders and parent generations untouched, roll back failed
promotion Forward/receipt inserts together, kill a second review immediately after
promotion commit, and retain 770 long names in bounded normalized rows while
source/output data stays preserved. No test process remains live.

Current live check: directory-expansion-clippy-01.log; use only the actual tool
session from the current conversation, not a guessed handle. Capture all new Rust paths in the eventual manifest (old864
plus directory_boundary.rs, cursor/expansion.rs and directory/publication.rs).
Full backend/default/static and schema42 fixture checks remain; don't claim this
checkpoint final or change goal status. Update this WIP's check status from actual
terminal results. Archive core-04 failure pairs separately before finalization.


Clippy01 requested one equivalent condition collapse; Clippy02 requested only eight
needless borrows in private fixtures; Clippy03 passed. inputs01 captured 867 paths,
including all three newly added Rust modules. lib-01.log passes 114 parents, 12
ignored helpers. The initial bounded backend runner passed job/checkpoint and both
required restart/execution targets, then core-12.log failed three legacy permission
fixture expectations (38 permission parents and all28 operation-transfer parents
passed). That runner is terminal101 and acknowledged. These fixtures manufactured
Forward journals with inode hints and no original ownership/context. Refusing them
is required; the new recovery must not silently chmod/publish/delete such input.

The permission fixtures now verify persistent review, unchanged modes/journal,
kept data and refused Undo/retry even after the hinted original returns. A new
public-service SIGKILL parent covers actual original readonly folder recovery at
Preparing, publication and append, then Continue with mode restoration. Clippy04
passes on this final test source. Exact before-legacy backups preserve both changed
test files. inputs02 captures the same867 paths; source-relation-01.json identifies
only these two test changes, with production source unchanged. Current check:
lib-02.log, live exec handle34575. The new bounded runner (checks-02.py) must start
only after this library finishes; its logs20..29, audits18..27 and batches02 are
fresh. Do not reuse earlier log/audit names. Broad/default/static results on
inputs02 remain pending. Extra red data for the count/panic cases is preserved in
directory-expansion-extra-red-fixtures-01.tar.gz,
df1d15134f6d81e3f99344ad99e4c67fafd98d4eeb39c76023e999a147a94df9,
23 decoded verified entries, two failure pairs. Source archives are not final yet.


lib-02.log is terminal0 and acknowledged: final inputs02 pass115 no-default library
parents,12 ignored helpers. This includes the real readonly crash parent (all three
preparation/publication/append phases). The final bounded runner is now active;
observe its actual exec handle from tools. Its results are batches02 and logs20..29.
No production changes followed inputs01. The final test-only changes are preserved
in before-legacy backups and explicitly related in source-relation-01.json.
