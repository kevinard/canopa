use canopa_notify::InternalNotificationId;
use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, NotificationBodyContent,
    NotificationBodyImage, NotificationBodyImageSource, NotificationBodySegment,
    NotificationCenterProjection, NotifyControl, normalize_notification_category,
};
use canopa_panel_protocol::{
    InteractionEvent, InteractionTarget, PanelAction, PanelContent, PanelControl,
    PanelControlOption, PanelItem, PanelMetadata, PanelSection, RichBody, RichBodyImage,
    RichBodyImageSource, RichBodySegment, StyledText, StyledTextRun, StyledTextStyle,
};
mod transport;

pub(crate) use transport::{ControlSocket, run_ctl, run_panel_provider};
use transport::{control_reply_line, panel_response_line};

const PANEL_NOTIFICATION_ACTION_KEY: &str = "canopa-notify:notification-action";
const PANEL_ACTIVATE_ITEM_KEY: &str = "canopa-notify:activate-item";
const PANEL_APP_PERMISSION_KEY: &str = "canopa-notify:app-permission";
const PANEL_CATEGORY_PERMISSION_KEY_PREFIX: &str = "canopa-notify:category-permission:";
const PANEL_CLEAR_ACTIVE_KEY: &str = "canopa-notify:clear-active";
const PANEL_DELETE_HISTORY_KEY: &str = "canopa-notify:delete-history";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CtlRequest {
    Status,
    ActiveJson,
    ActiveCount,
    Command(ControlCommand),
}

impl CtlRequest {
    fn line(&self) -> String {
        let line = match self {
            Self::Status => SocketControlRequest::Status.line(),
            Self::ActiveJson => SocketControlRequest::ActiveJson.line(),
            Self::ActiveCount => SocketControlRequest::ActiveCount.line(),
            Self::Command(command) => {
                return format!(
                    "control {}\n",
                    serde_json::to_string(command).expect("control command serialization")
                );
            }
        };
        line.to_string()
    }

    fn reply_text(&self, response: &str) -> anyhow::Result<String> {
        if response.starts_with("error ") {
            if response == transport::CONTROL_REPLY_TOO_LARGE {
                anyhow::bail!(
                    "Canopa Notify control reply exceeds 8 MiB; command outcome may be unknown"
                );
            }
            anyhow::bail!(
                "Canopa Notify request failed; inspect daemon status; command outcome may be unknown"
            );
        }
        match self {
            Self::Status if response == "Canopa Notify daemon available\n" => Ok(response.into()),
            Self::ActiveCount => {
                let count = response
                    .strip_suffix(" active notifications\n")
                    .and_then(|count| count.parse::<usize>().ok())
                    .ok_or_else(invalid_control_reply)?;
                let expected = format!("{count} active notifications\n");
                anyhow::ensure!(response == expected, invalid_control_reply());
                Ok(expected)
            }
            Self::ActiveJson => {
                serde_json::from_str::<Vec<canopa_notify::NotificationThread>>(response)
                    .map_err(|_| invalid_control_reply())?;
                Ok(response.into())
            }
            Self::Command(command) => {
                let reply: ControlReply =
                    serde_json::from_str(response).map_err(|_| invalid_control_reply())?;
                let matches = matches!(
                    (command, &reply),
                    (ControlCommand::Status, ControlReply::Status(_))
                        | (
                            ControlCommand::DoNotDisturb
                                | ControlCommand::ChangeDoNotDisturb { .. },
                            ControlReply::DoNotDisturb(_)
                        )
                        | (
                            ControlCommand::ActiveNotifications,
                            ControlReply::ActiveNotifications(_)
                        )
                        | (
                            ControlCommand::NotificationCenterProjection,
                            ControlReply::NotificationCenterProjection(_)
                        )
                        | (
                            ControlCommand::ClearActiveNotifications,
                            ControlReply::ClearPartiallyApplied { .. }
                        )
                        | (
                            ControlCommand::ReloadConfiguration { .. }
                                | ControlCommand::SetAppPermission { .. }
                                | ControlCommand::SetCategoryPermission { .. }
                                | ControlCommand::ClearActiveNotifications
                                | ControlCommand::DeleteNotificationHistory
                                | ControlCommand::InvokeNotificationAction { .. }
                                | ControlCommand::ActivateNotification { .. },
                            ControlReply::CommandApplied
                        )
                );
                anyhow::ensure!(matches, invalid_control_reply());
                match reply {
                    ControlReply::DoNotDisturb(enabled) => {
                        Ok(if enabled { "on\n" } else { "off\n" }.into())
                    }
                    ControlReply::CommandApplied => Ok("ok\n".into()),
                    ControlReply::ClearPartiallyApplied { dismissed } => anyhow::bail!(
                        "clear stopped after {dismissed} committed dismissals; remaining notifications were not cleared"
                    ),
                    ControlReply::Status(status) => {
                        Ok(serde_json::to_string_pretty(&status)? + "\n")
                    }
                    reply => Ok(serde_json::to_string(&reply)? + "\n"),
                }
            }
            _ => Err(invalid_control_reply()),
        }
    }
}

