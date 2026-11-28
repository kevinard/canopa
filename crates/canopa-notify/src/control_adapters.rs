use canopa_notify::InternalNotificationId;
use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, NotificationBodyContent,
    NotificationBodyImage, NotificationBodyImageSource, NotificationBodySegment,
    NotificationCenterAction, NotificationCenterProjection, NotificationCenterTarget,
    NotifyControl, normalize_notification_category,
};
use canopa_panel_protocol::{
    InteractionEvent, InteractionTarget, PanelAction, PanelAttribution, PanelContent, PanelControl,
    PanelControlOption, PanelIcon, PanelItem, PanelMetadata, PanelSection, RichBody, RichBodyImage,
    RichBodyImageSource, RichBodySegment, StyledText, StyledTextRun, StyledTextStyle,
};
mod pages;
mod transport;

pub(crate) use transport::{ControlSocket, PanelProtocol, run_ctl, run_panel_provider};
use transport::{control_reply_line, panel_response_line};

const PANEL_NOTIFICATION_ACTION_KEY: &str = "canopa-notify:notification-action";
const PANEL_ACKNOWLEDGE_ITEM_KEY: &str = "canopa-notify:acknowledge-item";
const PANEL_DISMISS_ITEM_KEY: &str = "canopa-notify:dismiss-item";
const PANEL_REMOVE_ITEM_KEY: &str = "canopa-notify:remove-item";
const PANEL_ACTIVATE_ITEM_KEY: &str = "canopa-notify:activate-item";
const PANEL_APP_PERMISSION_KEY: &str = "canopa-notify:app-permission";
const PANEL_CATEGORY_PERMISSION_KEY_PREFIX: &str = "canopa-notify:category-permission:";
const PANEL_CLEAR_ACTIVE_KEY: &str = "canopa-notify:clear-active";
const PANEL_DELETE_HISTORY_KEY: &str = "canopa-notify:delete-history";
const PANEL_MARK_ALL_READ_KEY: &str = "canopa-notify:mark-all-read";
const PANEL_CLEAR_ALL_KEY: &str = "canopa-notify:clear-all";
const PANEL_UNDO_CLEAR_KEY: &str = "canopa-notify:undo-clear";
const PANEL_DND_KEY: &str = "canopa-notify:do-not-disturb";

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
                            ControlCommand::RequestDoNotDisturb { .. },
                            ControlReply::DoNotDisturbStatus(_)
                        )
                        | (
                            ControlCommand::NotificationPreferences,
                            ControlReply::NotificationPreferences(_)
                        )
                        | (ControlCommand::ClearAll, ControlReply::Cleared { .. })
                        | (
                            ControlCommand::ActiveNotifications,
                            ControlReply::ActiveNotifications(_)
                        )
                        | (
                            ControlCommand::ActiveNotificationCount,
                            ControlReply::ActiveNotificationCount(_)
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
                            ControlCommand::NotificationCenterBatch { .. },
                            ControlReply::NotificationCenterBatch(_)
                        )
                        | (
                            ControlCommand::ReloadConfiguration { .. }
                                | ControlCommand::ReloadConfigurationSource { .. }
                                | ControlCommand::SetAppPermission { .. }
                                | ControlCommand::SetCategoryPermission { .. }
                                | ControlCommand::SetDoNotDisturbPreferences { .. }
                                | ControlCommand::SetAppPreferences { .. }
                                | ControlCommand::MarkAllRead
                                | ControlCommand::UndoClear { .. }
                                | ControlCommand::ClearActiveNotifications
                                | ControlCommand::DeleteNotificationHistory
                                | ControlCommand::InvokeNotificationAction { .. }
                                | ControlCommand::ActivateNotification { .. }
                                | ControlCommand::InteractWithNotificationCenter { .. },
                            ControlReply::CommandApplied
                        )
                );
                anyhow::ensure!(matches, invalid_control_reply());
                match reply {
                    ControlReply::DoNotDisturb(enabled) => {
                        Ok(if enabled { "on\n" } else { "off\n" }.into())
                    }
                    ControlReply::CommandApplied => Ok("ok\n".into()),
                    ControlReply::DoNotDisturbStatus(status) => Ok(dnd_status_text(&status)),
                    ControlReply::Cleared { undo } => Ok(format!("ok undo={undo}\n")),
                    ControlReply::NotificationPreferences(preferences) => {
                        Ok(serde_json::to_string_pretty(&preferences)? + "\n")
                    }
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

/// `off`, `on`, `on until 21:00` or `on (schedule) until 07:00`, in local time.
pub(crate) fn dnd_status_text(status: &canopa_notify::DoNotDisturbStatus) -> String {
    if !status.active {
        return "off\n".into();
    }
    let reason = match status.reason {
        Some(canopa_notify::DoNotDisturbReason::Schedule) => " (schedule)",
        _ => "",
    };
    match status
        .until
        .and_then(chrono::DateTime::from_timestamp_millis)
    {
        Some(until) => format!(
            "on{reason} until {}\n",
            until.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")
        ),
        None => format!("on{reason}\n"),
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
    SummarySubscribe,
    LockScreenSubscribe,
    PanelSession,
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
        request: SocketControlRequest::LockScreenSubscribe,
        line: canopa_notify_client::LOCK_SCREEN_REQUEST,
        command: None,
        format_reply: None,
    },
    SocketControlRequestSpec {
        request: SocketControlRequest::SummarySubscribe,
        line: canopa_notify_client::SUMMARY_REQUEST,
        command: None,
        format_reply: None,
    },
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
        command: Some(ControlCommand::ActiveNotificationCount),
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
            Self::PanelSession => "panel-session",
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
        if request == "panel-session" {
            return Self::PanelSession;
        }
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
    let context: PanelInteractionContext = serde_json::from_str(json).ok()?;
    let mut command = control_command_from_panel_interaction(context.interaction)?;
    if let Some(token) = context.activation_token {
        if token.is_empty()
            || token.len() > canopa_panel_protocol::v5::MAX_ACTIVATION_TOKEN_BYTES
            || token.contains('\0')
        {
            return None;
        }
        let ControlCommand::InteractWithNotificationCenter {
            action,
            activation_token,
            ..
        } = &mut command
        else {
            return None;
        };
        if matches!(
            action,
            NotificationCenterAction::Acknowledge | NotificationCenterAction::Dismiss
        ) {
            return None;
        }
        *activation_token = Some(token);
    }
    Some(SocketControlRequest::PanelInteraction(command))
}

/// Private socket context around the unchanged v4 invoke event. With no token
/// its JSON remains identical to legacy clients and provider forwarding.
#[derive(serde::Serialize, serde::Deserialize)]
struct PanelInteractionContext {
    #[serde(flatten)]
    interaction: InteractionEvent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activation_token: Option<String>,
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
    let ControlReply::ActiveNotificationCount(count) = reply else {
        return Err(unexpected_control_reply());
    };
    Ok(format!("{count} active notifications\n"))
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
    // Center interactions ignore payloads such as the clear undo token or DND status;
    // the next frame shows their effect.
    let (ControlReply::CommandApplied
    | ControlReply::Cleared { .. }
    | ControlReply::DoNotDisturbStatus(_)) = reply
    else {
        return Err(unexpected_control_reply());
    };
    Ok("ok\n".to_string())
}

fn unexpected_control_reply() -> anyhow::Error {
    anyhow::anyhow!("unexpected reply from Notify Control API")
}

fn receipt_time(received_at: i64) -> Option<PanelMetadata> {
    let time = chrono::DateTime::from_timestamp_millis(received_at)?.with_timezone(&chrono::Local);
    if received_at <= 0 {
        return None;
    }
    Some(PanelMetadata {
        key: "received".into(),
        label: fl!("notify-center-received"),
        value: received_text(time.naive_local(), chrono::Local::now().naive_local()),
    })
}

/// When a notification arrived, in the formats locale: how long ago within
/// the last hour (`5 minutes ago`), the time earlier today (`14:30`), then
/// the date and time (`Nov 11, 2026, 2:30 PM`). Stored times are unchanged.
fn received_text(time: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> String {
    let elapsed = now.signed_duration_since(time).num_seconds();
    if (0..3_600).contains(&elapsed) {
        canopa_i18n::format::relative(elapsed)
    } else if time.date() == now.date() {
        canopa_i18n::format::time(time.time())
    } else {
        canopa_i18n::format::date_time(time)
    }
}

/// Wherever a moment is shown in the Center: the time today, else date and time.
pub(crate) fn moment_text(time: chrono::NaiveDateTime, now: chrono::NaiveDateTime) -> String {
    if time.date() == now.date() {
        canopa_i18n::format::time(time.time())
    } else {
        canopa_i18n::format::date_time(time)
    }
}

/// The product name, used as the title of provider-visible errors.
const PRODUCT_NAME: &str = "Canopa Notify";

fn panel_content_from_projection(projection: NotificationCenterProjection) -> PanelContent {
    let unknown_application = fl!("notify-center-unknown-app");
    PanelContent {
        title: PRODUCT_NAME.to_string(),
        sections: projection
            .groups
            .into_iter()
            .map(|group| {
                let label = group
                    .items
                    .first()
                    .map(|item| item.app_name.trim())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(if group.app_identity.is_empty() {
                        &unknown_application
                    } else {
                        &group.app_identity
                    })
                    .to_owned();
                PanelSection {
                    id: format!("app/{}", group.app_identity),
                    title: label.clone(),
                    icon: panel_app_icon(
                        group.items.first().and_then(|item| item.app_icon.as_ref()),
                        &group.app_identity,
                        &projection.icon_settings,
                    ),
                    items: group
                        .items
                        .into_iter()
                        .map(|item| PanelItem {
                            slider: None,
                            id: item.notification_id.to_string(),
                            attribution: Some(PanelAttribution {
                                label: if item.app_name.trim().is_empty() {
                                    label.clone()
                                } else {
                                    item.app_name.clone()
                                },
                                icon: panel_app_icon(
                                    item.app_icon.as_ref(),
                                    &group.app_identity,
                                    &projection.icon_settings,
                                ),
                            }),
                            title: item.summary,
                            body: (!item.body.is_empty()).then_some(item.body),
                            styled_body: styled_text_from_body_content(item.body_content.clone()),
                            rich_body: rich_body_from_body_content(
                                item.body_content,
                                item.target.clone(),
                            ),
                            metadata: (item.requires_attention && !item.transient)
                                .then(|| PanelMetadata {
                                    key: "attention".into(),
                                    label: fl!("notify-center-unread"),
                                    value: String::new(),
                                })
                                .into_iter()
                                .chain(receipt_time(item.received_at))
                                .collect(),
                            actions: item
                                .can_reply
                                .then(|| {
                                    panel_center_action(
                                        item.target.clone(),
                                        NotificationCenterAction::Reply,
                                        fl!("notify-center-reply"),
                                    )
                                })
                                .into_iter()
                                .chain(item.actions.into_iter().enumerate().map(
                                    |(position, action)| {
                                        let action = panel_center_action(
                                            item.target.clone(),
                                            NotificationCenterAction::InvokeAction {
                                                action_key: action.key,
                                            },
                                            action.label,
                                        );
                                        // The application's first action leads its card.
                                        if position == 0 {
                                            action.primary()
                                        } else {
                                            action
                                        }
                                    },
                                ))
                                .chain(
                                    (item.status == canopa_notify::NotificationStatus::Active
                                        && item.has_default_action)
                                        .then(|| {
                                            panel_center_action(
                                                item.target.clone(),
                                                NotificationCenterAction::Activate,
                                                fl!("notify-center-open"),
                                            )
                                        }),
                                )
                                .chain(
                                    (item.requires_attention
                                        || item.status
                                            == canopa_notify::NotificationStatus::Active)
                                        .then(|| {
                                            let active = item.status
                                                == canopa_notify::NotificationStatus::Active;
                                            let (action, label) = if active {
                                                (
                                                    NotificationCenterAction::Dismiss,
                                                    fl!("notify-center-dismiss"),
                                                )
                                            } else {
                                                (
                                                    NotificationCenterAction::Acknowledge,
                                                    fl!("notify-center-mark-read"),
                                                )
                                            };
                                            panel_center_action(item.target.clone(), action, label)
                                        }),
                                )
                                .chain(
                                    (item.status != canopa_notify::NotificationStatus::Active)
                                        .then(|| {
                                            panel_center_action(
                                                item.target.clone(),
                                                NotificationCenterAction::Remove,
                                                fl!("notify-center-remove"),
                                            )
                                        }),
                                )
                                .collect(),
                        })
                        .collect(),
                    controls: if group.app_identity.is_empty() {
                        Vec::new()
                    } else {
                        std::iter::once(app_permission_control(group.permission))
                            .chain(group.category_permissions.into_iter().map(|control| {
                                category_permission_control(control.category, control.permission)
                            }))
                            .collect()
                    },
                }
            })
            .collect(),
        actions: vec![
            PanelAction::new(PANEL_CLEAR_ACTIVE_KEY, fl!("notify-center-clear-active")),
            PanelAction::new(
                PANEL_DELETE_HISTORY_KEY,
                fl!("notify-center-delete-history"),
            )
            .destructive(),
            PanelAction::new(PANEL_MARK_ALL_READ_KEY, fl!("notify-center-mark-all-read")),
            PanelAction::new(PANEL_CLEAR_ALL_KEY, fl!("notify-center-clear-all")).destructive(),
        ],
        error: None,
    }
}

fn panel_app_icon(
    reference: Option<&canopa_notify::NotificationVisualReference>,
    identity: &str,
    settings: &canopa_notify::BannerIconSettings,
) -> Option<PanelIcon> {
    use canopa_notify::NotificationVisualReference;
    let icon = match reference {
        Some(NotificationVisualReference::LocalPath(path)) => {
            PanelIcon::LocalPath { path: path.clone() }
        }
        None if let Some(icon) = desktop_entry_panel_icon(identity, settings) => icon,
        reference => PanelIcon::Theme {
            names: match reference {
                Some(NotificationVisualReference::IconName(name)) => vec![name.clone()],
                Some(NotificationVisualReference::IconNames(names)) => names.clone(),
                _ => canopa_notify::app_identity_icon_name_candidates(identity),
            },
            theme: settings.effective_theme(),
            scale: settings.scale,
        },
    };
    icon.validate().is_ok().then_some(icon)
}

/// The installed desktop entry's `Icon=` for an identity without a sender icon:
/// an absolute path, or a themed name tried before the identity names.
fn desktop_entry_panel_icon(
    identity: &str,
    settings: &canopa_notify::BannerIconSettings,
) -> Option<PanelIcon> {
    let candidates = canopa_notify::app_identity_icon_name_candidates(identity);
    let icon = candidates
        .iter()
        .find_map(|id| cached_desktop_entry_icon(id))?;
    if icon.starts_with('/') {
        return Some(PanelIcon::LocalPath { path: icon });
    }
    if icon.contains('/') {
        return None;
    }
    // As GIO does for desktop entries, drop a legacy image extension.
    let name = [".png", ".svg", ".xpm"]
        .iter()
        .find_map(|extension| icon.strip_suffix(extension))
        .filter(|name| !name.is_empty())
        .unwrap_or(&icon)
        .to_owned();
    let mut names = vec![name];
    for id in candidates {
        if !names.contains(&id) && names.len() < 8 {
            names.push(id);
        }
    }
    Some(PanelIcon::Theme {
        names,
        theme: settings.effective_theme(),
        scale: settings.scale,
    })
}

/// Desktop entries are read in the provider process, at most once per minute per id.
fn cached_desktop_entry_icon(id: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Entries = HashMap<String, (Instant, Option<String>)>;
    static CACHE: OnceLock<Mutex<Entries>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((read, icon)) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(id)
        && read.elapsed() < Duration::from_secs(60)
    {
        return icon.clone();
    }
    let icon = canopa_apps::desktop_entry_icon(id);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(id.to_owned(), (Instant::now(), icon.clone()));
    icon
}

fn panel_center_action(
    target: NotificationCenterTarget,
    action: NotificationCenterAction,
    label: String,
) -> PanelAction {
    PanelAction {
        request_activation_token: !matches!(
            action,
            NotificationCenterAction::Acknowledge
                | NotificationCenterAction::Dismiss
                | NotificationCenterAction::Remove
        ),
        destructive: matches!(
            action,
            NotificationCenterAction::Dismiss | NotificationCenterAction::Remove
        ),
        pressed: None,
        secondary: false,
        primary: false,
        symbol: None,
        key: panel_center_action_key(&action).to_string(),
        label,
        value: Some(
            serde_json::to_string(&ControlCommand::InteractWithNotificationCenter {
                activation_token: None,
                target,
                action,
            })
            .expect("notification center action serialization"),
        ),
    }
}

fn panel_center_action_key(action: &NotificationCenterAction) -> &'static str {
    match action {
        NotificationCenterAction::OpenLink { .. } => "canopa-notify:open-link",
        NotificationCenterAction::Acknowledge => PANEL_ACKNOWLEDGE_ITEM_KEY,
        NotificationCenterAction::Dismiss => PANEL_DISMISS_ITEM_KEY,
        NotificationCenterAction::Remove => PANEL_REMOVE_ITEM_KEY,
        NotificationCenterAction::Activate => PANEL_ACTIVATE_ITEM_KEY,
        NotificationCenterAction::InvokeAction { .. } => PANEL_NOTIFICATION_ACTION_KEY,
        NotificationCenterAction::Reply => "canopa-notify:reply",
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

fn rich_body_from_body_content(
    body_content: NotificationBodyContent,
    target: NotificationCenterTarget,
) -> Option<RichBody> {
    let rich_body = RichBody {
        segments: body_content
            .segments
            .into_iter()
            .enumerate()
            .map(|(index, segment)| {
                rich_body_segment_from_notification_segment(segment, index, &target)
            })
            .collect(),
    };

    (!rich_body.is_empty()).then_some(rich_body)
}

fn rich_body_segment_from_notification_segment(
    segment: NotificationBodySegment,
    segment_index: usize,
    target: &NotificationCenterTarget,
) -> RichBodySegment {
    match segment {
        NotificationBodySegment::Link { text, style, uri } => RichBodySegment::Link {
            uri: uri.as_str().to_owned(),
            action: panel_center_action(
                target.clone(),
                NotificationCenterAction::OpenLink { segment_index },
                text,
            ),
            style: StyledTextStyle {
                bold: style.bold,
                italic: style.italic,
                underline: style.underline,
            },
        },
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
        label: fl!("notify-center-app-permission"),
        value: permission.as_str().to_string(),
        options: AppPermissionControl::options()
            .iter()
            .copied()
            .map(|permission| PanelControlOption {
                value: permission.as_str().to_string(),
                label: permission.label(),
            })
            .collect(),
    }
}

fn category_permission_control(category: String, permission: AppPermissionControl) -> PanelControl {
    PanelControl {
        key: format!("{PANEL_CATEGORY_PERMISSION_KEY_PREFIX}{category}"),
        label: fl!(
            "notify-center-category-permission",
            category = category.as_str()
        ),
        value: permission.as_str().to_string(),
        options: AppPermissionControl::options()
            .iter()
            .copied()
            .map(|permission| PanelControlOption {
                value: permission.as_str().to_string(),
                label: permission.label(),
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
            InteractionTarget::Item { id } => {
                let command = serde_json::from_str(value.as_deref()?).ok()?;
                let ControlCommand::InteractWithNotificationCenter { target, action, .. } =
                    &command
                else {
                    return None;
                };
                let expected_key = panel_center_action_key(action);
                (target.notification_id() == id.parse().ok()? && action_key == expected_key)
                    .then_some(command)
            }
            InteractionTarget::Section { id } if action_key == PANEL_APP_PERMISSION_KEY => {
                Some(ControlCommand::SetAppPermission {
                    app_identity: id.strip_prefix("app/")?.to_owned(),
                    permission: app_permission_from_panel_value(value.as_deref()?)?,
                })
            }
            InteractionTarget::Section { id }
                if action_key.starts_with(PANEL_CATEGORY_PERMISSION_KEY_PREFIX) =>
            {
                let category = action_key.strip_prefix(PANEL_CATEGORY_PERMISSION_KEY_PREFIX)?;
                let category = normalize_notification_category(category)?;
                Some(ControlCommand::SetCategoryPermission {
                    app_identity: id.strip_prefix("app/")?.to_owned(),
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
            InteractionTarget::Content if action_key == PANEL_MARK_ALL_READ_KEY => {
                Some(ControlCommand::MarkAllRead)
            }
            InteractionTarget::Content if action_key == PANEL_CLEAR_ALL_KEY => {
                Some(ControlCommand::ClearAll)
            }
            InteractionTarget::Content if action_key == PANEL_UNDO_CLEAR_KEY => {
                Some(ControlCommand::UndoClear {
                    undo: value?.parse().ok()?,
                })
            }
            InteractionTarget::Section { id }
                if action_key == PANEL_DND_KEY && id == "do-not-disturb" =>
            {
                use canopa_notify::DoNotDisturbRequest;
                Some(ControlCommand::RequestDoNotDisturb {
                    request: match value.as_deref()? {
                        "off" => DoNotDisturbRequest::Off,
                        "on" => DoNotDisturbRequest::On,
                        "for-60" => DoNotDisturbRequest::For { minutes: 60 },
                        "until-tomorrow" => DoNotDisturbRequest::UntilTomorrow,
                        _ => return None,
                    },
                })
            }
            InteractionTarget::Content | InteractionTarget::Section { .. } => None,
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

    /// French until dropped; the binary's own lock, as its tests share a process.
    struct French(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl French {
        fn select() -> Self {
            static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
            canopa_i18n::select(canopa_i18n::Selection {
                languages: canopa_i18n::negotiate(&["fr".parse().unwrap()]),
                time: "fr".parse().unwrap(),
                numbers: "fr".parse().unwrap(),
                pseudo: false,
            });
            Self(guard)
        }
    }

    impl Drop for French {
        fn drop(&mut self) {
            canopa_i18n::select(canopa_i18n::Selection::default());
        }
    }

    fn local(text: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn received_times_are_relative_then_local_times_then_dates() {
        let now = local("2026-11-11 15:00");
        assert_eq!(received_text(local("2026-11-11 15:00"), now), "now");
        assert_eq!(
            received_text(local("2026-11-11 14:55"), now),
            "5 minutes ago"
        );
        assert_eq!(
            received_text(local("2026-11-11 09:05"), now),
            "9:05\u{202f}AM"
        );
        assert_eq!(
            received_text(local("2026-11-02 14:30"), now),
            "Nov 2, 2026, 2:30\u{202f}PM"
        );
        let french = {
            let _french = French::select();
            [
                received_text(local("2026-11-11 14:55"), now),
                received_text(local("2026-11-11 09:05"), now),
                received_text(local("2026-11-02 14:30"), now),
                fl!("notify-center-clear-all"),
            ]
        };
        assert_eq!(french[0], "il y a 5\u{a0}minutes");
        assert_eq!(french[1], "09:05");
        assert!(french[2].starts_with("2 nov. 2026"), "{}", french[2]);
        assert!(french[2].ends_with("14:30"), "{}", french[2]);
        assert_eq!(french[3], "Tout effacer");
    }

    #[test]
    fn do_not_disturb_state_is_composed_from_data() {
        use chrono::TimeZone;
        let now = local("2026-11-11 15:00");
        let until = chrono::Local
            .from_local_datetime(&local("2026-11-11 21:00"))
            .single()
            .unwrap()
            .timestamp_millis();
        let status = canopa_notify::DoNotDisturbStatus {
            active: true,
            reason: Some(canopa_notify::DoNotDisturbReason::Schedule),
            until: Some(until),
            next_change: None,
        };
        assert_eq!(
            pages::dnd_state_text(&status, now),
            "On (schedule) until 9:00\u{202f}PM"
        );
        let french = {
            let _french = French::select();
            pages::dnd_state_text(&status, now)
        };
        assert_eq!(french, "Activé (planification) jusqu’à 21:00");
    }

    fn center_target_fixture() -> NotificationCenterTarget {
        serde_json::from_value(serde_json::json!({
            "notification_id": 42, "revision": 1, "instance": vec![7; 16],
        }))
        .unwrap()
    }

    fn center_command_fixture(action: NotificationCenterAction) -> ControlCommand {
        ControlCommand::InteractWithNotificationCenter {
            activation_token: None,
            target: center_target_fixture(),
            action,
        }
    }

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
            icon_settings: Default::default(),
            groups: vec![canopa_notify::NotificationCenterGroup {
                app_identity: "Mail".to_string(),
                permission: AppPermissionControl::Allow,
                category_permissions: vec![canopa_notify::CategoryPermissionControl {
                    category: "email.arrived".to_string(),
                    permission: AppPermissionControl::Silence,
                }],
                items: vec![canopa_notify::NotificationCenterItem {
                    app_name: "Mail".into(),
                    app_icon: None,
                    requires_attention: false,
                    received_at: 1_700_000_000_000,
                    notification_id: InternalNotificationId::new(42).unwrap(),
                    target: center_target_fixture(),
                    transient: false,
                    has_default_action: false,
                    can_reply: false,
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

        let item = &content.sections[0].items[0];
        assert!(
            item.metadata
                .iter()
                .any(|metadata| metadata.key == "received" && !metadata.value.is_empty())
        );
        let item_action = &item.actions[0];
        assert_eq!(item_action.key, "canopa-notify:notification-action");
        assert_eq!(
            serde_json::from_str::<ControlCommand>(item_action.value.as_deref().unwrap()).unwrap(),
            center_command_fixture(NotificationCenterAction::InvokeAction {
                action_key: "canopa-notify:notification-action".into(),
            })
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
                value: Some(
                    serde_json::to_string(&center_command_fixture(
                        NotificationCenterAction::InvokeAction {
                            action_key: "reply".into()
                        }
                    ))
                    .unwrap()
                ),
            }),
            Some(center_command_fixture(
                NotificationCenterAction::InvokeAction {
                    action_key: "reply".into()
                }
            ))
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Section {
                    id: "app/Mail".to_string()
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
                    id: "app/Mail".to_string()
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
                value: Some(
                    serde_json::to_string(&center_command_fixture(
                        NotificationCenterAction::Activate
                    ))
                    .unwrap()
                ),
            }),
            Some(center_command_fixture(NotificationCenterAction::Activate))
        );
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Section {
                    id: "app/Mail".to_string()
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
            Some(ControlCommand::ActiveNotificationCount)
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
            value: Some(
                serde_json::to_string(&center_command_fixture(
                    NotificationCenterAction::InvokeAction {
                        action_key: "reply".into(),
                    },
                ))
                .unwrap(),
            ),
        })
        .unwrap();
        assert_eq!(
            SocketControlRequest::parse(&format!("panel-interaction {panel_interaction}"))
                .command(),
            Some(center_command_fixture(
                NotificationCenterAction::InvokeAction {
                    action_key: "reply".into()
                }
            ))
        );
        assert_eq!(SocketControlRequest::parse("bogus").command(), None);
    }

    #[test]
    fn panel_item_commands_reject_unguarded_mismatched_and_unrelated_payloads() {
        let activation =
            serde_json::to_string(&center_command_fixture(NotificationCenterAction::Activate))
                .unwrap();
        for (id, key, value) in [
            ("42", PANEL_ACTIVATE_ITEM_KEY, None),
            ("42", PANEL_NOTIFICATION_ACTION_KEY, Some("same".into())),
            ("41", PANEL_ACTIVATE_ITEM_KEY, Some(activation.clone())),
            (
                "42",
                PANEL_NOTIFICATION_ACTION_KEY,
                Some(activation.clone()),
            ),
            ("42", "default", Some(activation.clone())),
            ("0", PANEL_ACTIVATE_ITEM_KEY, Some(activation)),
            (
                "42",
                PANEL_ACTIVATE_ITEM_KEY,
                Some(
                    serde_json::to_string(&ControlCommand::ActivateNotification {
                        notification_id: InternalNotificationId::new(42).unwrap(),
                    })
                    .unwrap(),
                ),
            ),
            (
                "42",
                PANEL_ACTIVATE_ITEM_KEY,
                Some(serde_json::to_string(&ControlCommand::ClearActiveNotifications).unwrap()),
            ),
        ] {
            assert_eq!(
                control_command_from_panel_interaction(InteractionEvent::Invoke {
                    target: InteractionTarget::Item { id: id.into() },
                    action_key: key.into(),
                    value,
                }),
                None
            );
        }
    }
}
