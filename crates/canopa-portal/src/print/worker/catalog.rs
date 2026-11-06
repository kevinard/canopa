//! Serializable printer choices. Missing capabilities stay unavailable.
use super::{Destination, JobOptions, Sides};
use serde::{Deserialize, Serialize};

pub const MAX_PRINTERS: usize = 256;
pub const MAX_CHOICES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterState {
    Idle,
    Processing,
    Stopped,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Printer {
    pub destination: Destination,
    pub label: String,
    pub location: String,
    pub is_default: bool,
    pub accepting: bool,
    pub state: PrinterState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Media {
    pub id: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub top_mm: f64,
    pub bottom_mm: f64,
    pub left_mm: f64,
    pub right_mm: f64,
}

/// Dimensions in CUPS's integer hundredths of a millimetre. No rounding of a
/// user's requested sheet is allowed at the device boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomSize {
    pub width: u32,
    pub height: u32,
}
impl CustomSize {
    pub fn validate(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=1_000_000).contains(&self.width) && (1..=1_000_000).contains(&self.height),
            "Invalid custom paper dimensions"
        );
        Ok(())
    }
    pub fn parse(width: &str, height: &str) -> anyhow::Result<Self> {
        fn dimension(text: &str) -> anyhow::Result<u32> {
            anyhow::ensure!(text.len() <= 16, "Invalid paper dimension");
            let mut parts = text.split('.');
            let whole = parts.next().unwrap_or_default();
            let fraction = parts.next().unwrap_or_default();
            anyhow::ensure!(
                !whole.is_empty()
                    && whole.bytes().all(|b| b.is_ascii_digit())
                    && fraction.len() <= 2
                    && fraction.bytes().all(|b| b.is_ascii_digit())
                    && parts.next().is_none(),
                "Use millimetres with at most two decimal places"
            );
            let multiplier = if fraction.len() == 1 { 10 } else { 1 };
            let whole: u32 = whole.parse()?;
            let fraction: u32 = if fraction.is_empty() {
                0
            } else {
                fraction.parse()?
            };
            whole
                .checked_mul(100)
                .and_then(|whole| whole.checked_add(fraction * multiplier))
                .ok_or_else(|| anyhow::anyhow!("Paper dimension exceeds limits"))
        }
        let size = Self {
            width: dimension(width)?,
            height: dimension(height)?,
        };
        size.validate()?;
        Ok(size)
    }
    pub(crate) fn from_media(media: &Media) -> anyhow::Result<Self> {
        media.page_setup().validate()?;
        let size = Self {
            width: (media.width_mm * 100.).round() as u32,
            height: (media.height_mm * 100.).round() as u32,
        };
        size.validate()?;
        anyhow::ensure!(
            (f64::from(size.width) / 100. - media.width_mm).abs() < 0.000001
                && (f64::from(size.height) / 100. - media.height_mm).abs() < 0.000001,
            "Custom paper precision exceeds device units"
        );
        Ok(size)
    }
    pub(crate) fn name(self) -> String {
        format!(
            "custom_canopa_{}x{}mm",
            f64::from(self.width) / 100.,
            f64::from(self.height) / 100.
        )
    }
    pub(crate) fn matches(self, media: &Media) -> bool {
        Self::from_media(media).is_ok_and(|size| size == self) && media.id == self.name()
    }
}
impl Media {
    pub fn page_setup(&self) -> crate::print::PageSetup {
        crate::print::PageSetup {
            name: Some(self.id.clone()),
            display_name: self
                .id
                .starts_with("custom_canopa_")
                .then(|| format!("{} × {} mm", self.width_mm, self.height_mm)),
            width: Some(self.width_mm),
            height: Some(self.height_mm),
            top: Some(self.top_mm),
            bottom: Some(self.bottom_mm),
            left: Some(self.left_mm),
            right: Some(self.right_mm),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub destination: Destination,
    pub copies: Option<(u32, u32)>,
    pub media: Vec<Media>,
    pub default_media: Option<String>,
    #[serde(default)]
    pub custom_media: bool,
    #[serde(default)]
    pub default_custom_media: Option<Media>,
    pub sides: Vec<Sides>,
    pub default_sides: Option<Sides>,
    pub colors: Vec<bool>,
    pub default_color: Option<bool>,
    pub formats: Vec<String>,
    #[serde(default)]
    pub controls: Box<super::Controls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferences: Option<Box<super::PreferenceSelection>>,
    #[serde(default)]
    pub authentication: crate::print::authentication::Requirement,
}

fn text(value: &str, limit: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.len() <= limit && !value.chars().any(char::is_control),
        "Invalid printer metadata text"
    );
    Ok(())
}

pub(super) fn printers(values: &[Printer]) -> anyhow::Result<()> {
    anyhow::ensure!(values.len() <= MAX_PRINTERS, "Too many configured printers");
    let mut identities = std::collections::HashSet::new();
    for value in values {
        value.destination.validate()?;
        text(&value.label, 256)?;
        text(&value.location, 256)?;
        anyhow::ensure!(
            identities.insert(value.destination.clone()),
            "Duplicate printer destination"
        );
    }
    Ok(())
}

impl Capabilities {
    pub fn validate(&self) -> anyhow::Result<()> {
        self.destination.validate()?;
        self.controls.validate()?;
        self.authentication.validate()?;
        if let Some(selection) = &self.preferences {
            let options = &selection.options;
            anyhow::ensure!(
                options.copies == 1
                    && options.custom_media.is_none()
                    && self.controls.accepts(&options.settings)
                    && options
                        .media
                        .as_ref()
                        .is_none_or(|id| self.media.iter().any(|m| &m.id == id))
                    && options.sides.is_none_or(|v| self.sides.contains(&v))
                    && options.color.is_none_or(|v| self.colors.contains(&v)),
                "Invalid initial printer selection"
            );
            if let Some(size) = selection.custom_size {
                size.validate()?;
                anyhow::ensure!(
                    self.custom_media && options.media.is_none(),
                    "Invalid initial custom paper"
                );
            }
        }
        if let Some((minimum, maximum)) = self.copies {
            anyhow::ensure!(
                minimum > 0 && minimum <= maximum && maximum <= 9999,
                "Invalid printer copies range"
            );
        }
        anyhow::ensure!(
            self.media.len() <= MAX_CHOICES
                && self.formats.len() <= MAX_CHOICES
                && self.sides.len() <= 3
                && self.colors.len() <= 2,
            "Printer capabilities exceed limits"
        );
        let mut identities = std::collections::HashSet::new();
        for media in &self.media {
            text(&media.id, 127)?;
            anyhow::ensure!(
                !media.id.is_empty() && identities.insert(media.id.as_str()),
                "Invalid printer media identity"
            );
            media.page_setup().validate()?;
        }
        for format in &self.formats {
            text(format, 127)?;
        }
        if let Some(media) = &self.default_custom_media {
            anyhow::ensure!(
                self.custom_media
                    && self.default_media.is_none()
                    && CustomSize::from_media(media)?.matches(media),
                "Invalid default custom paper"
            );
        }
        anyhow::ensure!(
            self.default_media
                .as_ref()
                .is_none_or(|id| self.media.iter().any(|media| &media.id == id)),
            "Unavailable default paper"
        );
        anyhow::ensure!(
            self.default_sides
                .is_none_or(|value| self.sides.contains(&value)),
            "Unavailable default duplex mode"
        );
        anyhow::ensure!(
            self.default_color
                .is_none_or(|value| self.colors.contains(&value)),
            "Unavailable default color mode"
        );
        Ok(())
    }

