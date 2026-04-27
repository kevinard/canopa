# Module Tooltips — Design

Status: approved (sections 1–5)
Date: 2026-04-28

## Goal

Add hover tooltips to bar modules. Tooltip content is configurable per module
using the same template grammar already used by the `format` family, with
support for styled segments and module-specific placeholders. Tray icons get
tooltips driven by the SNI `ToolTip` property.

## Non-goals

- Mode-specific tooltip variants (no `expanded-tooltip` / `horizontal-tooltip`).
- Rendering icon-name or pixmap fields from SNI `ToolTip`.
- Per-bar / per-module tooltip placement override.
- Visual regression / screenshot testing.

## Section 1 — Config schema

### Per-module `tooltip` field

Every module that today accepts `format` (`clock`, `battery`, `volume`,
`network`, `focused-window`, `workspaces`, `button`, `script`) plus `tray`
gains an optional `tooltip` field.

Accepted forms:

| Form                     | Meaning                                            |
|--------------------------|----------------------------------------------------|
| `tooltip "..."`          | Template string, same grammar as `format`.         |
| `tooltip { ... }`        | Styled-segment block, same grammar as `format`.    |
| `tooltip false`          | Tooltip disabled for this module.                  |
| (field absent)           | Use the module's built-in default tooltip.         |
| `tooltip true`           | Parse error: "use absence to mean default".        |

Placeholder set per module mirrors that module's `format` allowed set,
so `{icon}`, `{value}`, `{state}`, `{label}`, `{title}`, etc. resolve
identically. Tray adds two placeholders: `{title}` and `{description}`,
both sourced from the SNI `ToolTip` property.

### Per-module built-in defaults

| Module           | Default tooltip               |
|------------------|-------------------------------|
| `clock`          | `"{date}"` (clock allows `icon`, `value`, `time`, `date`) |
| `battery`        | `"{value}% {state}"`          |
| `volume`         | `"{value}%"`                  |
| `network`        | `"{label}"`                   |
| `focused-window` | `"{title}"`                   |
| `workspaces`     | none                          |
| `button`         | none                          |
| `script`         | none                          |
| `tray`           | `"{title}"`                   |

Implementation note: each placeholder above is verified against the module's
existing allowed-placeholders list. Do not expand any module's allowed set
to make a default work — pick a different placeholder.

For modules whose default is "none", `TooltipSetting::Default` resolves to
`None` at config-load time (runtime treats it identically to `Disabled`).

### Bar-level `tooltip-delay`

New optional field on `BarConfig`:

- `tooltip-delay <ms>` — milliseconds the cursor must dwell over a module
  before its tooltip appears. Default `500`. Non-integer / negative values
  rejected at parse time. Hide is always immediate on hover-exit.

Sibling to the existing `hover-delay` (used by `expand-trigger "hover"`).

### Storage

- New `pub tooltip: TooltipSetting` field on each module config struct
  (`BatteryConfig`, `VolumeConfig`, `ClockConfig`, `NetworkConfig`,
  `FocusedWindowConfig`, `WorkspacesConfig`, `ButtonConfig`, `ScriptConfig`,
  `TrayConfig`).
- `TooltipSetting`:
  ```rust
  pub enum TooltipSetting {
      Default,                 // field absent → use module default
      Disabled,                // tooltip false
      Custom(FormatTemplate),  // string or styled block
  }
  ```
  The "default" arm resolves to a concrete `FormatTemplate` at config-load
  time so runtime code only ever sees `Option<FormatTemplate>` (None =
  disabled). This keeps the runtime path uniform.
- New `pub tooltip_delay_ms: u32` on `BarConfig`, default `500`.

## Section 2 — Runtime: hover dwell and tooltip state

### Per-surface state

Extend `SurfaceState` in `src/surface.rs`:

- `tooltip_pending: Option<(ModuleId, f32)>` — module currently being dwelled
  on, with elapsed milliseconds since hover-enter.

Existing `hovered_module` continues to track the cursor target for hover
expand and focus rendering.

### Flow

