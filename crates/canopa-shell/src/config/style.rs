use anyhow::{anyhow, bail};
use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeToken {
    Accent,
    Text,
    Subtext,
    Base,
    Surface0,
    Surface1,
    Border,
    Overlay0,
    Critical,
    Warning,
    Success,
}

impl ThemeToken {
    pub fn resolve(self) -> Color {
        use crate::theme;
        match self {
            ThemeToken::Accent => theme::palette().accent,
            ThemeToken::Text => theme::palette().text,
            ThemeToken::Subtext => theme::palette().muted,
            ThemeToken::Base => theme::palette().background,
            ThemeToken::Surface0 => theme::palette().surface,
            ThemeToken::Surface1 => theme::palette().raised,
            ThemeToken::Border => theme::palette().border,
            ThemeToken::Overlay0 => theme::palette().disabled,
            ThemeToken::Critical => theme::palette().danger,
            ThemeToken::Warning => theme::palette().warning,
            ThemeToken::Success => theme::palette().success,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        // Appearance role names, as everywhere else, with the older palette
        // token names kept as aliases.
        Some(match name.to_ascii_lowercase().as_str() {
            "accent" => ThemeToken::Accent,
            "text" => ThemeToken::Text,
            "muted" | "subtext" => ThemeToken::Subtext,
            "background" | "base" => ThemeToken::Base,
            "surface" | "surface0" => ThemeToken::Surface0,
            "raised" | "surface1" => ThemeToken::Surface1,
            "border" => ThemeToken::Border,
            "disabled" | "overlay0" => ThemeToken::Overlay0,
            "danger" | "critical" => ThemeToken::Critical,
            "warning" => ThemeToken::Warning,
            "success" => ThemeToken::Success,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorValue {
    Token(ThemeToken),
    Rgba(Color),
}

impl ColorValue {
    pub fn resolve(self) -> Color {
        match self {
            ColorValue::Token(t) => t.resolve(),
            ColorValue::Rgba(c) => c,
        }
    }

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        if raw.is_empty() {
            bail!("color value must not be empty.");
        }
        if let Some(token) = ThemeToken::from_name(raw) {
            return Ok(ColorValue::Token(token));
        }
        if let Some(hex) = raw.strip_prefix('#') {
            return parse_hex(hex)
                .map(ColorValue::Rgba)
                .ok_or_else(|| anyhow!("invalid hex color {raw:?}."));
        }
        Err(anyhow!(
            "unknown color {raw:?}; expected theme token or #rrggbb[aa]."
        ))
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    let bytes = match hex.len() {
        6 | 8 => hex.as_bytes(),
        _ => return None,
    };
    let mut out = [0u8; 4];
    out[3] = 0xff;
    for (i, chunk) in bytes.chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(Color::from_rgba8(
        out[0],
        out[1],
        out[2],
        out[3] as f32 / 255.0,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Light,
    Normal,
    Bold,
}

impl Weight {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        Ok(match raw {
            "light" => Weight::Light,
            "normal" => Weight::Normal,
            "bold" => Weight::Bold,
            _ => bail!("unknown weight {raw:?}; expected light|normal|bold."),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub color: Option<ColorValue>,
    pub size: Option<f32>,
    pub weight: Option<Weight>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        self.color.is_none() && self.size.is_none() && self.weight.is_none()
    }

    pub fn merge_over(self, base: Style) -> Style {
        Style {
            color: self.color.or(base.color),
            size: self.size.or(base.size),
            weight: self.weight.or(base.weight),
        }
    }

    pub fn resolve(&self) -> ResolvedStyle {
        ResolvedStyle {
            color: self.color.map(ColorValue::resolve),
            size: self.size,
            weight: self.weight,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResolvedStyle {
    pub color: Option<Color>,
    pub size: Option<f32>,
    pub weight: Option<Weight>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum SegmentStyles {
    #[default]
    None,
    Static(Style),
    Stateful {
        default: Option<Style>,
        by_state: Vec<(String, Style)>,
    },
}

impl SegmentStyles {
    pub fn style_for(&self, state: Option<&str>) -> Style {
        match self {
            SegmentStyles::None => Style::default(),
            SegmentStyles::Static(s) => *s,
            SegmentStyles::Stateful { default, by_state } => {
                if let Some(state) = state
                    && let Some((_, style)) = by_state.iter().find(|(k, _)| k == state)
                {
                    return *style;
                }
                default.unwrap_or_default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theme_token_color() {
        assert_eq!(
            ColorValue::parse("accent").unwrap(),
            ColorValue::Token(ThemeToken::Accent)
        );
        assert_eq!(
            ColorValue::parse("CRITICAL").unwrap(),
            ColorValue::Token(ThemeToken::Critical)
        );
    }

    #[test]
    fn appearance_role_names_and_palette_tokens_name_the_same_colors() {
        for (role, token) in [
            ("muted", "subtext"),
            ("background", "base"),
            ("surface", "surface0"),
            ("raised", "surface1"),
            ("disabled", "overlay0"),
            ("danger", "critical"),
        ] {
            assert_eq!(
                ColorValue::parse(role).unwrap(),
                ColorValue::parse(token).unwrap(),
                "{role} and {token}"
            );
            // Both spellings mean the same appearance role everywhere else.
            assert_eq!(
                canopa_theme::ColorRole::parse(role),
                canopa_theme::ColorRole::parse(token)
            );
        }
    }

    #[test]
    fn parse_hex_rgb_color() {
        let c = ColorValue::parse("#ff8800").unwrap();
        match c {
            ColorValue::Rgba(rgba) => {
                assert!((rgba.r - 1.0).abs() < 1e-3);
                assert!((rgba.g - 0.533).abs() < 1e-2);
                assert!((rgba.b - 0.0).abs() < 1e-3);
                assert!((rgba.a - 1.0).abs() < 1e-3);
            }
            _ => panic!("expected hex"),
        }
    }

    #[test]
    fn parse_hex_rgba_color() {
        let c = ColorValue::parse("#ff880080").unwrap();
        match c {
            ColorValue::Rgba(rgba) => assert!((rgba.a - 0.502).abs() < 1e-2),
            _ => panic!("expected hex"),
        }
    }

    #[test]
    fn parse_invalid_color_string() {
        assert!(ColorValue::parse("magenta").is_err());
        assert!(ColorValue::parse("#ff").is_err());
        assert!(ColorValue::parse("#gggggg").is_err());
        assert!(ColorValue::parse("").is_err());
    }

    #[test]
    fn parse_weight() {
        assert_eq!(Weight::parse("light").unwrap(), Weight::Light);
        assert_eq!(Weight::parse("normal").unwrap(), Weight::Normal);
        assert_eq!(Weight::parse("bold").unwrap(), Weight::Bold);
        assert!(Weight::parse("extrabold").is_err());
        assert!(Weight::parse("").is_err());
    }

    #[test]
    fn segment_styles_none_resolves_empty() {
        let s = SegmentStyles::None;
        assert_eq!(s.style_for(None).resolve(), ResolvedStyle::default());
        assert_eq!(
            s.style_for(Some("critical")).resolve(),
            ResolvedStyle::default()
        );
    }

    #[test]
    fn segment_styles_static_always_returns_same() {
        let style = Style {
            color: Some(ColorValue::Token(ThemeToken::Accent)),
            ..Default::default()
        };
        let s = SegmentStyles::Static(style);
        let expected = style.resolve();
        assert_eq!(s.style_for(None).resolve(), expected);
        assert_eq!(s.style_for(Some("critical")).resolve(), expected);
    }

    #[test]
    fn segment_styles_stateful_picks_matching_state() {
        let default = Style {
            color: Some(ColorValue::Token(ThemeToken::Accent)),
            ..Default::default()
        };
        let crit = Style {
            color: Some(ColorValue::Token(ThemeToken::Critical)),
            ..Default::default()
        };
        let s = SegmentStyles::Stateful {
            default: Some(default),
            by_state: vec![("critical".into(), crit)],
        };
        assert_eq!(s.style_for(Some("critical")).resolve(), crit.resolve());
        assert_eq!(s.style_for(Some("normal")).resolve(), default.resolve());
        assert_eq!(s.style_for(None).resolve(), default.resolve());
    }

    #[test]
    fn segment_styles_stateful_no_default_no_match_is_empty() {
        let s = SegmentStyles::Stateful {
            default: None,
            by_state: vec![(
                "critical".into(),
                Style {
                    color: Some(ColorValue::Token(ThemeToken::Critical)),
                    ..Default::default()
                },
            )],
        };
        assert_eq!(
            s.style_for(Some("normal")).resolve(),
            ResolvedStyle::default()
        );
    }
}
