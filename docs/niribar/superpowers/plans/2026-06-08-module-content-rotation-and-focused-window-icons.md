# Module Content Rotation and Focused Window Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement shared `content-rotation` for formatted bar module content, and make `focused-window` show application icon plus title with `content-rotation "auto"` by default.

**Architecture:** `content-rotation` is module-level config that applies after existing **Format Template** selection. Rendering keeps all existing module behavior unless formatted bar content is selected and rotation is non-`none`. Focused-window gets kind-level defaults and a richer render state that distinguishes no active **Compositor Window** from an active window with missing title/app id. Application icon resolution is lazy and process-lifetime cached.

**Design spec:** `docs/niribar/superpowers/specs/2026-06-08-module-content-rotation-and-focused-window-icons-design.md`

**Tech Stack:** Rust 2024, KDL config parser, iced, existing module render helpers, `rtk cargo test`.

---

## File Structure

- Modify: `crates/niribar/src/config/template.rs`
  - Add `ContentRotation` and resolved rotation helpers near `FormatConfig`.
  - Add format-segment metadata helpers needed to detect exact `{icon}` segments.
- Modify: `crates/niribar/src/config/module.rs`
  - Add `content_rotation` sibling fields to every non-tray formatted module config.
  - Add `fallback_icon` to `FocusedWindowConfig`.
  - Change `FocusedWindowConfig::default()` to default `format "{icon}" "{title}"`, `content_rotation: Auto`, and fallback icon `"□"`.
- Modify: `src/config/parse/modules/*.rs`
  - Parse `content-rotation` for every non-tray module with format support.
  - Keep tray rejecting `content-rotation`.
  - Parse focused-window `fallback-icon`.
  - Add focused-window `{app-id}` placeholder.
- Modify: `crates/niribar/src/modules.rs`
  - Thread `content_rotation` and focused-window fallback icon into constructors and synthesized defaults.
- Modify: `crates/niribar/src/modules/render.rs`
  - Introduce or extend bar render context so formatted modules can resolve `auto` from **Edge**.
- Modify: `crates/niribar/src/app/module_runtime.rs`
  - Pass **Edge** through bar module rendering.
  - Keep fallback selection unchanged except for the new render context.
- Modify: `crates/niribar/src/app/render.rs`
  - Include **Edge** in `BarModuleRenderRequest`.
  - Extend focused-window per-output state projection from title-only to window-present/title/app-id/icon.
- Modify: `crates/niribar/src/niri.rs`
  - Add helper(s) to return the active/focused `niri_ipc::Window`, not only title.
- Add or modify: `crates/niribar/src/focused_window_icons.rs`, `crates/niribar/src/desktop_apps.rs`, or `crates/niribar/src/icon_theme.rs`
  - Add lazy application-icon resolution from app id to icon path/name.
- Modify: `crates/niribar/src/modules/view.rs` or add `src/modules/rotated.rs`
  - Centralize rotated formatted-content rendering.
- Modify: concrete modules under `crates/niribar/src/modules/`
  - Use the shared helper for formatted bar content when `content_rotation` resolves to rotation.
  - Keep panel, tooltip, unavailable, empty, and unformatted fallback behavior unchanged.
- Modify: `crates/niribar/config/default.kdl` and `crates/niribar/config/new_default.kdl`
  - Document shared `content-rotation`.
  - Make focused-window defaults explicit.

---

## Slice 1: Config Model and Parser

**Files:**
- Modify: `crates/niribar/src/config/template.rs`
- Modify: `crates/niribar/src/config/module.rs`
- Modify: `crates/niribar/src/config/parse/modules/formats.rs`
- Modify: `crates/niribar/src/config/parse/modules/{battery,volume,network,clock,script,button,workspaces,focused_window,tray}.rs`
- Modify tests under: `src/config/parse/modules/*_tests.rs`, `crates/niribar/src/config/parse/tests.rs`, `crates/niribar/src/config/template_tests.rs`

