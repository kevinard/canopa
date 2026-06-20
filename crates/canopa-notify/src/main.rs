use anyhow::Context;
use canopa_notify::{
    AppPermissionControl, BannerInteraction, ControlCommand, ControlReply, Daemon, DaemonOptions,
    IcedLayerShellBannerPresenter, NotificationAction, NotificationCenterProjection,
    NotificationInput, NotificationTimeout, NotifyControl, Urgency,
};
use canopa_panel_protocol::{
    PanelAction, PanelContent, PanelControl, PanelControlOption, PanelItem, PanelMetadata,
    PanelSection, ProviderResponse,
};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use zbus::zvariant::OwnedValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DaemonMode {
    Headless,
    Service,
}

impl DaemonMode {
    fn as_str(self) -> &'static str {
        match self {
            DaemonMode::Headless => "headless",
            DaemonMode::Service => "service",
        }
    }
}

#[derive(Parser)]
#[command(name = "canopa-notify")]
#[command(about = "Canopa Notify desktop notification daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Daemon(DaemonArgs),
    Ctl(CtlArgs),
    PanelProvider(PanelProviderArgs),
}

#[derive(Parser)]
struct DaemonArgs {
    #[arg(long)]
    headless: bool,
    #[arg(long)]
    socket: PathBuf,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long)]
    dev_bus_name: Option<String>,
    #[arg(long)]
    service: bool,
}

#[derive(Parser)]
struct CtlArgs {
    #[arg(long)]
    socket: PathBuf,
    #[command(subcommand)]
    command: CtlCommand,
}

#[derive(Subcommand)]
enum CtlCommand {
    Status,
    Active {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Parser)]
struct PanelProviderArgs {
    #[arg(long)]
    socket: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Daemon(args) => run_daemon(args).await,
        Command::Ctl(args) => run_ctl(args),
        Command::PanelProvider(args) => run_panel_provider(args),
    }
}

async fn run_daemon(args: DaemonArgs) -> anyhow::Result<()> {
    let mode = daemon_mode(&args)?;
    let database = args
        .database
        .unwrap_or_else(|| args.socket.with_extension("db"));
    if mode == DaemonMode::Service {
        ensure_wayland_session()?;
    }
    let startup = daemon_startup(mode, database)?;
    let daemon = Daemon::open(startup.options).await?;
    let control = NotifyControl::new(daemon.clone());
    if let Some(interactions) = startup.banner_interactions {
        spawn_banner_interaction_forwarder(daemon.clone(), interactions)?;
    }
    let bus_name = match (args.dev_bus_name.as_deref(), args.service) {
        (Some(_), true) => anyhow::bail!("--dev-bus-name and --service are mutually exclusive"),
        (Some(name), false) => Some(name.to_string()),
        (None, true) => Some("org.freedesktop.Notifications".to_string()),
        (None, false) => None,
    };

    if args.socket.exists() {
        if StdUnixStream::connect(&args.socket).is_ok() {
            anyhow::bail!(
                "Canopa Notify daemon already running at {}",
                args.socket.display()
            );
        }
        std::fs::remove_file(&args.socket)?;
    }
    if let Some(parent) = args.socket.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&args.socket)
        .with_context(|| format!("failed to bind control socket {}", args.socket.display()))?;
    eprintln!(
        "level=info component=daemon event=startup mode={} socket={}",
        mode.as_str(),
        args.socket.display()
    );

    let _bus_connection = if let Some(bus_name) = bus_name {
        Some(start_notification_server(bus_name, daemon.clone()).await?)
    } else {
        None
    };

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            incoming = listener.accept() => {
                let (stream, _) = incoming?;
                let control = control.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_control_connection(stream, control).await {
                        eprintln!("level=warn component=control event=request_failed error={error:#}");
                    }
                });
            }
            result = &mut shutdown => {
                result?;
                eprintln!("level=info component=daemon event=shutdown");
                break;
            }
        }
    }

    std::fs::remove_file(&args.socket).ok();
    Ok(())
}

