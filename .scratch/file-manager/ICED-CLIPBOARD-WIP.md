# Iced clipboard integration

Status: verified scoped progress, not full product acceptance. Current proof:
iced-clipboard-verification.json. Candidate fc469284 uses Iced/canopa-ui without
GTK linkage. Final source04 binds518 unchanged inputs. The full PRD stays active.
The preceding transport-only proof remains in file-clipboard-verification.json.

Current vertical tracer connects actual Iced Copy/Cut commands to the verified
service, including configured toolbar and context menus. Original selection,
resolved source directory and Storage snapshot are captured before async work.
Only a matching acknowledged owner retains that metadata. Pending publication
and actual clipboard ownership hold the application. Controller construction
starts no clipboard worker; the GUI application starts it explicitly.

Evidence: native red01 (33346) failed the fixture's navigation precondition,
so it is not Copy evidence. Explicit focus/resize readiness fixes the fixture;
red02 (95263) terminated1 at the actual missing Ctrl+C publication, after source
navigation succeeded. Both exact private native handles are terminal.

Verified 9df07f9f default and7f2ecc23 protocol test binaries were archived with
streamed SHA256 checks in iced-clipboard-cache-preservation.json. Cargo package
clean dry-run15025 reported9053 generated files/4GiB; clean28204 removed9046
files/3.9GiB. Only canopa-files generated outputs were cleared; shared dependency
caches and source remain. The default preserved binary was restored as
canopa-files-clipboard-before for genuine native red verification.

Compiler22528 terminated101 on the new message missing from the secondary
Browser routing match; it is fixed. Compiler44490 then terminated0. Native66964
terminated0: Ctrl+C from a340px Iced Browser publishes the exact selected real
file to wl-paste. Both captures were inspected against the selected original.

The next vertical tracer adds actual Paste, completion notification and shared
Undo. Native80843 terminated1 at the missing destination file after Ctrl+V;
Copy and destination navigation passed first. Paste now captures its destination
Origin before read and retains the matching original Copy/Cut Storage snapshot.
External source placement is observed at Paste. Admission uses the shared serial
transfer service; actual reported destinations are revealed only for a still-
matching origin. Failed admission releases the application hold, and accepted
work can open Operations after its Browser has already closed.

Copy preparation is generation-bound before publishing: a slower older
preparation cannot overwrite a newer user invocation. Older publication errors
cannot replace the new invocation's status. This preserves async preparation
without relying on worker scheduling order.

Paste compiler91151 terminated0. Native76327 terminated0: Ctrl+C, actual
Ctrl+V copy, completion notification and shared Ctrl+Z all pass in a340px Iced
Browser. The copied bytes and unchanged source are checked directly.

Conflict-red01 (91819) failed a fixture assumption that Undo notification titles
start with Undo; actual public completion uses the action title and a reversed
count in its body. The fixture now checks those public reversed facts. Red02
(73367) terminated1 at the missing Iced file conflict after Copy/Paste/Undo and
collision setup passed. Neither invalid precondition run is claimed as a product
regression beyond its actual scope.

The shared question window now handles both typed Undo and transfer questions;
question_dialog.rs replaces undo_dialog.rs. Transfer facts/choices render in
transfer_dialog.rs with actual sizes/dates, default Cancel, Replace/Keep both/
Skip or Merge, batch-choice toggle and original-destination Retry. Source focus,
manual Operations review, generation checks and close/escape cancellation share
the existing lifecycle. Real dialog choices are still awaiting native green.

Conflict compiler51579 terminated0. Native19924 terminated0 with4 checks:
Copy, Paste/Undo, floating file-conflict default Cancel, and Keep both/Undo.
The conflict and selected actual Keep both destination captures were inspected.

Cut-red23687 terminated1 at stale Cut ownership after the actual Move succeeded
and its completion notification arrived. The current UI does not yet update Cut.
Before wiring that update, ADR0070 adds engine-owned source remainder facts:
new work arriving during Merge must keep the original folder, while an unrelated
new folder at a completed source pathname must not be adopted.

Public source remainder facts: API-red29072 terminated101 on the missing
outcome field; runtime-red93989 terminated101 on an empty report while actual
new source bytes remain. Engine scope-identity reporting fixes it. Green59674
terminated0 (1 parent); green91916 terminated0 (2 parents), including the
recreated-folder negative case with distinct observed identities.

Report API-red50558 terminated101. Propagation, raw-path serialization, optional
legacy field default and shared-report validation now preserve these facts.
Service green7226 terminated0: the new public report/reopen parent passes with
actual a/b moves, Replace, newly arriving source data and orderly service reopen.

Cut context now follows its original token and requested Selection through
admission to completion. Known reports retain failed/skipped/remaining roots
and original source folder remainders; uncertain reports retain the selection.
Subset construction runs asynchronously and does not probe the filesystem from
Iced. The original source is updated through replace_cut only; a new owner wins.
The outstanding concurrent partial-update monotonicity still needs its public
protocol regression/fix before claiming safe complete Cut behavior.

