# Iced folder views and private preferences

Status: accepted under delegated implementation authority, 2026-10-18.
Folder-view implementation and scoped native verification pass. The full product
brief, including the global preferences editor, remains required.

Render the effective `PreferenceStore` view rather than treating every folder as
columns. Columns remain the global default. Ctrl+1, Ctrl+2 and Ctrl+3 choose
Columns, List and Icons, and the same commands can appear on the configured
toolbar. View options in the folder menu provides mode, sort, direction, column
width, list details and Use defaults. These are deliberate immediate choices for
the captured folder. They are independent of the pending global Views, Toolbar,
Locations and Shortcuts editor in ADR 0011.

List view uses a filename line and the configured detail line, with sizes in
binary units, local modification dates and file kinds. Folder byte size is not
presented as the size of its contents. Search remains a temporary list with
containing locations. Choosing a folder mode leaves search before applying the
folder choice; sorting/details can still update their underlying folder defaults.

Icon view uses a grid whose column count comes from actual layout bounds. Keep
cells readable, at least 128 logical pixels wide (with typography/padding also
included), and at most 16 per row. Build only nearby rows while retaining full
scroll extent. Each entry retains raw path identity and keyed interaction state.
One geometry model drives layout, Up/Down row movement, PageUp/PageDown, viewport
reconciliation and revealing focus. Home/End choose the first/last entry. Ctrl
moves focus without replacing selection; Shift extends the existing linear
selection range through row-major ordering. Left remains Parent and Right remains
Enter folder, as in columns and list views. They do not become horizontal grid
selection keys.

When the grid reflows, map the first visible row's file into the new row geometry
and preserve its fractional offset. If focus was visible, make the minimum
additional adjustment to keep it visible. Deliberately changing view/sort reveals
the focused file. Clamp stored offsets to the new scroll extent so model and
widget cannot disagree after moving from a long list to a short grid. Guard
scroll/viewport delivery with the list epoch, including the actual viewport size.
List/icon views never read hidden ancestor columns merely to draw their context.

An empty Search bar still displays the underlying folder. Its automatic height
reduction must not reveal the old folder focus and thereby change the scroll
that closing Search should restore. Suppress only that automatic reveal while
the bar is empty; keep deliberate reveal requests and grid reflow mapping.
A middle-folder native probe reproduced an 82 px shift despite the earlier 180
native checks passing at the bottom of folders, where clamping hid it. The
controller regression independently reproduces a 100 px change and covers all
three modes. Exact red/green and final scope belong in the verification record.

File/folder/link symbols use the existing admitted asset worker, with separate
typed handles and the shared MenuRow foreground. Look up themed symbols at 48
nominal pixels and prepare within 144×144 for 48 logical pixels. Small application-
owned SVG fallbacks keep kinds recognizable without an installed theme. These
are symbols, not thumbnail reads of the selected files. Admit these three symbols
before optional custom-action icons into the existing shared byte/pixel budgets.
The raster thumbnail ceiling becomes 144 pixels; custom-action SVGs retain their 72
pixel preparation. This extends ADR 0061 without adding another worker, toolkit or
unbounded icon scan.

Apply each property change to the current shared preference store, preserving
sparse inheritance in ADR 0011. Publish the new effective folder view to every
matching tab/window, preserving actual selection and focus. Capture window, tab,
navigation revision and raw folder path for controls; reject events after the
source navigates or switches tabs. Selection changes do not change the folder a
control edits. Use defaults removes all overrides, including deliberately pinned
fields equal to the current defaults.

Private-state writes run off the UI thread, with one admitted writer and one
replaceable pending complete snapshot. The shared in-memory store remains the
authoritative source for subsequent property changes. Do not queue an unbounded
sequence of snapshots or let an older acknowledgement clear a newer writer.
Closing the last Browser keeps the daemon alive through the final write result.
Successful completion permits natural exit. A failed write retains the effective
in-memory choice and displays Retry; if all Browsers already closed, open the
last edited folder to show the failure. Closing that visible error Browser may
exit normally. Retry writes the current complete choices, not the old failing
snapshot. Existing bounded atomic state-file replacement remains the write seam.

View options is one in-window popup with shared Canopa controls and synchronous
keyboard traversal. Close is the initial focus; Escape closes it. Unfocus only
the originating Browser subtree when opening, so a delayed widget operation
cannot erase the popup's default focus or another window's input state. Both
configuration and state-write notices keep stable sibling slots in the Browser,
preserving a search/location field when a notice appears or clears.

