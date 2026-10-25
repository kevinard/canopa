# Iced folder views checkpoint

Current goal turn: progress. The entire PRD stays active and incomplete.
ADR0062 records the verified folder-view decisions; global Views/Toolbar/Locations/Shortcuts
editing and the other full-product gates remain required.

## Current code

`listing_ui.rs` supplies details, linear entry cards, virtualized responsive icon
rows and shared movement/reveal/reflow geometry. `Tab.column_paths` uses only the
current folder in List/Icons/search. Ctrl+1/2/3 and configured toolbar commands
apply sparse folder choices. A folder-menu View options popup provides modes,
sorting, direction, column width, details and Use defaults. It uses the existing
synchronous focus group with Close initially focused and a scoped unfocus of
the Browser only. `folder_views.rs` publishes property changes to all matching
tabs/windows and rejects stale navigation/tab targets.

Private-state writes use one admitted std worker and one coalesced pending full
snapshot; closing every Browser waits for the final acknowledgement. On failure,
keep memory choices, show Retry and reopen the last edited folder if necessary.
The boot API accepts an explicit private-state path so controller tests cannot
write to developer state. Config/state notices keep stable Browser sibling slots.

The asset worker now also prepares typed folder/file/link symbols at144 pixels,
with built-in SVG fallbacks and a separate MenuRow foreground. They share the
existing budgets and single worker, prepared before optional action icons.
No filesystem metadata/icon lookup/decode occurs while drawing entry cards.
Sidebar Always now follows ADR0011's560px threshold instead of forcing a narrow
sidebar. The default Auto threshold remains960px.

## Verified final candidate

37 adapter tests pass in iced-views-tests-03.log. The17 focused public tests pass
in iced-views-public-tests-02.log. All-target Clippy lints, build02, workspace
fmt-final-02 and tracked whitespace pass. Four existing unused-operation-helper
warning groups remain. Python AST checks pass for the seven Iced fixture/driver
files. The default dependency graph uses Iced and canopa-ui with no GTK widgets;
see iced-views-dependency-tree.log.
No Rust source changed after these checks.

Final executable:102,033,336 bytes,
`c3dda02002a383de2263e9fb00d9341e827797bd509e048a006c43af65eec131`.
`iced-views-final-candidate.json` and `iced-views-current-source.json` record479
inputs. Final cell942 completed all12 sequential native cases:
iced-views-final-native, interface, actions, browsing, search, previews01/02,
dark100/light125. They pass182 declared checks (23/9/15/13/16/15 per scheme).
All14 application logs, including reopened-view processes, are empty. Sixteen
selected final captures were inspected; this is not exhaustive visual/focus/
accessibility acceptance. All479 hashes and the executable were revalidated after
the matrix. iced-views-verification.json binds the exact evidence and limitations.
No compiler or native fixture is live. Host audit found only original Niri1238
among the queried compiler/app/compositor processes. No agents are live or were
spawned. Do not edit Rust during a compiler, overlap compiler/native work, or run
native fixtures concurrently.

## Actual scroll regression and fix

The preceding e85 executable passes36 adapters,17 public tests and180 native
checks. Those are valid scoped baseline results in
iced-views-pre-standby-verification.json and archived source-pre-standby/candidate.
They did not establish the added empty-Search middle-folder restoration case.

Native iced-views-standby-before-01 reproduces a real82px shift of the selected
List card after Ctrl+F then Escape without a query. Row-background measurements
exclude isolated antialiased glyphs: selected top716 becomes634. See
iced-views-standby-red-pixels.json and both actual before/after captures.
The new controller test fails with1389->1489 before the first result in Columns;
its loop covers Columns/List/Icons. Red log/source are retained separately.

Cause: the newly shared row geometry allowed viewport shrinkage from the empty
Search bar to automatically reveal the real folder focus, mutating normal folder
scroll. End-of-folder native tests hid the movement by clamping. The fix suppresses
only automatic reveal when Tab.search exists but Browser.search has no results
view. Deliberate reveal_on_show and width reflow mapping remain. All37 adapter
tests now pass. Both final native view runs pass the added middle-scroll check.
iced-views-standby-green-pixels.json measures the selected dark card at716..769
before and after empty Search: shift0, compared with the baseline -82px.

## Harness diagnosis retained

Test01 has35 passes and an inactive-tab fixture setup failure. Test02 switches
through real tab flow and loads each actual revision;36 tests pass before the
additional search regression. Native01 correctly exited0, but Session rejected
that expected exit. The harness now follows job-lifetime ownership, monitoring
other services and restoring app cleanup only if the settling app remains live.

Native01's initial List capture was an old Columns frame. Native02 did render
correct List metadata; its probe started beyond the text. The calibrated metadata
probe records negatives0/0 and positive174. Native03 reached restart and correctly
showed the failed-write banner; an overly exact foreground probe missed Retry.
The calibrated antialiasing probe records negative0 and positive46. Both JSON
probe receipts and matching source/candidate01..04 are retained. These were not
application publication failures. Native04/05 subsequently pass18 checks, and
06/07 pass22 checks including actual floating Quick Look from List/Icons on e85.
The final harness adds empty-Search restoration (23 checks per new-view run).

## Required continuation

The folder-view slice and its regression matrix are complete at the scope above.
The e85 evidence remains baseline only. ADR0062, CONTEXT and IMPLEMENTATION now
link the final c3 proof. Keep the full PRD checklist unchecked until proven.

Next, implement the global graphical Views/Toolbar/Locations/Shortcuts editor,
using the shared guarded configuration writer and controls. The folder popup is
not a substitute for that editor. Continue the full Iced operations,
Undo/Trash/Recovery/devices/file-exchange/media ports. Durable job hydration,
source leases and exact crash continuations stay open; never replay external
commands. Do not suppress unused helpers or retain GTK as the final GUI.
No agents were spawned. Original Niri1238/PipeWire1537/1562, unrelated Shell
research, `.git` read-only policy and offline,-j1/incremental safeguards remain.
