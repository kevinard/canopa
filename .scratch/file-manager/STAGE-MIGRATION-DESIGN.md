# Saved-stage migration design (work in progress)

Full product goal active. Previous goal turn progress verified ADR0045 core259,
native88/92 retry21 each +90 transfers14,139 source hashes; packaged docs24/39 and
all8 binaries/examples pass. Current desktop remains7178de319160b4acb3289adf08dd243f36c90105688ed7b3eb3930f63825c846.

Read CONTEXT-MAP, Files AGENTS/CONTEXT, ADR0037/0043/0006/0013. Existing protected
Recovery tracer allowed Err/defer. Strengthened it to require successful complete
return and old source root absent. Stage-migration-red.log/35097 terminal101:
WouldBlock private saved-data guard. No application changes yet. Need keep its
Recovery reopen/copy-out assertions, not merely remove the guard.

Ranked causes shown: prefix guard stops return; unguarded copying loses physical
ownership; wrong catalog timing can make active backups unavailable. Design:

- Stage directory is owned by identified Retained records, not a name prefix.
  Add schema14 derived saved-stage owner indexes for active changes and garbage
  records, keyed by Stage identity. Populate on upgrade and maintain with normal
  change/record indexing. Query actual resolved record paths before adopting.
  An ordinary user directory with a reserved-looking name remains ordinary data.
- Reuse fingerprint::prepare_copy with Pending::Preparing while copying the one
  known previous file/link into a new sibling stage. The original stays intact.
  This already journals partial-copy cleanup; no source mutation in this phase.
- New Pending::StageMigration descriptor: id/ordinal/change, actual source Stage,
  source Version + source Fingerprint, prepared copy Stage + copy Fingerprint,
  copied Identity, optional logical relocation, optional Recovery Context, and
  catalogued bool. Source stage must contain only previous; preserve unexpected
  entries/unavailable/directory payloads for explicit handling, never recursive
  deletion. No prototype-only workarounds in the final implementation.
- Prepared copy's folder payload becomes previous, then its complete identified
  container is renamed no-replace to the exact child original target (or Keep
  both target). Check captured parents, actual source/version and verified copy.
  The new Stage's physical identity changes; its old name is retained at target.
- Atomic catalog cutover updates every actual owner of that source stage and
  consumes the derived Stage Rename child in the same transaction. Logical
  Retained.stage.directory in active history stays historical; the derived
  child disappears at the same commit, so existing follow_later resolves the
  returned location correctly. Archived physical paths use ordinary reversed
  namespace following. Optional Keep both relocation uses existing logic.
  No extra global location table or physical-path placement field is needed.
- Retained.identity remains the logical original saved-item identity for older
  history bindings. Add optional physical_identity for the original item after
  recreation. observed_identity uses reviewed Version then physical_identity
  then logical identity. Recovery status and ordinary expiry compare baseline
  physical identity, preserving the existing protection of reviewed/replaced
  unexpected versions. Update physical_identity only if it owned the source.
  Update Version to final installed destination Version only when old expected
  Version equals source snapshot (or legacy None with owned identity). If source
  contains edits, preserve the changed-version review; do not bless those edits.
  Version includes ctime, so capture after payload rename. Fingerprint verifies
  content/type/mode/mtime across journal phases; don't just change Version.inode.
- Catalog cutover reuses Store reversal transaction (factor private helper from
  reversed_with_recovery), then writes catalogued=true intent in that transaction.
  Source cleanup remains journaled after the child row is consumed. Root marker
  still exists; normal mutations wait until recovery clears this intent.
- Cleanup verifies canonical destination, original source Stage/parent and copy
  fingerprint. Rename old previous to old stage's folder slot as quarantine,
  verify owned identity/fingerprint there before unlinking, remove only the empty
  old stage and sync. Recovery can detect either slot after interruption. Extra
  entries/changed copies stop, preserving both versions. Missing old stage after
  completed cleanup is idempotent. Catalogued phase needs no deleted child row's
  parent lookup; use captured Stage and parent identities.
- Interrupted views expose staged new copy before cutover, old source after it,
  using previous/folder slot as present; include both paths for storage checks.
- Cross-directory preparation needs Unchanged/Expanded/Returned instead of bool,
  so direct saved-stage return contributes one completed count. Update both Undo
  call sites. Generic directory copying stays for unowned prefix lookalikes.

Next: implement owner indexes + Retained baseline identity, descriptor/recovery,
atomic Store cutover and cross-filesystem dispatch. Then focused public green,
active UndoBackup visibility + successive older Undo, unchanged expiry, changed
payload preservation, raw symlink, prefix lookalike, Keep both collision, source/
destination race and process-kill boundaries. Upgrade authentic v13 schema input.
No private database output assertions. All binaries/test compilers terminal at
this checkpoint. Default incremental -j1, no Rust edits while compiler live.
Root2.2GiB free, /tmp1.3GiB free recently; prior cache aliases must remain valid.
Entire ledger Next work still active; no agents/approval/blocker.

## Implemented checkpoint (supersedes initial no-code status)

