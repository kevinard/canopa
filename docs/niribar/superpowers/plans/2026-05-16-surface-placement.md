# Surface Placement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate Menu, Tooltip, and Panel screen-placement math behind one pure `surface::placement` module while preserving current behavior.

**Architecture:** Add a pure geometry module under `crates/niribar/src/surface/placement.rs` and expose it from `crates/niribar/src/surface.rs`. App modules keep lifecycle, widget-bound lookup, and iced settings construction, but delegate edge-relative geometry, clamping, anchor selection, and margin conversion to placement helpers.

**Tech Stack:** Rust, iced, iced-layershell, existing `cargo test` and `cargo clippy` checks.

---

## File Structure

- Create: `crates/niribar/src/surface/placement.rs`
  - Owns pure placement value types and functions.
  - Contains unit tests for edge-adjacent placement, panel placement, clamping, gaps, zero screen-size behavior, and layer-shell margin conversion.
- Modify: `crates/niribar/src/surface.rs`
  - Adds `pub mod placement;`.
- Modify: `crates/niribar/src/app/menu.rs`
  - Replaces local menu popup placement math with `surface::placement`.
  - Keeps menu lifecycle, popup size, and `IcedNewPopupSettings` construction.
- Modify: `crates/niribar/src/app/surfaces.rs`
  - Replaces tooltip anchor and margin math with `surface::placement`.
  - Keeps tooltip dwell, rendering, popup lifecycle, and current tooltip input semantics.
- Modify: `crates/niribar/src/app/panel.rs`
  - Replaces panel position and margin math with `surface::placement`.
  - Keeps panel lifecycle, focus, inferred surface origin, and `NewLayerShellSettings` construction.
- Modify: `crates/niribar/src/app/tray.rs`
  - Replaces tray anchor orientation inference with a small edge-aware helper.

## Task 1: Add Pure Placement Module

**Files:**
- Create: `crates/niribar/src/surface/placement.rs`
- Modify: `crates/niribar/src/surface.rs`

- [ ] **Step 1: Write failing placement tests**

Create `crates/niribar/src/surface/placement.rs` with this initial test module and empty implementation shells above it:

