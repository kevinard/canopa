//! Toolkit-independent appearance snapshots. No environment or process-global state.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse_hex(value: &str) -> Option<Self> {
        let hex = value.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let number = u32::from_str_radix(hex, 16).ok()?;
        Some(Self(
            (number >> 16) as u8,
            (number >> 8) as u8,
            number as u8,
        ))
    }

    pub fn contrast(self, other: Self) -> f64 {
        fn luminance(Rgb(r, g, b): Rgb) -> f64 {
            [r, g, b]
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(v, w)| {
                    let s = f64::from(v) / 255.0;
                    w * if s <= 0.04045 {
                        s / 12.92
                    } else {
                        ((s + 0.055) / 1.055).powf(2.4)
                    }
                })
                .sum()
        }
        let a = luminance(self);
        let b = luminance(other);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    pub fn contrasting_text(self) -> Self {
        let black = Self(0, 0, 0);
        let white = Self(255, 255, 255);
        if self.contrast(black) >= self.contrast(white) {
            black
        } else {
            white
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    pub accent: Option<Rgb>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub surface: Rgb,
    pub raised: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub disabled: Rgb,
    pub border: Rgb,
    pub accent: Rgb,
    pub danger: Rgb,
    pub warning: Rgb,
    pub success: Rgb,
}

impl Appearance {
    pub fn palette(self) -> Palette {
        let mut palette = match self.color_scheme {
            ColorScheme::Dark => Palette {
                background: Rgb(0x1e, 0x1e, 0x2e),
                surface: Rgb(0x31, 0x32, 0x44),
                raised: Rgb(0x45, 0x47, 0x5a),
                text: Rgb(0xcd, 0xd6, 0xf4),
                muted: Rgb(0xa6, 0xad, 0xc8),
                disabled: Rgb(0x6c, 0x70, 0x86),
                border: Rgb(0x58, 0x5b, 0x70),
                accent: Rgb(0x89, 0xb4, 0xfa),
                danger: Rgb(0xf3, 0x8b, 0xa8),
                warning: Rgb(0xf9, 0xe2, 0xaf),
                success: Rgb(0xa6, 0xe3, 0xa1),
            },
            // Catppuccin Latte with darker status colors and subdued text for contrast.
            ColorScheme::Light => Palette {
                background: Rgb(0xef, 0xf1, 0xf5),
                surface: Rgb(0xe6, 0xe9, 0xef),
                raised: Rgb(0xdc, 0xe0, 0xe8),
                text: Rgb(0x4c, 0x4f, 0x69),
                muted: Rgb(0x5c, 0x5f, 0x77),
                disabled: Rgb(0x8c, 0x8f, 0xa1),
                border: Rgb(0x9c, 0xa0, 0xb0),
                accent: Rgb(0x1e, 0x5c, 0xd5),
                danger: Rgb(0xb0, 0x18, 0x43),
                warning: Rgb(0x85, 0x59, 0x00),
                success: Rgb(0x32, 0x6a, 0x1f),
            },
        };
        if let Some(accent) = self.accent {
            palette.accent = accent;
        }
        palette
    }
}

/// Shared logical dimensions; compositor scaling is applied by the toolkit.
pub mod metrics {
    pub const BODY: f32 = 14.0;
    pub const LABEL: f32 = 12.0;
    pub const TITLE: f32 = 16.0;
    pub const GAP: f32 = 8.0;
    pub const PADDING: f32 = 10.0;
    pub const CONTROL_RADIUS: f32 = 6.0;
    pub const SURFACE_RADIUS: f32 = 8.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_readable_text_status_and_focus_colors() {
        for color_scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let p = Appearance {
                color_scheme,
                ..Default::default()
            }
            .palette();
            for background in [p.background, p.surface] {
                for foreground in [p.text, p.muted, p.accent, p.danger, p.warning, p.success] {
                    assert!(
                        foreground.contrast(background) >= 4.5,
                        "{color_scheme:?}: {foreground:?} on {background:?}"
                    );
                }
            }
            assert!(p.text.contrast(p.raised) >= 4.5);
            assert!(p.accent.contrast(p.raised) >= 3.0);
            for fill in [p.accent, p.danger] {
                assert!(fill.contrasting_text().contrast(fill) >= 4.5);
            }
        }
    }

    #[test]
    fn colors_are_strict_rgb_and_foreground_is_readable() {
        assert_eq!(Rgb::parse_hex("#aB12ef"), Some(Rgb(171, 18, 239)));
        for value in ["#fff", "#aabbccdd", "ffffff", "#💚ab", "#gg0000"] {
            assert!(Rgb::parse_hex(value).is_none());
        }
        for fill in [Rgb(255, 255, 255), Rgb(0, 0, 0), Rgb(127, 127, 127)] {
            assert!(fill.contrast(fill.contrasting_text()) >= 4.5);
        }
    }
}
