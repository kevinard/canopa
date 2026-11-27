//! Visual tokens shared by the session surfaces.
//!
//! Full-screen surfaces (sign-in and unlock) float translucent "glass" over a
//! softened wallpaper; dialogs use the opaque application surface. Both share
//! one set of button treatments so a control reads the same everywhere.
use canopa_theme::{ColorRole, Palette, Rgb, UiKind};
use iced::widget::button;
use iced::{Background, Border, Color, Shadow, Vector};

pub(crate) fn rgba(color: Rgb, alpha: f32) -> Color {
    let mut color = canopa_ui::color(color);
    color.a = alpha;
    color
}

fn lift(color: Color, toward: Color, amount: f32) -> Color {
    Color {
        r: color.r + (toward.r - color.r) * amount,
        g: color.g + (toward.g - color.g) * amount,
        b: color.b + (toward.b - color.b) * amount,
        a: color.a,
    }
}

/// Corner radii from the theme's `ui` element kinds, in logical pixels, so
/// the session surfaces share the shapes of the rest of the desktop.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Radii {
    pub button: f32,
    pub input: f32,
    pub row: f32,
    pub card: f32,
    pub panel: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Look {
    pub palette: Palette,
    pub dark: bool,
    pub focus: Rgb,
    pub on_accent: Rgb,
    pub radius: Radii,
}

impl Look {
    pub fn new(settings: &crate::settings::Settings) -> Self {
        let mut palette = settings.appearance.palette();
        let dark = settings.appearance.color_scheme == canopa_theme::ColorScheme::Dark;
        // Secondary text keeps at least 4.5:1 on the session's cards.
        palette.muted =
            super::raster::mix(palette.muted, palette.text, if dark { 0.22 } else { 0.42 });
        Self {
            palette,
            dark: settings.appearance.color_scheme == canopa_theme::ColorScheme::Dark,
            // One focus colour for fields, buttons and rows: the accent.
            focus: settings.appearance.palette().accent,
            on_accent: settings.appearance.role(ColorRole::OnAccent),
            radius: {
                let radius = |kind| settings.appearance.style(kind).radius[0].px();
                Radii {
                    button: radius(UiKind::Button),
                    input: radius(UiKind::Input),
                    row: radius(UiKind::MenuRow),
                    card: radius(UiKind::Card),
                    panel: radius(UiKind::Panel),
                }
            },
        }
    }

    /// A control's corner radius at this scale, never past a pill.
    pub fn button_radius(&self, height: f32, scale: f32) -> f32 {
        (self.radius.button * scale).min(height / 2.)
    }

    /// A control or field floating over the wallpaper.
    pub fn glass(&self) -> Color {
        if self.dark {
            Color::from_rgba(1., 1., 1., 0.10)
        } else {
            Color::from_rgba(1., 1., 1., 0.62)
        }
    }

    /// A larger card (choosers, power) over the wallpaper.
    pub fn glass_card(&self) -> Color {
        if self.dark {
            rgba(self.palette.surface, 0.78)
        } else {
            rgba(self.palette.surface, 0.84)
        }
    }

    pub fn glass_edge(&self) -> Color {
        if self.dark {
            Color::from_rgba(1., 1., 1., 0.16)
        } else {
            Color::from_rgba(0.1, 0.2, 0.16, 0.16)
        }
    }

    /// The faint full circle under a spinner's arc.
    pub fn track(&self) -> Color {
        if self.dark {
            Color::from_rgba(1., 1., 1., 0.16)
        } else {
            Color::from_rgba(0., 0., 0., 0.14)
        }
    }

    /// A one-pixel separator inside a card.
    pub fn hairline(&self) -> Color {
        if self.dark {
            Color::from_rgba(1., 1., 1., 0.09)
        } else {
            Color::from_rgba(0., 0., 0., 0.08)
        }
    }

    pub fn shadow(&self, depth: f32) -> Shadow {
        Shadow {
            color: Color::from_rgba(0., 0., 0., if self.dark { 0.38 } else { 0.16 }),
            offset: Vector::new(0., depth * 0.4),
            blur_radius: depth,
        }
    }

    /// The tinted backing of a heading emblem.
    pub fn tint(&self, strength: f32) -> Color {
        rgba(
            self.palette.accent,
            strength * if self.dark { 0.16 } else { 0.22 },
        )
    }

    /// Inset fill for dialog fields.
    pub fn inset(&self) -> Color {
        if self.dark {
            rgba(self.palette.background, 0.55)
        } else {
            Color::from_rgba(1., 1., 1., 0.9)
        }
    }

    /// The darker band that carries a dialog's actions.
    pub fn footer(&self) -> Color {
        if self.dark {
            rgba(self.palette.background, 0.38)
        } else {
            rgba(self.palette.raised, 0.55)
        }
    }

