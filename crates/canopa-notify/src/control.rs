use crate::{
    Daemon, DeliveryOutcome, NotificationAction, NotificationBodyMarkup,
    NotificationStateSubscription, NotificationStatus, NotificationThread, UserRule,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct NotifyControl {
    daemon: Daemon,
}

impl NotifyControl {
    pub fn new(daemon: Daemon) -> Self {
        Self { daemon }
    }

    pub async fn execute(&self, command: ControlCommand) -> anyhow::Result<ControlReply> {
        match command {
            ControlCommand::Status => Ok(ControlReply::Status(ControlStatus { available: true })),
            ControlCommand::ActiveNotifications => Ok(ControlReply::ActiveNotifications(
                self.daemon.snapshot().await?.active,
            )),
            ControlCommand::NotificationCenterProjection => {
                Ok(ControlReply::NotificationCenterProjection(
                    self.notification_center_projection().await?,
                ))
            }
            ControlCommand::SetAppPermission {
                app_identity,
                permission,
            } => {
                self.daemon
                    .upsert_user_rule(UserRule::for_app(
                        app_identity,
                        permission.delivery_outcome(),
                    ))
                    .await?;
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::ClearActiveNotifications => {
                self.daemon.clear_active_notifications().await?;
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::DeleteNotificationHistory => {
                self.daemon.delete_notification_history().await?;
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::InvokeNotificationAction {
                notification_id,
                action_key,
            } => {
                self.daemon
                    .invoke_action(notification_id, &action_key)
                    .await?;
                Ok(ControlReply::CommandApplied)
            }
        }
    }

    pub fn subscribe_notification_state(&self) -> NotificationStateSubscription {
        self.daemon.subscribe_notification_state()
    }

    async fn notification_center_projection(&self) -> anyhow::Result<NotificationCenterProjection> {
        self.daemon.notification_center_projection().await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlCommand {
    Status,
    ActiveNotifications,
    NotificationCenterProjection,
    SetAppPermission {
        app_identity: String,
        permission: AppPermissionControl,
    },
    ClearActiveNotifications,
    DeleteNotificationHistory,
    InvokeNotificationAction {
        notification_id: i64,
        action_key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlReply {
    Status(ControlStatus),
    ActiveNotifications(Vec<NotificationThread>),
    NotificationCenterProjection(NotificationCenterProjection),
    CommandApplied,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlStatus {
    pub available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterProjection {
    pub groups: Vec<NotificationCenterGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterGroup {
    pub app_identity: String,
    pub permission: AppPermissionControl,
    pub items: Vec<NotificationCenterItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterItem {
    pub notification_id: i64,
    pub summary: String,
    pub body: String,
    pub body_markup: NotificationBodyMarkup,
    pub status: NotificationStatus,
    pub delivery_outcome: DeliveryOutcome,
    pub actions: Vec<NotificationAction>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppPermissionControl {
    Allow,
    Silence,
    Drop,
}

const APP_PERMISSION_CONTROL_OPTIONS: [AppPermissionControl; 3] = [
    AppPermissionControl::Allow,
    AppPermissionControl::Silence,
    AppPermissionControl::Drop,
];

impl AppPermissionControl {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Silence => "silence",
            Self::Drop => "drop",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Allow => "Allow",
            Self::Silence => "Silence",
            Self::Drop => "Drop",
        }
    }

    pub fn options() -> &'static [Self] {
        &APP_PERMISSION_CONTROL_OPTIONS
    }

    fn delivery_outcome(self) -> DeliveryOutcome {
        match self {
            Self::Allow => DeliveryOutcome::Shown,
            Self::Silence => DeliveryOutcome::Silenced,
            Self::Drop => DeliveryOutcome::Dropped,
        }
    }

    pub(crate) fn from_delivery_outcome(outcome: DeliveryOutcome) -> Self {
        match outcome {
            DeliveryOutcome::Shown => Self::Allow,
            DeliveryOutcome::Silenced => Self::Silence,
            DeliveryOutcome::Dropped => Self::Drop,
        }
    }
}
