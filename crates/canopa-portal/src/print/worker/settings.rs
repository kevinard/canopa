//! Four bounded device controls. Only fixed IPP option names cross this seam.
use super::catalog::MAX_CHOICES;
use serde::{Deserialize, Serialize};
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Source,
    Type,
    Quality,
    Resolution,
}
impl Field {
    pub const ALL: [Self; 4] = [Self::Source, Self::Type, Self::Quality, Self::Resolution];
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Paper source",
            Self::Type => "Paper type",
            Self::Quality => "Print quality",
            Self::Resolution => "Resolution",
        }
    }
    pub(super) fn ipp(self) -> &'static CStr {
        match self {
            Self::Source => c"media-source",
            Self::Type => c"media-type",
            Self::Quality => c"print-quality",
            Self::Resolution => c"printer-resolution",
        }
    }
    pub(super) fn ppd(self) -> &'static CStr {
        match self {
            Self::Source => c"InputSlot",
            Self::Type => c"MediaType",
            Self::Quality => c"cupsPrintQuality",
            Self::Resolution => c"Resolution",
        }
    }
    pub(super) fn mapping(self) -> &'static CStr {
        match self {
            Self::Source | Self::Type => c"media",
            _ => self.ipp(),
        }
    }
    pub(super) fn validate(self, value: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !value.is_empty()
                && value.len() <= 127
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "Invalid printer control value"
        );
        match self {
            Self::Quality => {
                anyhow::ensure!(["3", "4", "5"].contains(&value), "Invalid print quality")
            }
            Self::Resolution => {
                Resolution::parse(value)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn equivalent(self, a: &str, b: &str) -> bool {
        if self == Self::Resolution {
            Resolution::parse(a)
                .ok()
                .zip(Resolution::parse(b).ok())
                .is_some_and(|(a, b)| a == b)
        } else {
            a == b
        }
    }
    pub(super) fn value_label(self, value: &str) -> String {
        match (self, value) {
            (Self::Quality, "3") => "Draft".into(),
            (Self::Quality, "4") => "Normal".into(),
            (Self::Quality, "5") => "High".into(),
            _ => value.replace('-', " "),
        }
    }
}

/// Equality preserves device units. Integer DPI rendering hints are deliberately
/// separate, so rounded DPCM hints cannot select a different device resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Resolution {
    pub x: u32,
    pub y: u32,
    pub dpcm: bool,
}
impl Resolution {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let (dimensions, dpcm) = value
            .strip_suffix("dpi")
            .map(|v| (v, false))
            .or_else(|| value.strip_suffix("dpcm").map(|v| (v, true)))
            .ok_or_else(|| anyhow::anyhow!("Invalid printer resolution units"))?;
        let mut axes = dimensions.split('x');
        let axis = |v: &str| -> anyhow::Result<u32> {
            anyhow::ensure!(
                !v.starts_with('0') && v.bytes().all(|b| b.is_ascii_digit()),
                "Invalid printer resolution"
            );
            let v: u32 = v.parse()?;
            anyhow::ensure!((1..=1_000_000).contains(&v), "Invalid printer resolution");
            Ok(v)
        };
        let x = axis(axes.next().unwrap_or_default())?;
        let y = axes.next().map(axis).transpose()?.unwrap_or(x);
        anyhow::ensure!(axes.next().is_none(), "Invalid printer resolution");
        Ok(Self { x, y, dpcm })
    }
    pub fn dpi(self) -> (u32, u32) {
        let convert = |v: u32| {
            if self.dpcm {
                (v * 254).div_ceil(100)
            } else {
                v
            }
        };
        (convert(self.x), convert(self.y))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub source: Option<String>,
    pub paper_type: Option<String>,
    pub quality: Option<String>,
    pub resolution: Option<String>,
}
impl Settings {
    /// Prepared clients use GTK-compatible rendering hints. Device resolution
    /// remains the original IPP value; DPCM is rounded upward only for these
    /// integer-DPI rasterization hints, never for the printer option.
    pub(crate) fn renderer_hints(&self) -> anyhow::Result<Vec<(&'static str, String)>> {
        self.validate()?;
        let mut hints = Vec::new();
        for (key, value) in [
            ("default-source", &self.source),
            ("media-type", &self.paper_type),
        ] {
            if let Some(value) = value {
                hints.push((key, value.clone()));
            }
        }
        if let Some(quality) = &self.quality {
            hints.push((
                "quality",
                match quality.as_str() {
                    "3" => "draft",
                    "4" => "normal",
                    "5" => "high",
                    _ => unreachable!("validated quality"),
                }
                .into(),
            ));
        }
        if let Some(resolution) = &self.resolution {
            let (x, y) = Resolution::parse(resolution)?.dpi();
            hints.extend([
                ("resolution", x.to_string()),
                ("resolution-x", x.to_string()),
                ("resolution-y", y.to_string()),
                ("canopa-printer-resolution", resolution.clone()),
            ]);
        }
        Ok(hints)
    }
    pub fn get(&self, field: Field) -> &Option<String> {
        match field {
            Field::Source => &self.source,
            Field::Type => &self.paper_type,
            Field::Quality => &self.quality,
            Field::Resolution => &self.resolution,
        }
    }
    pub fn set(&mut self, field: Field, value: Option<String>) {
        *match field {
            Field::Source => &mut self.source,
            Field::Type => &mut self.paper_type,
            Field::Quality => &mut self.quality,
            Field::Resolution => &mut self.resolution,
        } = value;
    }
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        for field in Field::ALL {
            if let Some(value) = self.get(field) {
                field.validate(value)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingChoice {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub choices: Vec<SettingChoice>,
    pub default: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controls {
    pub source: Control,
    pub paper_type: Control,
    pub quality: Control,
    pub resolution: Control,
}
impl Controls {
    pub fn get(&self, field: Field) -> &Control {
        match field {
            Field::Source => &self.source,
            Field::Type => &self.paper_type,
            Field::Quality => &self.quality,
            Field::Resolution => &self.resolution,
        }
    }
    pub(super) fn set(&mut self, field: Field, value: Control) {
        *match field {
            Field::Source => &mut self.source,
            Field::Type => &mut self.paper_type,
            Field::Quality => &mut self.quality,
            Field::Resolution => &mut self.resolution,
        } = value;
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        for field in Field::ALL {
            let control = self.get(field);
            anyhow::ensure!(
                control.choices.len() <= MAX_CHOICES,
                "Too many printer control choices"
            );
            let mut values = std::collections::HashSet::new();
            for choice in &control.choices {
                field.validate(&choice.value)?;
                anyhow::ensure!(
                    values.insert(&choice.value)
                        && !choice.label.is_empty()
                        && choice.label.len() <= 256
                        && !choice.label.chars().any(char::is_control),
                    "Invalid printer control choices"
                );
            }
            anyhow::ensure!(
                control.default.as_ref().is_none_or(|v| values.contains(v)),
                "Unavailable printer control default"
            );
        }
        Ok(())
    }
    pub(crate) fn accepts(&self, settings: &Settings) -> bool {
        settings.validate().is_ok()
            && Field::ALL.into_iter().all(|field| {
                settings
                    .get(field)
                    .as_ref()
                    .is_none_or(|v| self.get(field).choices.iter().any(|c| &c.value == v))
            })
    }
    pub fn defaults(&self) -> Settings {
        let mut result = Settings::default();
        for field in Field::ALL {
            result.set(field, self.get(field).default.clone());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_rendering_hints_keep_both_axes_without_changing_device_units() {
        let mut settings = Settings {
            source: Some("manual".into()),
            paper_type: Some("labels".into()),
            quality: Some("5".into()),
            resolution: Some("600x1200dpi".into()),
        };
        let hints = settings.renderer_hints().unwrap();
        assert_eq!(
            hints,
            [
                ("default-source", "manual"),
                ("media-type", "labels"),
                ("quality", "high"),
                ("resolution", "600"),
                ("resolution-x", "600"),
                ("resolution-y", "1200"),
                ("canopa-printer-resolution", "600x1200dpi")
            ]
            .map(|(k, v)| (k, v.into()))
        );
        settings.resolution = Some("118x236dpcm".into());
        let hints = settings.renderer_hints().unwrap();
        assert!(
            hints.contains(&("resolution-x", "300".into()))
                && hints.contains(&("resolution-y", "600".into()))
        );
        assert_eq!(settings.resolution.as_deref(), Some("118x236dpcm"));
        assert!(Settings::default().renderer_hints().unwrap().is_empty());
    }
    #[test]
    fn resolution_preserves_units_and_both_axes_and_rejects_injected_options() {
        for v in ["300dpi", "600x1200dpi", "118x236dpcm"] {
            Field::Resolution.validate(v).unwrap();
        }
        for v in [
            "0dpi",
            "600x0dpi",
            "600x1200x300dpi",
            "600",
            "600DPI",
            "-1dpi",
            "01dpi",
            "600dpi,manual",
        ] {
            assert!(Field::Resolution.validate(v).is_err(), "{v}");
        }
        assert!(Field::Source.validate("manual,labels").is_err());
        assert!(Field::Quality.validate("6").is_err());
    }
}