- [ ] **Step 1: Add failing tests for `ContentRotation`**

Add tests proving:

- `ContentRotation::default()` is `None`.
- `auto` resolves to counter-clockwise on left edge.
- `auto` resolves to clockwise on right edge.
- `auto` resolves to no rotation on top/bottom edges.
- explicit clockwise/counter-clockwise resolve as-is on every edge.
- `none` always resolves to no rotation.

Run:

```bash
rtk cargo test config::template_tests --lib
```

Expected: fail before implementation.

- [ ] **Step 2: Implement `ContentRotation`**

Add near `FormatConfig`:

```rust
pub enum ContentRotation {
    None,
    Auto,
    Clockwise,
    CounterClockwise,
}

pub enum ResolvedContentRotation {
    None,
    Clockwise,
    CounterClockwise,
}
```

Implement `Default`, parsing from string, accepted value listing for diagnostics, and `resolve_for_edge(&Edge)`.

- [ ] **Step 3: Add config fields and defaults**

Add `content_rotation: ContentRotation` to:

- `BatteryConfig`
- `NetworkConfig`
- `VolumeConfig`
- `ClockConfig`
- `ScriptConfig`
- `ButtonConfig`
- `WorkspacesConfig`
- `FocusedWindowConfig`

Add `fallback_icon: String` to `FocusedWindowConfig`.

Defaults:

- all non-focused-window modules: `ContentRotation::None`
- focused-window: `ContentRotation::Auto`
- focused-window fallback icon: `"□"`
- focused-window default `format`: exactly `"{icon}" "{title}"`
- no focused-window default `expanded-format` or `horizontal-format`

Add or expose a safe constructor for static default `FormatTemplate` values instead of duplicating parser internals.

- [ ] **Step 4: Parse `content-rotation`**

Add a reusable parser helper, likely in `formats.rs`, that reads an optional `content-rotation` string and returns `ContentRotation`.

Parser requirements:

- Accepted values: `none`, `auto`, `clockwise`, `counter-clockwise`.
- No aliases.
- Unknown values produce a diagnostic at `modules[N].<type>.content-rotation`.
- Help text lists all accepted values.

Wire it into every non-tray formatted module parser.

- [ ] **Step 5: Keep tray rejecting rotation**

Do not add `content-rotation` to the tray parser schema. Add a parser test proving:

```kdl
modules { module "tray" { content-rotation "auto" } }
```

is rejected as an unknown field.

- [ ] **Step 6: Parse focused-window fallback/app-id**

In `focused_window.rs` parser:

- allow child fields `content-rotation` and `fallback-icon`
- reject empty `fallback-icon`
- add `app-id` to format and tooltip placeholder allow-lists

Tests:

- parse `fallback-icon "app"`
- reject `fallback-icon ""`
- parse `format "{app-id}"`
- parse `tooltip "{app-id}"`
- reject unknown placeholder still points at the right field

- [ ] **Step 7: Run parser/config tests**

Run:

```bash
rtk cargo test config::parse::modules --lib
rtk cargo test config::parse::tests::parse_formats_for_all_non_tray_modules --lib
rtk cargo test config::template_tests --lib
```

Expected: pass.

---

## Slice 2: Focused Window State and Application Icon Resolution

**Files:**
- Modify: `crates/niribar/src/modules/focused_window.rs`
- Modify: `crates/niribar/src/niri.rs`
- Modify: `crates/niribar/src/app/module_state.rs`
- Modify: `crates/niribar/src/app/render.rs`
- Add or modify: `crates/niribar/src/focused_window_icons.rs`, `crates/niribar/src/desktop_apps.rs`, `crates/niribar/src/icon_theme.rs`
- Modify tests in: `crates/niribar/src/modules/focused_window.rs`, `crates/niribar/src/niri.rs`, `crates/niribar/src/app/render.rs`, `crates/niribar/src/app/niri.rs`

