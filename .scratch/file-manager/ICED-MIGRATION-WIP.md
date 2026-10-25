Latest completed continuation: [Iced filename search](ICED-SEARCH-WIP.md).
Its 134-input proof and b71a4a17… executable supersede the initial browsing and
loading references below. Search and ordinary browsing pass in both themes;
Quick Look and the complete feature port/durability gates remain. See
[search proof](iced-search-verification.json). Full product goal stays active.

# Iced migration — active, full product scope

The user's toolkit correction is authoritative: use the workspace's Iced stack
and `canopa-ui`. Root [ADR 0018](../../docs/adr/0018-files-iced-and-shared-controls.md)
supersedes GTK. The entire product brief, native acceptance and durable-operation
gates remain active. This is a completed browsing migration slice, not completion
of the file manager. There is no blocker and no request to pause.

## Current implementation

The default `desktop` feature depends on Iced and `canopa-ui`, with no GTK
Cargo dependency. `src/lib.rs` points its `desktop` module to
`src/iced_desktop/mod.rs`; the old `src/desktop/` tree is inactive migration
reference. The actual executable was rebuilt and has no GTK/GDK widget libraries.

The Iced daemon provides ordinary `org.canopa.Files` windows with independent
Tabs, shared theme/font/action/input/row treatments, per-folder view defaults,
responsive ancestor columns, hidden-file navigation, keyboard and pointer
selection, focus-only Ctrl arrows, range selection, type-to-select, editable
locations, Back/Forward/Parent, refresh and GIO default-application launching.
Filesystem reads run on workers, and results carry window/tab/revision identity.
Late scans cannot replace another tab or a superseding request; launch errors
stay with their original tab. Relative locations resolve from the current folder;
local file URIs and non-UTF-8 Unix paths retain their original bytes.

Rows use shared typography and MenuRow padding. Column count uses the actual
content width after the configured sidebar. Keyboard movement only scrolls when
the focused item leaves the viewport. Resizing restores the Browser's scroll into
the rebuilt list, and an explicit file location or returning child is revealed.
Escape closes the location editor even though Iced TextInput consumes that key.

## Verification — all handles terminal

See [browsing verification](iced-migration-browsing-verification.json).

- Final source manifest: `iced-migration-native-source.json`, 129 inputs.
- Final executable: `fe2cd6c0d735d7602fec291986e1ea61845e67885f6aeb11ab36d91a26d080e7`,
  79,286,496 bytes. It replaced the historical GTK reference.
- Final compiler pipeline 75619 TERMINAL 0: formatter, five adapter tests,
  library/binary Clippy lints, desktop build and fmt-check.
- Seven core browsing regressions passed in verification 01; that core source
  has not changed. All-target Clippy lints passed in verification 02 before the
  final renderer scroll repair; final library/binary Clippy also passed.
- Dark native 15218 TERMINAL 0: `iced-native-03-dark`, scale 1.0.
- Light native 82012 TERMINAL 0: `iced-native-04-light`, scale 1.25.
- Both final native runs assert visible selection/scroll at 360 logical pixels,
  hidden-parent Left/Right, widening, tabs, two ordinary tiled windows, location
  cancellation and actual GIO argv for a raw filename. Pixel assertions ensure
  the selected row and both tiles are painted. Final screenshots were inspected.
- Final host process check found only the user's original Niri PID 1238; all
  private Files, Weston, Niri and input processes retired.

Earlier native runs 01/02 passed their functional checks but screenshot review
caught a scroll reset, an offscreen explicitly selected file and a viewport that
showed only one tile. They are not final acceptance. The repaired native runs
include pixel assertions that reject those earlier images (deltas 35/43).
Verification 01's original adapter test filter selected zero tests; verification
02/03 ran all five tests with an enforced count. Preserve this distinction.

Five operation adapter helpers currently produce four Rust dead-code warnings
because the Iced operations adapter is not ported yet. Do not suppress or present
this as warning-free workspace acceptance. Root free disk was about 650 MiB;
check resources before another large build and clean only regenerable outputs
with no live compiler/native session if needed. No source/evidence was deleted.

## Remaining migration work

Quick Look and its floating/source/focus/media controls; operations UI and durable
scheduling/source ownership; Clipboard and interoperable Wayland drag-and-drop;
devices; Search/Quick Look integration; custom actions and editor; preferences editor, complete
configurable toolbar, list/icon modes and persistence; FileManager1/single-instance
activation; live configuration; context menus, accessibility and packaging.
The complete native matrix must pass again for Iced. GTK evidence is historical.

Visible-only ancestor loading and large-directory responsiveness have now been
verified in the linked continuation. The original eager seven-ancestor scanner
was replaced; next port the filename search UI. Continue porting the complete
product brief, including meaningful shared-control focus/customization checks.

Normal Iced window settings do not expose a Wayland transient parent; source-bound
floating Quick Look needs an actual supported platform solution. Standard Iced
Wayland file-drop events are another integration gate. Investigate shared stack
and platform capabilities, follow `docs/vendor-maintenance.md` for any fork, and
do not substitute another GUI toolkit or omit a requirement to claim completion.

Durable request foundations remain in
[job registry proof](job-registry-foundation-verification.json),
[atomic metadata handoff proof](job-handoff-verification.json) and
[durable jobs checkpoint](DURABLE-JOBS-WIP.md). They still need source leases,
crash-safe running plans and native save/hydration. This GUI correction does not
close or replace any of those gates.
