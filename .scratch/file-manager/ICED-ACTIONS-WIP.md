# Iced custom actions — active continuation

Latest state (supersedes every historical LIVE note below): ALL compiler/native
handles are TERMINAL. Current executable eeee87ee2e31a898fb5a8999ab57f6e4615e96a990cebafda26801a6f44f9828,93660608bytes.472 frozen inputs are unchanged; see
iced-actions-current-source.json, iced-actions-current-candidate.json and
iced-actions-verification.json.
Build07,29 adapters, public actions5/configuration_reload5/preferences7, all-target
Clippy04 (-Dclippy::all) and workspace fmt check pass. Four existing unused
operation-helper warning groups remain; strict/full release validation is open.
Actions16/17 + browsing05/06 + search05/06 + previews05/06 pass118 native checks in
dark100/light125 on this SAME executable. Application logs empty. Selected current
captures inspected (12 listed in proof); exhaustive visual/a11y review is open.

The FINAL focus implementation is synchronous focus.rs widget groups/wrappers,
not app-message queries, counters or focus acknowledgements. Those approaches
were superseded after actual batched Tab/Ctrl+A lost replacement selection.
Buttons/checkboxes expose Iced Focusable state with Canopa border treatment;
traversal inspects, focuses and scrolls inside the keyboard event. Text input owns
its native state. Modal Keep editing defaults are applied before events; both old
async prompt-unfocus Tasks were removed. Native driver batches repeated keys in
one wtype call and asserts exact saved Name and copied output pages.

Full PRD/goal remains ACTIVE. Next: complete icons, action/source/stale-save/focus
matrix, then the rest of Iced interfaces and durable runtime. Explicitly verify
location-edit focus/caret across wide/narrow toolbar reparenting, dynamic banners
and external config changes, and actual live font-family reload. These gates are
not proven by file-list resize or draft-preservation tests. Commands only have a
canonical working-folder recheck; full volume/source-lease authority is open.

Resource: root515MiB free,/tmp1094MiB. Before the next build, inspect host ps and
old generated caches. Do not clean while a compiler/native fixture is live.
Preserve user Niri1238 and actual PipeWire1537/1562, source/evidence/binaries/
libraries/symlinks. Cargo offline,-j1. No subagents; .git read-only; unrelated Shell
research preserved. Full source forks/shared native/release validation remain.

Previous goal turn: progress. It completed scoped Iced preview verification,
88 native checks/60 inspected captures, 19 adapter/10 preview tests, shared fork
and workspace all-target checks and the full serial core entry point. See
ICED-PREVIEW-WIP.md and iced-preview-verification.json. Entire PRD remains active.

This turn ports custom actions/editor/invocation/Operations to Iced. Read root
and Files AGENTS/context/PRD, custom action ADR0003/0009 and durability ADR0056.
ADR0060 records the implementation decisions, not feature completion.
No subagents have been spawned. .git is read-only; preserve unrelated research.

Initial host ps: only user Niri1238 among cargo/rustc/rustfmt/Files/Niri.
Initial root1744MiB free, /tmp1094MiB. Never overlap compiler/native runs or clean
caches during either. Cargo offline,-j1; native fixtures private and sequential.
Core tests left Shell/Notify binaries potentially headless: rebuild default GUI
binaries before shared native acceptance. Files baseline:f1bbba93361f2dd95a080cc7000ffccd439e022e61e6620898b0b5a2a3f4a087.

