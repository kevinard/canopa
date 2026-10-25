# Shared interface preferences and deliberate folder choices

Status: accepted under delegated implementation authority, 2026-10-16.

Files Preferences opens through the folder menu or Ctrl+comma. It edits one
transactional draft with Views, Toolbar, Locations and Shortcuts pages. Explicit
Save (Ctrl+S) validates and atomically replaces the Files namespace through the
same stale-source/target checks as the custom action editor. Each editor
preserves the other editor's settings. Reload (Ctrl+R) reads the selected source;
closing or reloading a changed draft asks before discarding it. Explicit
`--defaults` remains read-only. Saving or reloading publishes a complete Files
candidate to every browser window; external automatic reload is separate work.

The primary toolbar is an ordered sequence of built-in command IDs and explicit
`action:<id>` references, with exactly one location field. Buttons can be added,
removed and reordered around it. The overflow menu and window chrome remain
available. Button groups scroll horizontally when needed so configuration cannot
force a wide browser. The window title bar retains New tab, New window and
Operations as window controls. Custom actions marked for the toolbar but without
an explicit position keep their existing separate scrolling row. An explicitly
positioned action appears once. An explicit position enables the action’s
toolbar switch. Disabling that switch removes its position, and removing an
explicit position disables the switch so the same button does not reappear in the fallback row. Deleting an
action removes its position. Hand-written explicit positions imply enabled
toolbar actions. Both editors show the same effective choice.

Locations are an ordered list of built-in places and named local bookmarks. The
editor supports adding, editing, removing and reordering them. Built-in personal
folders resolve through the XDG user-directory definitions with conventional
home-folder fallbacks. Bookmarks retain actual Unix path bytes; KDL uses `path`
for UTF-8 paths and hexadecimal `path-bytes` otherwise. Labels are never path
identifiers. Missing folders remain bookmarked and report a normal navigation
error when opened. The sidebar scrolls. Automatic shows it at widths of at least
960 logical pixels; Show when there is room lowers this threshold to 560. Both
hide below their threshold, retaining the same Locations page in the overflow
menu. That page shares the existing popup and provides a Back control, retaining focus instead of handing it to
a nested popup.
Hidden suppresses the sidebar. Sidebar space is subtracted before projecting
readable columns.

Global view defaults specify mode, sort, direction, column width and visible
details. Folder state remembers only the fields deliberately changed from that
folder's effective settings. Other fields continue to inherit later global
changes. A deliberate change back to the current default still pins that field;
Use defaults clears every override for the folder. The shared preference store
publishes folder changes to other tabs showing that folder, avoiding stale tabs
pinning unrelated fields. Version 2 of private `views.json` stores these sparse
overrides. Version 1's full snapshots migrate as full overrides, preserving the
previous user's choices instead of guessing which fields were intentional.

Built-in command bindings are configurable with multiple accelerators; omission
uses defaults and an empty binding list disables a command. Navigation and custom
actions share one toolkit-independent canonical conflict catalog, including
Ctrl/Control/Primary, modifier order/case and keypad aliases. Invalid or colliding
bindings reject the entire candidate, whether hand-written or graphically saved.
Custom actions require a Ctrl, Alt, Super or function-key chord. Selection arrows,
Home/End, Escape, Tab focus, Menu/Shift+F10 and unmodified typing remain available.
Native text fields retain editing keys; Location and Search remain explicit
navigation commands from text fields. Quick Look routes commands to its source
browser as at every window width. This replaces ADR 0009's fixed reserved-key
list and F6–F12 restriction.

Public preference/configuration tests cover sparse inheritance and reset,
raw-path round trips, legacy state migration, action/settings preservation,
synchronized action-toolbar switches/positions, shortcut aliases/conflicts and
immutable refusal. Native preferences workflows pass in dark and light at 1.25×
(`native-preferences-11` and `native-preferences-14`), including actual 360-pixel
editor/browser geometry, bookmark navigation, remapped search, reset and restart.
Existing native action, drag, browsing, preview, transfer and Trash workflows
also pass against this adapter. See the implementation ledger for exact artifacts.

Native verification additionally caught a missing string expression in the
searchable command chooser, uncommitted spin-field text on Ctrl+S, transient
parent activation ordering, notebook minimum width, focus-out handling while
saving, and nested-popup focus loss. The chooser now explicitly filters by its
label expression; the Locations page stays in one popup. The fractional-scale
fixture waits for native button activation before issuing Save. Temporary
instrumentation and reduced repro code are removed. Automatic external config
reload and the remaining custom-action lifecycle/control acceptance stay separate
work.
