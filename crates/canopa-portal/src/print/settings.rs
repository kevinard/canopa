use super::{PageChoices, dictionary, invalid, text};
use crate::requests::{Options, option};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zbus::{fdo, zvariant::Value};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
    ReversePortrait,
    ReverseLandscape,
}
impl Orientation {
    fn parse(value: &str, page_setup: bool) -> fdo::Result<Self> {
        match value {
            "portrait" => Ok(Self::Portrait),
            "landscape" => Ok(Self::Landscape),
            "reverse_portrait" if !page_setup => Ok(Self::ReversePortrait),
            "reverse_landscape" if !page_setup => Ok(Self::ReverseLandscape),
            "reverse-portrait" if page_setup => Ok(Self::ReversePortrait),
            "reverse-landscape" if page_setup => Ok(Self::ReverseLandscape),
            _ => Err(invalid("Invalid print orientation")),
        }
    }
    fn name(self, page_setup: bool) -> &'static str {
        match (self, page_setup) {
            (Self::Portrait, _) => "portrait",
            (Self::Landscape, _) => "landscape",
            (Self::ReversePortrait, false) => "reverse_portrait",
            (Self::ReverseLandscape, false) => "reverse_landscape",
            (Self::ReversePortrait, true) => "reverse-portrait",
            (Self::ReverseLandscape, true) => "reverse-landscape",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pages {
    #[default]
    All,
    Current,
    Selection,
    Ranges,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageRange {
    pub first: u32,
    pub last: u32,
}
impl PageRange {
    pub fn parse(value: &str) -> fdo::Result<Vec<Self>> {
        if value.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut ranges = Vec::new();
        for item in value.split(',') {
            if ranges.len() == 128 {
                return Err(invalid("Too many print page ranges"));
            }
            let (first, last) = item
                .trim()
                .split_once('-')
                .unwrap_or((item.trim(), item.trim()));
            let first = integer(first, 0, u32::MAX - 1)?;
            let last = integer(last, first, u32::MAX - 1)?;
            ranges.push(Self { first, last });
        }
        Ok(ranges)
    }
    /// CUPS uses one-based ranges; the portal and client layout use zero-based.
    pub fn cups(ranges: &[Self]) -> fdo::Result<String> {
        if ranges.len() > 128 {
            return Err(invalid("Too many print page ranges"));
        }
        ranges
            .iter()
            .map(|range| {
                let first = range
                    .first
                    .checked_add(1)
                    .ok_or_else(|| invalid("Page overflow"))?;
                let last = range
                    .last
                    .checked_add(1)
                    .ok_or_else(|| invalid("Page overflow"))?;
                if first > last {
                    return Err(invalid("Reversed print page range"));
                }
                Ok(if first == last {
                    first.to_string()
                } else {
                    format!("{first}-{last}")
                })
            })
            .collect::<fdo::Result<Vec<_>>>()
            .map(|parts| parts.join(","))
    }
}
fn integer(value: &str, minimum: u32, maximum: u32) -> fdo::Result<u32> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("Invalid print integer"));
    }
    let value: u32 = value
        .parse()
        .map_err(|_| invalid("Invalid print integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid("Print integer outside supported limits"));
    }
    Ok(value)
}
fn number(value: &str, minimum: f64, maximum: f64) -> fdo::Result<f64> {
    let value: f64 = value.parse().map_err(|_| invalid("Invalid print number"))?;
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(invalid("Print number outside supported limits"));
    }
    Ok(value)
}
fn boolean(value: &str) -> fdo::Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid("Print booleans must be true or false strings")),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub orientation: Orientation,
    pub copies: u32,
    pub collate: bool,
    pub reverse: bool,
    pub scale: f64,
    pub pages: Pages,
    pub ranges: Vec<PageRange>,
    pub page_set: String,
    pub number_up: u32,
    pub number_up_layout: String,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            orientation: Orientation::Portrait,
            copies: 1,
            collate: false,
            reverse: false,
            scale: 100.0,
            pages: Pages::All,
            ranges: Vec::new(),
            page_set: "all".into(),
            number_up: 1,
            number_up_layout: "lrtb".into(),
        }
    }
}
impl Layout {
    pub fn validate(&self, choices: PageChoices) -> fdo::Result<()> {
        if !(1..=9999).contains(&self.copies)
            || !(1..=16).contains(&self.number_up)
            || !self.scale.is_finite()
            || !(1.0..=1000.0).contains(&self.scale)
            || !["all", "even", "odd"].contains(&self.page_set.as_str())
            || ![
                "lrtb", "lrbt", "rltb", "rlbt", "tblr", "tbrl", "btlr", "btrl",
            ]
            .contains(&self.number_up_layout.as_str())
            || self.ranges.len() > 128
            || self
                .ranges
                .iter()
                .any(|r| r.first > r.last || r.last == u32::MAX)
            || self.pages == Pages::Ranges && self.ranges.is_empty()
            || self.pages == Pages::Current && !choices.current
            || self.pages == Pages::Selection && !choices.selection
        {
            return Err(invalid("Invalid or unavailable print layout"));
        }
        Ok(())
    }
    /// Only known layout fields are published; advisory printer/output strings
    /// cannot become destination authority or unchecked CUPS options.
    pub fn options(&self, choices: PageChoices) -> fdo::Result<Options> {
        self.validate(choices)?;
        let ranges = self
            .ranges
            .iter()
            .map(|range| {
                if range.first == range.last {
                    range.first.to_string()
                } else {
                    format!("{}-{}", range.first, range.last)
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let values = [
            ("orientation", self.orientation.name(false).to_owned()),
            ("n-copies", self.copies.to_string()),
            ("collate", self.collate.to_string()),
            ("reverse", self.reverse.to_string()),
            ("scale", self.scale.to_string()),
            (
                "print-pages",
                match self.pages {
                    Pages::All => "all",
                    Pages::Current => "current",
                    Pages::Selection => "selection",
                    Pages::Ranges => "ranges",
                }
                .into(),
            ),
            ("page-ranges", ranges),
            ("page-set", self.page_set.clone()),
            ("number-up", self.number_up.to_string()),
            ("number-up-layout", self.number_up_layout.clone()),
        ];
        Ok(values
            .into_iter()
            .map(|(key, value)| {
                (
                    key.into(),
                    Value::from(value).try_into().expect("owned string"),
                )
            })
            .collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsHints {
    pub layout: Layout,
    advisory: BTreeMap<String, String>,
}
impl SettingsHints {
    pub fn parse(values: Options) -> fdo::Result<Self> {
        dictionary(&values)?;
        let mut strings = BTreeMap::new();
        let mut budget = 0;
        for (key, value) in values {
            let value =
                String::try_from(value).map_err(|_| invalid("Print settings must be strings"))?;
            text(&key, &mut budget)?;
            text(&value, &mut budget)?;
            strings.insert(key, value);
        }
        let mut layout = Layout::default();
        if let Some(value) = strings.remove("orientation") {
            layout.orientation = Orientation::parse(&value, false)?;
        }
        if let Some(value) = strings.remove("n-copies") {
            layout.copies = integer(&value, 1, 9999)?;
        }
        if let Some(value) = strings.remove("collate") {
            layout.collate = boolean(&value)?;
        }
        if let Some(value) = strings.remove("reverse") {
            layout.reverse = boolean(&value)?;
        }
        if let Some(value) = strings.remove("scale") {
            layout.scale = number(&value, 1.0, 1000.0)?;
        }
        if let Some(value) = strings.remove("print-pages") {
            layout.pages = match value.as_str() {
                "all" => Pages::All,
                "current" => Pages::Current,
                "selection" => Pages::Selection,
                "ranges" => Pages::Ranges,
                _ => return Err(invalid("Invalid print page mode")),
            };
        }
        if let Some(value) = strings.remove("page-ranges") {
            layout.ranges = PageRange::parse(&value)?;
        }
        if let Some(value) = strings.remove("page-set") {
            layout.page_set = value;
        }
        if let Some(value) = strings.remove("number-up") {
            layout.number_up = integer(&value, 1, 16)?;
        }
        if let Some(value) = strings.remove("number-up-layout") {
            layout.number_up_layout = value;
        }
        layout.validate(PageChoices {
            current: true,
            selection: true,
        })?;
        Ok(Self {
            layout,
            advisory: strings,
        })
    }
    /// Hints can prefill controls, but must never be forwarded wholesale.
    pub fn advisory(&self, key: &str) -> Option<&str> {
        self.advisory.get(key).map(String::as_str)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSetup {
    pub ppd_name: Option<String>,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub orientation: Option<Orientation>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
}
impl PageSetup {
    pub fn parse(mut values: Options) -> fdo::Result<Self> {
        dictionary(&values)?;
        let orientation: Option<String> = option(&mut values, "Orientation")?;
        let setup = Self {
            ppd_name: option(&mut values, "PPDName")?,
            name: option(&mut values, "Name")?,
            display_name: option(&mut values, "DisplayName")?,
            orientation: orientation
                .map(|s| Orientation::parse(&s, true))
                .transpose()?,
            width: option(&mut values, "Width")?,
            height: option(&mut values, "Height")?,
            top: option(&mut values, "MarginTop")?,
            bottom: option(&mut values, "MarginBottom")?,
            left: option(&mut values, "MarginLeft")?,
            right: option(&mut values, "MarginRight")?,
        };
        setup.validate()?;
        Ok(setup)
    }
    pub fn validate(&self) -> fdo::Result<()> {
        let mut budget = 0;
        for value in [&self.ppd_name, &self.name, &self.display_name]
            .into_iter()
            .flatten()
        {
            text(value, &mut budget)?;
        }
        if self.width.is_some() != self.height.is_some()
            || [self.width, self.height]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || v <= 0.0 || v > 10000.0)
            || [self.top, self.bottom, self.left, self.right]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=10000.0).contains(&v))
            || self
                .width
                .is_some_and(|w| self.left.unwrap_or(0.0) + self.right.unwrap_or(0.0) >= w)
            || self
                .height
                .is_some_and(|h| self.top.unwrap_or(0.0) + self.bottom.unwrap_or(0.0) >= h)
        {
            return Err(invalid("Invalid print paper size or margins"));
        }
        Ok(())
    }
    pub fn options(&self) -> fdo::Result<Options> {
        self.validate()?;
        let mut values = Options::new();
        for (key, value) in [
            ("PPDName", &self.ppd_name),
            ("Name", &self.name),
            ("DisplayName", &self.display_name),
        ] {
            if let Some(value) = value {
                values.insert(
                    key.into(),
                    Value::from(value.clone()).try_into().expect("owned string"),
                );
            }
        }
        if let Some(value) = self.orientation {
            values.insert(
                "Orientation".into(),
                Value::from(value.name(true))
                    .try_into()
                    .expect("orientation"),
            );
        }
        for (key, value) in [
            ("Width", self.width),
            ("Height", self.height),
            ("MarginTop", self.top),
            ("MarginBottom", self.bottom),
            ("MarginLeft", self.left),
            ("MarginRight", self.right),
        ] {
            if let Some(value) = value {
                values.insert(key.into(), value.into());
            }
        }
        Ok(values)
    }
}
