//! The geometry of the display arrangement: how the drawing fits the
//! displays, where an edge snaps while a display is dragged, and where a
//! dropped display settles so the compositor can place every display as
//! drawn.

/// The drawing's size.
pub(super) const WIDTH: f32 = 560.0;
pub(super) const HEIGHT: f32 = 240.0;
/// How far, in drawing pixels, an edge reaches for a neighbour's edge.
pub(super) const REACH: f32 = 14.0;

/// A display's place in logical pixels, its size rounded as the compositor
/// rounds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Screen {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl Screen {
    fn right(&self) -> f32 {
        self.x + self.width
    }
    fn bottom(&self) -> f32 {
        self.y + self.height
    }
    /// Whether the two share any area.
    fn overlaps(&self, other: &Self) -> bool {
        self.x < other.right() - 0.5
            && other.x < self.right() - 0.5
            && self.y < other.bottom() - 0.5
            && other.y < self.bottom() - 0.5
    }
    /// Whether the two share a length of edge, so the pointer can cross.
    fn touches(&self, other: &Self) -> bool {
        let across = |a: f32, b: f32| (a - b).abs() < 0.5;
        let side = (across(self.right(), other.x) || across(other.right(), self.x))
            && self.y < other.bottom() - 0.5
            && other.y < self.bottom() - 0.5;
        let stacked = (across(self.bottom(), other.y) || across(other.bottom(), self.y))
            && self.x < other.right() - 0.5
            && other.x < self.right() - 0.5;
        side || stacked
    }
    fn rounded(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            ..self
        }
    }
}

/// The drawing's scale. It depends only on the displays' sizes, so it stays
/// the same however they are arranged: there is room for all of them side
/// by side with one more to spare, or stacked.
pub(super) fn fit(screens: &[Screen]) -> f32 {
    let widest = screens.iter().map(|s| s.width).fold(0.0, f32::max);
    let width: f32 = screens.iter().map(|s| s.width).sum::<f32>() + widest;
    let height: f32 = screens.iter().map(|s| s.height).sum();
    (WIDTH / width.max(1.0)).min(HEIGHT / height.max(1.0))
}

/// Where the arrangement's logical origin lands in the drawing so it is
/// centred.
pub(super) fn origin(screens: &[Screen], fit: f32) -> (f32, f32) {
    let left = screens.iter().map(|s| s.x).fold(f32::MAX, f32::min);
    let top = screens.iter().map(|s| s.y).fold(f32::MAX, f32::min);
    let right = screens.iter().map(Screen::right).fold(f32::MIN, f32::max);
    let bottom = screens.iter().map(Screen::bottom).fold(f32::MIN, f32::max);
    (
        (WIDTH - (right - left) * fit) / 2.0 - left * fit,
        (HEIGHT - (bottom - top) * fit) / 2.0 - top * fit,
    )
}

/// `moving` with its edges pulled onto the nearest edges of `others` that
/// are within `reach` logical pixels.
pub(super) fn snap(moving: Screen, others: &[Screen], reach: f32) -> Screen {
    let nearest = |pairs: &mut dyn Iterator<Item = (f32, f32)>| {
        pairs
            .map(|(edge, target)| target - edge)
            .filter(|shift| shift.abs() < reach)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(0.0)
    };
    let dx = nearest(&mut others.iter().flat_map(|o| {
        [
            (moving.x, o.x),
            (moving.x, o.right()),
            (moving.right(), o.x),
            (moving.right(), o.right()),
        ]
    }));
    let dy = nearest(&mut others.iter().flat_map(|o| {
        [
            (moving.y, o.y),
            (moving.y, o.bottom()),
            (moving.bottom(), o.y),
            (moving.bottom(), o.bottom()),
        ]
    }));
    Screen {
        x: moving.x + dx,
        y: moving.y + dy,
        ..moving
    }
}

/// The place nearest `moving` where it touches one of `neighbours` and
/// overlaps none of `obstacles`.
fn settle(moving: Screen, neighbours: &[Screen], obstacles: &[Screen]) -> Screen {
    let moving = moving.rounded();
    let free = |candidate: &Screen| obstacles.iter().all(|o| !candidate.overlaps(o));
    if neighbours.is_empty() || (free(&moving) && neighbours.iter().any(|n| moving.touches(n))) {
        return moving;
    }
    let at = |x: f32, y: f32| Screen { x, y, ..moving };
    neighbours
        .iter()
        .flat_map(|n| {
            // Slide along the neighbour's side, keeping some edge shared.
            let y = moving.y.clamp(n.y - moving.height + 1.0, n.bottom() - 1.0);
            let x = moving.x.clamp(n.x - moving.width + 1.0, n.right() - 1.0);
            [
                at(n.right(), y),
                at(n.x - moving.width, y),
                at(x, n.bottom()),
                at(x, n.y - moving.height),
            ]
        })
        .filter(free)
        .min_by(|a, b| {
            let distance = |c: &Screen| (c.x - moving.x).powi(2) + (c.y - moving.y).powi(2);
            distance(a).total_cmp(&distance(b))
        })
        .unwrap_or_else(|| {
            // Everything around is taken: after the rightmost display.
            let right = obstacles.iter().map(Screen::right).fold(0.0, f32::max);
            let top = neighbours.iter().map(|n| n.y).fold(f32::MAX, f32::min);
            at(right, top)
        })
}

