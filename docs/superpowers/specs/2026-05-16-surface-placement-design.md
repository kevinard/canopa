# Surface Placement Refactor Design

## Context

Niribar currently has multiple implementations for placing transient overlay surfaces on screen:

- **Menus** calculate popup-window positions in `src/app/menu.rs`.
- **Panels** calculate layer-shell positions and margins in `src/app/panel.rs`.
- **Tooltips** calculate layer-shell anchors and margins in `src/app/surfaces.rs`.

These paths all depend on the same domain facts: the **Bar Surface** edge, the overlay size, an anchor point or rectangle, the logical screen size, the bar thickness, and a gap. The repeated logic makes placement inconsistent and hard to maintain. The first refactor must preserve current behavior, including the current tooltip behavior where tooltip placement uses the stored `bar_size` value that is populated from the screen size.

## Goals

- Create one deep placement **Module** for edge-relative overlay geometry.
- Keep **Menu**, **Tooltip**, and **Panel** content models focused on content rather than placement math.
- Keep app-owned lifecycle and widget-bound lookup behavior in the app layer.
- Make window-system differences explicit through small adapters.
- Add focused tests around placement rules so future behavior changes are intentional.

## Non-Goals

- Do not redesign overlay visuals.
- Do not change **Menu**, **Tooltip**, or **Panel** lifecycle behavior.
- Do not replace iced or iced-layershell window creation.
- Do not silently fix tooltip placement semantics in this pass.
- Do not introduce a broad overlay coordinator that owns lifecycle, rendering, and geometry together.

## Chosen Approach

Add a pure `src/surface/placement.rs` module. This module is the primary **seam** for overlay geometry. Its **interface** accepts plain placement facts and returns logical placement results. It does not open windows, query widget bounds, inspect app state, or know about D-Bus, module actions, panel menus, or tooltip templates.

The app layer remains responsible for:

- Resolving module/widget bounds with `FindBounds`.
- Choosing pointer fallback behavior.
- Looking up **Bar** and **Bar Surface** metadata.
- Building `IcedNewPopupSettings` or `NewLayerShellSettings`.
- Managing overlay lifecycle and exclusivity.

This keeps the placement module deep: callers provide a small set of facts and receive consistent edge-aware placement behavior.

## Placement Module Shape

The new module should expose small value types or functions around the existing primitives:

- A placement input containing:
  - **Edge**
  - overlay size
  - screen size
  - bar thickness
  - gap
  - anchor point or anchor rectangle
  - optional surface origin for **Panel** placement
- A placement output containing:
  - logical `(x, y)` position
  - layer-shell anchor selection when needed
  - conversion to `(top, right, bottom, left)` margin

The module should provide two placement families:

- Edge-adjacent placement for **Menus** and **Tooltips**.
- Centered-on-anchor placement for **Panels**.

Both families share clamping and edge-axis rules.

## Adapters

### Menu Adapter

`src/app/menu.rs` keeps `popup_size(tree)` and popup lifecycle. It asks `surface::placement` for an edge-adjacent position and converts that result to `IcedNewPopupSettings`.

The existing behavior is preserved:

- Top **Bar Surface** menus open below the bar.
- Bottom **Bar Surface** menus open above the bar.
- Left **Bar Surface** menus open to the right.
- Right **Bar Surface** menus open to the left.
- Cross-axis position is clamped to the screen when screen size is known.

### Tooltip Adapter

`src/app/surfaces.rs` keeps tooltip rendering, dwell handling, and tooltip lifecycle. Tooltip window settings delegate edge-relative margin and anchor computation to `surface::placement`.

The current tooltip behavior is preserved in this pass, including using the existing tooltip placement inputs. A test should make the current `bar_size` semantics explicit so a later fix is deliberate.

### Panel Adapter

`src/app/panel.rs` keeps panel lifecycle, focus behavior, search state, and panel content decisions. Panel window settings delegate centered-on-anchor placement and margin conversion to `surface::placement`.

The existing inferred **Bar Surface** origin behavior remains app-owned because it depends on configured bars for the output.

## Anchor Resolution

Anchor resolution stays separate from geometry:

- Module and tray bounds are still resolved with `FindBounds`.
- Pointer fallback remains app-owned.
- The tray menu anchor should stop inferring orientation from `screen_size.0 > screen_size.1`; it should derive anchor behavior from the originating **Bar** edge when that information is available.
- Tooltip anchor resolution continues to use `last_pointer_position` in the behavior-preserving pass.
- **Panel** anchor rectangles continue to flow through `PanelEvent`.

This can be implemented as small app-layer helpers before or alongside the geometry refactor, but these helpers should not become a broad overlay coordinator.

## Testing

Add unit tests for `surface::placement` covering:

- Top, bottom, left, and right edge-adjacent placement.
- Centered-on-anchor placement for top, bottom, left, and right panels.
- Cross-axis clamping.
- Oversized overlay clamping.
- Gap handling.
- Position-to-layer-shell-margin conversion.
- Zero screen-size fallback behavior where existing code currently allows unclamped positions.

Keep adapter tests narrow:

- Menu adapter still returns the expected popup settings shape.
- Tooltip adapter still returns the expected layer-shell anchor and margin shape.
- Panel adapter still preserves panel size, output targeting, keyboard interactivity, and margin behavior.

## Migration Steps

1. Add `surface::placement` with tests for current placement behavior.
2. Move menu popup placement to the placement module through a small adapter.
3. Move tooltip layer-shell anchor and margin calculation to the placement module through a small adapter.
4. Move panel position and margin calculation to the placement module through a small adapter.
5. Extract small app-layer anchor helpers where duplication remains, especially tray menu anchor selection.
6. Run the existing Rust test suite and clippy checks.

## Follow-Up

After the behavior-preserving refactor lands, revisit tooltip placement semantics. The likely fix is to make tooltip placement use the **Bar Surface** thickness along the anchored edge rather than the full screen size, but that should be a separate intentional behavior change with before/after tests.