fn invalid_control_reply() -> anyhow::Error {
    anyhow::anyhow!(
        "invalid or incomplete Canopa Notify control reply; command outcome may be unknown"
    )
}

async fn control_socket_response(
    request: SocketControlRequest,
    control: &NotifyControl,
) -> anyhow::Result<String> {
    let Some(command) = request.command() else {
        return Ok("error unsupported control request\n".to_string());
    };
    request.format_reply(control.execute(command).await?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SocketControlRequest {
    Status,
    ActiveJson,
    ActiveCount,
    PanelSnapshot,
    PanelSubscribe,
    InvokeNotificationAction {
        notification_id: InternalNotificationId,
        action_key: String,
    },
    PanelInteraction(ControlCommand),
    TypedCommand(ControlCommand),
    Unsupported,
}

struct SocketControlRequestSpec {
    request: SocketControlRequest,
    line: &'static str,
    command: Option<ControlCommand>,
    format_reply: Option<fn(ControlReply) -> anyhow::Result<String>>,
}

const SOCKET_CONTROL_REQUESTS: &[SocketControlRequestSpec] = &[
    SocketControlRequestSpec {
        request: SocketControlRequest::Status,
        line: "status\n",
        command: Some(ControlCommand::Status),
        format_reply: Some(format_status_reply),
    },
    SocketControlRequestSpec {
        request: SocketControlRequest::ActiveJson,
        line: "active-json\n",
        command: Some(ControlCommand::ActiveNotifications),
        format_reply: Some(format_active_json_reply),
    },
    SocketControlRequestSpec {
        request: SocketControlRequest::ActiveCount,
        line: "active\n",
        command: Some(ControlCommand::ActiveNotifications),
        format_reply: Some(format_active_count_reply),
    },
    SocketControlRequestSpec {
        request: SocketControlRequest::PanelSnapshot,
        line: "panel-snapshot\n",
        command: Some(ControlCommand::NotificationCenterProjection),
        format_reply: Some(format_panel_snapshot_reply),
    },
    SocketControlRequestSpec {
        request: SocketControlRequest::PanelSubscribe,
        line: "panel-subscribe\n",
        command: None,
        format_reply: None,
    },
];

impl SocketControlRequest {
    fn kind(&self) -> &'static str {
        match self {
            Self::TypedCommand(_) => "control",
            Self::PanelInteraction(_) => "panel-interaction",
            Self::InvokeNotificationAction { .. } => "invoke-action",
            _ => self
                .spec()
                .map(SocketControlRequestSpec::request_token)
                .unwrap_or("unsupported"),
        }
    }

    fn parse(request: &str) -> Self {
        if let Some(json) = request.strip_prefix("control ") {
            return serde_json::from_str(json)
                .map(Self::TypedCommand)
                .unwrap_or(Self::Unsupported);
        }
        if let Some(rest) = request.strip_prefix("invoke-action ") {
            return parse_invoke_action_request(rest).unwrap_or(Self::Unsupported);
        }
        if let Some(json) = request.strip_prefix("panel-interaction ") {
            return parse_panel_interaction_request(json).unwrap_or(Self::Unsupported);
        }

        SOCKET_CONTROL_REQUESTS
            .iter()
            .find(|spec| spec.request_token() == request)
            .map(|spec| spec.request.clone())
            .unwrap_or(Self::Unsupported)
    }

    fn line(&self) -> &'static str {
        self.spec().map(|spec| spec.line).unwrap_or("unsupported\n")
    }

    fn command(&self) -> Option<ControlCommand> {
        match self {
            Self::InvokeNotificationAction {
                notification_id,
                action_key,
            } => Some(ControlCommand::InvokeNotificationAction {
                notification_id: *notification_id,
                action_key: action_key.clone(),
            }),
            Self::PanelInteraction(command) | Self::TypedCommand(command) => Some(command.clone()),
            _ => self.spec().and_then(|spec| spec.command.clone()),
        }
    }

    fn format_reply(&self, reply: ControlReply) -> anyhow::Result<String> {
        if matches!(self, Self::TypedCommand(_)) {
            return control_reply_line(&reply);
        }
        if matches!(
            self,
            Self::InvokeNotificationAction { .. } | Self::PanelInteraction(_)
        ) {
            return format_command_applied_reply(reply);
        }

        let Some(format_reply) = self.spec().and_then(|spec| spec.format_reply) else {
            return Err(unexpected_control_reply());
        };
        format_reply(reply)
    }

    fn spec(&self) -> Option<&'static SocketControlRequestSpec> {
        SOCKET_CONTROL_REQUESTS
            .iter()
            .find(|spec| &spec.request == self)
    }
}

