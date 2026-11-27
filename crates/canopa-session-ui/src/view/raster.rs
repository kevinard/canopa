//! Software drawing for the shared authentication panel and default session backdrop.
use crate::background;
use canopa_theme::{Appearance, ColorRole, ColorScheme, Rgb};
use image::RgbImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emblem {
    Canopa,
    Lock,
    Shield,
    Key,
    Cycle,
    Chevron,
    /// A chevron pointing back, toward the start of a left-to-right line.
    ChevronBack,
    Accounts,
    Check,
    Display,
    Keyboard,
    /// One person, for an account without a picture.
    Person,
    Power,
    /// An arrow toward the end of a left-to-right line (submit).
    Arrow,
    ArrowBack,
    /// A chevron pointing down (opens a menu).
    ChevronDown,
    Battery,
    /// Show the typed secret.
    Eye,
    /// Mask the typed secret again.
    EyeOff,
    /// A circular arrow (restart).
    Restart,
    /// Caps Lock is engaged.
    CapsLock,
    /// A notice in a circle (errors).
    Alert,
    /// A crescent (suspend).
    Moon,
    /// A circle with a check mark (selected choice).
    Selected,
    /// Out-going arrow from a door (log out, switch user).
    Exit,
    /// A full circle: the track under a spinner.
    Ring,
    /// A quarter arc that turns while waiting.
    Spinner,
    /// A ring buoy (troubleshooting sessions).
    Lifebuoy,
}

fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    let steps = (((to - from).abs() / 12.).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / steps as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Paths drawn from arcs, in the 24-unit emblem grid.
fn dynamic_paths(emblem: Emblem) -> Option<Vec<Vec<(f32, f32)>>> {
    Some(match emblem {
        Emblem::Restart => vec![
            arc(12., 12., 7.5, -60., 250.),
            vec![(15.2, 2.9), (15.75, 5.5), (13.2, 6.4)],
        ],
        Emblem::CapsLock => vec![
            vec![
                (12., 4.),
                (19., 11.),
                (15.5, 11.),
                (15.5, 15.),
                (8.5, 15.),
                (8.5, 11.),
                (5., 11.),
                (12., 4.),
            ],
            vec![(8.5, 19.), (15.5, 19.)],
        ],
        Emblem::Alert => vec![
            arc(12., 12., 9., 0., 360.),
            vec![(12., 7.5), (12., 13.)],
            vec![(12., 16.4), (12., 16.6)],
        ],
        Emblem::Moon => {
            let mut path = arc(12., 12., 8.5, 55., 335.);
            let mut inner = arc(15.2, 8.6, 7., 135., 252.);
            inner.reverse();
            path.extend(inner);
            vec![path]
        }
        Emblem::Selected => vec![
            arc(12., 12., 9., 0., 360.),
            vec![(8., 12.2), (10.8, 15.), (16.2, 9.4)],
        ],
        Emblem::Exit => vec![
            vec![(10., 4.), (5., 4.), (5., 20.), (10., 20.)],
            vec![(9., 12.), (20., 12.)],
            vec![(16., 8.), (20., 12.), (16., 16.)],
        ],
        Emblem::Ring => vec![arc(12., 12., 9., 0., 360.)],
        Emblem::Lifebuoy => vec![
            arc(12., 12., 9., 0., 360.),
            arc(12., 12., 4., 0., 360.),
            vec![(5.6, 5.6), (9.2, 9.2)],
            vec![(18.4, 5.6), (14.8, 9.2)],
            vec![(5.6, 18.4), (9.2, 14.8)],
            vec![(18.4, 18.4), (14.8, 14.8)],
        ],
        Emblem::Spinner => vec![arc(12., 12., 9., -90., 10.)],
        _ => return None,
    })
}

pub(super) fn blend(target: &mut image::Rgb<u8>, color: Rgb, alpha: f32) {
    let Rgb(r, g, b) = mix(Rgb(target[0], target[1], target[2]), color, alpha);
    *target = image::Rgb([r, g, b]);
}

pub(super) fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let channel = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Rgb(channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

pub(super) fn emblem(
    image: &mut RgbImage,
    (x, y): (f32, f32),
    scale: f32,
    emblem: Emblem,
    color: Rgb,
) {
    let owned = dynamic_paths(emblem);
    // Progress strokes are heavier so they read at small sizes.
    let half_width = if matches!(emblem, Emblem::Spinner | Emblem::Ring) {
        1.5
    } else {
        0.95
    };
    let paths: &[&[(f32, f32)]] = match emblem {
        _ if owned.is_some() => &[],
        Emblem::Restart
        | Emblem::CapsLock
        | Emblem::Alert
        | Emblem::Moon
        | Emblem::Selected
        | Emblem::Exit
        | Emblem::Ring
        | Emblem::Spinner
        | Emblem::Lifebuoy => &[],
        Emblem::Keyboard => &[
            &[(2., 6.), (22., 6.), (22., 18.), (2., 18.), (2., 6.)],
            &[(6., 10.), (7., 10.)],
            &[(11.5, 10.), (12.5, 10.)],
            &[(17., 10.), (18., 10.)],
            &[(7., 14.), (17., 14.)],
        ],
        Emblem::Canopa => &[
            &[
                (3., 13.),
                (5., 9.),
                (8., 6.),
                (12., 4.5),
                (16., 6.),
                (19., 9.),
                (21., 13.),
            ],
            &[(5., 17.), (8., 13.5), (12., 11.5), (16., 13.5), (19., 17.)],
            &[(12., 12.), (12., 20.)],
        ],
        Emblem::Lock => &[
            &[(5., 10.), (19., 10.), (19., 21.), (5., 21.), (5., 10.)],
            &[
                (8., 10.),
                (8., 6.),
                (9., 4.),
                (12., 3.),
                (15., 4.),
                (16., 6.),
                (16., 10.),
            ],
            &[(12., 14.), (12., 17.)],
        ],
        Emblem::Shield => &[
            &[
                (12., 2.),
                (20., 5.),
                (20., 12.),
                (18., 17.),
                (12., 22.),
                (6., 17.),
                (4., 12.),
                (4., 5.),
                (12., 2.),
            ],
            &[(8.5, 12.), (11., 14.5), (16., 9.)],
        ],
        Emblem::Chevron => &[&[(9., 6.), (15., 12.), (9., 18.)]],
        Emblem::ChevronBack => &[&[(15., 6.), (9., 12.), (15., 18.)]],
        Emblem::Check => &[&[(5., 12.), (10., 17.), (19., 7.)]],
        Emblem::ChevronDown => &[&[(6., 9.), (12., 15.), (18., 9.)]],
        Emblem::Eye => &[
            &[
                (2.0, 12.0),
                (4.0, 10.11),
                (6.0, 8.64),
                (8.0, 7.59),
                (10.0, 6.96),
                (12.0, 6.75),
                (14.0, 6.96),
                (16.0, 7.59),
                (18.0, 8.64),
                (20.0, 10.11),
                (22.0, 12.0),
            ],
            &[
                (2.0, 12.0),
                (4.0, 13.89),
                (6.0, 15.36),
                (8.0, 16.41),
                (10.0, 17.04),
                (12.0, 17.25),
                (14.0, 17.04),
                (16.0, 16.41),
                (18.0, 15.36),
                (20.0, 13.89),
                (22.0, 12.0),
            ],
            &[
                (15.0, 12.0),
                (14.7, 13.3),
                (13.87, 14.35),
                (12.67, 14.92),
                (11.33, 14.92),
                (10.13, 14.35),
                (9.3, 13.3),
                (9.0, 12.0),
                (9.3, 10.7),
                (10.13, 9.65),
                (11.33, 9.08),
                (12.67, 9.08),
                (13.87, 9.65),
                (14.7, 10.7),
                (15.0, 12.0),
            ],
        ],
        Emblem::EyeOff => &[
            &[
                (2.0, 12.0),
                (4.0, 10.11),
                (6.0, 8.64),
                (8.0, 7.59),
                (10.0, 6.96),
                (12.0, 6.75),
                (14.0, 6.96),
                (16.0, 7.59),
                (18.0, 8.64),
                (20.0, 10.11),
                (22.0, 12.0),
            ],
            &[
                (2.0, 12.0),
                (4.0, 13.89),
                (6.0, 15.36),
                (8.0, 16.41),
                (10.0, 17.04),
                (12.0, 17.25),
                (14.0, 17.04),
                (16.0, 16.41),
                (18.0, 15.36),
                (20.0, 13.89),
                (22.0, 12.0),
            ],
            &[
                (15.0, 12.0),
                (14.7, 13.3),
                (13.87, 14.35),
                (12.67, 14.92),
                (11.33, 14.92),
                (10.13, 14.35),
                (9.3, 13.3),
                (9.0, 12.0),
                (9.3, 10.7),
                (10.13, 9.65),
                (11.33, 9.08),
                (12.67, 9.08),
                (13.87, 9.65),
                (14.7, 10.7),
                (15.0, 12.0),
            ],
            &[(4., 4.), (20., 20.)],
        ],
        Emblem::Battery => &[
            &[(3., 7.), (19., 7.), (19., 17.), (3., 17.), (3., 7.)],
            &[(21.5, 10.5), (21.5, 13.5)],
        ],
        Emblem::Arrow => &[
            &[(5., 12.), (19., 12.)],
            &[(13., 6.), (19., 12.), (13., 18.)],
        ],
        Emblem::ArrowBack => &[
            &[(19., 12.), (5., 12.)],
            &[(11., 6.), (5., 12.), (11., 18.)],
        ],
        Emblem::Power => &[
            &[(12., 3.5), (12., 11.)],
            &[
                (16.3, 6.86),
                (17.7, 8.13),
                (18.74, 9.71),
                (19.35, 11.5),
                (19.49, 13.39),
                (19.15, 15.26),
                (18.36, 16.97),
                (17.16, 18.44),
                (15.64, 19.56),
                (13.88, 20.26),
                (12.0, 20.5),
                (10.12, 20.26),
                (8.36, 19.56),
                (6.84, 18.44),
                (5.64, 16.97),
                (4.85, 15.26),
                (4.51, 13.39),
                (4.65, 11.5),
                (5.26, 9.71),
                (6.3, 8.13),
                (7.7, 6.86),
            ],
        ],
        Emblem::Person => &[
            &[
                (15.6, 8.0),
                (15.33, 9.38),
                (14.55, 10.55),
                (13.38, 11.33),
                (12.0, 11.6),
                (10.62, 11.33),
                (9.45, 10.55),
                (8.67, 9.38),
                (8.4, 8.0),
                (8.67, 6.62),
                (9.45, 5.45),
                (10.62, 4.67),
                (12.0, 4.4),
                (13.38, 4.67),
                (14.55, 5.45),
                (15.33, 6.62),
                (15.6, 8.0),
            ],
            &[
                (5.0, 20.5),
                (5.24, 18.69),
                (5.94, 17.0),
                (7.05, 15.55),
                (8.5, 14.44),
                (10.19, 13.74),
                (12.0, 13.5),
                (13.81, 13.74),
                (15.5, 14.44),
                (16.95, 15.55),
                (18.06, 17.0),
                (18.76, 18.69),
                (19.0, 20.5),
            ],
        ],
        Emblem::Display => &[
            &[(3., 4.), (21., 4.), (21., 16.), (3., 16.), (3., 4.)],
            &[(12., 16.), (12., 20.)],
            &[(8., 20.), (16., 20.)],
        ],
        Emblem::Accounts => &[
            &[
                (5., 9.),
                (5., 6.),
                (7., 4.),
                (10., 4.),
                (12., 6.),
                (12., 9.),
                (10., 11.),
                (7., 11.),
                (5., 9.),
            ],
            &[
                (2., 20.),
                (2., 17.),
                (5., 14.),
                (12., 14.),
                (15., 17.),
                (15., 20.),
            ],
            &[(16., 5.), (19., 6.), (20., 9.), (18., 11.)],
            &[(18., 14.), (22., 17.), (22., 20.)],
        ],
        Emblem::Cycle => &[
            &[(6., 9.), (12., 4.), (18., 9.)],
            &[(6., 15.), (12., 20.), (18., 15.)],
        ],
        Emblem::Key => &[
            &[
                (14., 4.),
                (18., 4.),
                (21., 7.),
                (21., 11.),
                (18., 14.),
                (14., 14.),
                (7., 21.),
                (3., 21.),
                (3., 17.),
                (11., 9.),
                (11., 7.),
                (14., 4.),
            ],
            &[(16.5, 8.5), (16.8, 8.5)],
        ],
    };
    for py in y.floor().max(0.) as u32..(y + 24. * scale).ceil().min(image.height() as f32) as u32 {
        for px in
            x.floor().max(0.) as u32..(x + 24. * scale).ceil().min(image.width() as f32) as u32
        {
            let p = ((px as f32 + 0.5 - x) / scale, (py as f32 + 0.5 - y) / scale);
            let distance = paths
                .iter()
                .copied()
                .chain(owned.iter().flatten().map(Vec::as_slice))
                .flat_map(|path| path.windows(2))
                .map(|line| {
                    let (a, b) = (line[0], line[1]);
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let t =
                        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
                    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
                })
                .fold(f32::INFINITY, f32::min);
            blend(
                image.get_pixel_mut(px, py),
                color,
                ((half_width - distance) * scale + 0.5).clamp(0., 1.),
            );
        }
    }
}

/// An emblem for passive widgets outside the authentication panel.
pub(super) fn emblem_at(
    image: &mut RgbImage,
    at: (f32, f32),
    scale: f32,
    kind: Emblem,
    color: Rgb,
) {
    emblem(image, at, scale, kind, color);
}

/// A static theme-derived wallpaper for the greeter and lock when no image is configured.
/// Rasterized once per output by the existing background cache.
pub(super) fn backdrop(
    appearance: &Appearance,
    width: u32,
    height: u32,
) -> anyhow::Result<RgbImage> {
    background::dimensions(width, height)?;
    let palette = appearance.palette();
    let light = appearance.color_scheme == ColorScheme::Light;
    let deep = if light {
        palette.surface
    } else {
        appearance.role(ColorRole::Spine)
    };
    let base = mix(palette.background, deep, if light { 0.52 } else { 0.66 });
    let brand = appearance.role(ColorRole::Brand);
    let ratio = (width as f32 / height as f32).min(2.3);
    let columns: Vec<_> = (0..width)
        .map(|x| {
            let x = x as f32 / width as f32;
            let phase = x * 5.1 - 1.08;
            let bend = phase.sin();
            let fold = (phase * 1.34 - 1.6).sin();
            let crest = 0.71 + 0.18 * bend + 0.12 * fold;
            // Stay within the green family: the brand only tints the left edge.
            let hue = mix(brand, palette.accent, (0.55 + x * 0.6).min(1.));
            (x, crest, 1. + 0.43 * bend, hue)
        })
        .collect();
    Ok(RgbImage::from_fn(width, height, |x, y| {
        let (x, crest, span, hue) = columns[x as usize];
        let y = y as f32 / height as f32;
        let distance = y - crest;
        let ribbon = (1. - distance.abs() * 4.6).clamp(0., 1.);
        let contour = (distance * 22. / span).fract().abs();
        let ridge = (1. - contour.min(1. - contour) * 18.).clamp(0., 1.) * ribbon * ribbon;
        let side_light = ((x - 0.5).abs() * 1.4).clamp(0., 1.);
        let ambient =
            (1. - 0.75 * ((x - 0.7) * ratio).powi(2) - (y - 0.84).powi(2) * 2.4).clamp(0., 1.);
        let sheen = (1. - ((distance + 0.032) * 16.).abs())
            .clamp(0., 1.)
            .powi(3);
        let strength = (0.13 * ambient
            + (0.12 + 0.20 * side_light) * ribbon * ribbon
            + 0.10 * ridge
            + 0.05 * sheen)
            * if light { 0.72 } else { 1. };
        let Rgb(r, g, b) = mix(base, hue, strength);
        image::Rgb([r, g, b])
    }))
}
