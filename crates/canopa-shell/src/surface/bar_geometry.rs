//! One geometry result drives rendering and the compositor input region. The host
//! spans the edge so all alignments retain layer-shell's exclusive-edge semantics.
use crate::config::{BarAlignment, BarConfig, BarLength, Edge};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarGeometry {
    pub frame: iced::Rectangle,
}

impl BarGeometry {
    pub fn host_size(bar: &BarConfig, drawer: u32) -> (u32, u32) {
        let (near, _, _, _) = bar.outer_margins();
        let cross = near
            .saturating_add(bar.collapsed_width)
            .saturating_add(drawer);
        if bar.edge.is_horizontal() {
            (0, cross)
        } else {
            (cross, 0)
        }
    }
    pub fn reservation(bar: &BarConfig) -> i32 {
        let (near, inner, _, _) = bar.outer_margins();
        if bar.reserves_space() {
            (near + bar.collapsed_width + inner) as i32
        } else {
            0
        }
    }
    pub fn resolve(bar: &BarConfig, host: (u32, u32), drawer: u32) -> Self {
        let horizontal = bar.edge.is_horizontal();
        let span = if horizontal { host.0 } else { host.1 }.max(1) as f32;
        let (near, _, start, end) = bar.outer_margins();
        // Preserve at least one logical pixel, even after an output becomes smaller.
        let start = (start as f32).min(span - 1.0);
        let end = (end as f32).min(span - start - 1.0);
        let available = (span - start - end).max(1.0);
        let length = match bar.length {
            BarLength::Fill => available,
            BarLength::Pixels(n) => (n as f32).min(available),
            BarLength::Percent(n) => (available * n as f32 / 10000.0).max(1.0),
        };
        let offset = start
            + match bar.align {
                BarAlignment::Start => 0.0,
                BarAlignment::Center => (available - length) / 2.0,
                BarAlignment::End => available - length,
            };
        let host_cross = if horizontal { host.1 } else { host.0 }.max(1) as f32;
        let thickness = (bar.collapsed_width as f32).min(host_cross);
        let cross = if matches!(bar.edge, Edge::Top | Edge::Left) {
            ((near + drawer) as f32).min((host_cross - thickness).max(0.0))
        } else {
            0.0
        };
        Self {
            frame: if horizontal {
                iced::Rectangle {
                    x: offset,
                    y: cross,
                    width: length,
                    height: thickness,
                }
            } else {
                iced::Rectangle {
                    x: cross,
                    y: offset,
                    width: thickness,
                    height: length,
                }
            },
        }
    }

    /// The thin strip along the screen edge, beside the bar, that wakes a
    /// hidden bar. `host` is the bar surface's logical size.
    pub fn edge_strip(self, edge: &Edge, host: (u32, u32), strip: i32) -> (i32, i32, i32, i32) {
        let b = self.frame;
        let (along_start, along_len) = if edge.is_horizontal() {
            (b.x.floor() as i32, b.width.ceil() as i32)
        } else {
            (b.y.floor() as i32, b.height.ceil() as i32)
        };
        match edge {
            Edge::Left => (0, along_start, strip, along_len),
            Edge::Right => (host.0 as i32 - strip, along_start, strip, along_len),
            Edge::Top => (along_start, 0, along_len, strip),
            Edge::Bottom => (along_start, host.1 as i32 - strip, along_len, strip),
        }
    }

    /// The space between the screen edge and a revealed bar, so the pointer
    /// does not leave the bar surface on its way from the edge to the bar.
    pub fn edge_band(self, edge: &Edge, host: (u32, u32)) -> Option<(i32, i32, i32, i32)> {
        let b = self.frame;
        let rect = match edge {
            Edge::Left => (0, b.y as i32, b.x.ceil() as i32, b.height as i32),
            Edge::Right => {
                let x = (b.x + b.width).floor() as i32;
                (x, b.y as i32, host.0 as i32 - x, b.height as i32)
            }
            Edge::Top => (b.x as i32, 0, b.width as i32, b.y.ceil() as i32),
            Edge::Bottom => {
                let y = (b.y + b.height).floor() as i32;
                (b.x as i32, y, b.width as i32, host.1 as i32 - y)
            }
        };
        (rect.2 > 0 && rect.3 > 0).then_some(rect)
    }