configuration_ui.rs adds one serialized worker for poll/save/reload with monotonic
publications, latest-only mailbox, bounded notification channel and independent
one-shot command acknowledgements. Desktop installs complete snapshots, retains
last valid settings on rejection, updates inherited view settings, and preserves
selection and preview state. An early compile found Iced's selected executor has
no time::every; replaced UI polling with a bounded worker-notification subscription
without enabling another executor feature. Check01 TERMINAL101. Check02 handle21695
TERMINAL0. Check03/04 caught private shortcut helpers and Iced's ContextMenu key
name; both corrected. Check05 handle5922 TERMINAL0, four previously recorded
operation-helper warning groups remain. Adapter tests01 handle71093 TERMINAL101:
20 passed and one invalid notification namespace fixture failed before worker
startup. Corrected `notify` to the shared `notifications` envelope. Tests02 failed
one closure index type inference, corrected with usize. Tests03 compiled but the
iced_desktop filter matched zero tests (the public module is desktop); no passing
behavior claim from that run. Tests04 TERMINAL0: all24 adapter tests passed,
including source guards/context selection and ten independently acknowledged saves
with an undrained bounded configuration notification channel.
Sandbox ps cannot see
host processes; obtain host ps before cache cleanup or desktop acceptance.

The Iced editor, source-guarded context menu and shortcuts, four-worker command
admission, selectable bounded output, process-group cancellation, and independent
notification bridge now compile. They have not received native acceptance.
Configuration publication and editor draft tests exist; broader source-lifetime
acceptance remains pending. Jobs::Drop cancels unfinished tokens.
The toolbar now honors configured order and explicit custom-action positions;
narrow tiles have location above a horizontal command strip. Text is clipped to
bounded widths with tooltips, implicit toolbar actions keep catalog order. Both
built-in and custom controls guard captured selection/catalog. Editor virtual
buttons and native fields share keyboard traversal, with scoped focus inspection
and scroll reveal. Operations has the same traversal and captured Open folder.
Viewport shrinkage retains a previously visible focused file. These last
Operations/viewport changes postdate the24-test run. Build01 handle62811 TERMINAL0.
Actual source leases, durable operation hydration and filesystem
continuation are still open and must not be inferred from the in-memory queue.

New scripts/desktop-fixture/files-iced-actions.py and --iced-actions-only fixture
cover graphical save, keyboard form/buttons/recording/discard, raw argv/cwd,
bounded readonly copied output, toolbar/menu, live reload, background lifetime and
process-group cancellation. Python syntax passes; native has not run yet.
Native01/02 failed copying output because the fixture assumed keys had painted
immediately. Diagnostic03 TERMINAL0 passes14 native checks, including raw argv,
actual notification, output copy/read-only, toolbar/menu, safe discard, source
closure and owned parent+child cancellation. All captures still need the final
inspection pass. Output paints after3.72s for long unbroken64KiB streams; this
explains why the earlier fixture input arrived before the visible state.

To improve responsiveness, the next candidate pages output in8KiB Unicode-safe
chunks, retaining all bounded output and adding Copy all output. Public tests
cover Unicode page concatenation/bounds and refusal of a retargeted working folder.
Tests05 handle91214 TERMINAL143, no compiler diagnostic or test execution; the
reason for external termination is unproven. Tests06 handle22538 TERMINAL0: all26
adapter tests pass. New fixture also verifies
recording changes the old accelerator and rejects file-only action on a folder.
Native03 is evidence for the preceding candidate, not these new changes.
Icons/condition matrix/stale-save native flows and full focus matrix remain open.

Screenshot inspection of native03 found context-menu opening changed the Browser
root widget kind, resetting its painted scroll, and a toolbar tooltip painted over
the menu. The overlay now always retains Browser slot0 in one Stack, suppressing
toolbar tooltips while the owner's menu is open. New native pixel assertion guards
the selected bottom row across menu opening. Search's pointer fixture now targets
the actual narrow command strip and checks the location/menu gap. No final native
run has verified these changes yet. Native03 captures05b–10 have been inspected.

At632MiB free, host ps showed only user Niri1238 and no compiler/native. Removed
only12 obsolete root-local Files incremental caches last touched before01:00UTC,
569106545 bytes; retained current caches, every executable/library/source/evidence
and all symlinks. Receipt iced-actions-cache-cleanup.json. Build02 handle43669 is
LIVE; poll exact handle before any compiler/native/Rust mutation.