Schema14 and operations/store/stages.rs now index identified saved owners for
active changes and archived records; indexes populate on upgrade and update with
normal change/record indexing. Retained has optional physical_identity, original_
identity() baseline, and observed_identity keeps reviewed-version precedence.
Expiry and Recovery status use the physical baseline. Logical identity remains
for older causal history bindings. All constructors initialize None.

New operations/saved_stage.rs implements prepared-copy -> dedicated Pending::
StageMigration -> installed/verified previous payload -> atomic catalog cutover
and child consumption -> journaled source quarantine/cleanup. Store's reversal
transaction is factored into reverse_transaction and reused by cutover. Active
historical Stage paths stay logical, archived paths use normal reverse following.
The current implementation retains the original private basename; occupied
private destinations are preserved/refused (unfinished). It rejects extra contents
and non-file/link payload copying without recursive removal. Interrupted views
include both locations and show the prepared copy before catalog cutover, source
copy after it. Captured source/destination parent identity checks guard cleanup.

Cross-filesystem preparation now returns Unchanged/Expanded/Returned. Both Undo
call sites count direct saved-stage completion and continue. Ownership lookup
replaces the broad prefix guard, so unowned prefix lookalikes traverse normally.

Index check26790 terminal0, two pending unused helpers only. First migration40338
was a compile failure due to pub(super) visibility in Store's nested module; both
methods now pub(in crate::operations). Fixed33665 terminal0, stage-migration-
second.log: stronger existing protected Recovery test passes1/6 filtered after
real cross-device completion, source removal, reopen, read and copy-out. The
job temporarily waited on filesystem I/O: read-only host ps observed cargo683627
D/folio_wait_bit_common, then the same handle completed; no duplicate restart.

Added tests/undo_saved_stage_move.rs: active backup visible via reentrant public
Recovery read during z conflict, partial Skip/recovery copy+Undo/reopen, Keep both
continuation, newer destination preservation, replacement Undo and older original
Copy Undo; and ordinary user prefix directory with previous file. These use real
private files/public Engine, no database output assertions. Parent guards were
added after first green. Current focused pipeline81847 is live: fmt -p Files then
new suite + undo_directory_cross + history_upgrade, log stage-migration-focused-
first.log. Do not edit Rust while this compiler is live; poll same handle.

New ADR0046 in progress and CONTEXT index. No full core/Clippy/default build or
native proof on schema14 yet. Default desktop7178 remains the old verified13
binary. All ADR0045/package proof is historical prior-scope evidence, not current
schema14 verification. Finish current focused feedback first, then changed/expired/
raw-link backups, collision/unexpected-data policy, actual installed-destination
and post-cutover interruption/reopen, authentic v13 upgrade. Entire product goal
and ledger Next work remain active. No agents or external blocker; .git read-only.

## Current focused green and remaining protocol work

81847 terminal0:13 parents/1 invoked helper (new2 + directory-cross6 + upgrades5)
pass. Added actual schema13 input by dropping both schema14 ownership tables
before setting user_version13; upgrade helper now removes these tables for every
older input and includes13 in its v4-through-v13 loop. New public saved-stage
suite now has6 parents: active/partial/older Undo, prefix lookalike, v13 active+
archived owners, saved-version edit via open FD requiring review, unchanged expiry,
and raw symlink target/older Copy Undo. No private database output assertions;
raw SQL only prepares authentic prior-version input.

86766 terminal101: upgraded fixtures5 and new suite5 passed, raw-link older Copy
Undo falsely conflicted. Metadata probe85097 terminal101 proves source/restored
link modification timestamps differ; mode and raw target bytes agree. Fix restores
source Fingerprint metadata on the identified verified prepared payload before
migration, then verifies identity/fingerprint again and captures installed Version
only after renames (ctime matters). Source logical identity is not rewritten.
45251 terminal0: all6 new parents + all5 upgrade parents pass after fix. Current
full core -> all-target Clippy pipeline is newly launched (stage-migration-core-
first.log / stage-migration-clippy-first.log); get its exec handle from this turn.
No default desktop build/native/source manifest for schema14 yet. Binary7178
remains prior verified13. Root2.1GiB and /tmp1.3GiB free recently.

Still required before treating this scope verified: occupied private destination
choice, unexpected container contents/payload handling, source/destination races,
actual install/cutover interruption/reopen (journal persists after consumed child),
and native active/protected Recovery/Undo proof. Add descriptor relation validation
(source.directory == change.current, plain Rename/no retained, copy identity ==
stage.payload_identity, distinct source/prepared/target paths) and recheck guards.
Consider saved-stage owner refcounts during garbage cleanup; original active data
must not be collected through another archive owner. Do not weaken changed saved
version review or infer unrelated legacy ownership.

Forward cross-filesystem folder transfers containing owned saved stages still
need the same data/catalog protocol; ordinary recursive transfer copying is not
proof their catalog follows. A useful reuse direction: a dedicated forward saved-
stage Move commits an internal Rename namespace step (old logical Stage directory
identity with explicit new physical expectation) in the same catalog transaction,
so active historical Stage paths follow source->destination immediately, before
parent-directory completion. On Undo that child uses this new migration path.
Capture source causal bindings before recreation, keep source cleanup journaled,
and integrate progress/retry/continuation idempotence. This is a design lead,
not implemented or verified; do not substitute a same-filesystem evacuation.

