use crate::config::Edge;
use iced_layershell::reexport::Anchor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Placement<'a> {
    pub position: Point,
    pub size: Size,
    pub screen: Size,
    pub edge: &'a Edge,
}

impl Placement<'_> {
    pub fn layer_margin(self) -> (i32, i32, i32, i32) {
        let x = self.position.x;
        let y = self.position.y;
        let width = self.size.width as i32;
        let height = self.size.height as i32;
        let screen_w = self.screen.width as i32;
        let screen_h = self.screen.height as i32;

        match self.edge {
            Edge::Top => (y, 0, 0, x),
            Edge::Bottom => (0, 0, screen_h - y - height, x),
            Edge::Left => (y, 0, 0, x),
            Edge::Right => (y, screen_w - x - width, 0, 0),
        }
    }
}

fn clamp_axis(value: i32, surface_origin: i32, screen: i32, overlay: i32) -> i32 {
    if screen == 0 {
        value
    } else {
        let min = surface_origin.max(0);
        let max = (surface_origin + screen - overlay).max(min);
        value.clamp(min, max)
    }
}

fn get_placement(
    edge: &'_ Edge,
    anchor: Point,
    overlay: Size,
    screen: Size,
    surface_origin: Point,
    bar_thickness: u32,
    gap: i32,
) -> Placement<'_> {
    let bar_thickness = bar_thickness as i32;
    let overlay_w = overlay.width as i32;
    let overlay_h = overlay.height as i32;
    let screen_w = screen.width as i32;
    let screen_h = screen.height as i32;
    let raw_cross_x = surface_origin.x + anchor.x;
    let raw_cross_y = surface_origin.y + anchor.y;
    let cross_x = clamp_axis(raw_cross_x, surface_origin.x, screen_w, overlay_w);
    let cross_y = clamp_axis(raw_cross_y, surface_origin.y, screen_h, overlay_h);

    let position = match edge {
        Edge::Top => Point {
            x: cross_x,
            y: bar_thickness + gap,
        },
        Edge::Bottom => Point {
            x: cross_x,
            y: screen_h - bar_thickness - overlay_h - gap,
        },
        Edge::Left => Point {
            x: bar_thickness + gap,
            y: cross_y,
        },
        Edge::Right => Point {
            x: screen_w - bar_thickness - overlay_w - gap,
            y: cross_y,
        },
    };
    Placement {
        position,
        size: overlay,
        screen,
        edge,
    }
}

pub fn layer_anchor(edge: &Edge) -> Anchor {
    match edge {
        Edge::Top => Anchor::Left | Anchor::Top,
        Edge::Bottom => Anchor::Left | Anchor::Bottom,
        Edge::Left => Anchor::Left | Anchor::Top,
        Edge::Right => Anchor::Right | Anchor::Top,
    }
}

pub fn edge_adjacent_position(
    edge: &'_ Edge,
    anchor: Point,
    overlay: Size,
    screen: Size,
    surface_origin: Point,
    bar_thickness: u32,
    gap: i32,
) -> Placement<'_> {
    get_placement(
        edge,
        anchor,
        overlay,
        screen,
        surface_origin,
        bar_thickness,
        gap,
    )
}

pub fn centered_position(
    edge: &'_ Edge,
    anchor: Option<iced::Rectangle>,
    overlay: Size,
    screen: Size,
    surface_origin: Point,
    bar_thickness: u32,
    gap: i32,
) -> Placement<'_> {
    let centered_anchor = anchor
        .map(|rect| Point {
            x: (rect.x + (rect.width / 2.0) - (overlay.width as f32 / 2.0)) as i32,
            y: (rect.y + (rect.height / 2.0) - (overlay.height as f32 / 2.0)) as i32,
        })
        .unwrap_or_default();
    get_placement(
        edge,
        centered_anchor,
        overlay,
        screen,
        surface_origin,
        bar_thickness,
        gap,
    )
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
            &Edge::Top,
            point(490, 30),
            size(150, 34),
            size(500, 48),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(350, 48));
    }

    #[test]
    fn edge_adjacent_bottom_places_above_bar() {
        let placement = edge_adjacent_position(
            &Edge::Bottom,
            point(490, 30),
            size(150, 34),
            size(500, 48),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(350, -34));
    }

    #[test]
    fn edge_adjacent_left_places_right_of_bar_and_clamps_y() {
        let placement = edge_adjacent_position(
            &Edge::Left,
            point(20, 2000),
            size(150, 34),
            size(48, 1080),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(48, 1046));
    }

    #[test]
    fn edge_adjacent_right_places_left_of_bar() {
        let placement = edge_adjacent_position(
            &Edge::Right,
            point(20, 2000),
            size(150, 34),
            size(48, 1080),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(-150, 1046));
    }

    #[test]
    fn edge_adjacent_zero_screen_keeps_unclamped_anchor() {
        let placement = edge_adjacent_position(
            &Edge::Top,
            point(490, 30),
            size(150, 34),
            size(0, 0),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(490, 48));
    }

    #[test]
    fn edge_adjacent_gap_offsets_away_from_bar() {
        let placement = edge_adjacent_position(
            &Edge::Top,
            point(24, 500),
            size(64, 34),
            size(48, 1080),
            point(0, 0),
            48,
            4,
        );
        assert_eq!(placement.position, point(0, 52));
    }

    #[test]
    fn layer_margin_for_right_edge_is_distance_from_right_edge() {
        let placement = edge_adjacent_position(
            &Edge::Right,
            point(24, 500),
            size(64, 34),
            size(1920, 1080),
            point(0, 0),
            48,
            4,
        );
        assert_eq!(placement.position, point(1804, 500));
        assert_eq!(placement.layer_margin(), (500, 52, 0, 0));
    }

    #[test]
    fn layer_margin_for_bottom_edge_is_distance_from_bottom_edge() {
        let placement = edge_adjacent_position(
            &Edge::Bottom,
            point(490, 30),
            size(150, 34),
            size(500, 800),
            point(0, 0),
            48,
            4,
        );
        assert_eq!(placement.position, point(350, 714));
        assert_eq!(placement.layer_margin(), (0, 0, 52, 350));
    }
    #[test]
    fn centered_panel_top_centers_on_anchor_and_clamps_x() {
        let placement = centered_position(
            &Edge::Top,
            Some(iced::Rectangle {
                x: 480.0,
                y: 4.0,
                width: 40.0,
                height: 40.0,
            }),
            size(300, 600),
            size(1920, 1080),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(350, 48));
    }

    #[test]
    fn centered_panel_right_matches_existing_in_screen_rule() {
        let placement = centered_position(
            &Edge::Right,
            Some(iced::Rectangle {
                x: 0.0,
                y: 300.0,
                width: 48.0,
                height: 48.0,
            }),
            size(300, 600),
            size(1920, 1080),
            point(0, 0),
            48,
            0,
        );
        assert_eq!(placement.position, point(1572, 24));
    }

    #[test]
    fn centered_panel_uses_surface_origin_for_clamping() {
        let placement = centered_position(
            &Edge::Left,
            Some(iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 48.0,
                height: 48.0,
            }),
            size(300, 600),
            size(1920, 1080),
            point(0, 24),
            48,
            0,
        );
        assert_eq!(placement.position, point(48, 24));
    }
}