Build02 handle43669 TERMINAL0. Candidate472 inputs / executable8079f128d8c73b62
c50336fbfe4b138c6f128ff51c7a8053bca9f56d92bd9673 (93809576bytes) passes native04
dark100 and native06 light125,15 checks each. Native05 received external SIGTERM
before output activation; no product diagnostic, cause unproven, no product change
before successful06. Output paint measured1.04s/0.62s for8KiB pages versus the
earlier3.72s complete-output layout; these are fixture/debug observations, not a
general performance benchmark. Application logs empty. Context-menu scroll pixel
guard passes, accelerator actually changes u→i, file-only action refuses a folder.
Both output pages and full copied output are checked; source close and parent+
child process cancellation pass. Light06's final capture shows Cancelled; dark04
captures its earlier Cancelling frame. Native UI capture uses asynchronous frames.
Selected final captures inspected:04 {01,02,03,03b,05b,07,08,10};06 {02,05b,07,10}.
Broader capture inspection/regression remains pending; do not claim all captures.

Public actions tests5 and configuration_reload tests5 pass. Clippy01 rejected two
style issues (collapsible condition/redundant closure), corrected with equivalent
control flow. Clippy02 handle97795 TERMINAL0 all-target -Dclippy::all, four existing
operation-helper dead-code warning groups remain. Native04/06 apply to the earlier
8079 executable, not a rebuilt post-lint executable. The action fixture now also
captures Operations again after reopening a Browser so settled terminal UI state
can be inspected. Rebuild/refreeze/reverify after these3 source-file changes.
Preferences tests handle56603 is LIVE; poll before Rust edits/new compiler/native.

Next: finish the editor and shared job model, guarded context/toolbar/shortcut
invocation, shared asset icons and focused native coverage. Reuse existing action
executor and stale-safe configuration writer. Preserve durable queue/source lease,
media, file-operation, accessibility, packaging/release and independent-review
requirements. No completion or pause claim.

Subsequent progress: candidate bee5 passed actions07/09 + browsing01/02,
search01/02 + previews01/02 =118 checks, both themes/scales. Only selected captures
were inspected. Review found a lingering Search toolbar tooltip painting over
input. The initial pixel probe missed its actual glyph band; measured prior
capture has30 overlap pixels vs3 allowed. Candidate0c953 suppression passes
search03/04, zero overlap pixels. See iced-actions-tooltip-diagnosis.json.

The next actions10 failed page2 copy with Copy all still focused. Deterministic
controller tests reproduced dropped Tab steps while a focus query was pending:
both rapid_tabs tests fail on old logic. Replaced latest-request cancellation of
Tab intents with a bounded directional counter and serialized query/mutation
acknowledgements; keys arriving during a mutation wait for its acknowledgement.
Both red→green regressions plus a mixed-direction/wrap regression pass among29
adapter tests. Further native key-to-text/caret/activation timing belongs to the
full focus matrix; do not infer that matrix from these controller tests.

Build04 and05, Clippy03 pass. No semantic core command/config/preference change
since their5+5+7 public tests passed. The native action fixture now batches repeated
keys (no sleeps between them), retains concise output page receipts, and captures
the settled cancelled job after reopening a Browser. Old actions10 failure and
initial incorrect pixel probe are retained as diagnosis, not passing evidence.

Native05,08 and combined tests05 received unexplained external SIGTERM; causes
unproven. Later command drivers kept write_stdin attached until terminal and
completed. Do not claim this proves the signal's cause or silently rerun failures.

Final continuation: intent counter superseded. Native12 showed InspectRecord
selection (Ctrl+A had arrived before native focus), despite29 controller tests.
Synchronous widget focus replaces the app-message model; obsolete callback tests
were replaced by actual Focusable-operation contract tests. Native14 passed,15
found old prompt-unfocus Task erasing Keep focus. Removed both tasks and establish
default before first widget event. Tests synchronous-focus-02:29 pass; build07 and
Clippy04 pass. Native16/17 batch key events, save exact Name and copy correct pages,
confirm Keep editing, run/cancel commands with descendants and reopen Browser.
All6 final browsing/search/preview regressions pass, yielding118 checks on eeee.
No completion or pause claim; inspect remaining gates in iced-actions-verification.json.