    /// A draft may choose printer defaults by leaving optional fields unset.
    /// Approval cannot invent an unadvertised explicit device choice.
    pub fn accepts(&self, options: &JobOptions) -> bool {
        self.validate().is_ok()
            && options.validate_media().is_ok()
            && self.controls.accepts(&options.settings)
            && (options.custom_media.is_none() || self.custom_media)
            && self
                .copies
                .is_some_and(|(minimum, maximum)| (minimum..=maximum).contains(&options.copies))
            && options
                .media
                .as_ref()
                .is_none_or(|id| self.media.iter().any(|media| &media.id == id))
            && options
                .sides
                .is_none_or(|sides| self.sides.contains(&sides))
            && options
                .color
                .is_none_or(|color| self.colors.contains(&color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Capabilities {
        Capabilities {
            destination: Destination {
                queue: "Raw".into(),
                instance: None,
            },
            copies: Some((1, 9999)),
            media: vec![],
            default_media: None,
            custom_media: false,
            default_custom_media: None,
            sides: vec![],
            default_sides: None,
            colors: vec![],
            default_color: None,
            formats: vec!["application/pdf".into()],
            controls: Default::default(),
            preferences: None,
            authentication: Default::default(),
        }
    }

    #[test]
    fn custom_dimensions_preserve_hundredth_mm_and_reject_rounded_or_unbounded_input() {
        assert_eq!(
            CustomSize::parse("125", "225.01").unwrap(),
            CustomSize {
                width: 12500,
                height: 22501
            }
        );
        assert_eq!(CustomSize::parse("76.2", "127").unwrap().width, 7620);
        for value in [
            "0",
            "-1",
            "NaN",
            "inf",
            "1e2",
            " 125",
            ".5",
            "1.001",
            "1..2",
            "１２５",
            "10000.01",
            "9999999999999999",
        ] {
            assert!(CustomSize::parse(value, "225").is_err(), "{value}");
        }
        assert!(CustomSize::parse("0.01", "10000").is_ok());
    }

    #[test]
    fn custom_paper_needs_explicit_capability_and_cannot_conflict_with_named_paper() {
        let size = CustomSize::parse("125", "225").unwrap();
        let media = Media {
            id: size.name(),
            width_mm: 125.,
            height_mm: 225.,
            top_mm: 3.18,
            bottom_mm: 12.7,
            left_mm: 3.18,
            right_mm: 3.18,
        };
        let mut options = JobOptions {
            custom_media: Some(media.clone()),
            ..Default::default()
        };
        let mut caps = raw();
        assert!(!caps.accepts(&options));
        caps.custom_media = true;
        assert!(caps.accepts(&options));
        options.media = Some("A4".into());
        assert!(!caps.accepts(&options));
        options.media = None;
        options.custom_media.as_mut().unwrap().width_mm = 125.001;
        assert!(!caps.accepts(&options));
        options.custom_media = Some(Media {
            id: "custom_canopa_126x225mm".into(),
            ..media.clone()
        });
        assert!(!caps.accepts(&options));
        caps.default_custom_media = Some(media);
        assert!(caps.validate().is_ok());
        caps.default_media = Some("A4".into());
        assert!(caps.validate().is_err());
    }

    #[test]
    fn missing_capabilities_allow_defaults_without_authorizing_explicit_device_choices() {
        let caps = raw();
        assert!(caps.accepts(&JobOptions::default()));
        assert!(!caps.accepts(&JobOptions {
            media: Some("iso_a4_210x297mm".into()),
            ..Default::default()
        }));
        assert!(!caps.accepts(&JobOptions {
            sides: Some(Sides::LongEdge),
            ..Default::default()
        }));
        assert!(!caps.accepts(&JobOptions {
            color: Some(true),
            ..Default::default()
        }));
        assert!(!caps.accepts(&JobOptions {
            copies: 10_000,
            ..Default::default()
        }));
    }

    #[test]
    fn malformed_geometry_and_defaults_are_rejected_before_they_reach_controls() {
        let mut caps = raw();
        caps.media.push(Media {
            id: "A4".into(),
            width_mm: 210.,
            height_mm: 297.,
            top_mm: 0.,
            bottom_mm: 0.,
            left_mm: 0.,
            right_mm: 0.,
        });
        caps.default_media = Some("missing".into());
        assert!(caps.validate().is_err());
        caps.default_media = Some("A4".into());
        assert!(caps.validate().is_ok());
        caps.media[0].width_mm = f64::NAN;
        assert!(caps.validate().is_err());
    }

    #[test]
    fn printer_instances_have_distinct_identities_and_duplicate_rows_are_rejected() {
        let base = Printer {
            destination: raw().destination,
            label: "Office".into(),
            location: String::new(),
            is_default: true,
            accepting: true,
            state: PrinterState::Idle,
        };
        assert!(printers(&[base.clone(), base.clone()]).is_err());
        let instance = Printer {
            destination: Destination {
                instance: Some("Duplex".into()),
                ..base.destination.clone()
            },
            ..base.clone()
        };
        assert!(printers(&[base, instance]).is_ok());
    }
}
