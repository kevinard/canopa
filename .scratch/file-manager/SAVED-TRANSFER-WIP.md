# Saved ownership during forward folder transfers: work in progress

Full product goal active; previous goal turn was progress. ADR0046 scope was
verified268 core/19 helpers, same e322 desktop native93/94 each25 checks,18
captures inspected, clean logs. Those source manifests are historical now:
current schema15 work intentionally changes the source. No15 default desktop
or full-suite/native validation yet. No agents, no external blocker, .git remains
read-only. Do not edit Rust while a compiler is live; keep same handles on waits.

Public regression additions in undo_saved_stage_move.rs:
- Copy containing folder omits catalog-owned stages but keeps unowned prefix
  lookalikes. saved-transfer-red.log/75845 terminal101: unwanted extra private
  folder. Copy omission added,30537 terminal0, copy-first.log.
- Move containing folder crossFS then Recovery inspect/copy, Undo Move, older
  replacement Undo and older Copy Undo.75845 terminal101: Recovery Unavailable.
- Cancel after the actual saved-box handoff before note.txt moves; Recovery must
  retain source context; reopen/resume, separate Undo groups, older history.
 44255 terminal101 saved-transfer-partial-red.log: eager context shows target
 while note is still at source. This is real public evidence, not a DB assertion.

Current implementation:
- saved_stage Movement {parent_identity, context: Option<ContextMovement>};
  ContextMovement has byte-preserving from/to directory paths (not sibling-only
  Undo Relocation). Optional Change::Rename.saved field, omitted on older wire.
  Four constructors initialize None; explicit patterns have .. where appropriate.
- Migration forward bool(default false); source/destination methods branch by
  direction. prepare_inner uses Preparation/Forward request objects and live
  transfer progress/placement callbacks. Undo validates recorded original parent
  through scoped refs; both directions preserve original through copied install.
- Store VERSION15. append_transaction factored from prior append alongside
  reverse_transaction. publish_saved_stage forward atomically rebinds stage owners,
  appends namespace Rename and adopts installed PRIMARY expectation; Undo uses
  previous reverse helper. context::prepare captures Pending::Forward bindings
  for a forward StageMigration. Parent role for Rename.saved protects parent
  recreation in partial/complete reversal. Original logical identities stay.
- Record context_moves Vec<ContextMovement>, empty omitted/defaulted for older
  records. Identified saved-step following marks deferred display-context moves.
  SourceRemoved/plain Rename or RemovedDirectory actual commits change that
  context only within the recorded from/to parent mapping. Keep mappings until
  saved-step reversal, so archives can follow reverse ownership independently.
  Record.visit_paths includes these scoped mapping paths. follow_archived accepts
  SourceRemoved and RemovedDirectory for this context following.
- Transfer queries identified owner once per directory. Nested Copy omits only a
  known file/link previous payload in a stage with no extras, preserving source.
  Internal nested Move collisions get a fresh private sibling name automatically.
  Same-FS saved box uses intact Rename+Movement; crossFS uses forward migration.
  Direct saved-box Merge is refused to avoid merging ownership. Unexpected
  contents/payload handling remains incomplete beyond preservation.

Feedback:
90734 check terminal101 only four explicit Rename patterns needed saved field.
74039 Move first terminal101: sibling-only Undo Relocation validation refused
cross-parent context; new ContextMovement corrects that interface.
62075 terminal0: initial Copy+completed Move2 public parents pass.
80940 terminal101 only two Record Self constructors needed context_moves default;
fixed. Current17727 live: saved-transfer-partial-second-fix.log, filter folder_
(includes Copy, completed Move, partial Move). Poll SAME handle first.

Immediate small correction after17727 terminal: same-FS Change::Rename.saved
currently uses fs::metadata(...).unwrap() inside Option::map. Replace with an
if saved_record.is_some() expression using ? so unavailable source parents return
an error rather than panic. Consider restricting direct Merge guard to Move.
Do not edit while compiler live. Then act on public partial result.

Required next: forward collision and file Skip/Keep both/partial contexts;
sameFS Merge; active+archived ownership; changed/raw-link payloads; cancellation/
placement/retry provenance when reconciliation already completed a handoff;
actual process kills before and after catalog cutover; authentic14 input;
full core/Clippy/default build/native/source manifests. Add native forward saved
backup scenario to directory_moves=True branch of files-undo-cross-directories.py.
No complete Move claim from the first2 green tracers. ADR0047+CONTEXT link created
as in-progress decisions. Entire ledger Next work remains active.