```rust
use crate::config::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerAnchor {
    TopLeft,
    BottomLeft,
    LeftTop,
    RightTop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossAxisAnchor {
    Start,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub position: Point,
}

impl Placement {
    pub fn layer_margin(self, edge: Edge, overlay: Size, screen: Size) -> (i32, i32, i32, i32) {
        let _ = (edge, overlay, screen);
        unimplemented!("implemented in Step 3")
    }
}

pub fn layer_anchor(edge: Edge) -> LayerAnchor {
    let _ = edge;
    unimplemented!("implemented in Step 3")
}

pub fn edge_adjacent_position(
    edge: Edge,
    anchor: Point,
    overlay: Size,
    screen: Size,
    bar_thickness: u32,
    gap: i32,
    cross_axis_anchor: CrossAxisAnchor,
) -> Placement {
    let _ = (edge, anchor, overlay, screen, bar_thickness, gap, cross_axis_anchor);
    unimplemented!("implemented in Step 3")
}

pub fn centered_panel_position(
    edge: Edge,
    anchor: Option<Rect>,
    overlay: Size,
    screen: Size,
    bar_thickness: u32,
    surface_origin: Point,
) -> Placement {
    let _ = (edge, anchor, overlay, screen, bar_thickness, surface_origin);
    unimplemented!("implemented in Step 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> Size {
        Size { width, height }
    }

    fn point(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn edge_adjacent_top_places_below_bar_and_clamps_x() {
        let placement = edge_adjacent_position(
            Edge::Top,
            point(490, 30),
            size(150, 34),
            size(500, 48),
            48,
            0,
            CrossAxisAnchor::Start,
        );
        assert_eq!(placement.position, point(350, 48));
    }

    #[test]
    fn edge_adjacent_bottom_places_above_bar() {
        let placement = edge_adjacent_position(
            Edge::Bottom,
            point(490, 30),
            size(150, 34),
            size(500, 48),
            48,
            0,
            CrossAxisAnchor::Start,
        );
        assert_eq!(placement.position, point(350, -34));
    }

    #[test]
    fn edge_adjacent_left_places_right_of_bar_and_clamps_y() {
        let placement = edge_adjacent_position(
            Edge::Left,
            point(20, 2000),
            size(150, 34),
            size(48, 1080),
            48,
            0,
            CrossAxisAnchor::Start,
        );
        assert_eq!(placement.position, point(48, 1046));
    }

    #[test]
    fn edge_adjacent_right_places_left_of_bar() {
        let placement = edge_adjacent_position(
            Edge::Right,
            point(20, 2000),
            size(150, 34),
            size(48, 1080),
            48,
            0,
            CrossAxisAnchor::Start,
        );
        assert_eq!(placement.position, point(-150, 1046));
    }

    #[test]
    fn edge_adjacent_zero_screen_keeps_unclamped_anchor() {
        let placement = edge_adjacent_position(
            Edge::Top,
            point(490, 30),
            size(150, 34),
            size(0, 0),
            48,
            0,
            CrossAxisAnchor::Start,
        );
        assert_eq!(placement.position, point(490, 48));
    }

    #[test]
    fn edge_adjacent_gap_offsets_away_from_bar() {
        let placement = edge_adjacent_position(
            Edge::Top,
            point(24, 500),
            size(64, 34),
            size(48, 1080),
            48,
            4,
            CrossAxisAnchor::Center,
        );
        assert_eq!(placement.position, point(0, 52));
    }

    #[test]
    fn layer_anchor_matches_existing_tooltip_anchor_rules() {
        assert_eq!(layer_anchor(Edge::Top), LayerAnchor::TopLeft);
        assert_eq!(layer_anchor(Edge::Bottom), LayerAnchor::BottomLeft);
        assert_eq!(layer_anchor(Edge::Left), LayerAnchor::LeftTop);
        assert_eq!(layer_anchor(Edge::Right), LayerAnchor::RightTop);
    }

    #[test]
    fn layer_margin_for_right_edge_matches_existing_tooltip_margin() {
        let placement = edge_adjacent_position(
            Edge::Right,
            point(24, 500),
            size(64, 34),
            size(48, 1080),
            48,
            4,
            CrossAxisAnchor::Center,
        );
        assert_eq!(
            placement.layer_margin(Edge::Right, size(64, 34), size(48, 1080)),
            (483, 52, 0, 0)
        );
    }

    #[test]
    fn centered_panel_top_centers_on_anchor_and_clamps_x() {
        let placement = centered_panel_position(
            Edge::Top,
            Some(Rect {
                x: 480,
                y: 4,
                width: 40,
                height: 40,
            }),
            size(300, 600),
            size(1920, 1080),
            48,
            point(0, 0),
        );
        assert_eq!(placement.position, point(350, 48));
    }

    #[test]
    fn centered_panel_right_matches_existing_negative_x_rule() {
        let placement = centered_panel_position(
            Edge::Right,
            Some(Rect {
                x: 0,
                y: 300,
                width: 48,
                height: 48,
            }),
            size(300, 600),
            size(1920, 1080),
            48,
            point(0, 0),
        );
        assert_eq!(placement.position, point(0, 24));
    }

    #[test]
    fn centered_panel_uses_surface_origin_for_clamping() {
        let placement = centered_panel_position(
            Edge::Left,
            Some(Rect {
                x: 0,
                y: 0,
                width: 48,
                height: 48,
            }),
            size(300, 600),
            size(1920, 1080),
            48,
            point(0, 24),
        );
        assert_eq!(placement.position, point(48, 24));
    }
}
```

- [ ] **Step 2: Expose the module and run the failing tests**

Modify `crates/niribar/src/surface.rs`:

```rust
pub mod frame;
pub mod input;
pub mod layout;
pub mod menu_popup;
pub mod panel;
pub mod placement;
pub mod tooltip_popup;
```

Run: `cargo test surface::placement -- --nocapture`

Expected: FAIL because the new placement functions call `unimplemented!`.

- [ ] **Step 3: Implement the pure placement functions**

Replace the function bodies in `crates/niribar/src/surface/placement.rs` with:

