# Reproducible partial disposal handoff

The full five-area goal remains active. Schema48 fixes actual deletion of partial
bytes after source disappearance or partial edits. Preparation captures original
source parent/entry generations and a version from the actual copy reader into
its independent binding. It never constructs that proof from a restarted path.

Before consuming a claimed partial, verify original source generations/version
and compare the whole partial prefix or raw symlink target. Preserve the actual
source/payload handles and strict fresh snapshots across the consumed save; check
names and versions again before unlink. Missing/changed originals, differing bytes,
late edits/replacements and cancellation leave data for Recovery Copy/Keep.
Directories still have empty-only rmdir rights. Legacy missing source proof requires
explicit Keep; it is never upgraded from current paths. Exact returned originals
can have changed ctime, but live comparison snapshots retain strict ctime checks.

Current source is partial-disposal-inputs-07.json (879 build inputs). Clippy02 is
all-target/default with warnings denied; format02 and fixture-check02 pass. Full
backend library partial-disposal-lib-01.log passes 191 tests, with 21 ignored and
zero failures/filters. The 19-target integration matrix is in progress. Final
results and default-library receipt will be recorded when terminal.

Red01/02 use inputs01 and delete the sole partial after the source disappears.
Red03 uses inputs02 and separately fails public Recovery inspection for missing
sources and edited partials. Probe01 confirms no retained/published partial and
only the completed-prefix canonical change. Green01 uses inputs03 (2 parents);
green02 uses inputs04 (4 expanded parents). The final source has 6 new parents,
including cross-filesystem Move and a coherent schema47 binding fixture.

Failure archive partial-disposal-failure-fixtures-01.tar.gz preserves four private
roots and 35 decoded verified entries. Its SHA and member manifest are in the JSON.
The large vector assertion in red01/02 is retained as original evidence; red03
uses bounded public-catalog assertions instead. Do not print those vector lines.

The test-only reproduction hooks exercise actual public Engine retry after one
chunk and after the consumed save; they compile out of production. Tests check
cancel/retry, an edit after reading, an edit after saving and source replacement
after saving. Keep preserves late changes. Other public parents cover two restarts,
Recovery Copy/Keep, original returns and refusing same-byte replacement sources.

Next: restricted created-parent access and remaining-work behavior when an original
source is unavailable. The new missing-source test proves preservation/Copy/Keep,
not every remaining-part or abandonment action. Then replace generic Forward/Undo
abort clean_stage paths with durable task-bound disposal rights. Also measure
prefix-verification I/O during interrupted large-file startup in the performance
phase. Preserve original source generations and consumed-name rules; never turn
retention data or fresh path observations into execution authority.

Remaining goal requirements include broader Undo/expiry, queued lineage, large
reports/paged owners, Trash/Restore; backend navigation/search/preview separation;
product/media/drag/device/accessibility/physical acceptance; startup/focus and
optimized measurements; complete native/workspace/Shell/Notify, Niri distribution,
release package/docs and GTK reference retirement. Scoped checks do not prove them.

Inputs05 backend pass stopped at core43: live cancellation incorrectly forwarded
the cancelled token into compensation and produced Interrupted instead of Stopped.
The exact test reproduced again in cancel-red-02; input06 sharpened its terminal
state assertion, exposing operation-cancelled rather than sparse-file mismatch.
Input07 gives only live compensation its own token; cancel-green-01 passes.
Explicit Recovery cancellation still uses the caller's token. Final backend driver
02 uses lib02 and batches50..59, preserving all earlier results/failed attempts.

Restricted-parent acceptance to investigate next:

- Crash while copying into an owned folder whose original mode lacks owner search
  or write access; test Reconcile, Recovery listing, Copy and Keep after restart.
- Confirm both original permissions and partial bytes survive cancellation and
  failed metadata saves, including a second restart.
- Substitute the created parent or its device before permission compensation;
  no original permission-restoration record may authorize changing a replacement.
- Verify the missing-source owner's available actions after Keep: exact remaining
  work, unavailable parts and safe abandonment need direct public assertions.

These are test plans, not findings. Inspect directory_access.rs and the current
journal/task/created-folder proofs before deciding which implementation needs work.

Inputs07 library02 passes 191 tests; batches50..54 pass, including both gates and
live cancellation. Batch55 exposed a second live regression: changed source data
was preserved but its reproduction error escaped before failure reporting, so an
unrelated item did not continue. The exact test reproduced again (batch-red-02).
Inputs08 added a tagged diagnostic; batch-probe-01 found 65536 original bytes still
available in Recovery and no publication of the next item. The debug block is now
removed from the test.

Input09 introduces a private typed reproduction-problem marker and live-task-only
retention through the already verified keep transaction. It checks the current
operation/destination, original bound journal and root generations before archiving
both roots and clearing the journal. This gives no restart execution authority.
Normal source failures can then report the failed item and continue unrelated work;
explicit restart reconciliation still leaves the journal for user review. The
existing batch test now also proves retained bytes are copyable and every remaining
private root is catalogued. Current exact batch-green-01 is running; poll its actual
session rather than assuming a result. Static/full validation must be rerun on the
final source once this regression is green. Prior matrix attempts stay immutable.

Batch-green-01 on inputs09 passes the strengthened direct-engine test. Input10
added a real OperationJobs batch/cursor acceptance parent. Its first run
(live-batch-green-01, despite that filename) failed only because the test expected
Failed while the correct owner was Stopped with Retry/Abandon; the actual snapshot
already showed one completed unrelated item, one failed item and Recovery retention.
Input11 corrects that expectation, asserts Retry/Abandon, and extends missing-source
Keep acceptance to exercise safe abandonment. The full source_checks filter is
running in green03 on input11 (7 parents). Do not treat it as passed until terminal.
Final expected totals, if all selected tests pass: 192 library / 507 selected backend
across 20 targets, and 273 isolated default library. No final matrix/default claim
has been made for input11 yet. Clippy and format must be rerun after this change.

Final-candidate input11 now passes green03 (all 7 focused parents), Clippy03
(all targets/default, warnings denied), format03 and fixture-check03. Backend
driver03 is live: lib03 then batches60..69, source11, result manifest03. All older
results are retained and must not be mistaken for this final run. Expected full
counts remain unproven until the driver is terminal and the verifier checks them.

Final backend driver03 is terminal success: 507 passed / 42 ignored / 20 targets,
zero failures and filters. Lib03 contributes 192 passed / 21 ignored. Both required
batch restart gates pass in core62. The live cancellation regression passes in
core63 and the changed-source batch regression passes in core65. Inputs11 stay
unchanged. The default isolated library is live via atomic-rename-run-core.py,
label1792800566, desktop-lib-01.log, receipt continuation-core-1792800566-receipt.json.
Poll its actual handle and verify terminal output; a receipt path alone is not proof.

Final input11 verification is complete: backend507/42 ignored/20 targets and
normal isolated default library273/22 ignored, zero failures/filters. The default
receipt1792800566 is passed. Both required unignored gates, Clippy03/default/all
targets/-Dwarnings, format03 and fixture-check03 pass. The verifier01 JSON records
all exact attempts and source relations, including both introduced/fixed live
regressions and the corrected queue phase expectation. Existing Files/Niri binaries
and the current rebuilt gate are preserved. Four data-loss roots are archived with
35 verified entries; transient cancellation/batch diagnostic roots were not retained.
Final source plus preserved attempt manifests/logs will be archived in source01
with decoded/member/live verification. Full goal remains active.