Compiler13182 terminated0; native Cut55484 terminated0 with five real UI checks.
Protocol overlap-red58176 terminated1: a stale remainder reintroduced an already
consumed file. The worker now intersects remainders with the current same-token
selection. Protocol-green72393 terminated0 with six public parents and two child
helper definitions, including overlapping partial completion and newer owners.

Default compiler88731 terminated0. Native expanded-dark58062 terminated0 with
nine workflow checks: Copy, Paste/Undo, default Cancel, Keep both/Undo, Cut
completion, Move Replace while a newer external owner wins, destination binding
before a delayed external read, text editing/context menus and background owner
lifetime. Final-native71806 terminated0: the same nine light/125% checks and the
11-check existing dark name/Undo/Operations regression pass. Source02 binds518
inputs; changes02 records24 paths, including the dialog rename. The515-input
copy-stage snapshot is historical. Selected dark/light captures were inspected;
broader final checks and remaining capture QA are still pending.

Standards review returned no scoped findings. Spec review found missing desktop
attention notices for background transfer questions. Native regression52913
terminated1 at that missing notification after a real delayed Paste conflict
with an independent Browser focused. A bounded per-job latest-generation map
now notifies each live question once through the existing worker/holds/errors,
without activating its dialog. Both follow-up review axes returned no findings;
see iced-clipboard-reviews.json. Formatting51499 and default compiler29293
terminated0 (the same three existing helper warnings).

Native32699 terminated143 after both expanded dark and light/125% clipboard
workflows independently passed10 checks each. Their completed per-case summaries
and captures are retained. The third existing dark operation regression was
interrupted without a summary and is not claimed as acceptance. A guarded audit
identified its nine orphaned private-runtime processes, verified uid/start time/
runtime against pidfds, closed them and removed only that generated runtime.
All nine pidfds are terminal; see iced-clipboard-interruption-cleanup.json.
Standalone native75146 terminated0: operation-regression02 passes all11 checks.
The current fc469284 executable therefore passes31 declared native checks across
three completed cases. Two new captures from each clipboard theme and the
standalone regression were inspected; earlier captures are additional historical
QA. Dynamic linkage94397 terminated0: all libraries resolved, no GTK linkage,
version0.1.0. AST/matrix and tracked whitespace checks pass; all518 source03
inputs remain unchanged. Both question-attention review axes are clear.

Full core suite30548 terminated0:419 public tests pass, with22 ignored child
helper definitions exercised through their parents. Counts use the final result
per Cargo harness section to avoid counting child output as extra public tests;
see iced-clipboard-core-counts.json and iced-clipboard-full-core-01.log.
Iced library/controller suite72012 terminated0:51 tests pass.
Formatting72491 terminated0. All-target desktop Clippy6927 terminated101 on
one cloned-ref-to-slice-refs lint in the new public report assertion, line129 of
operation_transfers.rs. The assertion now compares borrowed slices. No production
input changed. Formatting94729 terminated0. Source04 still binds518 inputs and
25 changed paths; source03-to04 records the single test-only lint change.
Focused public transfer-service suite51286 terminated0:18 tests pass after the
test-only lint correction. All-target default-desktop Clippy59335 terminated0
with -D clippy::all. Inventory18795 terminated0:571 release dependencies.
Formatting, AST/matrix91 cases and tracked whitespace checks pass. No compiler
or native process remains live. Both independent review axes have no unresolved
scoped findings; source03-to04 changes only that tooling-enforced assertion.

Preservation55988 terminated0. The exact fc469284 executable is retained in
canopa-files-fc469284.gz; streamed decompression matches its118005024 bytes and
SHA256. iced-clipboard-source-04.tar.gz preserves all518 verified source inputs,
the PRD/manifests and the exact pre-lint source03 test. See iced-clipboard-preservation.json and the complete verification receipt.
Final hashing confirms all518 source04 inputs unchanged and the executable still
matches the native-tested build. The419-test full core suite,51 Iced tests and31
native checks ran on source03; final source04 production is identical. The18
post-lint service tests and Clippy verify its only changed test assertion.

Next: implement actual Operations progress and inspection of confirmed/failed/
skipped/remaining results, and finish native Folder Merge/batch/unavailable
question acceptance. Preserve separate safe Retry/Continue, source/descriptor/
media/journal authority gates; never synthesize a fresh plan from saved paths.

Remaining work includes transfer progress/details and full question acceptance,
retained-source Clipboard leases, journal/restart authority, real DnD,
devices/Recovery/media, Downloads, accessibility/performance, primary activation,
release/workspace checks and independent review/commit. Do not mark PRD boxes.
Iced/canopa-ui remain mandatory; Git is read-only and unrelated shell research
must remain untouched.
