//! Software drawing for the shared authentication panel and default session backdrop.
use crate::{background, layout::Rect};
use canopa_theme::{Appearance, ColorRole, ColorScheme, Rgb};
use image::RgbImage;

#[derive(Clone, Copy)]
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
}

impl Rect {
    fn inset(self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: self.width - 2. * amount,
            height: self.height - 2. * amount,
        }
    }
}

pub(super) fn framed(
    image: &mut RgbImage,
    bounds: Rect,
    radius: f32,
    stroke: f32,
    fill: Rgb,
    border: Rgb,
) {
    rounded(image, bounds, radius, border, 1.);
    rounded(image, bounds.inset(stroke), radius - stroke, fill, 1.);
}

pub(super) fn rounded(image: &mut RgbImage, r: Rect, radius: f32, color: Rgb, opacity: f32) {
    paint_distance(image, r, radius, 1., color, |distance| {
        (0.5 - distance).clamp(0., 1.) * opacity
    });
}

pub(super) fn shadow(image: &mut RgbImage, mut r: Rect, radius: f32, scale: f32, strength: f32) {
    r.y += 4. * scale;
    let spread = 29. * scale;
    paint_distance(image, r, radius, spread, Rgb(0, 0, 0), |distance| {
        (1. - (distance / spread).clamp(0., 1.)).powi(3) * strength
    });
}

fn paint_distance(
    image: &mut RgbImage,
    r: Rect,
    radius: f32,
    spread: f32,
    color: Rgb,
    opacity: impl Fn(f32) -> f32,
) {
    let radius = radius.clamp(0., (r.width.min(r.height) / 2.).max(0.));
    let area = r.inset(-spread);
    let x_end = (area.x + area.width).ceil().min(image.width() as f32) as u32;
    let y_end = (area.y + area.height).ceil().min(image.height() as f32) as u32;
    for y in area.y.floor().max(0.) as u32..y_end {
        for x in area.x.floor().max(0.) as u32..x_end {
            let dx = (x as f32 + 0.5 - r.x - r.width / 2.).abs() - (r.width / 2. - radius);
            let dy = (y as f32 + 0.5 - r.y - r.height / 2.).abs() - (r.height / 2. - radius);
            let distance = if dx > 0. && dy > 0. {
                dx.hypot(dy)
            } else {
                dx.max(dy)
            } - radius;
            blend(image.get_pixel_mut(x, y), color, opacity(distance));
        }
    }
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
    let paths: &[&[(f32, f32)]] = match emblem {
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
                ((0.95 - distance) * scale + 0.5).clamp(0., 1.),
            );
        }
    }
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
            let hue = mix(brand, palette.accent, (x * 1.13 + 0.10).min(1.));
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