fn daemon_mode(args: &DaemonArgs) -> anyhow::Result<DaemonMode> {
    match (args.headless, args.service) {
        (true, true) => anyhow::bail!("--headless and --service are mutually exclusive"),
        (true, false) => Ok(DaemonMode::Headless),
        (false, true) => Ok(DaemonMode::Service),
        (false, false) => anyhow::bail!(
            "Wayland banner rendering is only available from a graphical service build; use --headless for tests and development"
        ),
    }
}

struct DaemonStartup {
    options: DaemonOptions,
    banner_interactions: Option<mpsc::Receiver<BannerInteraction>>,
}

fn daemon_startup(mode: DaemonMode, database: PathBuf) -> anyhow::Result<DaemonStartup> {
    match mode {
        DaemonMode::Headless => Ok(DaemonStartup {
            options: DaemonOptions::headless(database),
            banner_interactions: None,
        }),
        DaemonMode::Service => {
            let (interactions, banner_interactions) = mpsc::channel();
            Ok(graphical_daemon_startup(
                database,
                IcedLayerShellBannerPresenter::start(interactions)?,
                Some(banner_interactions),
            ))
        }
    }
}

fn graphical_daemon_startup(
    database: PathBuf,
    banner_presenter: impl canopa_notify::BannerPresenter + 'static,
    banner_interactions: Option<mpsc::Receiver<BannerInteraction>>,
) -> DaemonStartup {
    DaemonStartup {
        options: DaemonOptions::graphical(database, banner_presenter),
        banner_interactions,
    }
}

fn ensure_wayland_session() -> anyhow::Result<()> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        anyhow::bail!(
            "service mode requires a Wayland Session; set WAYLAND_DISPLAY or use --headless for tests and development"
        );
    }
    wayland_client::Connection::connect_to_env().with_context(|| {
        "service mode requires a reachable Wayland Session; failed to connect to WAYLAND_DISPLAY"
    })?;
    Ok(())
}

fn spawn_banner_interaction_forwarder(
    daemon: Daemon,
    interactions: mpsc::Receiver<BannerInteraction>,
) -> anyhow::Result<()> {
    let handle = tokio::runtime::Handle::current();
    std::thread::Builder::new()
        .name("canopa-notify-banner-interactions".to_string())
        .spawn(move || {
            while let Ok(interaction) = interactions.recv() {
                let daemon = daemon.clone();
                handle.spawn(async move {
                    let result = match interaction {
                        BannerInteraction::Activate(notification_id) => {
                            daemon.activate_banner(notification_id).await
                        }
                        BannerInteraction::Dismiss(notification_id) => {
                            daemon.dismiss_notification(notification_id).await
                        }
                    };
                    if let Err(error) = result {
                        eprintln!(
                            "level=warn component=presentation event=banner_interaction_failed error={error:#}"
                        );
                    }
                });
            }
        })?;
    Ok(())
}

struct NotificationServer {
    daemon: Daemon,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let input = NotificationInput {
            app_name,
            desktop_entry: desktop_entry_hint(&hints),
            replaces_id: (replaces_id != 0).then_some(replaces_id as i64),
            summary,
            body,
            expire_timeout: NotificationTimeout::from_freedesktop_expire_timeout(expire_timeout),
            urgency: urgency_hint(&hints),
            actions: notification_actions(actions),
            category: string_hint(&hints, "category"),
            progress: progress_hint(&hints),
            ..NotificationInput::default()
        };

        let receipt = self
            .daemon
            .receive_notification(input)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        Ok(receipt.notification_id.try_into().unwrap_or(0))
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        self.daemon
            .close_notification(id as i64)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["actions".to_string(), "body".to_string()]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Canopa Notify".to_string(),
            "Canopa".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }
}

async fn start_notification_server(
    bus_name: String,
    daemon: Daemon,
) -> anyhow::Result<zbus::Connection> {
    let connection = zbus::connection::Builder::session()?
        .name(bus_name)?
        .serve_at(
            "/org/freedesktop/Notifications",
            NotificationServer { daemon },
        )?
        .build()
        .await?;
    Ok(connection)
}