- [ ] **Step 1: Add Niri window selection helpers**

Add helpers that return `Option<&niri_ipc::Window>`:

- globally focused window by `focused_window_id`
- active window for an output by active workspace

Keep existing title helper if tests rely on it, but make focused-window projection use the window helper.

Tests:

- returns active window for an output
- returns `None` when output has no active window
- returns globally focused window
- handles cached-window miss

- [ ] **Step 2: Redesign `FocusedWindowState`**

Replace title-only state with explicit active-window state, for example:

```rust
pub struct FocusedWindowState {
    pub window: Option<FocusedWindowInfo>,
}

pub struct FocusedWindowInfo {
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub application_icon: Option<ApplicationIcon>,
}
```

The exact type names can vary, but the state must distinguish:

- no active window: render nothing/no gap
- active window with missing title: render visible content with empty title value
- active window with missing app id: render visible content with fallback icon

`FocusedWindowState::default()` remains no active window.

- [ ] **Step 3: Add pure app-id matching tests**

Create a pure helper over a list of `DesktopApplication` values so matching can be tested without filesystem access.

Test conservative matching:

- exact id: `firefox.desktop`
- exact stem: `firefox` -> `firefox.desktop`
- reverse-DNS suffix: `org.mozilla.firefox` -> `firefox.desktop`
- no fuzzy display-name/keyword matching

- [ ] **Step 4: Implement lazy icon resolver**

Add a lazy, process-lifetime cached resolver:

1. On first lookup, discover desktop applications with current desktop filtering.
2. Match app id conservatively against filtered desktop entries.
3. Use the desktop entry `Icon` value when matched.
4. Otherwise try app id directly through `icon_theme::resolve_icon_path`.
5. Cache app-id resolution results, including misses.

Do not add file watchers or live invalidation.

Do not log per-window resolution failures.

- [ ] **Step 5: Project focused-window state with title/app-id/icon**

Update:

- `App::push_focused_window_state`
- `App::module_render_state`

Rules:

- output-specific behavior stays as it is today, but applies to title, app id, and icon
- output-less surfaces fall back to the globally focused window
- `None` window means no visible focused-window content
- `Some(window)` with `title: None` or `title: Some("")` means visible content with empty title value

- [ ] **Step 6: Update focused-window placeholder values**

Focused-window tooltip/format values:

- `{title}` and `{value}` are empty string when title is missing/empty
- `{app-id}` is empty string when missing
- `{icon}` text fallback is `fallback-icon`

Default tooltip remains `"{title}"`. If rendered tooltip text is empty, no tooltip should show.

- [ ] **Step 7: Run focused-window/Niri tests**

Run:

```bash
rtk cargo test niri::tests --lib
rtk cargo test app::render::tests --lib
rtk cargo test app::niri::tests --lib
rtk cargo test modules::focused_window::tests --lib
```

Expected: pass.

---

## Slice 3: Bar Render Context and Rotated Formatted Content Helper

**Files:**
- Modify: `crates/niribar/src/modules/render.rs`
- Modify: `crates/niribar/src/app/module_runtime.rs`
- Modify: `crates/niribar/src/app/render.rs`
- Modify or add: `crates/niribar/src/modules/view.rs` or `src/modules/rotated.rs`
- Modify: `crates/niribar/src/config/template.rs`

- [ ] **Step 1: Add bar render context**

Replace the current render path's edge-less shape with a context that includes at least:

- `window`
- `edge`
- `horizontal`

Update `BarModuleRenderRequest` and the internal module render calls so modules can resolve `ContentRotation::Auto`.

Keep existing render fallback order:

- expanded bar path still uses panel render if it exists in current code
- horizontal bar uses bar-horizontal override, then horizontal render, then panel fallback
- vertical bar uses bar-collapsed override, then collapsed render

- [ ] **Step 2: Add format segment metadata helpers**

