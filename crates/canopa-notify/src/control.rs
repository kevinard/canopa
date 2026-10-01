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
            ControlCommand::Status => {
                let (timeout_retrying, timeout_failed) = self.daemon.timeout_failure_counts();
                Ok(ControlReply::Status(ControlStatus {
                    available: true,
                    do_not_disturb: self.daemon.do_not_disturb().await?,
                    configuration: self.daemon.configuration_status(),
                    presentation_error: self.daemon.presentation_error(),
                    sender_publication_error: self.daemon.sender_publication_error(),
                    timeout_records: self.daemon.timeout_record_count(),
                    timeout_retrying,
                    timeout_failed,
                    link_opening: self.daemon.link_opening_status(),
                }))
            }
            ControlCommand::DoNotDisturb => Ok(ControlReply::DoNotDisturb(
                self.daemon.do_not_disturb().await?,
            )),
            ControlCommand::ChangeDoNotDisturb { enabled } => Ok(ControlReply::DoNotDisturb(
                self.daemon.change_do_not_disturb(enabled).await?,
            )),
            ControlCommand::ReloadConfiguration { path } => {
                if let Some(path) = path {
                    self.daemon.reload_configuration_file(&path).await?;
                } else if self.daemon.configuration_status().source.is_none()
                    && let Some(path) = &self.configuration_path
                {
                    self.daemon.reload_configuration_file(path).await?;
                } else {
                    self.daemon.reload_configuration_source(None).await?;
                }
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::ReloadConfigurationSource { source } => {
                self.daemon
                    .reload_configuration_source(Some(source))
                    .await?;
                Ok(ControlReply::CommandApplied)
            }
            ControlCommand::ActiveNotifications => Ok(ControlReply::ActiveNotifications(
                self.daemon.retained_active_threads().await?,
            )),
            ControlCommand::ActiveNotificationCount => Ok(ControlReply::ActiveNotificationCount(
                self.daemon.retained_active_count().await?,
            )),
            ControlCommand::NotificationCenterProjection => {
                Ok(ControlReply::NotificationCenterProjection(
                    self.notification_center_projection().await?,
                ))
            }
            ControlCommand::NotificationCenterBatch { after, limit } => {
                anyhow::ensure!(
                    (1..=16).contains(&limit),
                    "center batch limit must be between 1 and 16"
                );
                Ok(ControlReply::NotificationCenterBatch(
                    self.daemon.notification_center_batch(after, limit).await?,
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
            ControlCommand::InteractWithNotificationCenter { target, action } => {
                self.daemon
                    .handle_notification_center_interaction(target, action)
                    .await?;
                Ok(ControlReply::CommandApplied)
            }
        }
    }

    /// Capture read policy once for one Center frame, then fetch at most sixteen
    /// rows per read. A fresh reader observes subsequent policy/state changes.
    pub async fn notification_center_reader(
        &self,
    ) -> anyhow::Result<crate::NotificationCenterReader> {
        self.daemon.runtime.notification_center_reader().await
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
    ReloadConfigurationSource {
        source: canopa_config::Request,
    },
    ReloadConfiguration {
        path: Option<std::path::PathBuf>,
    },
    ActiveNotifications,
    ActiveNotificationCount,
    NotificationCenterProjection,
    NotificationCenterBatch {
        after: Option<NotificationCenterCursor>,
        limit: u8,
    },
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
    InteractWithNotificationCenter {
        target: NotificationCenterTarget,
        action: NotificationCenterAction,
    },
}

/// The displayed revision in one daemon lifetime. Obtain from a center projection;
/// keep it unchanged through queued interactions. This is not an authorization token.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationCenterTarget {
    pub(crate) instance: [u8; 16],
    pub(crate) notification_id: InternalNotificationId,
    pub(crate) revision: i64,
}

impl NotificationCenterTarget {
    pub fn notification_id(&self) -> InternalNotificationId {
        self.notification_id
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NotificationCenterAction {
    /// Open a captured body segment without invoking a sender action or acknowledging it.
    OpenLink {
        segment_index: usize,
    },
    /// Acknowledge retained content, dismissing it if its sender lifecycle is still active.
    Acknowledge,
    /// Invoke the captured revision's default action, or dismiss if it has none.
    Activate,
    InvokeAction {
        action_key: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "reply", content = "value", rename_all = "kebab-case")]
pub enum ControlReply {
    Status(ControlStatus),
    DoNotDisturb(bool),
    ActiveNotifications(Vec<NotificationThread>),
    ActiveNotificationCount(u64),
    NotificationCenterProjection(NotificationCenterProjection),
    NotificationCenterBatch(NotificationCenterBatch),
    CommandApplied,
    ClearPartiallyApplied { dismissed: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlStatus {
    #[serde(default)]
    pub link_opening: crate::LinkOpeningStatus,
    pub available: bool,
    pub do_not_disturb: bool,
    pub configuration: crate::ConfigurationStatus,
    pub presentation_error: Option<String>,
    pub sender_publication_error: Option<String>,
    /// Retained timeout bookkeeping records, not process memory or expired history.
    #[serde(default)]
    pub timeout_records: usize,
    /// Live expiry registrations awaiting a retry or retry result.
    #[serde(default)]
    pub timeout_retrying: usize,
    /// Registrations that exhausted bounded retries; explicit completion or
    /// replacement retires them. No notification content is exposed here.
    #[serde(default)]
    pub timeout_failed: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterProjection {
    pub groups: Vec<NotificationCenterGroup>,
    #[serde(default)]
    pub icon_settings: crate::BannerIconSettings,
}

/// Exclusive keyset position in newest-first creation order, with ID as tie-breaker. It carries
/// no notification content and does not authorize interactions with an item.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct NotificationCenterCursor {
    pub(crate) created_at: i64,
    pub(crate) id: InternalNotificationId,
}

impl NotificationCenterCursor {
    pub(crate) fn for_thread(thread: &NotificationThread) -> Self {
        Self {
            created_at: thread.created_at,
            id: thread.id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationCenterBatch {
    pub projection: NotificationCenterProjection,
    /// Cursor of the last returned item; None means that no more items were found.
    pub last: Option<NotificationCenterCursor>,
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
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub app_icon: Option<crate::NotificationVisualReference>,
    #[serde(default)]
    pub requires_attention: bool,
    pub notification_id: InternalNotificationId,
    pub target: NotificationCenterTarget,
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
