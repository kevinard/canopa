# Iced source-bound previews — verified slice, full goal active

No build or private native process is live. Core handle72312 is TERMINAL0.
Only original user Niri1238 and PipeWire1537/1562 remain among inspected services.
No subagents were spawned. No commit was made (.git is read-only). Preserve the
unrelated Shell research and the entire product brief.

## Settled candidate and evidence

`iced-preview-final-source.json` freezes458 inputs, checked unchanged after all
validation. Default executable:target/debug/canopa-files,82,864,920 bytes,
SHA256:f1bbba93361f2dd95a080cc7000ffccd439e022e61e6620898b0b5a2a3f4a087.
GUI dependency graph includes Iced/canopa-ui and no GTK GUI dependency.

- 19 adapter tests and10 public preview tests pass (verification02).
- All-target Files Clippy lints pass;4 existing dead-code warning groups remain.
- Verification02 build hit ENOSPC after passing those tests/lints; unchanged
  build/fmt retry03 passes. No source change between passed stages and retry.
- Final native20/21 previews pass15 each;22/23 browsing pass13 each;24/25 search
  pass16 each. All6 use this binary:88 native checks,60 inspected PNGs and clean
  application logs, in dark/100% and light/125%. Hashes are recorded in
  iced-preview-native-verification.json. Preview outer size760×635 beside360px
  browser; no application-specific Niri rule. All screenshots were inspected.
- Workspace all-target compilation passes (shared retry04).
- Complete fork graph builds in a disposable workspace;4 layer-shell library
  tests pass (shared retry02).
- Exact6 published archives, local inventories and reviewable patches verify.
- Formatting,558-package attribution CHECK, Python syntax, ADR links and tracked
  whitespace checks pass.
- Full `bash scripts/check.sh --core` passes outside sandbox with
  RUST_TEST_THREADS=1,CARGO_BUILD_JOBS=1,CARGO_NET_OFFLINE=true. See
  iced-preview-core-verification.json and iced-preview-core-serial-02.log.
  All Files core tests and the Shell/Notify/protocol/shared-package stages run.
- Strict workspace Clippy-Dwarnings still fails on4 known unused-operation-helper
  groups. No warnings were suppressed; full warning-free/workspace tests stay open.

Primary proof:iced-preview-verification.json. The full product is NOT complete.
Complete shared Shell/Notify native acceptance remains required. Core checks may
leave their debug binaries headless; rebuild their default GUI binaries before
attempting those native fixtures. No native/compiler overlap; run fixtures
sequentially, with private HOME/XDG/D-Bus and real temporary files only.

## Implementation

preview_ui.rs owns one floating Quick Look per Browser Window. It follows the
source's active Tab and selected focused file (fallback:first selected), including
search. Other window focus cannot retarget it. Source close cancels/destroys it;
preview close preserves source selection/scroll. Raw Unix paths are retained.
Window identity and monotonic revision reject stale preparation, control, text
and viewport callbacks. A native open failure is visible on the source.

One reader thread and one replaceable pending job bound preparation, with active
and pending cancellation. Core bounded text/image/PDF preparation runs off the UI
thread. Image handles share immutable RGBA bytes. Read-only text allows selection,
copy and cursor navigation but ignores edits. Tab leaves the editor for shared
styled actions; Space/Return activates the focused action. Default preview keys
browse the source; Space/Escape closes. PDF Page Up/Down changes pages. Fit/zoom/
100% work; directions beyond manual bounds are unavailable, preventing reversed
zoom for tiny/large fitted images. Prior PDF page remains only during preparation;
failed pages clear it. Syntax colors and audio/video controls are still unported.

Shared operation:`iced::window::open_transient(parent,settings)` returns an
optional opened ID; missing/unsupported parents fail without an unrelated tile.
Layer-shell declines ordinary transients. Typed Winit Wayland attributes retain
an Arc parent through creation, validate backend/connection, and set the parent
before initial commit/configure. No new unsafe bridge or direct protocol deps.
Files owns subsequent source/preview lifetime. See root ADR0019 and Files ADR0059.

New forks live only in root vendor:iced_runtime0.14.0,iced_winit0.14.0 from
3997291f318a8bc06fa522f5579836fb3feb94df, and winit0.30.13 from
e9809ef54b18499bb4f2cac945719ecc2a61061b. Original MIT/Apache notices preserved.
All6 archives:/tmp/canopa-iced-transient-upstream. Provenance refresh helper
asserts explicit allowed paths. test-vendor.py copies/patches the complete graph;
check-vendor.py includes changed dependency manifests in reviewable patches.

## Retained diagnosis and resource evidence

Native14's initial ordinary-browser timeout has no established cause. Old-binary
control15 passed; diagnostic16 traced normal startup but exposed1280×800 floating
size. Parent attachment after Winit construction was too late for initial sizing.
Both traced source files were restored. Typed parent-before-configure fixes real
760×635 geometry in untraced17/18/19/20/21. Only20/21 are final preview acceptance.
See iced-preview-startup-diagnosis.json; do not claim the startup timeout fixed.
Zoom red64053 reproduced direction reversal at manual bounds; final19 tests and
native zoom/Fit pass. See iced-preview-zoom-diagnosis.json.

Normal check's systemd fixture failed on sandbox socket options; exact isolated
session-unit check passes outside sandbox. First core attempt similarly failed
private D-Bus binding. Outside-sandbox parallel core then passed28 Shell tests but
8 private PipeWire startups timed out. An isolated audio case passed in0.68s;
full serial core passes. The original parallel failure cause remains unproven;
keep iced-preview-core-audio-diagnosis.json. No application code was changed to
make those audio fixtures pass.

Shared attempt42759 terminated143 without a compiler diagnostic; no children
survived. Shared11155 passed fork tests, then was deliberately stopped at125MiB
free during all-target check. Attempt03 ran no stages because its space guard
refused. Serial core50048 later stopped at270MiB during Files compilation after
successful Shell/shared tests. Host ps confirmed no compiler before each cleanup.

Only older generated incremental caches were removed, preserving source, library
and executable artifacts, evidence, symlinks, and directories touched since final
freeze. Receipts:iced-preview-cache-cleanup.json,iced-preview-startup-cache-cleanup.json,
iced-preview-final-cache-cleanup.json,iced-preview-shared-cache-cleanup.json,
iced-preview-core-cache-cleanup.json. The last2 remove1,791,882,876 and4,767,450,154
bytes respectively. Final root free:1,837,133,824 bytes. Never clean during live
compiler/native work. Source458 and f1bb executable hashes remain unchanged.

## Remaining full product

Complete Iced media/syntax and preview focus/accessibility/source-authority
matrix; other UI interfaces including custom actions/editors/preferences,
operations/Undo/Recovery/devices, interoperable clipboard/Wayland DnD,
FileManager1/single-instance integration; durable queue hydration/source leases
and filesystem-step crash continuation. Preserve DURABLE-JOBS-WIP.md and all
prior authority/ownership gates. Full warning-free workspace, shared native,
packaging/release and independent review are still required. No goal completion
or pause was requested or recorded. Continue the full PRD, not just this slice.
