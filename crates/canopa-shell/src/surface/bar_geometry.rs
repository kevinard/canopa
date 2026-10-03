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
        if bar.reserve_space {
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