/// Settle the display at `dropped` against the others, then join any
/// display it left apart back to the group, so every display touches
/// another and none overlap. Finally the arrangement starts at 0, 0.
pub(super) fn tidy(screens: &mut [Screen], dropped: usize) {
    if screens.len() > 1 {
        let others: Vec<_> = (0..screens.len())
            .filter(|&i| i != dropped)
            .map(|i| screens[i].rounded())
            .collect();
        screens[dropped] = settle(screens[dropped], &others, &others);
        let mut joined = vec![dropped];
        while joined.len() < screens.len() {
            let apart: Vec<usize> = (0..screens.len()).filter(|i| !joined.contains(i)).collect();
            if let Some(&next) = apart
                .iter()
                .find(|&&i| joined.iter().any(|&j| screens[i].touches(&screens[j])))
            {
                joined.push(next);
                continue;
            }
            // The display nearest the group moves to touch it.
            let gap = |i: usize| {
                joined
                    .iter()
                    .map(|&j| {
                        let (a, b) = (screens[i], screens[j]);
                        let dx = (b.x - a.right()).max(a.x - b.right()).max(0.0);
                        let dy = (b.y - a.bottom()).max(a.y - b.bottom()).max(0.0);
                        dx.hypot(dy)
                    })
                    .fold(f32::MAX, f32::min)
            };
            let next = *apart
                .iter()
                .min_by(|&&a, &&b| gap(a).total_cmp(&gap(b)))
                .expect("a display is apart");
            let neighbours: Vec<_> = joined.iter().map(|&j| screens[j]).collect();
            let obstacles: Vec<_> = (0..screens.len())
                .filter(|&i| i != next)
                .map(|i| screens[i])
                .collect();
            screens[next] = settle(screens[next], &neighbours, &obstacles);
            joined.push(next);
        }
    }
    normalize(screens);
}

/// Shift the arrangement so its top left corner is 0, 0.
pub(super) fn normalize(screens: &mut [Screen]) {
    let left = screens.iter().map(|s| s.x).fold(f32::MAX, f32::min);
    let top = screens.iter().map(|s| s.y).fold(f32::MAX, f32::min);
    if left == f32::MAX {
        return;
    }
    for screen in screens {
        screen.x = (screen.x - left).round();
        screen.y = (screen.y - top).round();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: f32, y: f32, width: f32, height: f32) -> Screen {
        Screen {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn edges_snap_to_the_nearest_neighbouring_edge_within_reach() {
        let left = screen(0., 0., 2560., 1440.);
        let snapped = snap(screen(2580., 30., 1920., 1080.), &[left], 100.);
        assert_eq!((snapped.x, snapped.y), (2560., 0.));
        let free = snap(screen(2800., 200., 1920., 1080.), &[left], 100.);
        assert_eq!((free.x, free.y), (2800., 200.));
    }

    #[test]
    fn a_display_dropped_over_another_settles_beside_it() {
        let mut screens = [
            screen(0., 0., 2560., 1440.),
            screen(1800., 200., 1920., 1080.),
        ];
        tidy(&mut screens, 1);
        assert_eq!(screens[1], screen(2560., 200., 1920., 1080.));
        assert!(!screens[0].overlaps(&screens[1]) && screens[0].touches(&screens[1]));
    }

    #[test]
    fn a_display_dropped_far_away_comes_back_to_touch_the_others() {
        let mut screens = [
            screen(0., 0., 1920., 1080.),
            screen(-500., -3000., 1920., 1080.),
        ];
        tidy(&mut screens, 1);
        // Above, sharing part of the top edge, and the whole starts at 0, 0.
        assert!(screens[0].touches(&screens[1]));
        assert_eq!(screens[1].y, 0.);
        assert_eq!(screens[0].y, 1080.);
        assert_eq!(screens.iter().map(|s| s.x).fold(f32::MAX, f32::min), 0.);
    }

    #[test]
    fn moving_the_middle_display_away_rejoins_the_one_left_apart() {
        let mut screens = [
            screen(0., 0., 1000., 500.),
            screen(1000., 0., 1000., 500.),
            screen(2000., 0., 1000., 500.),
        ];
        // The middle display goes below the first: the third is left apart.
        screens[1] = screen(0., 500., 1000., 500.);
        tidy(&mut screens, 1);
        for (i, a) in screens.iter().enumerate() {
            assert!(
                screens
                    .iter()
                    .enumerate()
                    .all(|(j, b)| i == j || !a.overlaps(b))
            );
            assert!(
                screens
                    .iter()
                    .enumerate()
                    .any(|(j, b)| i != j && a.touches(b))
            );
        }
    }

    #[test]
    fn the_scale_ignores_where_displays_are() {
        let side = [
            screen(0., 0., 2560., 1440.),
            screen(2560., 0., 1920., 1080.),
        ];
        let stacked = [
            screen(0., 0., 2560., 1440.),
            screen(0., 1440., 1920., 1080.),
        ];
        assert_eq!(fit(&side), fit(&stacked));
        // Both arrangements fit the drawing.
        for screens in [side, stacked] {
            let fit = fit(&screens);
            let (x, y) = origin(&screens, fit);
            for s in screens {
                assert!(x + s.x * fit >= 0. && x + s.right() * fit <= WIDTH);
                assert!(y + s.y * fit >= 0. && y + s.bottom() * fit <= HEIGHT);
            }
        }
    }
}