    /// Rounded rows are coalesced into rectangles; transparent corners and unused
    /// host space never catch pointer events intended for the desktop underneath.
    pub fn input_rectangles(self, radius: [canopa_theme::Pixels; 4]) -> Vec<(i32, i32, i32, i32)> {
        let b = self.frame;
        let r = radius.map(|r| r.px().min(b.width / 2.0).min(b.height / 2.0));
        let inset = |radius: f32, distance: f32| {
            if distance >= radius || radius == 0.0 {
                0.0
            } else {
                radius
                    - (radius * radius - (radius - distance).powi(2))
                        .max(0.0)
                        .sqrt()
            }
        };
        let mut rectangles: Vec<(i32, i32, i32, i32)> = Vec::new();
        for row in 0..b.height.ceil() as i32 {
            let y = row as f32 + 0.5;
            let left = inset(r[0], y).max(inset(r[3], b.height - y));
            let right = inset(r[1], y).max(inset(r[2], b.height - y));
            let x = (b.x + left).ceil() as i32;
            let end = (b.x + b.width - right).floor() as i32;
            let width = (end - x).max(0);
            if width == 0 {
                continue;
            }
            let y = b.y.floor() as i32 + row;
            if let Some(last) = rectangles.last_mut()
                && last.0 == x
                && last.2 == width
                && last.1 + last.3 == y
            {
                last.3 += 1;
            } else {
                rectangles.push((x, y, width, 1));
            }
        }
        rectangles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_bar_alignment_margins_reservation_and_input_agree() {
        let mut bar = BarConfig {
            edge: Edge::Top,
            collapsed_width: 40,
            length: BarLength::Pixels(600),
            margin: canopa_theme::Insets {
                top: canopa_theme::Pixels::whole(10),
                bottom: canopa_theme::Pixels::whole(4),
                left: canopa_theme::Pixels::whole(20),
                right: canopa_theme::Pixels::whole(40),
            },
            ..Default::default()
        };
        assert_eq!(BarGeometry::host_size(&bar, 0), (0, 50));
        assert_eq!(BarGeometry::reservation(&bar), 54);
        for (align, x) in [
            (BarAlignment::Start, 20.0),
            (BarAlignment::Center, 190.0),
            (BarAlignment::End, 360.0),
        ] {
            bar.align = align;
            let g = BarGeometry::resolve(&bar, (1000, 50), 0);
            assert_eq!(
                g.frame,
                iced::Rectangle {
                    x,
                    y: 10.0,
                    width: 600.0,
                    height: 40.0
                }
            );
            let input = g.input_rectangles([canopa_theme::Pixels::whole(10); 4]);
            assert!(input.iter().all(|&(ix, iy, w, h)| ix >= x as i32
                && iy >= 10
                && ix + w <= (x + 600.0) as i32
                && iy + h <= 50));
            assert!(input[0].0 > x as i32);
        }
    }
    #[test]
    fn auto_hide_reserves_nothing_and_wakes_from_a_strip_on_the_edge() {
        let mut bar = BarConfig {
            collapsed_width: 64,
            margin: canopa_theme::Insets {
                top: canopa_theme::Pixels::whole(0),
                bottom: canopa_theme::Pixels::whole(16),
                left: canopa_theme::Pixels::whole(12),
                right: canopa_theme::Pixels::whole(12),
            },
            ..Default::default()
        };
        assert_eq!(BarGeometry::reservation(&bar), 88);
        bar.visibility = crate::config::BarVisibility::AutoHide;
        assert_eq!(BarGeometry::reservation(&bar), 0);
        let host = BarGeometry::host_size(&bar, 0);
        let g = BarGeometry::resolve(&bar, (host.0, 800), 0);
        assert_eq!(g.edge_strip(&Edge::Left, (76, 800), 2), (0, 0, 2, 784));
        assert_eq!(g.edge_band(&Edge::Left, (76, 800)), Some((0, 0, 12, 784)));
        bar.edge = Edge::Bottom;
        let g = BarGeometry::resolve(&bar, (1000, 76), 0);
        assert_eq!(g.edge_strip(&Edge::Bottom, (1000, 76), 2).1, 74);
        let band = g.edge_band(&Edge::Bottom, (1000, 76)).unwrap();
        assert_eq!(band.1 + band.3, 76);
    }

    #[test]
    fn all_edges_fit_small_hosts_and_percentages() {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let mut bar = BarConfig {
                edge,
                length: BarLength::Percent(8000),
                ..Default::default()
            };
            let host = if bar.edge.is_horizontal() {
                (100, 48)
            } else {
                (48, 100)
            };
            let g = BarGeometry::resolve(&bar, host, 0);
            assert_eq!(
                if bar.edge.is_horizontal() {
                    g.frame.width
                } else {
                    g.frame.height
                },
                80.0
            );
            bar.margin = canopa_theme::Insets::all(4096);
            let g = BarGeometry::resolve(&bar, host, 0);
            assert_eq!(
                if bar.edge.is_horizontal() {
                    g.frame.width
                } else {
                    g.frame.height
                },
                1.0
            );
        }
    }
}