```rust
fn clamp_axis(value: i32, viewport: u32, overlay: u32) -> i32 {
    if viewport > 0 {
        value.clamp(0, (viewport as i32 - overlay as i32).max(0))
    } else {
        value
    }
}

impl Placement {
    pub fn layer_margin(self, edge: Edge, overlay: Size, screen: Size) -> (i32, i32, i32, i32) {
        let x = self.position.x;
        let y = self.position.y;
        let overlay_w = overlay.width as i32;
        let overlay_h = overlay.height as i32;
        let screen_w = screen.width as i32;
        let screen_h = screen.height as i32;

        match edge {
            Edge::Top => (y, 0, 0, x),
            Edge::Bottom => (0, 0, (screen_h - y - overlay_h).max(0), x),
            Edge::Left => (y, 0, 0, x),
            Edge::Right => (y, (screen_w - x - overlay_w).max(0), 0, 0),
        }
    }
}

pub fn layer_anchor(edge: Edge) -> LayerAnchor {
    match edge {
        Edge::Top => LayerAnchor::TopLeft,
        Edge::Bottom => LayerAnchor::BottomLeft,
        Edge::Left => LayerAnchor::LeftTop,
        Edge::Right => LayerAnchor::RightTop,
    }
}

pub fn edge_adjacent_position(
    edge: Edge,
    anchor: Point,
    overlay: Size,
    screen: Size,
    bar_thickness: u32,
    gap: i32,
    cross_axis_anchor: CrossAxisAnchor,
) -> Placement {
    let bar_thickness = bar_thickness as i32;
    let overlay_w = overlay.width as i32;
    let overlay_h = overlay.height as i32;
    let raw_cross_x = match cross_axis_anchor {
        CrossAxisAnchor::Start => anchor.x,
        CrossAxisAnchor::Center => anchor.x - overlay_w / 2,
    };
    let raw_cross_y = match cross_axis_anchor {
        CrossAxisAnchor::Start => anchor.y,
        CrossAxisAnchor::Center => anchor.y - overlay_h / 2,
    };
    let cross_x = clamp_axis(raw_cross_x, screen.width, overlay.width);
    let cross_y = clamp_axis(raw_cross_y, screen.height, overlay.height);

    let position = match edge {
        Edge::Top => Point {
            x: cross_x,
            y: bar_thickness + gap,
        },
        Edge::Bottom => Point {
            x: cross_x,
            y: -overlay_h - gap,
        },
        Edge::Left => Point {
            x: bar_thickness + gap,
            y: cross_y,
        },
        Edge::Right => Point {
            x: -overlay_w - gap,
            y: cross_y,
        },
    };
    Placement { position }
}

pub fn centered_panel_position(
    edge: Edge,
    anchor: Option<Rect>,
    overlay: Size,
    screen: Size,
    bar_thickness: u32,
    surface_origin: Point,
) -> Placement {
    let bar_thickness = bar_thickness as i32;
    let overlay_w = overlay.width as i32;
    let overlay_h = overlay.height as i32;
    let screen_w = screen.width as i32;
    let screen_h = screen.height as i32;
    let anchor = anchor.unwrap_or(Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    });
    let anchor_center_x = surface_origin.x + anchor.x + (anchor.width as i32 / 2);
    let anchor_center_y = surface_origin.y + anchor.y + (anchor.height as i32 / 2);
    let min_x = surface_origin.x;
    let min_y = surface_origin.y;
    let max_x = (surface_origin.x + screen_w - overlay_w).max(min_x);
    let max_y = (surface_origin.y + screen_h - overlay_h).max(min_y);

    let position = match edge {
        Edge::Left => Point {
            x: bar_thickness,
            y: (anchor_center_y - overlay_h / 2).clamp(min_y, max_y),
        },
        Edge::Right => Point {
            x: (screen_w - bar_thickness - overlay_w).min(0),
            y: (anchor_center_y - overlay_h / 2).clamp(min_y, max_y),
        },
        Edge::Top => Point {
            x: (anchor_center_x - overlay_w / 2).clamp(min_x, max_x),
            y: bar_thickness,
        },
        Edge::Bottom => Point {
            x: (anchor_center_x - overlay_w / 2).clamp(min_x, max_x),
            y: (screen_h - bar_thickness - overlay_h).min(0),
        },
    };
    Placement { position }
}
```

- [ ] **Step 4: Run placement tests**

Run: `cargo test surface::placement -- --nocapture`

