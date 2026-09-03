use anyhow::Context;
use canopa_notify::{
    BannerInteraction, Daemon, DaemonOptions, IcedLayerShellBannerPresenter,
    NotificationServerRequest, NotifyControl, RodioSoundPresenter, SenderEvent,
    WaylandActivationTokenProvider,
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
        options: DaemonOptions::graphical(database, banner_presenter)
            .with_activation_token_provider(WaylandActivationTokenProvider::connect_to_env())
            .with_sound_presenter(RodioSoundPresenter::start()),
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
                        BannerInteraction::Activate {
                            notification_id,
                            activation_token,
                        } => {
                            daemon
                                .activate_banner_with_activation_token(
                                    notification_id,
                                    activation_token,
                                )
                                .await
                        }
                        BannerInteraction::Dismiss(notification_id) => {
                            daemon.dismiss_notification(notification_id).await
                        }
                        BannerInteraction::InvokeAction {
                            notification_id,
                            action_key,
                            activation_token,
                        } => {
                            daemon
                                .invoke_action_with_activation_token(
                                    notification_id,
                                    &action_key,
                                    activation_token,
                                )
                                .await
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

const IMPLEMENTED_SPECIFICATION_CAPABILITIES: &[&str] = &[
    "actions",
    "action-icons",
    "body",
    "body-markup",
    "icon-static",
];
const ACTIVATION_TOKEN_EXTENSION_CAPABILITY: &str = "activation-token";

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapabilityAdvertisement {
    specification: Vec<&'static str>,
    extensions: Vec<&'static str>,
}

impl CapabilityAdvertisement {
    fn for_runtime(sound: bool, activation_token: bool) -> Self {
        let mut specification = IMPLEMENTED_SPECIFICATION_CAPABILITIES.to_vec();
        if sound {
            specification.push("sound");
        }

        let mut extensions = Vec::new();
        if activation_token {
            extensions.push(ACTIVATION_TOKEN_EXTENSION_CAPABILITY);
        }

        Self {
            specification,
            extensions,
        }
    }

    fn into_dbus_values(self) -> Vec<String> {
        self.specification
            .into_iter()
            .chain(self.extensions)
            .map(str::to_string)
            .collect()
    }
}

fn capability_advertisement(daemon: &Daemon) -> CapabilityAdvertisement {
    CapabilityAdvertisement::for_runtime(
        daemon.supports_sound(),
        daemon.supports_activation_tokens(),
    )
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

        self.daemon
            .receive_notification_with_receipt_hook(input, |receipt| {
                if receipt.notification_id != 0 {
                    self.sender_ids.register(
                        sender_id.value,
                        receipt.notification_id,
                        sender_id.origin,
                    );
                }
            })
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;

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
        capability_advertisement(&self.daemon).into_dbus_values()
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
    async fn activation_token(
        emitter: &SignalEmitter<'_>,
        id: u32,
        activation_token: &str,
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
    match event {
        SenderEvent::ActivationToken {
            notification_id,
            activation_token,
        } => {
            if let Some(sender_id) = sender_ids.sender_id_for_internal_id(*notification_id) {
                emitter
                    .activation_token(sender_id, activation_token)
                    .await?;
            }
        }
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } => {
            if let Some(sender_id) = sender_ids.sender_id_for_internal_id(*notification_id) {
                emitter.action_invoked(sender_id, action_key).await?;
            }
        }
        SenderEvent::NotificationClosed {
            notification_id,
            reason,
        } => {
            if let Some(closed_id) = sender_ids.unregister_internal_id(*notification_id) {
                emitter
                    .notification_closed(closed_id, close_reason_code(*reason))
                    .await?;
            }
        }
    }

    Ok(())
}

fn empty_dbus_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(String::new())
}

fn close_reason_code(reason: canopa_notify::SenderCloseReason) -> u32 {
    match reason {
        canopa_notify::SenderCloseReason::Expired => 1,
        canopa_notify::SenderCloseReason::Dismissed => 2,
        canopa_notify::SenderCloseReason::Closed => 3,
        canopa_notify::SenderCloseReason::Undefined => 4,
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
    use canopa_notify::{
        ActivationTokenFuture, ActivationTokenProvider, NotificationSoundPlayback,
        NotificationSoundSettings, SoundPresenter,
    };
    use futures_util::StreamExt;
    use std::collections::VecDeque;

    #[derive(Clone, Debug)]
    struct FakeActivationTokenProvider {
        tokens: Arc<Mutex<VecDeque<String>>>,
    }

    impl FakeActivationTokenProvider {
        fn new(tokens: impl IntoIterator<Item = impl Into<String>>) -> Self {
            Self {
                tokens: Arc::new(Mutex::new(tokens.into_iter().map(Into::into).collect())),
            }
        }
    }

    impl ActivationTokenProvider for FakeActivationTokenProvider {
        fn available(&self) -> bool {
            true
        }

        fn activation_token(&self, _app_identity: String) -> ActivationTokenFuture {
            let token = self.tokens.lock().unwrap().pop_front();
            Box::pin(async move { token })
        }
    }

    #[derive(Clone, Debug, Default)]
    struct AvailableSoundPresenter;

    impl SoundPresenter for AvailableSoundPresenter {
        fn apply_sound_settings(&self, _settings: NotificationSoundSettings) -> anyhow::Result<()> {
            Ok(())
        }

        fn play_notification_sound(
            &self,
            _playback: NotificationSoundPlayback,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        fn stop_all_sounds(&self) {}

        fn supports_sound(&self) -> bool {
            true
        }
    }

    #[derive(Clone, Default)]
    struct RecordingBannerPresenter {
        updates: Arc<Mutex<Vec<Vec<canopa_notify::BannerView>>>>,
    }

    impl RecordingBannerPresenter {
        fn displayed(&self, notification_id: i64) -> bool {
            self.updates.lock().unwrap().iter().any(|banners| {
                banners
                    .iter()
                    .any(|banner| banner.notification_id == notification_id)
            })
        }
    }

    impl canopa_notify::BannerPresenter for RecordingBannerPresenter {
        fn replace_visible_banners(
            &self,
            banners: Vec<canopa_notify::BannerView>,
        ) -> anyhow::Result<()> {
            self.updates.lock().unwrap().push(banners);
            Ok(())
        }
    }

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

    #[tokio::test]
    async fn banner_action_interactions_invoke_notification_actions() {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let (interactions, interaction_receiver) = mpsc::channel();
        spawn_banner_interaction_forwarder(daemon.clone(), interaction_receiver).unwrap();

        let receipt = daemon
            .receive_notification(canopa_notify::NotificationInput {
                app_name: "Mail".to_string(),
                summary: "Inbox".to_string(),
                actions: vec![canopa_notify::NotificationAction::new("reply", "Reply")],
                ..canopa_notify::NotificationInput::default()
            })
            .await
            .unwrap();

        let mut events = daemon.subscribe_sender_events();
        interactions
            .send(BannerInteraction::InvokeAction {
                notification_id: receipt.notification_id,
                action_key: "reply".to_string(),
                activation_token: None,
            })
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            event,
            SenderEvent::ActionInvoked {
                notification_id,
                action_key,
            } if notification_id == receipt.notification_id && action_key == "reply"
        ));
    }

    #[tokio::test]
    async fn banner_action_interactions_forward_supplied_activation_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let (interactions, interaction_receiver) = mpsc::channel();
        spawn_banner_interaction_forwarder(daemon.clone(), interaction_receiver).unwrap();

        let receipt = daemon
            .receive_notification(canopa_notify::NotificationInput {
                app_name: "Mail".to_string(),
                summary: "Inbox".to_string(),
                actions: vec![canopa_notify::NotificationAction::new("reply", "Reply")],
                ..canopa_notify::NotificationInput::default()
            })
            .await
            .unwrap();

        let mut events = daemon.subscribe_sender_events();
        interactions
            .send(BannerInteraction::InvokeAction {
                notification_id: receipt.notification_id,
                action_key: "reply".to_string(),
                activation_token: Some("banner-token".to_string()),
            })
            .unwrap();

        let token_event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            token_event,
            SenderEvent::ActivationToken {
                notification_id,
                activation_token,
            } if notification_id == receipt.notification_id && activation_token == "banner-token"
        ));

        let action_event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            action_event,
            SenderEvent::ActionInvoked {
                notification_id,
                action_key,
            } if notification_id == receipt.notification_id && action_key == "reply"
        ));
    }

    #[tokio::test]
    async fn notification_server_emits_activation_token_before_action_invoked() {
        let temp = tempfile::tempdir().unwrap();
        let bus_name = test_bus_name("ActivationToken");
        let daemon = Daemon::open(
            DaemonOptions::headless(temp.path().join("history.db"))
                .with_activation_token_provider(FakeActivationTokenProvider::new(["token-1"])),
        )
        .await
        .unwrap();
        let _connection = start_notification_server(bus_name.clone(), daemon.clone())
            .await
            .unwrap();
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            bus_name.as_str(),
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();
        let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();
        assert_eq!(
            capabilities,
            vec![
                "actions",
                "action-icons",
                "body",
                "body-markup",
                "icon-static",
                "activation-token",
            ]
        );
        let mut token_signals = proxy.receive_signal("ActivationToken").await.unwrap();
        let mut action_signals = proxy.receive_signal("ActionInvoked").await.unwrap();

        let sender_id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Calendar",
                    0u32,
                    "",
                    "Meeting",
                    "Starting soon",
                    vec!["open".to_string(), "Open".to_string()],
                    HashMap::<String, OwnedValue>::new(),
                    -1i32,
                ),
            )
            .await
            .unwrap();
        let notification_id = daemon.snapshot().await.unwrap().active[0].id;

        daemon.invoke_action(notification_id, "open").await.unwrap();

        let token_signal =
            tokio::time::timeout(std::time::Duration::from_secs(1), token_signals.next())
                .await
                .unwrap()
                .unwrap();
        let (token_id, activation_token): (u32, String) =
            token_signal.body().deserialize().unwrap();
        assert_eq!(token_id, sender_id);
        assert_eq!(activation_token, "token-1");

        let action_signal =
            tokio::time::timeout(std::time::Duration::from_secs(1), action_signals.next())
                .await
                .unwrap()
                .unwrap();
        let (action_id, action_key): (u32, String) = action_signal.body().deserialize().unwrap();
        assert_eq!(action_id, sender_id);
        assert_eq!(action_key, "open");
    }

    #[tokio::test]
    async fn notification_server_expires_notification_after_banner_display() {
        let temp = tempfile::tempdir().unwrap();
        let bus_name = test_bus_name("DisplayedTimeout");
        let presenter = RecordingBannerPresenter::default();
        let daemon = Daemon::open(DaemonOptions::graphical(
            temp.path().join("history.db"),
            presenter.clone(),
        ))
        .await
        .unwrap();
        let _server_connection = start_notification_server(bus_name.clone(), daemon.clone())
            .await
            .unwrap();
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            bus_name.as_str(),
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();
        let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

        let sender_id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Calendar",
                    0u32,
                    "",
                    "Displayed timeout",
                    "Starting soon",
                    Vec::<String>::new(),
                    HashMap::<String, OwnedValue>::new(),
                    100i32,
                ),
            )
            .await
            .unwrap();
        let internal_id = daemon.snapshot().await.unwrap().active[0].id;
        assert!(presenter.displayed(internal_id));

        let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
            .await
            .unwrap()
            .unwrap();
        let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
        assert_eq!((closed_id, reason), (sender_id, 1));
        assert!(daemon.visible_banners().await.is_empty());

        let snapshot = daemon.snapshot().await.unwrap();
        assert!(snapshot.active.is_empty());
        assert!(snapshot.history.iter().any(|thread| {
            thread.id == internal_id && thread.status == canopa_notify::NotificationStatus::Expired
        }));
        let close_result: zbus::Result<()> = proxy.call("CloseNotification", &(sender_id,)).await;
        assert!(close_result.is_err());
    }

    #[tokio::test]
    async fn notification_server_advertises_sound_when_runtime_supports_sound() {
        let temp = tempfile::tempdir().unwrap();
        let bus_name = test_bus_name("SoundCapability");
        let daemon = Daemon::open(
            DaemonOptions::graphical(
                temp.path().join("history.db"),
                canopa_notify::NoopBannerPresenter,
            )
            .with_sound_presenter(AvailableSoundPresenter),
        )
        .await
        .unwrap();
        let _connection = start_notification_server(bus_name.clone(), daemon)
            .await
            .unwrap();
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            bus_name.as_str(),
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();

        let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();

        assert_eq!(
            capabilities,
            vec![
                "actions",
                "action-icons",
                "body",
                "body-markup",
                "icon-static",
                "sound",
            ]
        );
    }

    #[tokio::test]
    async fn capability_classification_is_exact_for_each_runtime_mode() {
        let temp = tempfile::tempdir().unwrap();
        let headless = Daemon::open(DaemonOptions::headless(temp.path().join("headless.db")))
            .await
            .unwrap();
        let graphical = Daemon::open(DaemonOptions::graphical(
            temp.path().join("graphical.db"),
            canopa_notify::NoopBannerPresenter,
        ))
        .await
        .unwrap();
        let sound_enabled = Daemon::open(
            DaemonOptions::graphical(
                temp.path().join("sound.db"),
                canopa_notify::NoopBannerPresenter,
            )
            .with_sound_presenter(AvailableSoundPresenter),
        )
        .await
        .unwrap();
        let activation_token_capable = Daemon::open(
            DaemonOptions::headless(temp.path().join("activation.db"))
                .with_activation_token_provider(FakeActivationTokenProvider::new(["token"])),
        )
        .await
        .unwrap();
        let base = CapabilityAdvertisement {
            specification: vec![
                "actions",
                "action-icons",
                "body",
                "body-markup",
                "icon-static",
            ],
            extensions: vec![],
        };

        assert_eq!(capability_advertisement(&headless), base);
        assert_eq!(capability_advertisement(&graphical), base);
        assert_eq!(
            capability_advertisement(&sound_enabled),
            CapabilityAdvertisement {
                specification: vec![
                    "actions",
                    "action-icons",
                    "body",
                    "body-markup",
                    "icon-static",
                    "sound",
                ],
                extensions: vec![],
            }
        );
        assert_eq!(
            capability_advertisement(&activation_token_capable),
            CapabilityAdvertisement {
                specification: base.specification,
                extensions: vec!["activation-token"],
            }
        );
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

    fn test_bus_name(suffix: &str) -> String {
        format!("org.canopa.NotifyUnitTest{}{}", std::process::id(), suffix)
    }
}