## Latest feedback (supersedes live17727 above)

17727 terminal0:3 Copy/completed Move/partial Move parents pass after deferred
context correction. Same-FS saved Rename now captures parent with ? (unwrap
removed). Saved Merge guard restricted to Move. Nested internal Move collisions
choose a fresh sibling name, preserving unrelated occupants; direct saved Merge
refuses without mutation. Same-FS Merge uses intact Saved Rename, crossFS uses
Migration.20450 terminal0:4 parents, including four same/crossFS Merge with
Skip/retry and Keep both combinations. Recovery follows actual owner location and
chosen basename while saved data stays available; older Undo succeeds.

Added active+archived forward/return/expiry/copy-out tracer.83272 terminal0:
25 focused parents (upgrade5, directory-cross6, saved14),2 invoked helper
definitions. Existing Undo handoff helper runs both observed boundaries. No
warnings. SourceRemoved/RemovedDirectory archive context following now uses
garbage_paths namespace queries rather than scanning every archived record for
every moved file. Record.context_moves Vec remembers only movements its actual
identified stage followed; actual user file/directory commit changes context,
reverse saved-step following pops the corresponding movement. Empty vec omitted/
defaulted for old wire. Existing public root/legacy/Undo tracers pass.

Native directory_moves=True fixture now adds forward backup Move -> Recovery
Quick Look/copy -> Undo Recovery copy -> Undo Move -> refreshed Quick Look ->
older replacement and older Copy Undo. Python syntax passes, not executed yet
on15. Existing directory_moves=False scope unchanged. No default15 build/full
core/Clippy/native/source snapshot yet.

Current21145 live: saved-transfer-kill-first.log, new public forward handoff
parent observes installed vs catalogued with8MiB backup through the existing
SIGKILL helper (new forward env path). Freeze startup replay and verify the
observed public catalog path, retry real reconciliation, deliberately reissue
remaining source folder as a fresh Move, then undo its completion, undo the
interrupted prefix, undo older replacement and Copy. This proves completed-prefix
recovery and fresh deliberate retry, not persisted unfinished jobs/exact restart.
Poll SAME21145 first; do not edit Rust while compiler live.

Before final validation, consider excluding internal nested saved-box migrations
from user completed-item count (currently counts them as completed); preserve
recorded history id even if only internal work completed. Need add authentic14
upgrade input and forward raw-link/edited saved versions. Also test source/dest
placement interruption/provenance if reconciliation already finishes a handoff.
Broader unexpected payloads/contents, directory disposal/GC, legacy decomposed
returns, durable jobs/FD authority and entire ledger Next work remain active.

## Current final-validation checkpoint

21145 terminal0: forward kill parent observes installed and catalogued boundaries,
reconciles, fresh deliberate folder retry then separate completed-prefix/old Undo.
84170 terminal0:26 focused parents,2 invoked helper definitions (same saved helper
runs four boundaries via two parents), enhanced raw-link/edited payload traces
include forward Move+Undo before the old directory return.14 ordinary upgrade
input added; active+archived input loops13/14; original Undo kill input explicitly
sets14 before startup (wire extensions absent), proving interrupted14 upgrade.

Nested internal metadata transfers now return no user completed-item increment;
history id is still retained via actual recorded rows. Public Move count5 checks
only user data/directories. Native Jobs uses operation id for Undo availability.

40247 terminal101: new root Keep both return with saved backup fails context
parent validation. Movements.relocate_member rewrites directional Rename paths,
but had omitted saved.context.from/to.21476 terminal0 after updating context.from
with destination-role original and context.to with source-role current. Validation
stays strict; newer original folder identity/content and older Undo preserved.

49080 terminal101 real reconnect tracer: watcher observes installed/source-owned
8MiB saved copy through public Recovery, moves only private target parent aside,
replaces it with empty private directory and marks DestinationFolder check offline.
Transfer attention retains actual pending journal. Restoring parent then resume
lets reconciliation finish handoff; old queue incorrectly retries removed source
and produces false missing-source TransferFailure.93172 terminal0 after adding
Continuation.saved receipts {id,ordinal,source identity}, registered before
prepare_forward and retained through attention/failure. Store.saved_move_completed
requires unconsumed exact Saved Rename + expected actual directory + actual saved
owner, no remaining journal. Resume skips that completed metadata task and retains
its actual prior operation id. Source path absence is never sufficient. Normal
error/cancel recovery also recognizes a handoff it just finished, preserving
remaining queue and avoiding rollback of already migrated data.