Sidebar thresholds follow ADR 0011: Auto at 960 logical pixels, Show when there is
room at 560, Hidden never. Neither setting forces a sidebar into a narrow tile.
The global preferences editor, full view/focus/accessibility and performance
acceptance, and all remaining file-manager requirements remain open.

Verification on 479 unchanged inputs passes 37 adapter tests, 17 public
action/configuration/preferences tests, all-target Clippy lints, the desktop build,
workspace formatting and tracked whitespace. Four existing operation-helper
warning groups remain; strict/full workspace and release acceptance are open.
The default dependency graph uses Iced and `canopa-ui` with no GTK widgets.

Twelve sequential native workflows pass 182 declared checks in dark at 100% and
light at 125%, including folder views and editing/actions/browsing/search/preview
regressions. All 14 application logs are empty. Sixteen selected final captures
were inspected; this is not the exhaustive visual/focus/accessibility matrix.
The final executable is
`c3dda02002a383de2263e9fb00d9341e827797bd509e048a006c43af65eec131`.
The added middle-folder empty-Search check measures zero displacement after the
fix, compared with the prior 82 px native regression. See the
[source-bound scoped evidence](../../../.scratch/file-manager/iced-views-verification.json)
and [checkpoint](../../../.scratch/file-manager/ICED-VIEWS-WIP.md). No full-product
completion checkbox is satisfied by this slice alone.

## Follow-up: scroll feedback during native reflow

The later global Preferences regression matrix exposed an actual light/125%
selection-visibility loss after widening an icon grid. Narrow/wide stress and
original-prefix repeats can pass, so a green retry does not erase that failure.
A targeted native trace demonstrates that Viewport remaps the logical anchor,
then an old native offset (clamped to the new extent) can overwrite it. Successful
runs recover when the restore operation reports its corrected offset.

Current-folder scroll feedback now carries both the model viewport captured by
its producing view and the measured native bounds. Accept it only when both equal
the current tab's model viewport. Reject a transitional native size even if its
Scroll arrives before Viewport; reject an old producer after Viewport remaps the
model. Keep existing tab/list-epoch/raw-path guards. Ancestor-column scrolling
retains its independent column state. Settled ordinary scrolling is admitted,
even when it moves focus offscreen; this is not an unconditional focus reveal.

Two controller regressions fail before this guard for opposite message orderings;
a third protects deliberate settled scrolling. All 47 adapter tests and 17 public
tests pass afterward. Diagnostic code is removed and archived separately. The
final clean matrix passes 14 workflows/198 declared checks plus a twenty-cycle
resize stress case, with 17 empty application logs and eight selected reviewed
captures. Exact scope and immutable source hashes are in
[the Preferences follow-up evidence](../../../.scratch/file-manager/iced-preferences-verification.json).

The original terminal native failure was not captured by the trace: original-
prefix and stress retries could pass before the guard. The stale feedback hazard
itself is directly reproduced by the controller regression, while the final
original/stress native loops pass without diagnostics. Do not attribute causal
closure solely to the green repeats. Selected captures also show rightmost icon
cards clipped at the scrollbar edge; reconcile that gutter before final visual
acceptance. Full large-folder, focus, accessibility, performance and product gates
remain required.

## Follow-up: reserve the icon scrollbar gutter

A native border regression fails on the preceding Preferences candidate: Iced's
default floating 10px rail paints over the rightmost card. Embed the rail with
4px spacing, and use the same conservative 14px allowance in grid geometry.
Column count and keyboard stride therefore agree even when a height change makes
the scrollbar appear. Ordinary list/column presentation retains its own geometry.

The unchanged border probe passes afterward in light/125% and dark/100%, with
actual selected-file launch checks through wide/narrow reflow. Both complete view
workflows and a twenty-cycle reflow stress case also pass: five native workflows,
53 declared checks, seven empty application logs and three selected reviewed
captures. The candidate passes 47 controller tests, build, Clippy and formatting.
See [the scoped source-bound evidence](../../../.scratch/file-manager/iced-gutter-verification.json).
This closes the recorded icon-card border defect; exhaustive visual/accessibility
and remaining full-product gates stay open.
