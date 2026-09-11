use crate::InternalNotificationId;
use crate::{
    Daemon, DeliveryOutcome, NotificationAction, NotificationBodyContent,
    NotificationStateSubscription, NotificationStatus, NotificationThread, UserRule,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct NotifyControl {
    daemon: Daemon,
    configuration_path: Option<std::path::PathBuf>,
}

impl NotifyControl {
    pub fn new(daemon: Daemon) -> Self {
        Self {
            daemon,
            configuration_path: None,
        }
    }

    pub fn with_configuration_path(mut self, path: std::path::PathBuf) -> Self {
        self.configuration_path = Some(path);
        self
    }

    pub async fn execute(&self, command: ControlCommand) -> anyhow::Result<ControlReply> {
        match command {
            ControlCommand::Status => Ok(ControlReply::Status(ControlStatus {
                available: true,
                do_not_disturb: self.daemon.do_not_disturb().await?,
                configuration: self.daemon.configuration_status(),
                presentation_error: self.daemon.presentation_error(),
                sender_publication_error: self.daemon.sender_publication_error(),
                timeout_records: self.daemon.timeout_record_count(),
            })),
            ControlCommand::DoNotDisturb => Ok(ControlReply::DoNotDisturb(
                self.daemon.do_not_disturb().await?,
            )),
            ControlCommand::ChangeDoNotDisturb { enabled } => Ok(ControlReply::DoNotDisturb(
                self.daemon.change_do_not_disturb(enabled).await?,
            )),
            ControlCommand::ReloadConfiguration { path } => {
                let path = path
                    .or_else(|| self.configuration_path.clone())
                    .or_else(|| self.daemon.configuration_status().path)
                    .ok_or_else(|| anyhow::anyhow!("no configuration path is configured"))?;
                self.daemon.reload_configuration_file(&path).await?;
                Ok(ControlReply::CommandApplied)
            }
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
            ControlCommand::SetCategoryPermission {
                app_identity,
                category,
                permission,
            } => {
                self.daemon
                    .upsert_user_rule(UserRule::for_app_category(
                        app_identity,
                        category,
                        permission.delivery_outcome(),
                    )?)
                    .await?;
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::ClearActiveNotifications => {
                match self.daemon.clear_active_notifications().await {
                    Ok(()) => Ok(ControlReply::CommandApplied),
                    Err(error) => match error.downcast_ref::<crate::PartialClearError>() {
                        Some(partial) => Ok(ControlReply::ClearPartiallyApplied {
                            dismissed: partial.dismissed,
                        }),
                        None => Err(error),
                    },
                }
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
            ControlCommand::ActivateNotification { notification_id } => {
                self.daemon.activate_notification(notification_id).await?;
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ControlCommand {
    Status,
    DoNotDisturb,
    ChangeDoNotDisturb {
        enabled: Option<bool>,
    },
    ReloadConfiguration {
        path: Option<std::path::PathBuf>,
    },
    ActiveNotifications,
    NotificationCenterProjection,
    SetAppPermission {
        app_identity: String,
        permission: AppPermissionControl,
    },
    SetCategoryPermission {
        app_identity: String,
        category: String,
        permission: AppPermissionControl,
    },
    ClearActiveNotifications,
    DeleteNotificationHistory,
    InvokeNotificationAction {
        notification_id: InternalNotificationId,
        action_key: String,
    },
    ActivateNotification {
        notification_id: InternalNotificationId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "reply", content = "value", rename_all = "kebab-case")]
pub enum ControlReply {
    Status(ControlStatus),
    DoNotDisturb(bool),
    ActiveNotifications(Vec<NotificationThread>),
    NotificationCenterProjection(NotificationCenterProjection),
    CommandApplied,
    ClearPartiallyApplied { dismissed: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlStatus {
    pub available: bool,
    pub do_not_disturb: bool,
    pub configuration: crate::ConfigurationStatus,
    pub presentation_error: Option<String>,
    pub sender_publication_error: Option<String>,
    /// Retained timeout bookkeeping records, not process memory or expired history.
    #[serde(default)]
    pub timeout_records: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterProjection {
    pub groups: Vec<NotificationCenterGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterGroup {
    pub app_identity: String,
    pub permission: AppPermissionControl,
    pub category_permissions: Vec<CategoryPermissionControl>,
    pub items: Vec<NotificationCenterItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryPermissionControl {
    pub category: String,
    pub permission: AppPermissionControl,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterItem {
    pub notification_id: InternalNotificationId,
    /// Live runtime-only content, never an entry in durable Notification History.
    #[serde(default)]
    pub transient: bool,
    /// Whether activating this live item invokes a sender action instead of dismissing it.
    #[serde(default)]
    pub has_default_action: bool,
    pub summary: String,
    pub body: String,
    pub body_content: NotificationBodyContent,
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