fn parse_invoke_action_request(rest: &str) -> Option<SocketControlRequest> {
    let (notification_id, action_key) = rest.split_once(' ')?;
    let notification_id = notification_id.parse().ok()?;
    (!action_key.is_empty()).then(|| SocketControlRequest::InvokeNotificationAction {
        notification_id,
        action_key: action_key.to_string(),
    })
}

fn parse_panel_interaction_request(json: &str) -> Option<SocketControlRequest> {
    let event = serde_json::from_str(json).ok()?;
    control_command_from_panel_interaction(event).map(SocketControlRequest::PanelInteraction)
}

impl SocketControlRequestSpec {
    fn request_token(&self) -> &'static str {
        self.line.trim_end_matches('\n')
    }
}

fn format_status_reply(reply: ControlReply) -> anyhow::Result<String> {
    let ControlReply::Status(_) = reply else {
        return Err(unexpected_control_reply());
    };
    Ok("Canopa Notify daemon available\n".to_string())
}

fn format_active_json_reply(reply: ControlReply) -> anyhow::Result<String> {
    let ControlReply::ActiveNotifications(active) = reply else {
        return Err(unexpected_control_reply());
    };
    control_reply_line(&active)
}

fn format_active_count_reply(reply: ControlReply) -> anyhow::Result<String> {
    let ControlReply::ActiveNotifications(active) = reply else {
        return Err(unexpected_control_reply());
    };
    Ok(format!("{} active notifications\n", active.len()))
}

fn format_panel_snapshot_reply(reply: ControlReply) -> anyhow::Result<String> {
    let ControlReply::NotificationCenterProjection(projection) = reply else {
        return Err(unexpected_control_reply());
    };
    let content = panel_content_from_projection(projection);
    panel_response_line(content)
}

fn format_command_applied_reply(reply: ControlReply) -> anyhow::Result<String> {
    if let ControlReply::ClearPartiallyApplied { dismissed } = reply {
        return Ok(format!(
            "error clear stopped after {dismissed} committed dismissals\n"
        ));
    }
    let ControlReply::CommandApplied = reply else {
        return Err(unexpected_control_reply());
    };
    Ok("ok\n".to_string())
}

fn unexpected_control_reply() -> anyhow::Error {
    anyhow::anyhow!("unexpected reply from Notify Control API")
}

fn panel_content_from_projection(projection: NotificationCenterProjection) -> PanelContent {
    PanelContent {
        title: "Canopa Notify".to_string(),
        sections: projection
            .groups
            .into_iter()
            .map(|group| PanelSection {
                id: group.app_identity.clone(),
                title: group.app_identity,
                items: group
                    .items
                    .into_iter()
                    .map(|item| PanelItem {
                        id: item.notification_id.to_string(),
                        title: item.summary,
                        body: (!item.body.is_empty()).then_some(item.body),
                        styled_body: styled_text_from_body_content(item.body_content.clone()),
                        rich_body: rich_body_from_body_content(item.body_content),
                        metadata: vec![
                            PanelMetadata {
                                key: "status".to_string(),
                                label: "Status".to_string(),
                                value: item.status.as_str().to_string(),
                            },
                            PanelMetadata {
                                key: "delivery_outcome".to_string(),
                                label: "Delivery outcome".to_string(),
                                value: item.delivery_outcome.as_str().to_string(),
                            },
                        ],
                        actions: item
                            .actions
                            .into_iter()
                            .map(|action| panel_sender_action(action.key, action.label))
                            .collect(),
                    })
                    .collect(),
                controls: std::iter::once(app_permission_control(group.permission))
                    .chain(group.category_permissions.into_iter().map(|control| {
                        category_permission_control(control.category, control.permission)
                    }))
                    .collect(),
            })
            .collect(),
        actions: vec![
            PanelAction::new(PANEL_CLEAR_ACTIVE_KEY, "Clear active notifications"),
            PanelAction::new(PANEL_DELETE_HISTORY_KEY, "Delete notification history"),
        ],
        error: None,
    }
}

