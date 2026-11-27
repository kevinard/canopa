//! Visual tokens shared by the session surfaces.
//!
//! Full-screen surfaces (sign-in and unlock) float translucent "glass" over a
//! softened wallpaper; dialogs use the opaque application surface. Both share
//! one set of button treatments so a control reads the same everywhere.
use canopa_theme::{ColorRole, Palette, Rgb};
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct Look {
    pub palette: Palette,
    pub dark: bool,
    pub focus: Rgb,
    pub on_accent: Rgb,
}

impl Look {
    pub fn new(settings: &crate::settings::Settings) -> Self {
        let mut palette = settings.appearance.palette();
        let dark = settings.appearance.color_scheme == canopa_theme::ColorScheme::Dark;
        // Secondary text keeps at least 4.5:1 on the session's cards.
        palette.muted = super::raster::mix(palette.muted, palette.text, if dark { 0.22 } else { 0.42 });
        Self {
            palette,
            dark: settings.appearance.color_scheme == canopa_theme::ColorScheme::Dark,
            // One focus colour for fields, buttons and rows: the accent.
            focus: settings.appearance.palette().accent,
            on_accent: settings.appearance.role(ColorRole::OnAccent),
        }
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
        rgba(self.palette.accent, strength * if self.dark { 0.16 } else { 0.22 })
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
            Kind::Outline => self.palette.muted,
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
    /// No fill, a thin outline: a visible but light secondary action.
    Outline,
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
                let toward = if look.dark { Color::WHITE } else { Color::BLACK };
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
                        Color::from_rgba(1., 0.85, 0.9, 0.35)
                    } else {
                        Color::TRANSPARENT
                    },
                    if look.dark { 1. } else { 0. },
                )
            }
            Kind::Danger => {
                // A destructive action is unmistakable: a solid, deep danger fill.
                let toward = if look.dark { Color::WHITE } else { Color::BLACK };
                let base = if look.dark {
                    // White text keeps at least 4.5:1 on this fill.
                    Color::from_rgb8(0xb8, 0x36, 0x5c)
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
            Kind::Outline => {
                let alpha = if pressed {
                    0.12
                } else if hovered {
                    0.08
                } else {
                    0.
                };
                (
                    (alpha > 0.).then(|| Color { a: alpha, ..text }),
                    if look.dark {
                        Color::from_rgba(1., 1., 1., 0.3)
                    } else {
                        Color::from_rgba(0., 0., 0., 0.26)
                    },
                    1.,
                )
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
                    (alpha > 0.).then(|| Color { a: alpha, ..text }),
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
                    if look.dark { 0.18 } else { 0.28 },
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
            text_color: fade(if disabled && kind == Kind::Primary || kind == Kind::Resting {
                let mut label = canopa_ui::color(p.text);
                label.a = if look.dark { 0.5 } else { 0.8 };
                label
            } else {
                let mut color = canopa_ui::color(foreground);
                if disabled {
                    color.a *= 0.6;
                }
                color
            }),
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
pub(crate) fn avatar_tones(seed: &str) -> (Color, Color) {
    // Quiet, nature-derived tones that sit with the green accent.
    const TONES: [((u8, u8, u8), (u8, u8, u8)); 8] = [
        ((0x86, 0xb3, 0x9b), (0x5b, 0x8c, 0x74)),
        ((0x7f, 0xae, 0xb3), (0x55, 0x86, 0x8d)),
        ((0x93, 0xa4, 0xbf), (0x67, 0x7a, 0x99)),
        ((0x7c, 0xb8, 0xb0), (0x3f, 0x7d, 0x78)),
        ((0xc4, 0x9e, 0x98), (0x9b, 0x74, 0x6e)),
        ((0xc4, 0xab, 0x84), (0x9a, 0x81, 0x5b)),
        ((0x9f, 0xb3, 0x82), (0x75, 0x8a, 0x58)),
        ((0x8f, 0xa8, 0xa1), (0x65, 0x7f, 0x78)),
    ];
    let hash = seed
        .chars()
        .flat_map(char::to_lowercase)
        .fold(2166136261u32, |hash, c| {
            (hash ^ c as u32).wrapping_mul(16777619)
        });
    let ((r1, g1, b1), (r2, g2, b2)) = TONES[hash as usize % TONES.len()];
    (Color::from_rgb8(r1, g1, b1), Color::from_rgb8(r2, g2, b2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_tones_are_stable_per_account_and_ignore_case() {
        assert_eq!(avatar_tones("Alex Morgan"), avatar_tones("alex morgan"));
        let distinct: std::collections::BTreeSet<_> = ["AM", "SR", "JD", "KO", "LP", "QX"]
            .iter()
            .map(|seed| format!("{:?}", avatar_tones(seed)))
            .collect();
        assert!(distinct.len() > 2);
    }
}
