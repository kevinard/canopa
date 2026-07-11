//! Shared external panel provider protocol types for Canopa desktop tools.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 3;

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
    pub items: Vec<PanelItem>,
    pub controls: Vec<PanelControl>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelItem {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    pub styled_body: Option<StyledText>,
    pub rich_body: Option<RichBody>,
    pub metadata: Vec<PanelMetadata>,
    pub actions: Vec<PanelAction>,
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
    fn protocol_messages_carry_explicit_version() {
        let message = ProviderResponse::replace_content(PanelContent::error(
            "Example Panel",
            "provider unavailable",
        ));
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("\"protocol_version\":3"));
        assert!(encoded.contains("provider unavailable"));
    }

    #[test]
    fn panel_items_can_carry_generic_styled_text() {
        let item = PanelItem {
            id: "1".to_string(),
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
            id: "1".to_string(),
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
