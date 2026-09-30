//! The existing `canopa-v4` wire contract.

use serde::{Deserialize, Serialize};

mod frame;
pub use frame::Frame;

pub const PROTOCOL_VERSION: u16 = 4;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_INTERACTIVE_ELEMENTS: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPanelContent;

impl std::fmt::Display for InvalidPanelContent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid or excessive panel identifiers, actions, or controls")
    }
}

impl std::error::Error for InvalidPanelContent {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHandshake {
    pub protocol_version: u16,
    pub provider: String,
}

impl ProviderHandshake {
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            provider: provider.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderResponse {
    ReplaceContent {
        protocol_version: u16,
        content: PanelContent,
    },
}

impl ProviderResponse {
    pub fn replace_content(content: PanelContent) -> Self {
        Self::ReplaceContent {
            protocol_version: PROTOCOL_VERSION,
            content,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelContent {
    pub title: String,
    pub sections: Vec<PanelSection>,
    pub actions: Vec<PanelAction>,
    pub error: Option<String>,
}

impl PanelContent {
    /// Validate the same opaque interaction contract in providers and hosts.
    pub fn validate(&self) -> Result<(), InvalidPanelContent> {
        use std::collections::HashSet;
        let mut section_ids = HashSet::new();
        let mut item_ids = HashSet::new();
        let mut count = self.actions.len();
        validate_actions(self.actions.iter())?;
        for section in &self.sections {
            if section.id.is_empty() || !section_ids.insert(&section.id) {
                return Err(InvalidPanelContent);
            }
            if let Some(icon) = &section.icon {
                icon.validate()?;
            }
            let mut controls = HashSet::new();
            for control in &section.controls {
                if control.key.is_empty()
                    || control.label.is_empty()
                    || !controls.insert(&control.key)
                {
                    return Err(InvalidPanelContent);
                }
                let mut values = HashSet::new();
                for option in &control.options {
                    if option.label.is_empty() || !values.insert(&option.value) {
                        return Err(InvalidPanelContent);
                    }
                }
                count = count.saturating_add(control.options.len());
            }
            for item in &section.items {
                if item.id.is_empty() || !item_ids.insert(&item.id) {
                    return Err(InvalidPanelContent);
                }
                if let Some(attribution) = &item.attribution {
                    if attribution.label.trim().is_empty() {
                        return Err(InvalidPanelContent);
                    }
                    if let Some(icon) = &attribution.icon {
                        icon.validate()?;
                    }
                }
                for segment in item.rich_body.iter().flat_map(|body| &body.segments) {
                    if let RichBodySegment::Link { uri, .. } = segment
                        && (uri.is_empty() || uri.len() > 2048 || uri.chars().any(char::is_control))
                    {
                        return Err(InvalidPanelContent);
                    }
                }
                let links = item
                    .rich_body
                    .iter()
                    .flat_map(|body| &body.segments)
                    .filter_map(|segment| match segment {
                        RichBodySegment::Link { action, .. } => Some(action),
                        _ => None,
                    });
                count = count
                    .saturating_add(item.actions.len())
                    .saturating_add(links.clone().count());
                let actions = item.actions.iter().chain(links);
                validate_actions(actions.clone())?;
                if let Some(slider) = &item.slider {
                    slider.validate()?;
                    if actions.clone().any(|action| action.key == slider.key) {
                        return Err(InvalidPanelContent);
                    }
                    count = count.saturating_add(1);
                }
            }
        }
        if count > MAX_INTERACTIVE_ELEMENTS || item_ids.len() > 4096 || section_ids.len() > 1024 {
            return Err(InvalidPanelContent);
        }
        Ok(())
    }

    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            sections: Vec::new(),
            actions: Vec::new(),
            error: None,
        }
    }

    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            sections: Vec::new(),
            actions: Vec::new(),
            error: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelSection {
    pub id: String,
    pub title: String,
    /// Decorative icon beside the section title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PanelIcon>,
    pub items: Vec<PanelItem>,
    pub controls: Vec<PanelControl>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelItem {
    pub id: String,
    pub title: String,
    /// Optional source identity, separate from the message body and its images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<PanelAttribution>,
    pub body: Option<String>,
    pub styled_body: Option<StyledText>,
    pub rich_body: Option<RichBody>,
    pub metadata: Vec<PanelMetadata>,
    pub actions: Vec<PanelAction>,
    /// Optional bounded numeric control. Its values use the item's opaque action key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slider: Option<PanelSlider>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelSlider {
    pub key: String,
    pub label: String,
    pub min: u16,
    pub max: u16,
    pub step: u16,
    pub value: u16,
    pub unit: String,
}

impl PanelSlider {
    pub fn validate(&self) -> Result<(), InvalidPanelContent> {
        if self.key.is_empty()
            || self.label.trim().is_empty()
            || self.min >= self.max
            || self.step == 0
            || !(self.max - self.min).is_multiple_of(self.step)
            || !self.accepts(self.value)
        {
            return Err(InvalidPanelContent);
        }
        Ok(())
    }

    pub fn accepts(&self, value: u16) -> bool {
        self.step > 0
            && value >= self.min
            && value <= self.max
            && (value - self.min).is_multiple_of(self.step)
    }

    pub fn parse_value(&self, value: Option<&str>) -> Option<u16> {
        let text = value?;
        let value = text.parse::<u16>().ok()?;
        (self.accepts(value) && value.to_string() == text).then_some(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelAttribution {
    /// Also identifies the source when its icon cannot be loaded.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PanelIcon>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PanelIcon {
    LocalPath {
        path: String,
    },
    Theme {
        names: Vec<String>,
        theme: String,
        scale: u16,
    },
}

impl PanelIcon {
    pub fn validate(&self) -> Result<(), InvalidPanelContent> {
        let valid_name = |value: &str| {
            !value.is_empty()
                && value.len() <= 255
                && !matches!(value, "." | "..")
                && !value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        };
        let valid = match self {
            Self::LocalPath { path } => {
                path.starts_with('/') && path.len() <= 4096 && !path.contains('\0')
            }
            Self::Theme {
                names,
                theme,
                scale,
            } => {
                (1..=2).contains(&names.len())
                    && names.iter().all(|name| valid_name(name))
                    && valid_name(theme)
                    && theme.is_ascii()
                    && !theme.bytes().any(|c| c.is_ascii_whitespace() || c == b',')
                    && (1..=8).contains(scale)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(InvalidPanelContent)
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichBody {
    pub segments: Vec<RichBodySegment>,
}

impl RichBody {
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RichBodySegment {
    /// A labeled provider action. URI is assistive-technology metadata; activation stays opaque.
    Link {
        uri: String,
        action: PanelAction,
        style: StyledTextStyle,
    },
    Text {
        text: String,
        style: StyledTextStyle,
    },
    Image {
        image: RichBodyImage,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichBodyImage {
    pub source: RichBodyImageSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RichBodyImageSource {
    LocalPath { path: String },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyledText {
    pub runs: Vec<StyledTextRun>,
}

impl StyledText {
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            Self::default()
        } else {
            Self {
                runs: vec![StyledTextRun::plain(text)],
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.text.is_empty())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyledTextRun {
    pub text: String,
    pub style: StyledTextStyle,
}

impl StyledTextRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: StyledTextStyle::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyledTextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelMetadata {
    pub key: String,
    pub label: String,
    /// Empty for a presence indicator: hosts may show a marker and expose `label`
    /// through a tooltip and accessibility. Nonempty values remain labeled text.
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelAction {
    pub key: String,
    pub label: String,
    pub value: Option<String>,
}

impl PanelAction {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: None,
        }
    }
}

fn validate_actions<'a>(
    actions: impl IntoIterator<Item = &'a PanelAction>,
) -> Result<(), InvalidPanelContent> {
    let mut keys = std::collections::HashSet::new();
    for action in actions {
        if action.key.is_empty()
            || action.label.is_empty()
            || !keys.insert((&action.key, &action.value))
        {
            return Err(InvalidPanelContent);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelControl {
    pub key: String,
    pub label: String,
    pub value: String,
    pub options: Vec<PanelControlOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelControlOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractionEvent {
    Invoke {
        target: InteractionTarget,
        action_key: String,
        value: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum InteractionTarget {
    Content,
    Section { id: String },
    Item { id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_body_links_carry_opaque_actions_and_share_item_validation() {
        let input = serde_json::json!({
            "title":"Links", "sections":[{"id":"s", "title":"Section", "controls":[], "items":[{
                "id":"i", "title":"Message", "body":null, "styled_body":null, "metadata":[], "actions":[],
                "rich_body":{"segments":[
                    {"kind":"text", "text":"Before ", "style":{"bold":false,"italic":false,"underline":false}},
                    {"kind":"link", "uri":"https://example.org/page", "action":{"key":"opaque", "label":"Read the page", "value":"revision-owned-target"}, "style":{"bold":true,"italic":false,"underline":false}},
                    {"kind":"text", "text":" after", "style":{"bold":false,"italic":false,"underline":false}}
                ]}
            }]}], "actions":[], "error":null
        });
        let mut content: PanelContent = serde_json::from_value(input.clone()).unwrap();
        content.validate().unwrap();
        assert_eq!(serde_json::to_value(&content).unwrap(), input);
        content.sections[0].items[0].actions.push(PanelAction {
            key: "opaque".into(),
            label: "Duplicate".into(),
            value: Some("revision-owned-target".into()),
        });
        assert!(
            content.validate().is_err(),
            "link and item controls must not collide"
        );
    }

    #[test]
    fn protocol_messages_carry_explicit_version() {
        let message = ProviderResponse::replace_content(PanelContent::error(
            "Example Panel",
            "provider unavailable",
        ));
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("\"protocol_version\":4"));
        assert!(encoded.contains("provider unavailable"));
    }

    #[test]
    fn panel_items_can_carry_generic_styled_text() {
        let item = PanelItem {
            slider: None,
            id: "1".to_string(),
            attribution: None,
            title: "Inbox".to_string(),
            body: Some("One unread".to_string()),
            styled_body: Some(StyledText {
                runs: vec![StyledTextRun {
                    text: "One".to_string(),
                    style: StyledTextStyle {
                        bold: true,
                        ..StyledTextStyle::default()
                    },
                }],
            }),
            rich_body: None,
            metadata: Vec::new(),
            actions: Vec::new(),
        };

        let encoded = serde_json::to_string(&item).unwrap();
        assert!(encoded.contains("\"styled_body\""));
        assert!(encoded.contains("\"bold\":true"));
    }

    #[test]
    fn panel_items_can_carry_ordered_rich_body_content() {
        let item = PanelItem {
            slider: None,
            id: "1".to_string(),
            attribution: None,
            title: "Report".to_string(),
            body: Some("Open chart".to_string()),
            styled_body: None,
            rich_body: Some(RichBody {
                segments: vec![
                    RichBodySegment::Text {
                        text: "Open ".to_string(),
                        style: StyledTextStyle {
                            bold: true,
                            ..StyledTextStyle::default()
                        },
                    },
                    RichBodySegment::Image {
                        image: RichBodyImage {
                            source: RichBodyImageSource::LocalPath {
                                path: "/tmp/chart.png".to_string(),
                            },
                            alt: Some("chart".to_string()),
                        },
                    },
                ],
            }),
            metadata: Vec::new(),
            actions: Vec::new(),
        };

        let encoded = serde_json::to_string(&item).unwrap();

        assert!(encoded.contains("\"rich_body\""));
        assert!(encoded.contains("\"kind\":\"text\""));
        assert!(encoded.contains("\"bold\":true"));
        assert!(encoded.contains("\"kind\":\"image\""));
        assert!(encoded.contains("\"path\":\"/tmp/chart.png\""));
        assert!(!encoded.contains("<img"));
    }
}
