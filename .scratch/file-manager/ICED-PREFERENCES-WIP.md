# Iced global preferences checkpoint

Latest continuation: [Iced personal locations](ICED-LOCATIONS-WIP.md). The scoped
Preferences checkpoint below is historical; current source/binary and open full
product work are recorded there.

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

Current goal turn: progress. The full PRD remains active and incomplete. Previous
folder-view slice is verified separately in iced-views-verification.json and
ADR0062; that evidence does not bind the new preferences sources/executable.

## Current implementation

ADR0063 records decisions. New preferences_editor.rs and preferences_view.rs
implement one global transactional draft with Views/Toolbar/Locations/Shortcuts,
Alt+1–4 page switching, width and multiple-shortcut text drafts, command filtering,
reordering/add/remove, raw local file URI bookmark text, recording/reset, explicit
Save/Reload and dirty close/reload prompts. Shared focus/control frames preserve
batched keyboard traversal. Fixed notice slots retain native fields. The ordinary
Iced window is separate from the action editor and part of application lifetime.
Ctrl+comma, Ctrl+D, folder menu and configured toolbar routes are wired.

ConfigurationSnapshot exposes is_editable. The existing single configuration
worker also accepts SaveSettings. Shared pending request IDs serialize the two
editors; existing stale source/target checks preserve other namespaces/actions.
Publications update Browsers but leave drafts unchanged. The default Save button
is disabled for built-in defaults; the actual writer refuses even Ctrl+S there.

Narrow Locations replaces the current folder popup with Back and raw-path places.
It and the sidebar use Desktop::place_path. This still uses conventional personal
folder fallbacks: migrate XDG resolution before declaring Locations complete.
Trash/devices and full focus/source-authority/accessibility acceptance remain open.
The original View options menu item remains last so its established End traversal
stays available; new Preferences/Bookmark/Locations items precede it.

## Verification and live work

Adapter tests02 pass42. Test01 failed compile because wrapping Row has no push;
wrap is now applied after filling each Row. Clippy01 rejected one new method impl
below the test module; it was moved before tests. No lints were suppressed. Those
artifacts are retained. Four old unused-operation-helper warning groups remain.

Pipeline29251 completed tests03 (42), Clippy02, public tests01 (17), build01
and fmt01 successfully. Candidate01 is102,657,776 bytes,
88e29ccaf28d77fb886dd8ccc7495839ba2e1c9a0d1358c75e6363ff75d1b995.
Its frozen482 inputs are archived in iced-preferences-source-pre-accelerator.json.

Native01 failed a real page-accelerator defect. The failure capture visibly shows
width4201 and Columns still selected: Alt+1 inserted1 into the focused field
before the application listener switched pages, then batched Tab began from the
retained field. The new view uses focus::group_with_shortcuts to intercept form
accelerators before native inputs and unfocus synchronously; application-level
Alt page handling is removed to avoid duplicate publication. Recording retains
its own key route. The existing native sequence remains the regression.

Pipeline1018 finished tests04 (42) and build02 successfully. Candidate02/03
is102,687,208 bytes, fe9c3e40d29e1114846bb17e9c33090f4039acbe9e46cbf631b4170e902e1c73.
Native02 visibly saves420/List but stops because the harness wrongly expected
quoted KDL strings. The writer correctly emits bare strings (mode=list, toolbar
IDs). Harness03 uses shlex for these single-line nodes and accepts quoted or bare
values. The driver now retains private config on every preferences failure.

Native03 gets through actual Views and toolbar order/action saves, then reproduces
a second actual bug: keyboard Enter on Record also reaches the app's recorder,
so its shortcut becomes Return before F8 arrives. Failure.png and retained KDL
show the real conflict and unchanged prior saved source. The shared control now
supports a separate keyboard message. Preferences uses Change::Record(true) for
that activation, action editor uses RecordKeyboard; controllers ignore the
activation callback and retain the first actual key. Mouse recording still
captures its immediate first key. Two new controller regressions cover both.

Cell1034 completed tests05 (44) and build03. Candidate04/05 is102,695,928 bytes,
1651d94f04e64263829371c6ec36ee890341ce58e7a8a2cb931e62152c161f81.
Native04 reaches the recorder and remapped actual search, then the harness's
bookmark Tab count omits the three visible Sidebar controls. Screenshot shows
focus on the raw bookmark selector, not a field; fix count15->18. This is a
fixture error, not an application copy/focus bug.

Native05/06 pass8 checks each, dark100/light125, on1651. Clippy03, public02 (17)
and fmt02 also pass. The input manifest/candidate and scoped baseline are saved
as source/candidate-pre-scrollbar and iced-preferences-pre-scrollbar-verification.
Actual final-dark toolbar/source-notice/menu captures were inspected; visual QA
found the scrollbar overlaid the right edge of fields. That baseline is not final
visual acceptance. Add12 logical pixels of right content inset in both editors.