Expose enough template metadata to distinguish an exact `{icon}` segment from mixed text.

Required behavior:

- `"{icon}"` is an exact icon segment
- styled `segment "{icon}"` is an exact icon segment
- `" {icon} "` is not exact
- `"{icon}: {title}"` is not exact

Add unit tests in `crates/niribar/src/config/template_tests.rs`.

- [ ] **Step 3: Add rotated formatted-content helper tests**

Create tests for pure rotation resolution and helper construction where possible:

- no rotation returns ordinary formatted row/element
- clockwise/counter-clockwise can be constructed without panic
- styled segments preserve resolved style metadata before rendering
- image segment size uses configured segment `size` when present

If exact rendered geometry cannot be unit-tested with Iced, keep geometry tests at the pure helper level and cover visual integration with no-panic render tests.

- [ ] **Step 4: Implement centralized helper/widget**

Implement one shared helper for formatted bar content. The module-facing API should take resolved formatted segments and a resolved rotation direction.

Suggested local model:

```rust
pub(super) enum FormattedSegmentVisual {
    Text {
        text: String,
        style: ResolvedStyle,
    },
    FocusedWindowApplicationIcon {
        icon: ApplicationIcon,
        fallback_text: String,
        style: ResolvedStyle,
    },
}
```

The helper owns the choice of native Iced transform vs custom advanced widget. Do not leak that implementation detail into modules.

Requirements:

- rotation wraps the formatted group as one visual unit
- segment spacing is preserved
- newlines inside text segments are preserved before rotation
- rotated content may request long-axis space according to natural formatted content length
- clipping remains the existing zone/container behavior
- pointer hit boxes use the final visible module rectangle

- [ ] **Step 5: Keep panels and tooltips out of the helper path**

Only bar rendering should call the rotation helper. Do not apply `content-rotation` from:

- `render_panel_with_style`
- `render_detail_with_style`
- tooltip rendering

- [ ] **Step 6: Run render-context tests**

Run:

```bash
rtk cargo test app::module_runtime::tests --lib
rtk cargo test app::render::tests --lib
rtk cargo test modules::view::tests --lib
```

Expected: pass.

---

## Slice 4: Wire Rotation Through Formatted Modules

**Files:**
- Modify: `crates/niribar/src/modules/{battery,volume,network,clock,script,button,focused_window,workspaces}.rs`
- Modify: `crates/niribar/src/modules/view.rs`
- Modify: `crates/niribar/src/modules.rs`

- [ ] **Step 1: Thread `content_rotation` through constructors**

Update each formatted module struct and constructor to store `ContentRotation`.

Modules:

- battery
- volume
- network
- clock
- script
- button
- workspaces parent config and workspace item children
- focused-window

Keep tray unchanged.

- [ ] **Step 2: Update `IconLabel` helper path**

For battery, volume, and network, apply rotation only when:

- the current bar render selected a configured **Format Template**
- resolved rotation is not `None`

Do not rotate built-in fallback icon/label rendering.

- [ ] **Step 3: Update custom formatted modules**

For clock, script, button, and focused-window:

- existing format selection rules stay unchanged
- when a formatted segment path is selected in bar rendering, pass it through the rotation helper if needed
- unformatted fallback rendering stays unchanged, except focused-window now has a default `format`

Do not rotate panel rendering.

- [ ] **Step 4: Update workspace item inheritance**

Ensure `WorkspacesConfig.content_rotation` is carried into generated `WorkspaceItemModule` instances.

Tests:

- parent `content-rotation "auto"` is visible on child workspace item config/state
- workspace items still filter by output as before
- no configured format means no rotation effect

- [ ] **Step 5: Implement focused-window image icon rendering**

For focused-window formatted bar content:

- exact `{icon}` segment may render image/SVG **Application Icon**
- mixed `{icon}` segment renders `fallback-icon` text
- tooltip `{icon}` renders `fallback-icon` text
- segment `size` controls image size when present
- segment color/weight do not recolor application images
- unresolved icon renders fallback text

