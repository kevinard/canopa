//! Toolkit-independent appearance snapshots. No environment or process-global state.
mod customization;
pub use customization::*;

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    pub accent: Option<Rgb>,
    pub customization: std::sync::Arc<Customization>,
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
    pub fn palette(&self) -> Palette {
        let mut palette = match self.color_scheme {
            ColorScheme::Dark => Palette {
                background: Rgb(0x10, 0x19, 0x1d),
                surface: Rgb(0x20, 0x2d, 0x31),
                raised: Rgb(0x2b, 0x3b, 0x3f),
                text: Rgb(0xed, 0xf4, 0xf0),
                muted: Rgb(0xb5, 0xc7, 0xc3),
                disabled: Rgb(0x77, 0x8c, 0x88),
                border: Rgb(0x64, 0x7d, 0x76),
                accent: Rgb(0xb0, 0xde, 0xc3),
                danger: Rgb(0xf3, 0x8b, 0xa8),
                warning: Rgb(0xf9, 0xe2, 0xaf),
                success: Rgb(0xa6, 0xe3, 0xa1),
            },
            ColorScheme::Light => Palette {
                background: Rgb(0xe3, 0xea, 0xe6),
                surface: Rgb(0xfb, 0xfc, 0xf8),
                raised: Rgb(0xe7, 0xed, 0xe7),
                text: Rgb(0x20, 0x34, 0x2c),
                muted: Rgb(0x52, 0x67, 0x5e),
                disabled: Rgb(0x78, 0x89, 0x80),
                border: Rgb(0x88, 0x9b, 0x90),
                accent: Rgb(0x25, 0x69, 0x48),
                danger: Rgb(0xb0, 0x18, 0x43),
                warning: Rgb(0x85, 0x59, 0x00),
                success: Rgb(0x32, 0x6a, 0x1f),
            },
        };
        if let Some(accent) = self.accent {
            palette.accent = accent;
        }
        for role in ColorRole::ALL {
            if let Some(color) = self.customization.palette[role as usize] {
                match role {
                    ColorRole::Background => palette.background = color,
                    ColorRole::Surface => palette.surface = color,
                    ColorRole::Raised => palette.raised = color,
                    ColorRole::Text => palette.text = color,
                    ColorRole::Muted => palette.muted = color,
                    ColorRole::Disabled => palette.disabled = color,
                    ColorRole::Border => palette.border = color,
                    ColorRole::Accent => palette.accent = color,
                    ColorRole::Danger => palette.danger = color,
                    ColorRole::Warning => palette.warning = color,
                    ColorRole::Success => palette.success = color,
                    _ => {}
                }
            }
        }
        if self.high_contrast() {
            palette = palette.high_contrast(self.color_scheme);
        }
        palette
    }
}

impl Palette {
    /// Black and white surfaces, full-strength text and borders, and an
    /// accent kept only when it reads at 7:1 on the background.
    fn high_contrast(self, scheme: ColorScheme) -> Self {
        let (background, raised, text, muted, disabled, fallback) = match scheme {
            ColorScheme::Dark => (
                Rgb(0x00, 0x00, 0x00),
                Rgb(0x1a, 0x1a, 0x1a),
                Rgb(0xff, 0xff, 0xff),
                Rgb(0xe6, 0xe6, 0xe6),
                Rgb(0xb3, 0xb3, 0xb3),
                Rgb(0xff, 0xe0, 0x66),
            ),
            ColorScheme::Light => (
                Rgb(0xff, 0xff, 0xff),
                Rgb(0xec, 0xec, 0xec),
                Rgb(0x00, 0x00, 0x00),
                Rgb(0x1f, 0x1f, 0x1f),
                Rgb(0x4d, 0x4d, 0x4d),
                Rgb(0x00, 0x3d, 0x8f),
            ),
        };
        let strong = |color: Rgb, fallback: Rgb| {
            if color.contrast(background) >= 7.0 && color.contrast(raised) >= 4.5 {
                color
            } else {
                fallback
            }
        };
        let (danger, warning, success) = match scheme {
            ColorScheme::Dark => (
                Rgb(0xff, 0x9e, 0xb5),
                Rgb(0xff, 0xe0, 0x66),
                Rgb(0x9c, 0xf5, 0x9c),
            ),
            ColorScheme::Light => (
                Rgb(0x8f, 0x00, 0x2b),
                Rgb(0x5c, 0x3d, 0x00),
                Rgb(0x1d, 0x4d, 0x0d),
            ),
        };
        Self {
            background,
            surface: background,
            raised,
            text,
            muted,
            disabled,
            border: text,
            accent: strong(self.accent, fallback),
            danger: strong(self.danger, danger),
            warning: strong(self.warning, warning),
            success: strong(self.success, success),
        }
    }
}

/// Shared logical dimensions; compositor scaling is applied by the toolkit.
pub mod metrics {
    pub const BODY: f32 = 14.0;
    pub const LABEL: f32 = 12.0;
    pub const TITLE: f32 = 20.0;
    pub const GAP: f32 = 8.0;
    pub const PADDING: f32 = 16.0;
    pub const CONTROL_RADIUS: f32 = 12.0;
    pub const SURFACE_RADIUS: f32 = 24.0;
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

    #[test]
    fn composed_canopa_surfaces_and_selected_controls_have_readable_contrast() {
        for color_scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let appearance = Appearance {
                color_scheme,
                ..Default::default()
            };
            for background in [
                appearance.role(ColorRole::Spine),
                appearance.role(ColorRole::AccentSurface),
                appearance.palette().raised,
                appearance.palette().surface,
            ] {
                for foreground in [appearance.palette().text, appearance.palette().muted] {
                    assert!(foreground.contrast(background) >= 4.5);
                }
                assert!(appearance.role(ColorRole::Focus).contrast(background) >= 3.0);
            }
            assert!(
                appearance
                    .role(ColorRole::OnAccent)
                    .contrast(appearance.palette().accent)
                    >= 4.5
            );
        }
        // A user's accent still gets a readable foreground unless they override it.
        for accent in [Rgb(255, 255, 255), Rgb(0, 0, 0), Rgb(120, 120, 120)] {
            let appearance = Appearance {
                accent: Some(accent),
                ..Default::default()
            };
            assert!(appearance.role(ColorRole::OnAccent).contrast(accent) >= 4.5);
        }
    }

    #[test]
    fn high_contrast_reaches_seven_to_one_even_over_custom_colors() {
        for color_scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for accent in [
                None,
                Some(Rgb(0x80, 0x80, 0x80)),
                Some(Rgb(0xff, 0xff, 0xff)),
            ] {
                let mut customization = Customization {
                    high_contrast: true,
                    ..Default::default()
                };
                customization.palette[ColorRole::Muted as usize] = Some(Rgb(0x77, 0x77, 0x77));
                let appearance = Appearance {
                    color_scheme,
                    accent,
                    customization: std::sync::Arc::new(customization),
                };
                let p = appearance.palette();
                for background in [p.background, p.surface, appearance.role(ColorRole::Spine)] {
                    for foreground in [p.text, p.muted, p.accent, p.border] {
                        assert!(
                            foreground.contrast(background) >= 7.0,
                            "{color_scheme:?}: {foreground:?} on {background:?}"
                        );
                    }
                }
                assert!(p.text.contrast(p.raised) >= 7.0);
                assert!(appearance.role(ColorRole::OnAccent).contrast(p.accent) >= 4.5);
            }
        }
    }
}