Legacy already-decomposed private-stage directory Merge snapshots are distinct
from authentic prior schema before Undo. Current newly registered stages remain
single leaf Rename members. Do not adopt a legacy copied payload from path names
alone; require actual stored source/destination association and explicit review
where its original physical proof is unavailable. Full goal Next work unchanged.

## Handoff interruption, malformed records and collision review

Original64MiB SIGKILL fixture20649 terminal101 failed to observe the requested
boundary before its watcher deadline. ReadSource uses independent read-only WAL
snapshots, so the initial mutation-lock hypothesis was rejected by inspection.
Use8MiB + direct rustix SIGKILL and record observed catalog path before the kill;
freeze startup replay via public open_with_recovery_check, verify that same
catalog path, then retry real recovery.68997 terminal0 proves both installed/
source-owned and destination-catalogued/source-present boundaries, older Undo
and original bytes. No private journal phase/output assertions.

Malformed StageMigration with Change::CreatedFolder is a prepared raw SQL input.
77769 terminal101 reproduces record() reverse_path unwrap panic. Inspection now
checks an optional reverse path and falls back to the change's forward path for
context; execution still rejects the subtype.22974 malformed case terminal0,
Recovery inspect/copy preserves both original/prepared files and interruption.

22974 collision red: early cross-device prepare returns WouldBlock before the
normal conflict review. Dispatch now checks occupied targets first, allowing
Skip then reopened Keep both; the dedicated migration retains its final race
guard.92767 terminal0:20 focused parents (upgrades5, cross-directory6, saved9),
two invoked helper definitions, saved helper observes two distinct kill boundaries.
No warnings. Added native cross-directories scenario to inspect pending backup
with Quick Look, Keep both, refresh/copy Recovery, Undo Recovery copy, then older
replacement and older Copy Undo; unrelated occupant and seed must survive.

Final current pipeline62955: locked full Files core -> all-target Clippy ->
default desktop build -> workspace fmt check. Rust/build source snapshot is
stage-migration-compiled-source.json. Do not edit Rust until terminal; no current
schema14 native run yet. Companion validation12377 has inventory/syntax/tracked
whitespace checks. Final evidence/source hashes still pending. Prior schema13
binary7178 remains historical until the build completes. Broader forward
containing-folder Copy/Move, unexpected stage/payload contents, already-decomposed
legacy migrations and the entire ledger Next work remain open.

## Final core/build green; native in progress

62955 terminal0. stage-migration-final-core.log:268 parents passed,0 failures,
19 ignored helper definitions invoked by parents (saved handoff helper runs
twice). stage-migration-final-clippy.log clean all-target40.46s; default build
43.71s; workspace fmt check passes.97 Rust/build paths in stage-migration-
compiled-source.json remained unchanged across the pipeline. New desktop SHA256
e322f0b72d318941737d6bf6c295e372ffb3252a2d59e1ee122c9eb5617336a7.
12377 terminal0: inventory, Python syntax and tracked whitespace pass.
Native93 dark is live52811, output native-stage-migration-93-dark; await same
handle, inspect all captures and logs, then run light1.25 sequentially. Native94
output name reserved native-stage-migration-94-light (not launched yet). No Rust
edits after the source snapshot. ADR0046 final evidence manifest/source hash and
ledger update remain pending. Full product goal active; no blocker or agents.

## Current verified checkpoint

Native93/52811 dark and94/86206 light1.25 terminal0, each25 checks. Both use
actual device IDs35/2050, same e322 desktop, native keyboard/clipboard and
Quick Look/Recovery. All18 screenshots inspected using contact sheets and
full-size saved collision/pending/returned captures. Pending/returned backup
previews are pixel-identical as expected: same original filename and bytes
remain visible through the physical handoff. No panicked/CRITICAL/Gtk-WARNING
in either files.log. Both private secondary destinations removed. New native
scenario passed first attempt in each scheme; no UI/application fix needed.

Core268/19 helpers, Clippy/build/fmt62955 and inventory/syntax/whitespace12377
are terminal0.97 compiled paths unchanged. No Rust edits after the final source
snapshot. Source/evidence: stage-migration-source.json and stage-migration-
verification.json. ADR0046 records verified scope and remaining work. All current
sessions terminal, no agents or external blocker; full product goal active.

Next public regression should cover forward folder Copy/Move containing actual
owned backups. Current dedicated migration is Undo-only; forward recursive
transfers can still lose saved-container references or clone internal backups.
Complete the real data/catalog contract, not an evacuation workaround. Include
partial transfer/retry/restart and older Undo. Then unexpected stage contents/
directory or special payloads, legacy already-decomposed returns, ownership/GC
and remaining ledger Next work. The ordinary prefix-lookalike Merge restriction
and user-facing names for owned metadata conflicts also need attention.
