# Iced filename search — verified continuation

This goal turn is progress. The full product brief remains active. No blocker,
request to pause, or product-completion claim. The user's GUI correction is
recorded in root ADR 0018: Iced and canopa-ui are mandatory; the GTK tree is
inactive migration reference. No new subagents were spawned.

## Authoritative settled state

All compiler/native handles are terminal; there is no live verification session.
Proof: iced-search-verification.json. Source: iced-search-final-source.json,
134 inputs, unchanged after validation. Executable:
b71a4a178dc9cd51e827f86fe83b03fff3d18edcb856fc104633e5b84182a190
(80,176,280 bytes). No GTK/GDK widget dynamic dependencies (readelf recorded in
iced-search-static-verification.json).

Final pipeline 92312 / verification 04 TERMINAL 0: 14 adapter tests, all-target
Clippy lints, default build and fmt check. Focused public core tests passed in
verification 02:actions 5, browsing 7, preferences 7, search 3 (22 total). Relevant
core/build/test inputs remain unchanged; adapter 14 gives 36 focused tests total.
Four existing Rust dead-code warnings from five unported operations helpers
remain; do not describe this as warning-free full workspace validation.

Final native sessions, sequential on that executable:

- 33662 / iced-native-10-search-dark: TERMINAL 0, 16 checks, 11 captures.
- 8927 / iced-native-11-search-light: TERMINAL 0, 16 checks, 11 captures.
- 49893 / iced-native-12-search-browsing-dark: TERMINAL 0, 13 checks, 8 captures.
- 48001 / iced-native-13-search-browsing-light: TERMINAL 0, 13 checks, 8 captures.

All 58 checks pass; all 38 final captures inspected. Host ps after cleanup found
only original user Niri PID 1238, with no compiler/private desktop/input processes.
All files and configuration were private fixtures, not user folders or mounts.

Initial pipeline 38749 / verification 03 also passed. Initial native 43221 at
iced-native-09-search-dark passed 15 functional checks on f0e3ef87… but screenshot
review caught location text painting across Search. Clipping the location label
and a native painted-gap regression fix it (69 unexpected pixels before, 0 after).
The alleged missing checkbox label was disproven: its raw pixel region was
identical across captures. No checkbox fix is claimed. A fresh headless layout
probe did not reproduce that allegation and was removed. See
iced-search-toolbar-diagnosis.json. Earlier verification 01's invalid core target
and verification 02's two style lints are historical attempts, not final acceptance.

## Implementation

search_ui.rs owns SearchSession, guarded controls/results and Worker. Each Tab
has one reader thread and one replaceable pending query with bounded delivery.
Query/scope/navigation/close cancels old work; an inactive tab may continue.
Window/tab/navigation/search serial/session identity rejects old results and
controls, including close/reopen. No new dependency or vendor mutation.

Existing Browser SearchView preserves ordinary selection/scroll. List epochs
protect restoration from late Scroll/Viewport messages. Ctrl+F/Search opens the
current-folder query; Include subfolders is explicit. Empty query shows browsing
with controls open. Escape/Close/Back restores browsing. Ctrl+L closes search to
edit location. Return/Down leaves input for results through scoped focus tasks.
Activation and Show in folder retain recursive result raw paths. Results and
unreadable-location labels are relative to the search root; widgets are bounded
by the viewport. Stop keeps existing results. More adds 50,000 to the initial
50,000 budget and re-reads; cancellation retains previous matches/selection.

Five configurable commands add no default shortcuts: search-subfolders,
stop-search, more-search-results, reveal-search-result, search-problems. The private
native fixture assigns Alt+S/X/M/R/D through real shared configuration. ADR 0058,
Files context, README, runtime guide and implementation ledger are updated.

Native search tests actual Search clicks at 360px, input/list focus, exact GIO
argv, Escape selection/pixel-scroll restoration, filename-only negative matches,
recursive duplicates and location labels, result folder Right, hidden controls,
raw parent reveal, permission details, rapid supersession, independent tabs and
50,005 actual matching files. Ordered More+Stop preserves 50,000 prior matches;
More again reaches the last file. Browsing regressions cover two visible tiles,
resizing, hidden parent Left/Right, raw paths and a watched 5,000-sibling ancestor.
Hidden parents are unopened until widened in both themes; observed launches
remain usable at .455/.496 seconds (fixture samples, not universal guarantees).

## Next full-scope work

Quick Look is the next defining GUI integration: source-bound floating windows,
browsing keys and preview-control focus, image zoom, PDF pages, text/code and
media. Search+Quick Look is still open; do not check the complete PRD search box.
Keep prepared-preview/core behavior and original path bytes. Preserve the
historical GTK semantics as reference without reintroducing its GUI dependency.

Local primary-source inspection confirms Iced 0.14 Linux Settings has no parent
window field and window::run exposes only raw handles. winit 0.30.13 documents
with_parent_window as unsupported on Wayland. Investigate supported shared
platform integration; if a fork is needed, read docs/vendor-maintenance.md,
keep it solely in vendor with upstream provenance and reviewable patches.
Do not rely on a user-added Niri float rule to claim source transience.

Operations/Undo/Recovery and durable queue hydration/source leases/step crash
continuation, custom actions/editor, complete preferences and live reload,
devices, interoperable Wayland clipboard/DnD, FileManager1, accessibility,
packaging/runtime updates, full workspace/native matrix and independent review
all remain required. DURABLE-JOBS-WIP.md retains core registry/handoff details.
The entire goal remains active; earlier GTK native proofs do not prove Iced parity.

Use offline Cargo -j1, normal incremental builds, sequential native sessions and
no Rust edits during compilation or native runs. Last root free space was about
480 MiB and /tmp 1.1 GiB; check before another build. Only clean regenerable artifacts
when no owned compiler/native process is live. Preserve source/evidence and the
unrelated shell search-backend research. .git is read-only; no commit was made.
