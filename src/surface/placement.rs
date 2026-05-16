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

fn clamp_axis(value: i32, viewport: u32, overlay: u32) -> i32 {
    if viewport > 0 {
        value.clamp(0, (viewport as i32 - overlay as i32).max(0))
    } else {
        value
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
            x: (screen_w - bar_thickness - overlay_w).max(0),
            y: (anchor_center_y - overlay_h / 2).clamp(min_y, max_y),
        },
        Edge::Top => Point {
            x: (anchor_center_x - overlay_w / 2).clamp(min_x, max_x),
            y: bar_thickness,
        },
        Edge::Bottom => Point {
            x: (anchor_center_x - overlay_w / 2).clamp(min_x, max_x),
            y: (screen_h - bar_thickness - overlay_h).max(0),
        },
    };
    Placement { position }
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
    fn centered_panel_right_matches_existing_in_screen_rule() {
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
        assert_eq!(placement.position, point(1572, 24));
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