Forward descriptor validation now requires change identity == actual source stage
identity and captured original-parent identity == source Stage parent. No payload
or namespace authority is inferred from a private-looking name.

Formatting4310 terminal0. Current full pipeline38913 LIVE: locked full Files core
-> all-target Clippy -> default desktop build -> workspace fmt check. Logs:
saved-transfer-final-core/clippy/build/format.log.97 current compiled Rust/build
paths in saved-transfer-compiled-source.json. Do not edit Rust while compiler
live. No current15 default/native proof until it finishes. Previous e322/schema14
binary and stage-migration manifests are historical. Need parse actual core counts,
act on any failures, then native95 dark and96 light1.25 sequentially using
--undo-cross-only --undo-cross-case moves, outputs native-saved-transfer-95-dark
and native-saved-transfer-96-light (not launched). Inspect captures/logs, private
cleanup, final source/evidence manifests and ledger. No agents/blocker; full goal
and Next work remain active. Root~2.1GiB /tmp~1.3GiB free at last check.

Broader remaining: unexpected stage entries/directory/special saved payloads,
legacy already-decomposed migrations, shared-owner GC/disposal, bounded context
moves for long-lived protected archives, durable jobs/exact persisted restart,
directory-FD/original-media/device authority, and entire ledger Next work. Direct
private-folder selection policy and unowned prefix-lookalike Undo Merge/UI labels
remain to finish. Do not claim full Copy/Move/Undo from this scope alone.

## Full core/build green; native95 in progress

38913 terminal0. saved-transfer-final-core.log:276 parents pass,0 failures,19
parent-invoked helper definitions (saved helper four boundary calls through two
parents). saved-transfer-final-clippy.log all-target clean57.50s; default build
49.13s; workspace format check passes.97 current compiled source paths unchanged
through the pipeline. Desktop SHA f8b65103794d6ab8e8195f52d1aaa058617f90404cf7b63a9efb373102b59e07.
51829 terminal0: inventory/Python syntax/tracked whitespace. No Rust edits after
saved-transfer-compiled-source.json snapshot.

Native95 dark LIVE95336, output native-saved-transfer-95-dark, --undo-cross-case
moves. Observed captures01..07, no final result yet. Poll same handle. Once it
finishes, inspect10 captures and clean log/private cleanup, launch native96
light1.25 sequentially in native-saved-transfer-96-light (not launched yet).
Then final source/evidence manifests, ledger/ADR0047 update and show decision.
Entire product goal active, next work unchanged; no agents or external blocker.

## Verified current checkpoint

Native95/95336 dark and96/38868 light1.25 terminal0, each29 checks, actual
devices35/2050, same f8b desktop. All20 captures inspected via contact sheets
and full-size root Keep both/forward backup/returned backup images. Preview
contents and original filename remain correct; underlying Recovery original
location visibly changes from secondary filesystem to returned source. No
panicked/CRITICAL/Gtk-WARNING in files.log. Both private secondary folders removed.
Native additions passed on first attempt in each scheme; no application/UI fix
needed from native feedback.

38913 core276/19 helper definitions, Clippy/build/format and51829 inventory/
syntax/tracked whitespace all pass.97 Rust/build paths remained unchanged.
Final evidence/source files are saved-transfer-verification.json and
saved-transfer-source.json. ADR0047 records scoped verification. All current
sessions terminal; no agents or external blocker. Full goal stays active.

Next first sharpen archived-context provenance with a public regression: create
protected archived data for a file under SourceA, move containing folder A to B
(with saved stage), then create/move an unrelated file from the old A path to B
and Undo that unrelated file move. The archived data and its original-context
location in B must not follow the unrelated item back to A. Current context_moves
match parent pairs without an operation/ordinal epoch; possible stale marker
adoption is unverified and should be tested before claiming full context handling.
Consider recording actual metadata-step and owner-step keys when following a
Record, rather than weakening namespace/identity validation. Active records can
reconstruct from keyed history; legacy archived plain mappings need a conservative
upgrade/review policy. Do not infer data authority from matching paths. Also bound
long-lived archived context moves after their originating Undo steps expire.

Then finish unexpected stage contents/directory/special saved payloads, shared-
owner GC and disposal (an archived reference must not delete an active backup),
legacy already-decomposed returns, durable jobs/exact persisted restart, directory
FD/original-media/device authority, and the entire implementation ledger Next work.
Unowned prefix-lookalike Undo Merge and user-facing internal-collision wording
still need attention. Prior e322/schema14 proof remains historical; current
compiled default f8b/schema15 is verified only for the scope above.
