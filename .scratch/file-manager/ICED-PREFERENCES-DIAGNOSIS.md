# Preferences toolbar diagnosis

The keyboard/viewport diagnosis below remains preserved. Latest implementation
and runtime state: [Iced locations checkpoint](ICED-LOCATIONS-WIP.md).

## Authoritative current state — scoped Preferences/resize verification

The entire file-manager goal remains active. All product completion checkboxes
remain unchecked. The supported default GUI uses Iced and canopa-ui, with no GTK
widget dependency. Root ADR0018 records the user's consistency correction.

All compiler/native handles are TERMINAL. Cells1177,1185 and1187 completed;
sessions74268 and all matrix children exited. Host audit shows only original
Niri1238 among cargo/rustc/rustfmt/canopa-files/niri. No compiler or native fixture
is live. Original PipeWire1537/1562, unrelated Shell research and read-only Git
policy are preserved. No subagents were spawned.

Clean candidate a85b1012557e515143f27a8bc7940b48ce273a2381df70c86854920595d77ae4
is102,806,240 bytes, bound to483 unchanged inputs in
iced-preferences-reflow-final-source.json. All47 adapter tests,17 public tests,
Files all-target Clippy lints, build, workspace format and tracked whitespace pass.
Four existing unused operation-helper warning groups remain unsuppressed. The
unchanged root fork retains its prior workspace compile,4 library+3 queue tests,
formats and all6 published-archive checks; full shared native/strict workspace
and release gates remain required.

The clean final matrix passes all14 workflows/198 declared checks, plus a separate
light/125% original-prefix stress case with20 additional wide/narrow cycles. All17
application logs are empty. Eight selected current captures were inspected, not
the exhaustive visual/focus/accessibility matrix. Exact summaries and source-bound
scope are in iced-preferences-verification.json. The prior826a8c stopped matrix
remains historical evidence:3 workflows/39 checks pass,1 actual light-view failure,
10 unstarted workflows. It has not been relabeled successful.

The original Preferences keyboard failure has timed causal proof: Alt+2 preceded
13 Tabs on the old Views tree, with the page applied only afterward. The shared
widget-invalidated queue boundary fixes it. Twelve clean fast probes passed on
826a8c; both complete Preferences workflows also pass on the final candidate.

Native resize traces show an old clamped scroll offset overwriting the mapped
anchor. Two controller replays fail at this overwrite before the guard for
opposite message orders; all47 adapter tests pass afterward, including settled
deliberate scrolling. Current-folder Scroll carries captured model and measured
native viewport sizes; both must match the current model. Existing tab/epoch/raw-
path guards remain. No unconditional focus reveal was added. Active diagnostics
are removed; archived trace source, exact scope and red/green notes live in
reflow-debug/README.md. Four clean prefixes, four traced prefixes and both traced/
clean20-cycle stress runs also passed BEFORE the guard. The terminal original
native failure was not captured by the trace; do not claim those green repeats
alone proved its cause. The feedback hazard itself is directly reproduced and
corrected; the final original/stress native loops pass without diagnostics.

## Required continuation

Finish the icon-grid scrollbar gutter: selected rightmost cards are clipped at
the scrollbar edge in the inspected narrow (and rightmost wide) captures. This
is a known visual follow-up, not complete tiling/visual acceptance. Then resolve
built-in personal folders through XDG definitions rather than the conventional
HOME joins in preferences_editor::place_path. Sidebar and Locations already share
that resolver. Keep missing-folder handling, raw path bytes and worker-only I/O.

Continue the full Iced operations/Undo/Trash/Recovery/devices/file clipboard/DnD/
FileManager1/media/syntax-color ports, durable native job hydration, source lease
ownership and exact filesystem-step continuations. Never replay arbitrary custom
commands. Complete preference focus/source-authority/large-catalog/accessibility,
shared Shell/Notify native, strict/full workspace, performance, release/packaging/
review/commit gates. Root0020 and Files0062/0063 record the scoped fixes. Do not
mark the unbounded product goal complete, paused or narrowed to this slice.

## Earlier checkpoints (historical)

Goal turn classification: progress. Full file-manager PRD remains active.
Using /home/kevinard/.agents/skills/diagnose/SKILL.md.

## Feedback loop and actual reproduction

The `--iced-preferences-case toolbar-fast` workflow preserves the original fast
Views save -> Alt+2 -> batched Tab13 -> Refresh typing -> Tab -> Return -> Ctrl+S
sequence, then asserts the real serialized toolbar. No extra clipboard queries,
captures or sleeps intervene in its trigger. The reduced failure timeout is4s;
full acceptance retains15s. It stops after the first Add so one iteration is
roughly10–15s, including a fresh private desktop/application/configuration.
`toolbar-trace` adds the prior clipboard/capture probes as a comparison only.
The full workflow is restored to the original fast sequence.

Five sequential light125 probes on executable77f57 reproduce1 failure and4
passes: iced-preferences-fast-repro01 fails;02–05 pass. Its failure capture and
serialized toolbar match final-native02: actual Views420/List saved, Refresh
absent, toolbar otherwise unchanged and GUI says Saved. The traced success from
the previous turn is insufficient to establish cause or stability.

## Ranked hypotheses (shown to user before instrumentation)

1. Filtering replaces the focused command. Predict accepted Refresh text, a Tab
   focus on an old/unfiltered choice, then lost focus before Return and no Add.
2. Target/revision guard rejects the control. Predict an Add event reaching the
   controller with a mismatching page/selection revision or busy state.
3. Native window/modifier delivery differs. Predict missing or modified Return,
   wrong owning window, or no focused native control despite the intended Tab.