Cell1052 completed tests06 (44), build04, Clippy04 and fmt03 successfully. Four
existing unused-operation-helper warning groups remain. Public02 passes17, with
unchanged public implementation/test inputs; its only subsequent frozen-input
delta is the two GUI content insets (confirmed by exact source comparison).
The current executable is102,698,760 bytes,
77f57b428a31474dfb3f19377a0a9b07e12ba90dbb6b5d2a227c69737fa1cd4e.
iced-preferences-final-candidate.json and current-source bind482 unchanged inputs.
Python AST passes the driver and seven Iced workflows; tracked whitespace passes.

Final matrix cell1056 is TERMINAL. It passed final-native01 (dark100,8 checks),
then STOPPED on final-native02 (light125) while adding Refresh. None of the other
12 queued workflows started, so do not count them or198 as passed. The failure
capture shows Toolbar, empty filter and Saved status; retained config contains
Views420/List but no Refresh. This is unresolved; it is not yet classified as an
app or harness-only problem. No later Rust edits have occurred.

Diagnostic harness change adds actual filter copy and captures after typing,
focus traversal and Add activation. The original fast harness/source is archived
in iced-preferences-source-first-final-matrix.json. Current482 hashes include the
instrumented harness; executable stays77f57. Candidate's harness_change states it.
Cell1063 is also TERMINAL: toolbar-diagnosis01 light125 passes all8 checks, copies
exact Refresh and shows the real focused Add Refresh before successful addition.
These extra clipboard/capture steps change timing and selection; success does not
establish the cause or invalidate the original failure. Do not simply accept a
longer delay or this green retry as final acceptance.

Next reproduce the original fast Views->Toolbar->filter->Tab->Return->Save path
in repeated private light125 probes. Keep the failing case. Capture or instrument
actual focus/selection and editor target/revision before/after filtering and
activation to locate the first lost event. One hypothesis is that filtering removes
a previously focused command before the new choice view reconciles, losing Return;
this has not been demonstrated. Investigate rather than implementing a speculative
focus recovery rule. Also distinguish save acknowledgement/paint timing from
application events. Native05/06 and diagnostic successes provide comparison cases.

Only4 current-executable captures were actually inspected: final-native01 toolbar03,
source-notice08 (the input right border is now complete), final-native02 failure,
and toolbar-diagnosis01 add-focus02. No exhaustive final visual/focus/accessibility
acceptance is claimed. iced-preferences-verification-pending.json records exact
scope, logs, failures and unstarted cases. All3 current-run application logs are
empty. All482 inputs and the77f57 executable remain unchanged after diagnosis.
Host audit shows only original Niri1238 among compiler/app/compositor processes;
NO COMPILER OR NATIVE FIXTURE IS LIVE. No agents were spawned or remain live.

After resolving/reproducing the toolbar failure, freeze the actual final inputs,
rerun the appropriate original fast cases in both themes and the full14-run
regression matrix. Bind actual summaries/counts, clean logs, inspected captures,
red/green diagnosis and source hashes. Update ADR0063/IMPLEMENTATION/CONTEXT with
scoped proof only then. XDG locations and the full PRD remain required.

## Required continuation

Finish the compiler pipeline and diagnose any failures. Then native verification,
record scoped evidence and update ADR0063/IMPLEMENTATION/CONTEXT. Add meaningful
regressions for any actual bug rather than calibrating away application failures.
Keep the global editor requirement open until its scope is proven, and the full
PRD checklist unchecked. Continue XDG locations and the full Iced operations,
Undo/Trash/Recovery/devices/native clipboard/DnD/FileManager1/media ports, durable
job hydration/source ownership/exact step continuations and all workspace/release/
review gates. Never replay arbitrary custom commands. No subagents spawned.
Original Niri1238 and PipeWire1537/1562, unrelated Shell research and `.git` policy
remain preserved. No compiler/native overlap and no concurrent native fixtures.

## Superseding continuation, input ordering diagnosed

Read ICED-PREFERENCES-DIAGNOSIS.md for CURRENT state. The original final matrix is
still stopped; its failure now has causal timed proof and a shared queue fix.
The old filtered-choice hypothesis was false: Alt+2 precedes13 old-pane Tabs,
but the page message applied only after the batch. Three patched native trace
probes pass with page application first. Traces have been removed from active
source. Current uninstrumented compiler pipeline29193 is live; no native fixture
is live. It freezes483 inputs and will publish a new final candidate after checks.
Do not use the earlier NO COMPILER LIVE paragraph or77f57 as current executable
state; both describe that earlier checkpoint. Full PRD and all remaining gates
stay active. No subagents spawned; original Niri1238/PipeWire1537/1562 preserved.

## Current clean verification checkpoint

The superseding diagnosis record is authoritative. Compiler cell1111 is terminal0:
44+17 tests, Files Clippy/build, workspace all-target check,4 fork+3 queue tests,
formats and all6 published archives pass. Current clean executable is826a8c...
(102,804,488 bytes),483 frozen inputs. Traces are removed. Native cell1114 is live
with12 original fast probes (last observed light01–07 passed,08 starting). No
compiler is live. Finish that specific handle before native/compilers or source
edits. Then use a fresh boundary-final prefix for all14 regression workflows.
Entire PRD remains active and unchecked; full shared/release/review/performance
and XDG/operations/devices/media/native-exchange/durability work remain required.
