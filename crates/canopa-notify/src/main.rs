use anyhow::Context;
use canopa_notify::{
    BannerInteraction, Daemon, DaemonOptions, IcedLayerShellBannerPresenter,
    NotificationServerRequest, NotifyControl, SenderEvent,
};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

mod control_adapters;

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
                    if let Err(error) = control_adapters::handle_connection(stream, control).await {
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
    sender_ids: NotificationSenderIds,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        let sender_id = self.sender_ids.sender_id_for_request(replaces_id);
        let mut input = NotificationServerRequest {
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        }
        .into_notification_input();

        if let Some(internal_id) = sender_id.replacement_target {
            input.replaces_id = Some(internal_id);
        }

        let receipt = self
            .daemon
            .receive_notification(input)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;

        if receipt.notification_id > 0 {
            self.sender_ids
                .register(sender_id.value, receipt.notification_id, sender_id.origin);
        }

        Ok(sender_id.value)
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        let Some(internal_id) = self.sender_ids.internal_id_for_sender_id(id) else {
            return Err(empty_dbus_error());
        };

        self.daemon
            .close_notification(internal_id)
            .await
            .map_err(|_error| {
                self.sender_ids.unregister_sender_id(id);
                empty_dbus_error()
            })
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["actions".to_string(), "body".to_string()]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Canopa Notify".to_string(),
            "Canopa".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.3".to_string(),
        )
    }

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

async fn start_notification_server(
    bus_name: String,
    daemon: Daemon,
) -> anyhow::Result<zbus::Connection> {
    let sender_events = daemon.subscribe_sender_events();
    let sender_ids = NotificationSenderIds::new();
    let connection = zbus::connection::Builder::session()?
        .name(bus_name)?
        .serve_at(
            "/org/freedesktop/Notifications",
            NotificationServer {
                daemon,
                sender_ids: sender_ids.clone(),
            },
        )?
        .build()
        .await?;
    spawn_notification_signal_relay(connection.clone(), sender_events, sender_ids)?;
    Ok(connection)
}