    pub fn foreground(&self, kind: Kind) -> Rgb {
        match kind {
            Kind::Primary => self.on_accent,
            // Text set below with the same strength as a disabled default action.
            Kind::Resting => self.palette.text,
            Kind::Danger => self.palette.danger,
            _ => self.palette.text,
        }
    }
}

/// How a button is drawn; independent of its meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    /// The accent-filled default action.
    Primary,
    /// Translucent control over the wallpaper.
    Glass,
    /// No resting background (links, back, utilities).
    Quiet,
    /// The default action before there is anything to submit: its accent, faded.
    Resting,
    /// An outlined control in a dialog footer.
    Secondary,
    /// A destructive confirmation.
    Danger,
    /// A list row; `checked` marks the current choice.
    Row { checked: bool },
}

pub(crate) fn button_style(
    look: Look,
    kind: Kind,
    radius: f32,
    focused: bool,
    alpha: f32,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let fade = move |mut color: Color| {
        color.a *= alpha;
        color
    };
    move |_, status| {
        let p = look.palette;
        let hovered = status == button::Status::Hovered;
        let pressed = status == button::Status::Pressed;
        let disabled = status == button::Status::Disabled;
        let text = rgba(p.text, 1.);
        let (background, border_color, border_width) = match kind {
            Kind::Primary => {
                let base = canopa_ui::color(p.accent);
                let toward = if look.dark {
                    Color::WHITE
                } else {
                    Color::BLACK
                };
                let amount = if pressed {
                    0.16
                } else if hovered {
                    0.10
                } else {
                    0.
                };
                // In dark, a light edge keeps the fill distinct from neutral buttons.
                (
                    Some(lift(base, toward, amount)),
                    if look.dark {
                        rgba(super::raster::mix(p.accent, Rgb(255, 255, 255), 0.6), 0.35)
                    } else {
                        Color::TRANSPARENT
                    },
                    if look.dark { 1. } else { 0. },
                )
            }
            Kind::Danger => {
                // A destructive action is unmistakable: a solid, deep danger fill.
                let toward = if look.dark {
                    Color::WHITE
                } else {
                    Color::BLACK
                };
                let base = if look.dark {
                    // The theme's danger hue, saturated and deepened until white
                    // text keeps 5:1: unmistakable, never muddy.
                    let (hue, saturation, mut light) = hsl(p.danger);
                    let saturation = saturation.clamp(0.5, 0.62);
                    let mut fill = from_hsl_rgb(hue, saturation, light);
                    while fill.contrast(Rgb(255, 255, 255)) < 4.6 && light > 0.2 {
                        light -= 0.02;
                        fill = from_hsl_rgb(hue, saturation, light);
                    }
                    canopa_ui::color(fill)
                } else {
                    canopa_ui::color(p.danger)
                };
                let amount = if pressed {
                    0.16
                } else if hovered {
                    0.08
                } else {
                    0.
                };
                (Some(lift(base, toward, amount)), Color::TRANSPARENT, 0.)
            }
            Kind::Glass => {
                let mut fill = look.glass();
                fill.a += if pressed {
                    0.10
                } else if hovered {
                    0.06
                } else {
                    0.
                };
                (Some(fill), look.glass_edge(), 1.)
            }
            Kind::Resting => {
                // The same look as an unavailable default action, with hover feedback.
                let alpha = if pressed {
                    0.38
                } else if hovered {
                    0.34
                } else {
                    0.28
                };
                (Some(rgba(p.accent, alpha)), Color::TRANSPARENT, 0.)
            }
            Kind::Quiet => {
                let alpha = if pressed {
                    0.12
                } else if hovered {
                    0.08
                } else {
                    0.
                };
                (
                    (alpha > 0.).then_some(Color { a: alpha, ..text }),
                    Color::TRANSPARENT,
                    0.,
                )
            }
            Kind::Secondary => {
                let base = if look.dark {
                    lift(rgba(p.raised, 1.), text, 0.06)
                } else {
                    Color::WHITE
                };
                let amount = if pressed {
                    0.10
                } else if hovered {
                    0.06
                } else {
                    0.
                };
                (
                    Some(lift(base, text, amount)),
                    if look.dark {
                        Color::from_rgba(1., 1., 1., 0.14)
                    } else {
                        Color::from_rgba(0., 0., 0., 0.14)
                    },
                    1.,
                )
            }
            Kind::Row { checked } => {
                // Every row rests on a faint fill, so each reads as a choice;
                // the current one is tinted. Keyboard focus adds the ring.
                let mut alpha: f32 = if checked { 0.16 } else { 0.05 };
                alpha += if pressed {
                    0.10
                } else if hovered {
                    0.06
                } else {
                    0.
                };
                let fill = if checked {
                    rgba(p.accent, alpha)
                } else {
                    Color { a: alpha, ..text }
                };
                (Some(fill), Color::TRANSPARENT, 0.)
            }
        };
        let foreground = if kind == Kind::Danger {
            canopa_theme::Rgb(255, 255, 255)
        } else {
            look.foreground(kind)
        };
        button::Style {
            background: if disabled && kind == Kind::Primary {
                // The main action, not ready yet: its accent, faded, without a
                // border; the label stays readable in the text colour.
                Some(Background::Color(fade(rgba(
                    p.accent,
                    if look.dark { 0.36 } else { 0.18 },
                ))))
            } else {
                background.map(|mut color| {
                    if disabled {
                        color.a *= 0.6;
                    }
                    Background::Color(fade(color))
                })
            },
            // Unavailable controls keep their colours at reduced strength, so
            // they still read as present.
            text_color: fade(
                if disabled && kind == Kind::Primary || kind == Kind::Resting {
                    let mut label = canopa_ui::color(p.text);
                    label.a = if look.dark { 0.62 } else { 0.6 };
                    label
                } else {
                    let mut color = canopa_ui::color(foreground);
                    if disabled {
                        color.a *= 0.6;
                    }
                    color
                },
            ),
            border: Border {
                color: fade(if focused && !disabled {
                    // On list rows the ring contrasts with the current choice's tint.
                    if matches!(kind, Kind::Row { .. }) {
                        rgba(p.text, 0.85)
                    } else {
                        canopa_ui::color(look.focus)
                    }
                } else {
                    border_color
                }),
                width: if focused && !disabled {
                    2.
                } else {
                    border_width
                },
                radius: radius.into(),
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

/// Two harmonious tones for an avatar without a picture, chosen from `seed`
/// so each account keeps its colour.
pub(crate) fn avatar_tones(seed: &str, accent: Rgb) -> (Color, Color) {
    // Quiet tones around the theme's accent: each account keeps its own hue
    // step, so pictures differ while sitting with the accent.
    const STEPS: [f32; 8] = [0., 18., -18., 36., -36., 9., -27., 27.];
    let hash = seed
        .chars()
        .flat_map(char::to_lowercase)
        .fold(2166136261u32, |hash, c| {
            (hash ^ c as u32).wrapping_mul(16777619)
        });
    let (hue, saturation, _) = hsl(accent);
    let hue = (hue + STEPS[hash as usize % STEPS.len()]).rem_euclid(360.);
    // An accent without colour (a grey theme) gives grey pictures.
    let saturation = if saturation < 0.08 {
        saturation
    } else {
        saturation.clamp(0.22, 0.42)
    };
    (
        from_hsl(hue, saturation, 0.62),
        from_hsl(hue, saturation, 0.40),
    )
}

fn hsl(Rgb(r, g, b): Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255., g as f32 / 255., b as f32 / 255.);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let light = (max + min) / 2.;
    let delta = max - min;
    if delta < f32::EPSILON {
        return (0., 0., light);
    }
    let saturation = delta / (1. - (2. * light - 1.).abs());
    let hue = if max == r {
        60. * ((g - b) / delta).rem_euclid(6.)
    } else if max == g {
        60. * ((b - r) / delta + 2.)
    } else {
        60. * ((r - g) / delta + 4.)
    };
    (hue, saturation, light)
}

fn from_hsl_rgb(hue: f32, saturation: f32, light: f32) -> Rgb {
    let color = from_hsl(hue, saturation, light);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    Rgb(channel(color.r), channel(color.g), channel(color.b))
}

fn from_hsl(hue: f32, saturation: f32, light: f32) -> Color {
    let chroma = (1. - (2. * light - 1.).abs()) * saturation;
    let x = chroma * (1. - ((hue / 60.).rem_euclid(2.) - 1.).abs());
    let (r, g, b) = match (hue / 60.) as u32 {
        0 => (chroma, x, 0.),
        1 => (x, chroma, 0.),
        2 => (0., chroma, x),
        3 => (0., x, chroma),
        4 => (x, 0., chroma),
        _ => (chroma, 0., x),
    };
    let m = light - chroma / 2.;
    Color::from_rgb(r + m, g + m, b + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_tones_are_stable_per_account_and_ignore_case() {
        let accent = Rgb(0x25, 0x69, 0x48);
        assert_eq!(
            avatar_tones("Alex Morgan", accent),
            avatar_tones("alex morgan", accent)
        );
        let distinct: std::collections::BTreeSet<_> = ["AM", "SR", "JD", "KO", "LP", "QX"]
            .iter()
            .map(|seed| format!("{:?}", avatar_tones(seed, accent)))
            .collect();
        assert!(distinct.len() > 2);
        // Another accent moves every picture with it.
        assert_ne!(
            avatar_tones("AM", accent),
            avatar_tones("AM", Rgb(0x3b, 0x5b, 0xdb))
        );
    }
}