1. `ModuleHoverEnter(id, window)` — sets `hovered_module = Some(id)` (existing
   behaviour) and additionally `tooltip_pending = Some((id, 0.0))` if the
   module's resolved tooltip template is `Some(_)` (i.e., not disabled).
2. Animation tick — the existing tick loop already runs while
   `hover_pending_ms.is_some()`. Extend the predicate in
   `src/app/subscriptions.rs:127` to also keep ticking while any surface has
   `tooltip_pending.is_some()`. Each tick advances `tooltip_pending`'s elapsed
   field. When elapsed crosses `bar.tooltip_delay_ms`, transition to "open":
   render the template into segments, build a `TooltipTree`, open a tooltip
   popup surface, and clear `tooltip_pending`.
3. `ModuleHoverExit(id, window)` — if `tooltip_pending` matches `id`, drop it.
   If a tooltip popup is open for `id` on this window, close it.
4. Dismissals — opening a menu popup, clicking the module, escape press, bar
   focus-loss: close any open tooltip and cancel any pending dwell.
5. Cursor moves directly module A → module B without an explicit exit:
   the new HoverEnter on B implicitly cancels A (set
   `tooltip_pending = Some((B, 0.0))`, close any open tooltip for A).

### Disabled tooltips

When a module's resolved tooltip is disabled, HoverEnter still sets
`hovered_module` but never starts a dwell and never opens a popup.

## Section 3 — Tooltip popup surface

New module: `src/surface/tooltip_popup.rs`. Mirrors `menu_popup.rs` in shape
and ownership — popups belong to the surface layer, tooltip rendering is
generic and module-agnostic.

### Data type

```rust
pub struct TooltipTree {
    pub segments: Vec<RenderedSegment>, // pre-rendered text + resolved styles
    pub anchor: (i32, i32),              // module's screen-space anchor
    pub screen_size: (u32, u32),         // for edge-clamping
    pub bar_edge: BarEdge,               // dictates which side of the anchor
}
```

`RenderedSegment` is whatever type `menu_popup`'s styled text rendering
already consumes (resolved text + `ResolvedStyle`); reuse rather than
duplicate.

The app layer renders the module's `tooltip` template into `segments` at
popup-open time, using the same value bag the module's `format` template
already receives (so `{icon}`, `{value}`, `{state}`, `{title}` etc. resolve
consistently). For tray, the value bag is `{title}` and `{description}` from
the SNI `ToolTip` property.

### Popup registry

Add a sibling to `MenuPopup` in `src/app/surfaces.rs`:

```rust
pub struct TooltipPopup {
    pub window: iced::window::Id,
    pub target_window: iced::window::Id,
    pub target_module: ModuleId,
    pub tree: TooltipTree,
}
```

Stored alongside existing menu popups so dismissal logic (escape, outside
click, bar focus-loss) iterates both.

### Window / anchoring

Open via `iced_layershell` matching how menu popups open. Anchor on the side
of the module opposite the bar edge:

- `top` bar → tooltip below the module
- `bottom` bar → above
- `left` bar → right of the module
- `right` bar → left of the module

Edge-clamp horizontally / vertically using the existing `screen_size` clamp
logic from `menu_popup`.

At most one tooltip popup per surface at a time; opening a new one closes the
prior.

### View

A single styled row (or column for vertical bars) of segments using whatever
styled-text rendering helper `menu_popup` already exposes. Background, border,
padding from the bar's `theme`. No interactivity (no `mouse_area`, no
`button`).

## Section 4 — Tray tooltip data path

The tray module already produces one synthetic per-icon `ModuleId`, routed
through the same `ModuleHoverEnter/Exit` events as other modules. Tooltip
keying is automatic.

### SNI ToolTip capture

In `src/dbus/sni/`:

- On registering an SNI service, fetch the `ToolTip` property
  (signature `(s, a(iiay), s, s)` = icon-name, icon-pixmap, title,
  description). Store only `title` and `description` as
  `SniTooltip { title: String, description: String }` on the per-item state.
- Subscribe to `NewToolTip` signal; on signal, refetch and update the stored
  value.
- Drop icon-name and pixmap fields (out of scope, per Section 1).

### Config

`TrayConfig` gains the same `tooltip: TooltipSetting` field with default
template `"{title}"`.