fn desktop_entry_hint(hints: &HashMap<String, OwnedValue>) -> Option<String> {
    string_hint(hints, "desktop-entry")
}

fn string_hint(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    hints
        .get(key)
        .and_then(|value| value.downcast_ref::<String>().ok())
}

fn urgency_hint(hints: &HashMap<String, OwnedValue>) -> Urgency {
    match hints
        .get("urgency")
        .and_then(|value| value.downcast_ref::<u8>().ok())
    {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

fn progress_hint(hints: &HashMap<String, OwnedValue>) -> Option<i64> {
    hints
        .get("value")
        .and_then(|value| value.downcast_ref::<i32>().ok())
        .map(i64::from)
}

fn notification_actions(actions: Vec<String>) -> Vec<NotificationAction> {
    actions
        .chunks(2)
        .filter_map(|chunk| match chunk {
            [key, label] => Some(NotificationAction::new(key, label)),
            _ => None,
        })
        .collect()
}

fn run_ctl(args: CtlArgs) -> anyhow::Result<()> {
    let request = control_socket_request_for_ctl(&args.command);

    let mut stream = StdUnixStream::connect(&args.socket).map_err(|source| {
        anyhow::anyhow!(
            "Canopa Notify daemon unavailable at {}: {source}",
            args.socket.display()
        )
    })?;
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    print!("{response}");
    Ok(())
}

fn run_panel_provider(args: PanelProviderArgs) -> anyhow::Result<()> {
    let mut stream = match StdUnixStream::connect(&args.socket) {
        Ok(stream) => stream,
        Err(source) => {
            let message = format!(
                "Canopa Notify daemon unavailable at {}: {source}",
                args.socket.display()
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

async fn handle_control_connection(
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

fn control_socket_request_for_ctl(command: &CtlCommand) -> &'static str {
    match command {
        CtlCommand::Status => SocketControlRequest::Status.line(),
        CtlCommand::Active { json: true } => SocketControlRequest::ActiveJson.line(),
        CtlCommand::Active { json: false } => SocketControlRequest::ActiveCount.line(),
    }
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
        value: app_permission_value(permission).to_string(),
        options: vec![
            PanelControlOption {
                value: "allow".to_string(),
                label: "Allow".to_string(),
            },
            PanelControlOption {
                value: "silence".to_string(),
                label: "Silence".to_string(),
            },
            PanelControlOption {
                value: "drop".to_string(),
                label: "Drop".to_string(),
            },
        ],
    }
}

fn app_permission_value(permission: AppPermissionControl) -> &'static str {
    match permission {
        AppPermissionControl::Allow => "allow",
        AppPermissionControl::Silence => "silence",
        AppPermissionControl::Drop => "drop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_mode_uses_non_headless_daemon_options() {
        let args = daemon_args(false, true);
        let mode = daemon_mode(&args).unwrap();
        let startup = graphical_daemon_startup(
            PathBuf::from("history.db"),
            canopa_notify::NoopBannerPresenter,
            None,
        );

        assert_eq!(mode, DaemonMode::Service);
        assert!(!startup.options.headless);
        assert!(startup.banner_interactions.is_none());
    }

    #[test]
    fn daemon_mode_rejects_headless_service_combination() {
        let error = daemon_mode(&daemon_args(true, true)).unwrap_err();

        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn ctl_commands_map_to_socket_request_lines() {
        assert_eq!(
            control_socket_request_for_ctl(&CtlCommand::Status),
            "status\n"
        );
        assert_eq!(
            control_socket_request_for_ctl(&CtlCommand::Active { json: false }),
            "active\n"
        );
        assert_eq!(
            control_socket_request_for_ctl(&CtlCommand::Active { json: true }),
            "active-json\n"
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

    fn daemon_args(headless: bool, service: bool) -> DaemonArgs {
        DaemonArgs {
            headless,
            socket: PathBuf::from("control.sock"),
            database: None,
            dev_bus_name: None,
            service,
        }
    }
}
