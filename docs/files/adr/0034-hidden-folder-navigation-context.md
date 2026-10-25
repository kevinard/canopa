# 0034: Keep explicitly browsed hidden folders in navigation context

Status: accepted under delegated implementation authority; verified for the scope below.

An explicitly opened hidden folder must retain its place in the hierarchy while
hidden files are off. Previously an ancestor column omitted its active hidden
child and highlighted an unrelated first row. Going to the parent also lost
selection because the normal hidden-file filter rejected the folder just left.
This contradicted width-independent navigation and the parent-selection rule.

Directory snapshots now project visible rows with an optional navigation child.
Ancestor columns include their actual next child alongside ordinary visible
entries, preserving their real selection. Browser parent navigation retains the
child just left through the same visibility rule before selecting it. Other
hidden siblings stay hidden, and this does not enable the Show hidden setting.
This works for explicit hidden directory links as well as ordinary folders.
Paths retain their Unix bytes; display labels never determine the exception.

The current folder's navigation context belongs to its in-memory folder state.
Refresh, history and width changes preserve it alongside selection and scroll.
An explicit Hide hidden clears that temporary context in current and remembered
folder states, then prunes hidden selection normally. Search results continue
to obey their ordinary hidden-file preference without this navigation exception.
No configuration format or on-disk preference changes are needed.

Public Browser tests reproduce the lost parent selection and verify the fix,
refresh/history preservation, width projection, hidden sibling exclusion and
explicit hiding. Native acceptance opens a real private hidden hierarchy and
checks Left/Right, type-to-select, Quick Look and selection continuity at full
width and 360 logical pixels. Rendered inspection checks the actual hidden
ancestor row and explicit-hide listing in both maintained themes. Exact results
are recorded in `hidden-navigation-verification.json`; the full product goal
remains active.

Final focused validation passes fourteen tests: seven Browser tests, six
preferences tests and filename-search regression. Build and all-target Clippy
with warnings denied pass. Native 55b dark and 56 light at 1.25 scale each pass
twelve checks, with clean GTK logs. Rendered inspection confirms the actual
hidden ancestor, selected hidden child at narrow width, and explicit-hide result.
Native 55 also passed, but a stationary pointer's toolbar tooltip obscured the
narrow row in its screenshot. The fixture now uses the existing Wayland pointer
helper's new move-only mode to park away from the toolbar. This needed no app
change. Formatting, Python syntax, maintained inventory and whitespace checks
pass. The previous full 176-test operations/core proof and 46 cross-filesystem
native checks precede this navigation change and retain their own exact binary
hash in `cross-current-verification.json`; they are not reruns of this binary.