fn panel_sender_action(action_key: String, label: String) -> PanelAction {
    PanelAction {
        key: PANEL_NOTIFICATION_ACTION_KEY.to_string(),
        label,
        value: Some(action_key),
    }
}

fn styled_text_from_body_content(body_content: NotificationBodyContent) -> Option<StyledText> {
    let styled = StyledText {
        runs: body_content
            .text_runs()
            .into_iter()
            .map(|run| StyledTextRun {
                text: run.text,
                style: StyledTextStyle {
                    bold: run.style.bold,
                    italic: run.style.italic,
                    underline: run.style.underline,
                },
            })
            .collect(),
    };

    (!styled.is_empty()).then_some(styled)
}

fn rich_body_from_body_content(body_content: NotificationBodyContent) -> Option<RichBody> {
    let rich_body = RichBody {
        segments: body_content
            .segments
            .into_iter()
            .map(rich_body_segment_from_notification_segment)
            .collect(),
    };

    (!rich_body.is_empty()).then_some(rich_body)
}

fn rich_body_segment_from_notification_segment(
    segment: NotificationBodySegment,
) -> RichBodySegment {
    match segment {
        NotificationBodySegment::Text { text, style } => RichBodySegment::Text {
            text,
            style: StyledTextStyle {
                bold: style.bold,
                italic: style.italic,
                underline: style.underline,
            },
        },
        NotificationBodySegment::Image {
            image: NotificationBodyImage { source, alt },
        } => RichBodySegment::Image {
            image: RichBodyImage {
                source: rich_body_image_source_from_notification_source(source),
                alt,
            },
        },
    }
}

fn rich_body_image_source_from_notification_source(
    source: NotificationBodyImageSource,
) -> RichBodyImageSource {
    match source {
        NotificationBodyImageSource::LocalPath { path } => RichBodyImageSource::LocalPath { path },
    }
}

fn app_permission_control(permission: AppPermissionControl) -> PanelControl {
    PanelControl {
        key: PANEL_APP_PERMISSION_KEY.to_string(),
        label: "App permission".to_string(),
        value: permission.as_str().to_string(),
        options: AppPermissionControl::options()
            .iter()
            .copied()
            .map(|permission| PanelControlOption {
                value: permission.as_str().to_string(),
                label: permission.label().to_string(),
            })
            .collect(),
    }
}

fn category_permission_control(category: String, permission: AppPermissionControl) -> PanelControl {
    PanelControl {
        key: format!("{PANEL_CATEGORY_PERMISSION_KEY_PREFIX}{category}"),
        label: format!("Category: {category}"),
        value: permission.as_str().to_string(),
        options: AppPermissionControl::options()
            .iter()
            .copied()
            .map(|permission| PanelControlOption {
                value: permission.as_str().to_string(),
                label: permission.label().to_string(),
            })
            .collect(),
    }
}

fn control_command_from_panel_interaction(event: InteractionEvent) -> Option<ControlCommand> {
    match event {
        InteractionEvent::Invoke {
            target,
            action_key,
            value,
        } => match target {
            InteractionTarget::Item { id } if action_key == PANEL_NOTIFICATION_ACTION_KEY => {
                Some(ControlCommand::InvokeNotificationAction {
                    notification_id: id.parse().ok()?,
                    action_key: value?,
                })
            }
            InteractionTarget::Item { id } if action_key == PANEL_ACTIVATE_ITEM_KEY => {
                Some(ControlCommand::ActivateNotification {
                    notification_id: id.parse().ok()?,
                })
            }
            InteractionTarget::Section { id } if action_key == PANEL_APP_PERMISSION_KEY => {
                Some(ControlCommand::SetAppPermission {
                    app_identity: id,
                    permission: app_permission_from_panel_value(value.as_deref()?)?,
                })
            }
            InteractionTarget::Section { id }
                if action_key.starts_with(PANEL_CATEGORY_PERMISSION_KEY_PREFIX) =>
            {
                let category = action_key.strip_prefix(PANEL_CATEGORY_PERMISSION_KEY_PREFIX)?;
                let category = normalize_notification_category(category)?;
                Some(ControlCommand::SetCategoryPermission {
                    app_identity: id,
                    category,
                    permission: app_permission_from_panel_value(value.as_deref()?)?,
                })
            }
            InteractionTarget::Content if action_key == PANEL_CLEAR_ACTIVE_KEY => {
                Some(ControlCommand::ClearActiveNotifications)
            }
            InteractionTarget::Content if action_key == PANEL_DELETE_HISTORY_KEY => {
                Some(ControlCommand::DeleteNotificationHistory)
            }
            InteractionTarget::Content
            | InteractionTarget::Section { .. }
            | InteractionTarget::Item { .. } => None,
        },
    }
}

