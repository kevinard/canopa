use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, NotificationCenterProjection, NotifyControl,
};
use canopa_panel_protocol::{
    PanelAction, PanelContent, PanelControl, PanelControlOption, PanelItem, PanelMetadata,
    PanelSection, ProviderResponse,
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CtlRequest {
    Status,
    ActiveJson,
    ActiveCount,
}

impl CtlRequest {
    fn line(self) -> &'static str {
        match self {
            Self::Status => SocketControlRequest::Status.line(),
            Self::ActiveJson => SocketControlRequest::ActiveJson.line(),
            Self::ActiveCount => SocketControlRequest::ActiveCount.line(),
        }
    }
}

pub(crate) fn run_ctl(socket: &Path, request: CtlRequest) -> anyhow::Result<()> {
    let mut stream = StdUnixStream::connect(socket).map_err(|source| {
        anyhow::anyhow!(
            "Canopa Notify daemon unavailable at {}: {source}",
            socket.display()
        )
    })?;
    stream.write_all(request.line().as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    print!("{response}");
    Ok(())
}

pub(crate) fn run_panel_provider(socket: &Path) -> anyhow::Result<()> {
    let mut stream = match StdUnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(source) => {
            let message = format!(
                "Canopa Notify daemon unavailable at {}: {source}",
                socket.display()
            );
            println!(
                "{}",
                serde_json::to_string(&ProviderResponse::replace_content(PanelContent::error(
                    "Canopa Notify",
                    message
                )))?
            );
            return Ok(());
        }
    };
    stream.write_all(SocketControlRequest::PanelSubscribe.line().as_bytes())?;
    let mut stdout = std::io::stdout();
    std::io::copy(&mut stream, &mut stdout)?;
    Ok(())
}

pub(crate) async fn handle_connection(
    stream: tokio::net::UnixStream,
    control: NotifyControl,
) -> anyhow::Result<()> {
    let mut stream = BufReader::new(stream);
    let mut request = String::new();
    stream.read_line(&mut request).await?;
    let request = request.trim();
    eprintln!("level=info component=control event=request command={request}");
    let request = SocketControlRequest::parse(request);

    if request == SocketControlRequest::PanelSubscribe {
        stream_projection_subscription(stream.get_mut(), control).await?;
        return Ok(());
    }

    let response = control_socket_response(request, &control).await?;

    let stream = stream.get_mut();
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn stream_projection_subscription(
    stream: &mut tokio::net::UnixStream,
    control: NotifyControl,
) -> anyhow::Result<()> {
    let mut subscription = control.subscribe_notification_state();
    while let Some(projection) = subscription.next_projection().await? {
        let content = panel_content_from_projection(projection);
        let response = serde_json::to_string(&ProviderResponse::replace_content(content))? + "\n";
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
    }
    stream.shutdown().await?;
    Ok(())
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SocketControlRequest {
    Status,
    ActiveJson,
    ActiveCount,
    PanelSnapshot,
    PanelSubscribe,
    Unsupported,
}

impl SocketControlRequest {
    fn parse(request: &str) -> Self {
        match request {
            "status" => Self::Status,
            "active-json" => Self::ActiveJson,
            "active" => Self::ActiveCount,
            "panel-snapshot" => Self::PanelSnapshot,
            "panel-subscribe" => Self::PanelSubscribe,
            _ => Self::Unsupported,
        }
    }

    fn line(self) -> &'static str {
        match self {
            Self::Status => "status\n",
            Self::ActiveJson => "active-json\n",
            Self::ActiveCount => "active\n",
            Self::PanelSnapshot => "panel-snapshot\n",
            Self::PanelSubscribe => "panel-subscribe\n",
            Self::Unsupported => "unsupported\n",
        }
    }

    fn command(self) -> Option<ControlCommand> {
        match self {
            Self::Status => Some(ControlCommand::Status),
            Self::ActiveJson | Self::ActiveCount => Some(ControlCommand::ActiveNotifications),
            Self::PanelSnapshot => Some(ControlCommand::NotificationCenterProjection),
            Self::PanelSubscribe => None,
            Self::Unsupported => None,
        }
    }

    fn format_reply(self, reply: ControlReply) -> anyhow::Result<String> {
        match (self, reply) {
            (Self::Status, ControlReply::Status(_)) => {
                Ok("Canopa Notify daemon available\n".to_string())
            }
            (Self::ActiveJson, ControlReply::ActiveNotifications(active)) => {
                Ok(serde_json::to_string(&active)? + "\n")
            }
            (Self::ActiveCount, ControlReply::ActiveNotifications(active)) => {
                Ok(format!("{} active notifications\n", active.len()))
            }
            (Self::PanelSnapshot, ControlReply::NotificationCenterProjection(projection)) => {
                let content = panel_content_from_projection(projection);
                Ok(serde_json::to_string(&ProviderResponse::replace_content(content))? + "\n")
            }
            _ => anyhow::bail!("unexpected reply from Notify Control API"),
        }
    }
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
                            .map(|action| PanelAction::new(action.key, action.label))
                            .collect(),
                    })
                    .collect(),
                controls: vec![app_permission_control(group.permission)],
            })
            .collect(),
        actions: vec![
            PanelAction::new("clear_active_notifications", "Clear active notifications"),
            PanelAction::new("delete_notification_history", "Delete notification history"),
        ],
        error: None,
    }
}

fn app_permission_control(permission: AppPermissionControl) -> PanelControl {
    PanelControl {
        key: "app_permission".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctl_commands_map_to_socket_request_lines() {
        assert_eq!(CtlRequest::Status.line(), "status\n");
        assert_eq!(CtlRequest::ActiveCount.line(), "active\n");
        assert_eq!(CtlRequest::ActiveJson.line(), "active-json\n");
    }

    #[test]
    fn app_permission_control_uses_permission_values_and_labels() {
        let control = app_permission_control(AppPermissionControl::Silence);

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
        assert_eq!(SocketControlRequest::parse("bogus").command(), None);
    }
}