Expected: PASS for all `surface::placement` tests.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add crates/niribar/src/surface.rs crates/niribar/src/surface/placement.rs
rtk git commit -m "feat: add shared surface placement module"
```

Expected: commit succeeds.

## Task 2: Move Menu Popup Placement to the Placement Module

**Files:**
- Modify: `crates/niribar/src/app/menu.rs`

- [ ] **Step 1: Add or update failing menu adapter tests**

In `crates/niribar/src/app/menu.rs`, add this test next to `bottom_edge_menu_popup_opens_above_parent_bar`:

```rust
#[test]
fn top_edge_menu_popup_uses_shared_placement_clamping() {
    let tree = crate::surface::menu_popup::MenuTree {
        items: vec![crate::surface::menu_popup::MenuItem {
            id: "1".into(),
            label: "Open".into(),
            enabled: true,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: false,
        }],
        anchor: (490, 30),
        screen_size: (500, 48),
    };
    let settings = super::menu_popup_window_settings(&crate::config::Edge::Top, 48, &tree);

    assert_eq!(settings.size, crate::surface::menu_popup::popup_size(&tree));
    assert_eq!(settings.position, (350, 48));
}
```

- [ ] **Step 2: Run the focused menu test**

Run: `cargo test app::menu::tests::top_edge_menu_popup_uses_shared_placement_clamping -- --nocapture`

Expected: PASS before the refactor. This is a characterization test for current behavior.

- [ ] **Step 3: Replace local menu placement math**

Replace `menu_popup_window_settings` in `crates/niribar/src/app/menu.rs` with:

```rust
fn menu_popup_window_settings(
    edge: &crate::config::Edge,
    bar_size: u32,
    tree: &crate::surface::menu_popup::MenuTree,
) -> IcedNewPopupSettings {
    let size = crate::surface::menu_popup::popup_size(tree);
    let placement = crate::surface::placement::edge_adjacent_position(
        edge.clone(),
        crate::surface::placement::Point {
            x: tree.anchor.0,
            y: tree.anchor.1,
        },
        crate::surface::placement::Size {
            width: size.0,
            height: size.1,
        },
        crate::surface::placement::Size {
            width: tree.screen_size.0,
            height: tree.screen_size.1,
        },
        bar_size,
        0,
        crate::surface::placement::CrossAxisAnchor::Start,
    );

    IcedNewPopupSettings {
        size,
        position: (placement.position.x, placement.position.y),
        grab: true,
    }
}
```

- [ ] **Step 4: Run menu and placement tests**

Run: `cargo test app::menu::tests surface::placement -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add crates/niribar/src/app/menu.rs
rtk git commit -m "refactor: use shared placement for menu popups"
```

Expected: commit succeeds.

## Task 3: Move Tooltip Placement to the Placement Module

**Files:**
- Modify: `crates/niribar/src/app/surfaces.rs`

- [ ] **Step 1: Add a characterization test for current tooltip sizing semantics**

In `crates/niribar/src/app/surfaces.rs`, add this test near the existing tooltip popup settings tests:

```rust
#[test]
fn tooltip_anchor_for_uses_screen_size_as_current_bar_size_input() {
    let mut app = test_app();
    let wid = test_wid();
    app.surfaces.surfaces.insert(
        wid,
        crate::surface::SurfaceState {
            output_name: "DP-1".into(),
            bar_name: "main".into(),
            screen_size: (1920, 1080),
            last_pointer_position: (500, 24),
            ..Default::default()
        },
    );

    let placement = app.tooltip_anchor_for(wid);

    assert_eq!(placement.bar_size, (1920, 1080));
    assert_eq!(placement.screen_size, (1920, 1080));
    assert_eq!(placement.anchor, (500, 24));
}
```

- [ ] **Step 2: Run the new tooltip characterization test**

Run: `cargo test app::surfaces::tests::tooltip_anchor_for_uses_screen_size_as_current_bar_size_input -- --nocapture`

Expected: PASS before the refactor. This locks behavior instead of fixing tooltip semantics in this pass.

- [ ] **Step 3: Add conversion helper for layer anchors**

In `crates/niribar/src/app/surfaces.rs`, replace `tooltip_anchor` with:

```rust
fn tooltip_anchor(tree: &crate::surface::tooltip_popup::TooltipTree) -> Anchor {
    match crate::surface::placement::layer_anchor(tree.bar_edge.clone()) {
        crate::surface::placement::LayerAnchor::TopLeft => Anchor::Left | Anchor::Top,
        crate::surface::placement::LayerAnchor::BottomLeft => Anchor::Left | Anchor::Bottom,
        crate::surface::placement::LayerAnchor::LeftTop => Anchor::Left | Anchor::Top,
        crate::surface::placement::LayerAnchor::RightTop => Anchor::Right | Anchor::Top,
    }
}
```

- [ ] **Step 4: Replace tooltip margin math with shared placement**

Replace `tooltip_margin` in `crates/niribar/src/app/surfaces.rs` with:

```rust
fn tooltip_margin(
    tree: &crate::surface::tooltip_popup::TooltipTree,
    size: (u32, u32),
    gap: i32,
) -> (i32, i32, i32, i32) {
    let placement = crate::surface::placement::edge_adjacent_position(
        tree.bar_edge.clone(),
        crate::surface::placement::Point {
            x: tree.anchor.0,
            y: tree.anchor.1,
        },
        crate::surface::placement::Size {
            width: size.0,
            height: size.1,
        },
        crate::surface::placement::Size {
            width: tree.bar_size.0,
            height: tree.bar_size.1,
        },
        match tree.bar_edge {
            crate::config::Edge::Top | crate::config::Edge::Bottom => tree.bar_size.1,
            crate::config::Edge::Left | crate::config::Edge::Right => tree.bar_size.0,
        },
        gap,
        crate::surface::placement::CrossAxisAnchor::Center,
    );
    placement.layer_margin(
        tree.bar_edge.clone(),
        crate::surface::placement::Size {
            width: size.0,
            height: size.1,
        },
        crate::surface::placement::Size {
            width: tree.bar_size.0,
            height: tree.bar_size.1,
        },
    )
}
```

- [ ] **Step 5: Run tooltip and placement tests**

Run: `cargo test app::surfaces::tests::tooltip surface::placement -- --nocapture`

Expected: PASS, including existing tests `tooltip_popup_window_is_sized_to_content_not_full_overlay` and `right_edge_tooltip_uses_right_anchor_margin`.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add crates/niribar/src/app/surfaces.rs
rtk git commit -m "refactor: use shared placement for tooltips"
```

