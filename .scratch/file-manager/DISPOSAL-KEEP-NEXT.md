# Public disposal Keep handoff

The full five-area goal remains active. Schema47 adds public retention of both
original disposal roots, whole-root Recovery Copy, fresh nonserialized review,
original parent/root checks and atomic metadata retirement. Files never moves
retained contents as part of Keep. The original request/cursor supplies Continue.

Current input manifest: disposal-keep-inputs-06.json (878 build inputs).
Full no-default library: disposal-keep-lib-03.log, 185 passed / 21 ignored,
zero failures/filters. Earlier inputs04 passed 184 library tests before phase and
expiry acceptance was added. The selected backend matrix passes 500 tests across 20 targets with 42 ignored
helpers, including both required restart gates. All-target default Clippy with
warnings denied, formatting and fixture syntax/schema47 pass. Normal isolated desktop-feature
library passes 266 tests with 22 ignored helpers and no failures/filters; do not infer native/workspace/release acceptance from these checks.

Failures: disposal-keep-red-01.log is the missing public Keep acceptance on inputs01.
Red-writes-01/02 reproduce ignored journal deletion on inputs03. The probe shows
pending=1, stage_intents=0, garbage=3. Fix verifies record insertion, physical and
namespace indexes and all journal/context retirement inside the transaction.
Private failed roots are archived in disposal-keep-failure-fixtures-01.tar.gz,
with 43 decoded verified entries. Its JSON records exact roots and archive hash.

New public acceptance parents in transfer/stage_boundary/disposal_checks.rs cover:
- both retained roots, arbitrary raw-name extras, two restarts, whole-root copies,
  completed output/source edits, original continuation and history expiry;
- all relevant earlier phases, cross-filesystem Move and raw symlinks;
- stale other-root changes and deserialized review losing Keep authority;
- substituted work-root generation, original return and both generations preserved;
- aborted/ignored retention insertion, physical/namespace indexes and journal/binding
  deletion, followed by a successful fresh review and exact original continuation.

Next implementation scope: edited partials/source disappearance and restricted
created-parent access acceptance; then remaining generic Forward/Undo abort-stage
cleanup. clean_stage remains reachable from the generic Forward abort branch in
operations.rs and reverse Undo recovery. Do not generalize disposal rights from
retention rows or freshly captured paths. Preserve original generation proofs,
consumed-name dispositions and independent task bindings through every phase.

Keep the remaining five-area ledger: broader Undo/expiry, queued lineage, large
reports/paged owners, Trash/Restore; backend separation; product/device/accessibility
and physical acceptance; startup/focus/performance; full native/workspace/Shell/Notify,
Niri distribution, release package/docs and GTK reference retirement.

The default run receipt is continuation-core-1792795589-receipt.json. Exact final
results are in disposal-keep-verification-01.json. Source and attempt manifests/logs
are archived in disposal-keep-source-01.tar.gz with a decoded verification JSON.
All build-source changes from inputs05 to inputs06 are the single Clippy guard
style adjustment; final library, integration and desktop runs use inputs06.
