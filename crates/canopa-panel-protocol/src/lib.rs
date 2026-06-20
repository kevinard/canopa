//! Shared external panel provider protocol types for Canopa desktop tools.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;

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
    pub metadata: Vec<PanelMetadata>,
    pub actions: Vec<PanelAction>,
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
        assert!(encoded.contains("\"protocol_version\":1"));
        assert!(encoded.contains("provider unavailable"));
    }
}