Expected: commit succeeds.

## Task 4: Move Panel Placement to the Placement Module

**Files:**
- Modify: `crates/niribar/src/app/panel.rs`

- [ ] **Step 1: Add a panel placement characterization test**

In `crates/niribar/src/app/panel.rs`, add this test near the existing `panel_position_for_test` tests:

```rust
#[test]
fn panel_position_top_centers_on_anchor_and_clamps() {
    let position = super::panel_position_for_test(
        crate::config::Edge::Top,
        48,
        (300, 600),
        Some(iced::Rectangle {
            x: 480.0,
            y: 4.0,
            width: 40.0,
            height: 40.0,
        }),
        (1920, 1080),
        (0, 0),
    );

    assert_eq!(position, (350, 48));
}
```

- [ ] **Step 2: Run the panel characterization test**

Run: `cargo test app::panel::tests::panel_position_top_centers_on_anchor_and_clamps -- --nocapture`

Expected: PASS before the refactor.

- [ ] **Step 3: Replace panel position implementation**

Replace the body of `panel_position` in `crates/niribar/src/app/panel.rs` with:

```rust
fn panel_position(
    edge: &crate::config::Edge,
    bar_size: u32,
    panel_size: (u32, u32),
    anchor: Option<iced::Rectangle>,
    screen_size: (u32, u32),
    surface_origin: (i32, i32),
) -> (i32, i32) {
    let anchor = anchor.map(|rect| crate::surface::placement::Rect {
        x: (rect.x + rect.width / 2.0).round() as i32,
        y: (rect.y + rect.height / 2.0).round() as i32,
        width: 0,
        height: 0,
    });
    let placement = crate::surface::placement::centered_panel_position(
        edge.clone(),
        anchor,
        crate::surface::placement::Size {
            width: panel_size.0,
            height: panel_size.1,
        },
        crate::surface::placement::Size {
            width: screen_size.0,
            height: screen_size.1,
        },
        bar_size,
        crate::surface::placement::Point {
            x: surface_origin.0,
            y: surface_origin.1,
        },
    );
    (placement.position.x, placement.position.y)
}
```

- [ ] **Step 4: Replace panel margin implementation**

Replace the body of `panel_margin` in `crates/niribar/src/app/panel.rs` with:

```rust
fn panel_margin(
    edge: &crate::config::Edge,
    position: (i32, i32),
    panel_size: (u32, u32),
    screen_size: (u32, u32),
) -> (i32, i32, i32, i32) {
    crate::surface::placement::Placement {
        position: crate::surface::placement::Point {
            x: position.0,
            y: position.1,
        },
    }
    .layer_margin(
        edge.clone(),
        crate::surface::placement::Size {
            width: panel_size.0,
            height: panel_size.1,
        },
        crate::surface::placement::Size {
            width: screen_size.0,
            height: screen_size.1,
        },
    )
}
```

- [ ] **Step 5: Run panel and placement tests**

Run: `cargo test app::panel::tests surface::placement -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add crates/niribar/src/app/panel.rs
rtk git commit -m "refactor: use shared placement for panels"
```

Expected: commit succeeds.