- [ ] **Step 6: Add module tests**

Add or update tests proving:

- all non-focused-window modules default to no rotation
- focused-window defaults to auto rotation and default format
- explicit rotation applies only formatted bar content
- explicit rotation can apply on horizontal bars after `horizontal-format` selection
- `auto` on vertical bars uses `format`, not `horizontal-format`
- panels remain unrotated
- unavailable and empty states render nothing and have nothing to rotate
- active focused window with missing title still renders visible fallback icon content
- no active focused window renders no content/no gap

- [ ] **Step 7: Run targeted module tests**

Run:

```bash
rtk cargo test modules::battery::tests --lib
rtk cargo test modules::volume::tests --lib
rtk cargo test modules::network::tests --lib
rtk cargo test modules::clock::tests --lib
rtk cargo test modules::script::tests --lib
rtk cargo test modules::button::tests --lib
rtk cargo test modules::workspaces::tests --lib
rtk cargo test modules::focused_window::tests --lib
```

Expected: pass.

---

## Slice 5: Default Config and Documentation

**Files:**
- Modify: `crates/niribar/config/default.kdl`
- Modify: `crates/niribar/config/new_default.kdl`
- Modify: `docs/niribar/superpowers/specs/2026-06-08-module-content-rotation-and-focused-window-icons-design.md` only if implementation discovers a design mismatch

- [ ] **Step 1: Document shared `content-rotation` once**

In the shared module-format comment block, add:

```kdl
// Rotate formatted bar content. Values: "none", "auto", "clockwise", "counter-clockwise".
// "auto" rotates on left/right bars and stays unrotated on top/bottom bars.
```

Do not repeat the full explanation in every module block.

- [ ] **Step 2: Make focused-window config match Rust defaults**

In the focused-window block, explicitly include:

```kdl
format "{icon}" "{title}"
content-rotation "auto"
fallback-icon "□"
```

Keep default tooltip documentation as `"{title}"`.

Add `app-id` to the focused-window placeholder comment.

- [ ] **Step 3: Keep non-focused-window examples optional**

Do not change default behavior for battery, volume, network, clock, script, button, or workspaces. If examples mention `content-rotation`, keep them commented.

- [ ] **Step 4: Run default config tests**

Run:

```bash
rtk cargo test config::tests --lib
rtk cargo test config::parse::tests --lib
```

Expected: pass.

---

## Slice 6: Verification and Cleanup

- [ ] **Step 1: Format**

Run:

```bash
rtk cargo fmt --check
```

If it fails:

```bash
rtk cargo fmt
rtk cargo fmt --check
```

- [ ] **Step 2: Full tests**

Run:

```bash
rtk cargo test
```

Expected: pass.

- [ ] **Step 3: Clippy/build gate**

Run:

```bash
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: pass.

- [ ] **Step 4: Behavior audit**

Before finalizing, inspect these behaviors manually or through focused render tests:

- default focused-window on a left bar shows icon plus title rotated counter-clockwise
- default focused-window on a right bar shows icon plus title rotated clockwise
- focused-window on top/bottom bars shows icon plus title unrotated by auto
- other modules look unchanged by default
- no active window leaves no gap
- active window with missing title still shows fallback/app icon
- tooltip remains unrotated and title-only by default

## Self-Review

Spec coverage:

- Covers shared `content-rotation`, focused-window defaults, app id placeholder, app icon lookup, own-icon-segment image rendering, per-output behavior, workspace inheritance, tooltip/panel exclusions, and unavailable/empty behavior.

Behavior preservation:

- Existing module behavior remains unchanged outside focused-window defaults and explicitly configured `content-rotation`.

Implementation risk:

- Actual rotation support depends on Iced rendering capabilities. The plan isolates that risk inside one shared helper/widget so module code and config semantics remain stable.