fn spawn_notification_signal_relay(
    connection: zbus::Connection,
    mut sender_events: broadcast::Receiver<SenderEvent>,
    sender_ids: NotificationSenderIds,
) -> anyhow::Result<()> {
    let emitter = SignalEmitter::new(&connection, "/org/freedesktop/Notifications")?.into_owned();
    tokio::spawn(async move {
        loop {
            match sender_events.recv().await {
                Ok(event) => {
                    if let Err(error) = emit_sender_event(&emitter, &sender_ids, &event).await {
                        eprintln!(
                            "level=warn component=dbus event=sender_signal_failed error={error:#}"
                        );
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!(
                        "level=warn component=dbus event=sender_signal_lagged skipped={skipped}"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Ok(())
}

async fn emit_sender_event(
    emitter: &SignalEmitter<'_>,
    sender_ids: &NotificationSenderIds,
    event: &SenderEvent,
) -> zbus::Result<()> {
    if let Some(action_key) = event.action_key.as_deref() {
        if let Some(sender_id) = sender_ids.sender_id_for_internal_id(event.notification_id) {
            emitter.action_invoked(sender_id, action_key).await?;
        }
    }

    if let Some(closed_id) = event
        .closed_id
        .and_then(|id| sender_ids.unregister_internal_id(id))
    {
        emitter
            .notification_closed(closed_id, close_reason_code(event.close_reason.as_deref()))
            .await?;
    }

    Ok(())
}

fn empty_dbus_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(String::new())
}

fn close_reason_code(reason: Option<&str>) -> u32 {
    match reason {
        Some("expired") => 1,
        Some("dismissed") => 2,
        Some("closed") => 3,
        _ => 4,
    }
}

#[derive(Clone, Debug)]
struct NotificationSenderIds {
    state: Arc<Mutex<NotificationSenderIdState>>,
}

impl NotificationSenderIds {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NotificationSenderIdState::new())),
        }
    }

    fn sender_id_for_request(&self, replaces_id: u32) -> SenderIdForRequest {
        let mut state = self.state.lock().unwrap();
        state.sender_id_for_request(replaces_id)
    }

    fn register(&self, sender_id: u32, internal_id: i64, origin: SenderIdOrigin) {
        self.state
            .lock()
            .unwrap()
            .register(sender_id, internal_id, origin);
    }

    fn internal_id_for_sender_id(&self, sender_id: u32) -> Option<i64> {
        if sender_id == 0 {
            return None;
        }

        self.state
            .lock()
            .unwrap()
            .sender_to_internal
            .get(&sender_id)
            .map(|registration| registration.internal_id)
    }

    fn sender_id_for_internal_id(&self, internal_id: i64) -> Option<u32> {
        self.state
            .lock()
            .unwrap()
            .internal_to_sender
            .get(&internal_id)
            .copied()
    }

    fn unregister_internal_id(&self, internal_id: i64) -> Option<u32> {
        self.state
            .lock()
            .unwrap()
            .unregister_internal_id(internal_id)
    }

    fn unregister_sender_id(&self, sender_id: u32) -> Option<i64> {
        self.state.lock().unwrap().unregister_sender_id(sender_id)
    }
}

#[derive(Debug)]
struct NotificationSenderIdState {
    next_sender_id: u32,
    sender_to_internal: HashMap<u32, SenderIdRegistration>,
    internal_to_sender: HashMap<i64, u32>,
}

impl NotificationSenderIdState {
    fn new() -> Self {
        Self {
            next_sender_id: 1,
            sender_to_internal: HashMap::new(),
            internal_to_sender: HashMap::new(),
        }
    }

    fn sender_id_for_request(&mut self, replaces_id: u32) -> SenderIdForRequest {
        if replaces_id == 0 {
            return SenderIdForRequest {
                value: self.allocate_sender_id(),
                replacement_target: None,
                origin: SenderIdOrigin::Generated,
            };
        }

        match self.sender_to_internal.get(&replaces_id) {
            Some(registration) if registration.origin == SenderIdOrigin::Generated => {
                SenderIdForRequest {
                    value: replaces_id,
                    replacement_target: Some(registration.internal_id),
                    origin: SenderIdOrigin::Generated,
                }
            }
            _ => SenderIdForRequest {
                value: replaces_id,
                replacement_target: None,
                origin: SenderIdOrigin::CallerSupplied,
            },
        }
    }

    fn allocate_sender_id(&mut self) -> u32 {
        loop {
            let sender_id = self.next_sender_id;
            self.next_sender_id = self.next_sender_id.wrapping_add(1);
            if self.next_sender_id == 0 {
                self.next_sender_id = 1;
            }

            if sender_id != 0 && !self.sender_to_internal.contains_key(&sender_id) {
                return sender_id;
            }
        }
    }

    fn register(&mut self, sender_id: u32, internal_id: i64, origin: SenderIdOrigin) {
        if let Some(previous_registration) = self.sender_to_internal.remove(&sender_id) {
            self.internal_to_sender
                .remove(&previous_registration.internal_id);
        }
        if let Some(previous_sender_id) = self.internal_to_sender.remove(&internal_id) {
            self.sender_to_internal.remove(&previous_sender_id);
        }

        self.sender_to_internal.insert(
            sender_id,
            SenderIdRegistration {
                internal_id,
                origin,
            },
        );
        self.internal_to_sender.insert(internal_id, sender_id);
    }

    fn unregister_internal_id(&mut self, internal_id: i64) -> Option<u32> {
        let sender_id = self.internal_to_sender.remove(&internal_id)?;
        self.sender_to_internal.remove(&sender_id);
        Some(sender_id)
    }

    fn unregister_sender_id(&mut self, sender_id: u32) -> Option<i64> {
        let registration = self.sender_to_internal.remove(&sender_id)?;
        self.internal_to_sender.remove(&registration.internal_id);
        Some(registration.internal_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SenderIdForRequest {
    value: u32,
    replacement_target: Option<i64>,
    origin: SenderIdOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SenderIdOrigin {
    Generated,
    CallerSupplied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SenderIdRegistration {
    internal_id: i64,
    origin: SenderIdOrigin,
}

fn run_ctl(args: CtlArgs) -> anyhow::Result<()> {
    control_adapters::run_ctl(&args.socket, control_request_for_ctl(&args.command))
}

fn run_panel_provider(args: PanelProviderArgs) -> anyhow::Result<()> {
    control_adapters::run_panel_provider(&args.socket)
}

fn control_request_for_ctl(command: &CtlCommand) -> control_adapters::CtlRequest {
    match command {
        CtlCommand::Status => control_adapters::CtlRequest::Status,
        CtlCommand::Active { json: true } => control_adapters::CtlRequest::ActiveJson,
        CtlCommand::Active { json: false } => control_adapters::CtlRequest::ActiveCount,
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