### Value bag and suppression

For tray, `{title}` and `{description}` resolve from the per-icon stored
`SniTooltip`. If the SNI item has no tooltip set, both placeholders render
as empty strings. If the joined rendered template is empty (whitespace only),
suppress the popup — do not open an empty box.

## Section 5 — Testing

Follows existing patterns in `src/config/parse/tests.rs` and inline app-state
tests.

### Config parsing

- `tooltip "..."` parses to a `FormatTemplate`.
- `tooltip { ... }` styled block parses identically to `format { ... }`.
- `tooltip false` parses to `TooltipSetting::Disabled`.
- `tooltip true` returns a parse error referencing the field path.
- Absent field yields the module-specific default template — one test per
  module type that has a non-empty default; one test verifying modules with
  "none" default produce `Disabled`-equivalent runtime behaviour.
- Per-module placeholder validation: `tooltip "{bogus}"` errors with the same
  message style as `format "{bogus}"`.
- `tooltip-delay <ms>` parses on the bar; non-integer / negative rejected.
- Default `tooltip_delay_ms` is 500 when field absent.

### Runtime hover dwell

(in `src/app/surfaces.rs` or new `src/app/tooltip.rs`)

- HoverEnter starts dwell; advancing time below threshold does not open a
  tooltip.
- Time crossing `tooltip_delay_ms` opens a tooltip popup surface for that
  module.
- HoverExit before threshold cancels dwell; no popup opened.
- HoverExit after open closes the tooltip popup.
- HoverEnter on module B while A is dwelling cancels A's dwell, starts B's.
- HoverEnter on module B while A's tooltip is open: A's tooltip closes, B's
  dwell starts.
- Opening a menu popup closes any open tooltip and cancels any dwell.
- Module click closes any open tooltip and cancels any dwell.
- Bar focus-loss closes tooltips. Escape closes tooltips.
- Disabled tooltip: HoverEnter sets `hovered_module` but never schedules a
  dwell or opens a popup.

### Tray tooltip data path

- `ToolTip` property captured at SNI item registration.
- `NewToolTip` signal triggers a refetch and updates stored value.
- Empty / absent SNI tooltip → popup suppressed.

### Surface popup behaviour

(in `src/surface/tooltip_popup.rs`)

- Edge clamping near each screen edge mirrors menu_popup tests.
- Anchor side flips based on `BarEdge` (4 cases).

No screenshot / visual regression tests.

## File-touch summary

- `src/config.rs` — `TooltipSetting`, `tooltip` fields on module configs,
  `tooltip_delay_ms` on `BarConfig`.
- `src/config/parse/modules/formats.rs` — `parse_tooltip` helper accepting
  string / block / `false`.
- `src/config/parse/modules/{battery,volume,clock,network,focused_window,workspaces,button,script,tray}.rs`
  — wire the new field through.
- `src/config/parse/bars.rs` — `tooltip-delay` field.
- `src/config/parse/tests.rs` — new tests per Section 5.
- `src/surface.rs` — `tooltip_pending` field on `SurfaceState`.
- `src/surface/tooltip_popup.rs` — new module: `TooltipTree`,
  `view_tooltip_popup`.
- `src/surface/mod.rs` (or wherever `pub mod menu_popup` lives) — export
  the new module.
- `src/app/messages.rs` — any new events needed (e.g., a `TooltipOpen`
  internal event if the open path is async).
- `src/app/surfaces.rs` — `TooltipPopup` registry, dismissal wiring,
  HoverEnter/Exit dwell wiring.
- `src/app/subscriptions.rs` — extend tick predicate (~line 127).
- `src/app/animation.rs` (or equivalent) — advance `tooltip_pending`.
- `src/app/render.rs` — render the tooltip popup window when a `TooltipPopup`
  exists for it.
- `src/dbus/sni/proxy.rs` / `host.rs` / `subscription.rs` — fetch `ToolTip`
  property, subscribe to `NewToolTip` signal.
- `src/modules/tray.rs` — expose `SniTooltip` to the tooltip rendering layer.
- `config/default.kdl` — document the new `tooltip` field and `tooltip-delay`
  bar field.
