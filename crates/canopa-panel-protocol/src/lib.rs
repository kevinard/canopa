//! Shared external panel provider protocol types for Canopa desktop tools.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderHandshake {
    pub protocol_version: u16,
    pub provider: String,
}

impl ProviderHandshake {
    pub fn canopa_notify() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            provider: "canopa-notify".to_string(),
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
    pub groups: Vec<PanelGroup>,
    pub error: Option<String>,
}

impl PanelContent {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            title: "Canopa Notify".to_string(),
            groups: Vec::new(),
            error: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelGroup {
    pub app_identity: String,
    pub items: Vec<PanelItem>,
    pub permission: Option<AppPermission>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelItem {
    pub notification_id: i64,
    pub summary: String,
    pub body: String,
    pub status: String,
    pub delivery_outcome: String,
    pub actions: Vec<PanelAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppPermission {
    Allow,
    Silence,
    Drop,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractionEvent {
    SetAppPermission {
        app_identity: String,
        permission: AppPermission,
    },
    ClearActiveNotifications,
    DeleteNotificationHistory,
    InvokeNotificationAction {
        notification_id: i64,
        action_key: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_messages_carry_explicit_version() {
        let message = ProviderResponse::replace_content(PanelContent::error("daemon unavailable"));
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("\"protocol_version\":1"));
        assert!(encoded.contains("daemon unavailable"));
    }
}
