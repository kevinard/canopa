use crate::{
    Daemon, DeliveryOutcome, NotificationAction, NotificationSnapshot, NotificationStatus,
    NotificationThread, UserRule,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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
                let snapshot = self.daemon.snapshot().await?;
                let user_rules = self.daemon.user_rules().await?;
                Ok(ControlReply::NotificationCenterProjection(
                    projection_from_snapshot(snapshot, &user_rules),
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

impl AppPermissionControl {
    fn delivery_outcome(self) -> DeliveryOutcome {
        match self {
            Self::Allow => DeliveryOutcome::Shown,
            Self::Silence => DeliveryOutcome::Silenced,
            Self::Drop => DeliveryOutcome::Dropped,
        }
    }

    fn from_delivery_outcome(outcome: DeliveryOutcome) -> Self {
        match outcome {
            DeliveryOutcome::Shown => Self::Allow,
            DeliveryOutcome::Silenced => Self::Silence,
            DeliveryOutcome::Dropped => Self::Drop,
        }
    }
}

fn projection_from_snapshot(
    snapshot: NotificationSnapshot,
    user_rules: &[UserRule],
) -> NotificationCenterProjection {
    let mut grouped: BTreeMap<String, Vec<NotificationCenterItem>> = BTreeMap::new();
    let app_permissions: HashMap<_, _> = user_rules
        .iter()
        .filter(|rule| rule.urgency.is_none())
        .map(|rule| {
            (
                rule.app_identity.as_str(),
                AppPermissionControl::from_delivery_outcome(rule.outcome),
            )
        })
        .collect();

    for thread in snapshot.history {
        grouped
            .entry(thread.app_identity.clone())
            .or_default()
            .push(NotificationCenterItem {
                notification_id: thread.id,
                summary: thread.summary,
                body: thread.body,
                status: thread.status,
                delivery_outcome: thread.delivery_outcome,
                actions: thread.actions,
            });
    }

    NotificationCenterProjection {
        groups: grouped
            .into_iter()
            .map(|(app_identity, items)| NotificationCenterGroup {
                permission: app_permissions
                    .get(app_identity.as_str())
                    .copied()
                    .unwrap_or(AppPermissionControl::Allow),
                app_identity,
                items,
            })
            .collect(),
    }
}