fn app_permission_from_panel_value(value: &str) -> Option<AppPermissionControl> {
    match value {
        "allow" => Some(AppPermissionControl::Allow),
        "silence" => Some(AppPermissionControl::Silence),
        "drop" => Some(AppPermissionControl::Drop),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn partial_clear_reports_committed_count_over_control_socket() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let daemon =
            canopa_notify::Daemon::open(canopa_notify::DaemonOptions::headless(database.clone()))
                .await
                .unwrap();
        let blocked = daemon
            .receive_notification(canopa_notify::NotificationInput {
                app_name: "Mail".into(),
                summary: "First".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        daemon
            .receive_notification(canopa_notify::NotificationInput {
                app_name: "Mail".into(),
                summary: "Second".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(database))
            .await
            .unwrap();
        sqlx::query(&format!("CREATE TRIGGER fail_second_dismissal BEFORE UPDATE ON notification_threads WHEN OLD.id = {} AND NEW.status = 'dismissed' BEGIN SELECT RAISE(ABORT, 'injected'); END", blocked.notification_id)).execute(&pool).await.unwrap();
        let (mut client, server) = tokio::net::UnixStream::pair().unwrap();
        let control = NotifyControl::new(daemon.clone());
        ControlSocket::new(control).accept(server);
        client
            .write_all(b"control {\"command\":\"clear-active-notifications\"}\n")
            .await
            .unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert_eq!(
            serde_json::from_str::<ControlReply>(&response).unwrap(),
            ControlReply::ClearPartiallyApplied { dismissed: 1 }
        );
        assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
        pool.close().await;
    }

    #[test]
    fn ctl_commands_map_to_socket_request_lines() {
        assert_eq!(CtlRequest::Status.line(), "status\n");
        assert_eq!(CtlRequest::ActiveCount.line(), "active\n");
        assert_eq!(CtlRequest::ActiveJson.line(), "active-json\n");
    }

    #[test]
    fn socket_request_table_round_trips_lines() {
        for spec in SOCKET_CONTROL_REQUESTS {
            assert_eq!(
                SocketControlRequest::parse(spec.request_token()),
                spec.request
            );
            assert_eq!(spec.request.line(), spec.line);
        }

        assert_eq!(SocketControlRequest::Unsupported.line(), "unsupported\n");
    }

    #[test]
    fn app_permission_control_uses_permission_values_and_labels() {
        let control = app_permission_control(AppPermissionControl::Silence);

        assert_eq!(control.key, "canopa-notify:app-permission");
        assert_eq!(control.value, "silence");
        assert_eq!(
            control
                .options
                .iter()
                .map(|option| (option.value.as_str(), option.label.as_str()))
                .collect::<Vec<_>>(),
            vec![("allow", "Allow"), ("silence", "Silence"), ("drop", "Drop")]
        );
    }

    #[test]
    fn panel_content_uses_provider_owned_keys_for_controls_and_sender_actions() {
        let content = panel_content_from_projection(canopa_notify::NotificationCenterProjection {
            groups: vec![canopa_notify::NotificationCenterGroup {
                app_identity: "Mail".to_string(),
                permission: AppPermissionControl::Allow,
                category_permissions: vec![canopa_notify::CategoryPermissionControl {
                    category: "email.arrived".to_string(),
                    permission: AppPermissionControl::Silence,
                }],
                items: vec![canopa_notify::NotificationCenterItem {
                    notification_id: InternalNotificationId::new(42).unwrap(),
                    summary: "Inbox".to_string(),
                    body: String::new(),
                    body_content: NotificationBodyContent::default(),
                    status: canopa_notify::NotificationStatus::Active,
                    delivery_outcome: canopa_notify::DeliveryOutcome::Shown,
                    actions: vec![canopa_notify::NotificationAction::new(
                        "canopa-notify:notification-action",
                        "Sender action",
                    )],
                }],
            }],
        });

        let item_action = &content.sections[0].items[0].actions[0];
        assert_eq!(item_action.key, "canopa-notify:notification-action");
        assert_eq!(
            item_action.value.as_deref(),
            Some("canopa-notify:notification-action")
        );
        assert_eq!(
            content.sections[0].controls[0].key,
            "canopa-notify:app-permission"
        );
        assert_eq!(
            content.sections[0].controls[1].key,
            "canopa-notify:category-permission:email.arrived"
        );
        assert_eq!(content.sections[0].controls[1].value, "silence");
        assert_eq!(content.actions[0].key, "canopa-notify:clear-active");
        assert_eq!(content.actions[1].key, "canopa-notify:delete-history");
    }

    #[test]
    fn panel_interactions_map_to_notify_control_commands() {
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Item {
                    id: "42".to_string()
                },
                action_key: PANEL_NOTIFICATION_ACTION_KEY.to_string(),
                value: Some("reply".to_string()),
            }),
            Some(ControlCommand::InvokeNotificationAction {
                notification_id: InternalNotificationId::new(42).unwrap(),
                action_key: "reply".to_string(),
            })
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Section {
                    id: "Mail".to_string()
                },
                action_key: "canopa-notify:category-permission:email.arrived".to_string(),
                value: Some("drop".to_string()),
            }),
            Some(ControlCommand::SetCategoryPermission {
                app_identity: "Mail".to_string(),
                category: "email.arrived".to_string(),
                permission: AppPermissionControl::Drop,
            })
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Section {
                    id: "Mail".to_string()
                },
                action_key: "canopa-notify:category-permission:bad category".to_string(),
                value: Some("drop".to_string()),
            }),
            None
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Item {
                    id: "42".to_string()
                },
                action_key: PANEL_ACTIVATE_ITEM_KEY.to_string(),
                value: None,
            }),
            Some(ControlCommand::ActivateNotification {
                notification_id: InternalNotificationId::new(42).unwrap(),
            })
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Section {
                    id: "Mail".to_string()
                },
                action_key: PANEL_APP_PERMISSION_KEY.to_string(),
                value: Some("silence".to_string()),
            }),
            Some(ControlCommand::SetAppPermission {
                app_identity: "Mail".to_string(),
                permission: AppPermissionControl::Silence,
            })
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Content,
                action_key: PANEL_CLEAR_ACTIVE_KEY.to_string(),
                value: None,
            }),
            Some(ControlCommand::ClearActiveNotifications)
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Content,
                action_key: PANEL_DELETE_HISTORY_KEY.to_string(),
                value: None,
            }),
            Some(ControlCommand::DeleteNotificationHistory)
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Item {
                    id: "42".to_string()
                },
                action_key: "reply".to_string(),
                value: None,
            }),
            None
        );
    }

    #[test]
    fn socket_request_adapter_maps_to_control_commands() {
        assert_eq!(
            SocketControlRequest::parse("status").command(),
            Some(ControlCommand::Status)
        );
        assert_eq!(
            SocketControlRequest::parse("active").command(),
            Some(ControlCommand::ActiveNotifications)
        );
        assert_eq!(
            SocketControlRequest::parse("active-json").command(),
            Some(ControlCommand::ActiveNotifications)
        );
        assert_eq!(
            SocketControlRequest::parse("panel-snapshot").command(),
            Some(ControlCommand::NotificationCenterProjection)
        );
        assert_eq!(
            SocketControlRequest::parse("panel-subscribe").command(),
            None
        );
        assert_eq!(
            SocketControlRequest::parse("invoke-action 42 Open action").command(),
            Some(ControlCommand::InvokeNotificationAction {
                notification_id: InternalNotificationId::new(42).unwrap(),
                action_key: "Open action".to_string(),
            })
        );
        let panel_interaction = serde_json::to_string(&InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: "42".to_string(),
            },
            action_key: PANEL_NOTIFICATION_ACTION_KEY.to_string(),
            value: Some("reply".to_string()),
        })
        .unwrap();
        assert_eq!(
            SocketControlRequest::parse(&format!("panel-interaction {panel_interaction}"))
                .command(),
            Some(ControlCommand::InvokeNotificationAction {
                notification_id: InternalNotificationId::new(42).unwrap(),
                action_key: "reply".to_string(),
            })
        );
        assert_eq!(SocketControlRequest::parse("bogus").command(), None);
    }
}