4. Save precedes application of Add. Predict save payload without Refresh before
   an otherwise accepted Add event, or an out-of-order configuration completion.

## Instrumentation

Tag [DEBUG-pref-8e21], opt-in CANOPA_FILES_TRACE_PREFERENCES only. A new
`toolbar-log` probe uses the original fast input, with this variable set before
application launch. Native inputs/captures are otherwise identical to toolbar-fast.
Actual focus is re-read after each focus operation (not merely its requested ID).
Group key state, lost control identities, dispatched control message, editor
page/filter/Add/save/completion with revision/busy state, and actual save toolbar
are logged at the boundaries distinguishing the hypotheses. Do not retain this
instrumentation in final product source. No tests or native acceptance are
claimed for the trace candidate yet.

Original focus/preferences_editor source and the exact77f57 executable are saved
in preferences-toolbar-trace/; baseline-bin is a generated102MB copy for a direct
comparison if needed. Current source is temporarily instrumented, so the older
final-candidate/current-source receipt is not a new product proof. Trace build
uses its own source/candidate receipt. No shared vendor code or core logic changed.

## Causal result and fix

Untimed trace01 passes and02 fails. No Filter edit or Add reaches the failed
controller; nine Tabs traverse Views before Page(Toolbar) applies. Actual focus
operations succeed, Return selects toolbar item0, and Save faithfully writes that
unchanged draft. This rules out filtered-choice loss, target rejection and a
writer race for this failure.

Timed trace01 fails and02 passes on1afb95ba9841000aa9d93ce4c2d5ce13d2f5a0e2461187480013412936bb625f.
The failed widget receives Alt+2 at4.689527s; all13 Tabs use Views at4.689935–4.696410;
the controller only applies Toolbar at4.697315. Native delivery was correctly
ordered. The adapter processed the entire queued batch before program messages.
The passing comparison applies Toolbar before every Tab. Exact observations are
in iced-preferences-input-boundary-causal-proof.json.

The shared iced_winit fork now yields a window's untouched event tail when the
interface reports widgets outdated, applies messages/rebuilds, and polls to replay
the older tail before new events. Deferred events are not broadcast as ignored.
Files focus groups request widget invalidation for emitted form/control/input
messages; Tab-only traversal remains synchronous without a program rebuild.
Only iced_winit src/lib.rs and new std-only src/event_batch.rs change in vendor.
Root ADR0020 and Files ADR0063 record the seam. Source/patch/provenance and all6
exact published archives verify. Original versions/licenses remain unchanged.

Queue regressions: legacy whole-batch dispatch fails2/3 (page transition and
multiple page/filter transitions), with the red source/log retained. Correct
queue contract passes3/3; empty/ordinary traversal has no boundary. These tests
exercise the actual adapter queue helper, while native probes cover its full UI
integration. test-vendor.py runs the same std-only source from its disposable
snapshot after the existing layer-fork library tests. Cargo builds cover the
actual adapter integration.

Fixed trace candidate749bd78ae1b1d01ce9c1f359a4bd20550010baeb6985fa14e9cd51a3cc6b6215,
102,963,208 bytes, input-boundary-trace source483. Three native light125 probes
(boundary-trace01–03) pass the unchanged fast sequence: zero Tabs precede the page
acknowledgement, all7 filter edits apply, Add dispatches and Refresh enters Save.
Their only app log lines are the intentional tagged traces. Diagnostic13-Tab
elapsed spans are retained separately, not claimed as full performance proof.

Temporary traces/env flag and toolbar-log/toolbar-trace fixture branches are now
REMOVED from active crates/vendor/scripts. Archived source-map and source files
in preferences-toolbar-trace/fixed-source bind the traced implementation. The
normative full workflow and reduced toolbar-fast keep the original input without
extra clipboard/capture delays. The original baseline77f57 binary is retained in
preferences-toolbar-trace/baseline-bin for a comparison if needed.

## Current clean candidate and live verification

Compiler cell1111/session29193 is TERMINAL0. Tests07 pass44, public03 pass17,
build05, Clippy05, workspace all-target check01, vendor-tests01, fmt04,
vendor-fmt01 and published archives01 all pass. Vendor tests are4 layer-fork
library tests,0 in the other library and3 std-only event-queue tests. Four
existing unused-operation-helper warning groups remain; strict/full workspace
and release/native shared-stack/performance gates stay open.

Current UNINSTRUMENTED executable102,804,488 bytes:
826a8c37141a8a7eb565ee5057272e9f066866c199df0d0742305d21ff23235d.
Final-candidate.json and current-source.json bind483 unchanged inputs. All tagged
traces, trace env reads and diagnostic input branches are absent from active code.
The original fast input is retained in full acceptance and toolbar-fast.

Current cell1114 runs12 fresh native toolbar-fast probes,10 light125 and2 dark100,
sequentially on that exact executable. At the last observation light01–07 passed
and08 was starting. Do not edit frozen inputs or start another native/compiler
while it lives. No compiler is live. Poll its specific handle/host state; an
observation timeout is not process completion. Any new failure must be diagnosed,
not hidden by sleeps or the retired clipboard/capture probes.

After all12 actually pass, run the final14-workflow matrix under a NEW prefix
iced-preferences-boundary-final-{native,views,interface,actions,browsing,search,
previews}-01/02. Older final-native01/02 already exist and bind77f57, so do not
reuse or count those. Expected8/23/9/15/13/16/15 checks per scheme,198 only if all
actually pass. Verify hashes, all app logs empty, actual selected captures and
complete right input borders, then record final scoped evidence and update ledger/
decisions. Full shared-stack/native/performance, workspace/release/review gates,
XDG locations and the entire remaining PRD stay required. Keep the goal active;
never replay custom commands for recovery.
