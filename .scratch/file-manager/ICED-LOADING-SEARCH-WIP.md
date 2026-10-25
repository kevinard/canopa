# Iced loading complete; filename search UI next

The previous goal turn was progress: default GUI migrated to Iced/canopa-ui with
native browsing acceptance. This turn made concrete further progress: cancellable,
visible-only ancestor loading and bounded row construction, plus a native panic
repair. The full product brief remains active; no blocker or completion is claimed.

## Authoritative current state

- [Loading verification](iced-loading-verification.json) is the settled proof.
- Source manifest `iced-loading-ready-source.json`, 131 inputs, matches actual files.
- Executable `dffb5111751d6c6dcd90295ae2e5dcce9bf6e4e7648d1a6b514d1749d3a460b9`,
  79,492,992 bytes. No GTK/GDK widget libraries are linked.
- Final compiler pipeline93947 TERMINAL0: formatter, all seven adapter tests,
  library/binary Clippy lints, build and fmt-check. Nine public browsing/search
  tests and all-target Clippy lints passed earlier on the unchanged public core.
- Dark native91827 TERMINAL0:iced-native-07-loading-dark, scale1.0.
- Light native36659 TERMINAL0:iced-native-08-loading-light, scale1.25.
- Both runs observed zero opens of a hidden ancestor containing5,000 real private
  files, then an actual directory open on widening. The current file opened in
  0.579s /0.49s after widening in these runs. These are fixture observations.
- Existing keyboard, visible scroll/selection, raw-name GIO argv, independent tabs
  and two-visible-tile pixel assertions pass. Final screenshots were inspected.
- Host process check found only original user Niri1238; all owned compiler,
  Files, Niri, Weston and input processes retired. No live handle remains.

Directory::read_cancellable stops abandoned reads between entries without partial
snapshot publication; SearchRequest uses it too. The current-folder scan reads no
ancestors. Visible ancestors load independently after actual compositor sizing,
with window/tab/navigation/path/request guards. Hiding columns or tabs cancels and
releases them. Closing/navigating cancels reads; failed navigation restores context
for the still-visible location. The view builds only nearby row widgets with
spacers retaining full scroll extent. Current and ancestor scroll state differ.
See [ADR0057](../../docs/files/adr/0057-iced-directory-loading.md).

Native05/06 found a real panic before acceptance. The installed Iced keyed column
could directly diff a stateless spacer into a file MouseArea without changing its
stored widget tag. A real browser widget-tree replay made this deterministic:
red10354 TERMINAL101, green11112 TERMINAL0. A small transparent row wrapper now
keys state by tab plus original raw path, resets on identity changes, and uses
Tree::diff tag checks with ordinary Column. No vendor mutation was needed.
[Diagnosis](iced-row-state-diagnosis.json) records hypotheses, red/green evidence
and final native verification. Harness RUST_BACKTRACE is enabled for Iced tests;
application code contains no temporary diagnostic logs.

Four Rust dead-code warnings remain from five unported operations adapter helpers.
Do not call this warning-free workspace acceptance. Full product, durability and
native matrix gates remain. Root disk was about560MiB free, /tmp about1.1GiB;
check before more large builds. Six regenerable integration-test executables
(111,806,808 bytes) were removed while no compiler/native session was live, with
source/evidence/app/dependencies preserved; see iced-search-cache-cleanup-*.json.

## Filename search UI is the next port

Use the existing public SearchRequest/SearchResults and Browser.show_search /
close_search interfaces. Search belongs to its Tab and keeps regular folder
selection/scroll separate; Escape must restore that exact state. Keep filename
substring matching and Current folder / Include subfolders scopes, raw-path
operation targets, explicit cancellation/partial results, read failures and a
More results action after the initial 50,000-result budget. Typing to select is
still distinct from Ctrl+F / the visible search control.

Suggested private adapter structure: SearchSession per Tab with query, scope,
limit, running/editing/focus-after-delivery flags and cancellation. Keep its
monotonic request number outside the optional session so close/reopen cannot
reuse an old generation. Search result messages carry window/tab/navigation and
search request identity, an Arc<SearchResults>, and a final/progress flag. A
bounded Iced stream channel can publish core progress snapshots every 100ms and
await final delivery on the worker executor; drop/restart/close cancels old work.
Do not block the UI or queue sleeping debounce futures on the thread pool.

Render search as one result list with file name and containing path. Reuse bounded
row construction with a two-line height; keyboard reveal must use the same row
geometry. Pointer selection must use Browser.search results rather than assume
all selected paths are direct children of Browser.path. Directory activation also
must inspect visible search entries, not only Browser.directory entries. Offer
Show in containing folder for the focused result using original path bytes.

Add a monotonic list/view generation to scroll/viewport messages so a
late search-list scroll cannot overwrite the restored folder's scroll at the same
root path. Keep query/scope changes separate from ordinary folder state. Preserve
results already found on explicit Stop, and show cancelled/truncated/read-failure
status. Restarting/closing must reject old progress and final messages.

Search input focus: Ctrl+F opens/selects the query, Return/Down moves focus to
results and selects the first result when available; Escape closes search even
when Iced TextInput consumed it. Use the core focusable::unfocus operation via
iced::advanced::widget::operate, scoped to the source browser root, rather than
focusing a nonexistent widget. (Normal runtime widget helpers lack unfocus.)
Ctrl+L can deliberately close search and start location editing; record that
routine choice when implemented. Existing global shortcuts must not intercept
ordinary text editing keys. Iced stream/channel/checkbox APIs were inspected
locally; no new dependencies or vendor changes are needed for this port.

Normal Iced Window settings still lack a transient parent and standard Wayland
file-drop events. Full floating/source-bound Quick Look, operations/durability,
clipboard/DnD, devices, editors, preferences/views, accessibility, live reload and
packaging/full native matrix remain required. Keep the full active goal intact.