## Task 5: Make Tray Menu Anchors Edge-Aware

**Files:**
- Modify: `crates/niribar/src/app/tray.rs`

- [ ] **Step 1: Add focused tests for tray anchor selection**

In `crates/niribar/src/app/tray.rs`, add this helper and tests in the existing test module:

```rust
#[test]
fn tray_menu_anchor_for_top_edge_uses_icon_bottom_left() {
    let bounds = iced::Rectangle {
        x: 10.0,
        y: 4.0,
        width: 22.0,
        height: 18.0,
    };

    assert_eq!(
        super::tray_menu_anchor_for_bounds(crate::config::Edge::Top, bounds),
        (10, 22)
    );
}

#[test]
fn tray_menu_anchor_for_left_edge_uses_icon_top_right() {
    let bounds = iced::Rectangle {
        x: 10.0,
        y: 4.0,
        width: 22.0,
        height: 18.0,
    };

    assert_eq!(
        super::tray_menu_anchor_for_bounds(crate::config::Edge::Left, bounds),
        (32, 4)
    );
}
```

- [ ] **Step 2: Run the new tray anchor tests**

Run: `cargo test app::tray::tests::tray_menu_anchor_for -- --nocapture`

Expected: FAIL because `tray_menu_anchor_for_bounds` does not exist.

- [ ] **Step 3: Implement edge-aware tray anchor helper**

In `crates/niribar/src/app/tray.rs`, add this helper near the top-level app tray functions:

```rust
fn tray_menu_anchor_for_bounds(edge: crate::config::Edge, bounds: iced::Rectangle) -> (i32, i32) {
    match edge {
        crate::config::Edge::Top | crate::config::Edge::Bottom => {
            (bounds.x as i32, (bounds.y + bounds.height) as i32)
        }
        crate::config::Edge::Left | crate::config::Edge::Right => {
            ((bounds.x + bounds.width) as i32, bounds.y as i32)
        }
    }
}
```

- [ ] **Step 4: Use the helper in menu request handling**

In `handle_tray_menu_request_with_bounds`, replace the `Some(r)` branch:

```rust
Some(r) => {
    if screen_size.0 > screen_size.1 {
        (r.x as i32, (r.y + r.height) as i32)
    } else {
        ((r.x + r.width) as i32, r.y as i32)
    }
}
```

with:

```rust
Some(r) => {
    let edge = source_window
        .and_then(|w| self.surfaces.surface(w))
        .map(|surface| Self::bar_for_surface_name(&self.config.bars, &surface.bar_name).edge.clone())
        .unwrap_or_else(|| self.config.bars[0].edge.clone());
    tray_menu_anchor_for_bounds(edge, r)
}
```

- [ ] **Step 5: Run tray tests**

Run: `cargo test app::tray::tests::tray_menu_anchor_for -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add crates/niribar/src/app/tray.rs
rtk git commit -m "refactor: derive tray menu anchors from bar edge"
```

Expected: commit succeeds.

## Task 6: Full Verification

**Files:**
- Read-only verification of the repo.

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Expected: PASS. If clippy reports warnings from the changed files, fix only those warnings and rerun this command.

- [ ] **Step 3: Check worktree state**

Run: `git status --short`

Expected: only intentional changes are present. The pre-existing `crates/niribar/src/app/panel.rs` modification from before this plan may still appear if it was not part of the implementation branch; do not revert it without explicit user approval.

- [ ] **Step 4: Commit verification fixes if any were required**

If Step 2 required code changes, run:

```bash
rtk git add crates/niribar/src/surface/placement.rs crates/niribar/src/surface.rs crates/niribar/src/app/menu.rs crates/niribar/src/app/surfaces.rs crates/niribar/src/app/panel.rs crates/niribar/src/app/tray.rs
rtk git commit -m "fix: satisfy placement refactor checks"
```

Expected: commit succeeds when verification fixes were made. If no verification fixes were made, skip this step.

## Self-Review Notes

- Spec coverage: Tasks 1-4 implement the shared placement module and adapters for Menu, Tooltip, and Panel. Task 5 covers the tray anchor cleanup from the Anchor Resolution section. Task 6 covers test and clippy verification.
- Scope: The plan preserves behavior and explicitly defers the tooltip semantics fix.
- Type consistency: The plan consistently uses `Point`, `Size`, `Rect`, `Placement`, `LayerAnchor`, `edge_adjacent_position`, `centered_panel_position`, `layer_anchor`, and `layer_margin`.
