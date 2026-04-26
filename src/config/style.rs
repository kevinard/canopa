use anyhow::{anyhow, bail};
use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeToken {
    Accent, Text, Subtext, Base,
    Surface0, Surface1, Border, Overlay0,
    Critical, Warning, Success,
}

impl ThemeToken {
    pub fn resolve(self) -> Color {
        use crate::theme;
        match self {
            ThemeToken::Accent => theme::ACCENT,
            ThemeToken::Text => theme::TEXT,
            ThemeToken::Subtext => theme::SUBTEXT,
            ThemeToken::Base => theme::BASE,
            ThemeToken::Surface0 => theme::SURFACE0,
            ThemeToken::Surface1 => theme::SURFACE1,
            ThemeToken::Border => theme::BORDER,
            ThemeToken::Overlay0 => theme::OVERLAY0,
            ThemeToken::Critical => theme::CRITICAL,
            ThemeToken::Warning => theme::WARNING,
            ThemeToken::Success => theme::SUCCESS,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "accent" => ThemeToken::Accent,
            "text" => ThemeToken::Text,
            "subtext" => ThemeToken::Subtext,
            "base" => ThemeToken::Base,
            "surface0" => ThemeToken::Surface0,
            "surface1" => ThemeToken::Surface1,
            "border" => ThemeToken::Border,
            "overlay0" => ThemeToken::Overlay0,
            "critical" => ThemeToken::Critical,
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
        Err(anyhow!("unknown color {raw:?}; expected theme token or #rrggbb[aa]."))
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
    Some(Color::from_rgba8(out[0], out[1], out[2], out[3] as f32 / 255.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Light, Normal, Bold,
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
}
