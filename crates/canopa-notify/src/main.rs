#[cfg(feature = "gui")]
use anyhow::Context;
#[cfg(all(feature = "gui", feature = "audio"))]
use canopa_notify::RodioSoundPresenter;
use canopa_notify::{BannerInteraction, Daemon, DaemonOptions, NotifyControl};
#[cfg(feature = "gui")]
use canopa_notify::{IcedLayerShellBannerPresenter, WaylandActivationTokenProvider};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokio::sync::mpsc;

mod banner_interactions;

mod control_adapters;
mod notification_server;
mod startup;

use notification_server::attach_notification_server;

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
    CheckConfig(CheckConfigArgs),
}

#[derive(Parser)]
struct DaemonArgs {
    #[arg(long)]
    headless: bool,
    #[arg(long)]
    socket: Option<PathBuf>,
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long, conflicts_with = "canopa_config")]
    config: Option<PathBuf>,
    #[arg(long)]
    canopa_config: Option<PathBuf>,
    #[arg(long)]
    dev_bus_name: Option<String>,
    #[arg(long)]
    service: bool,
}

#[derive(Parser)]
struct CtlArgs {
    #[arg(long)]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    command: CtlCommand,
}

#[derive(Subcommand)]
enum CtlCommand {
    Status {
        #[arg(long)]
        json: bool,
    },
    Active {
        #[arg(long)]
        json: bool,
    },
    Dnd {
        #[arg(value_enum, default_value = "status")]
        state: DndCommand,
    },
    Reload {
        path: Option<PathBuf>,
    },
    ReloadCanopa {
        path: PathBuf,
    },
    Clear,
    DeleteHistory,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum DndCommand {
    Status,
    On,
    Off,
    Toggle,
}

#[derive(Parser)]
struct PanelProviderArgs {
    #[arg(long, value_enum, default_value = "canopa-v4")]
    protocol: control_adapters::PanelProtocol,
    #[arg(long)]
    socket: Option<PathBuf>,
}

#[derive(Parser)]
struct CheckConfigArgs {
    #[arg(long, conflicts_with_all = ["canopa_config", "defaults"])]
    config: Option<PathBuf>,
    #[arg(long, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    #[arg(long)]
    defaults: bool,
    #[arg(long)]
    show_source: bool,
}

fn resolve_socket(socket: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    socket
        .map(Ok)
        .unwrap_or_else(canopa_notify_client::default_socket)
}

fn configuration_request(
    legacy: Option<PathBuf>,
    common: Option<PathBuf>,
) -> canopa_config::Request {
    common
        .map(canopa_config::Request::common)
        .or_else(|| legacy.map(canopa_config::Request::legacy))
        .unwrap_or(canopa_config::Request::Discover)
}

fn check_config(args: CheckConfigArgs) -> anyhow::Result<()> {
    let request = if args.defaults {
        canopa_config::Request::Defaults
    } else {
        configuration_request(args.config, args.canopa_config)
    };
    let candidate = (|| {
        let input =
            canopa_config::Input::load(&request, canopa_config::Component::Notifications, "")?;
        let document = input
            .select(canopa_config::Component::Notifications, "")
            .inspect_err(|error| {
                if args.show_source
                    && error.code == canopa_config::Code::Syntax
                    && let Err(detail) = input.text().parse::<kdl::KdlDocument>()
                {
                    eprintln!("{:?}", miette::Report::new(detail));
                }
            })?;
        canopa_notify::check_configuration_document(&document)
    })();
    match candidate {
        Ok(source) => println!(
            "{}",
            serde_json::json!({"component": "notifications", "source": source})
        ),
        Err(error) => {
            let diagnostic = error
                .downcast_ref::<canopa_config::Diagnostic>()
                .cloned()
                .unwrap_or_else(|| {
                    canopa_config::Diagnostic::new(canopa_config::Code::Schema, None)
                });
            println!(
                "{}",
                serde_json::json!({"component": "notifications", "error": diagnostic})
            );
            if args.show_source {
                eprintln!("{error:#}");
            }
            anyhow::bail!("notification configuration check failed");
        }
    }
    Ok(())
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("could not start notification runtime");
    let result = runtime.block_on(run());
    // A native device/compositor call may remain blocked after adapter cancellation.
    // It must not keep service termination waiting forever.
    runtime.shutdown_timeout(std::time::Duration::from_secs(5));
    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::CheckConfig(args) => check_config(args),
        Command::Daemon(args) => run_daemon(args).await,
        Command::Ctl(args) => run_ctl(args).await,
        Command::PanelProvider(args) => run_panel_provider(args),
    }
}

async fn run_daemon(args: DaemonArgs) -> anyhow::Result<()> {
    let mode = daemon_mode(&args)?;
    let paths = startup::DaemonPaths::resolve(resolve_socket(args.socket)?, args.database)?;
    if mode == DaemonMode::Service {
        ensure_wayland_session()?;
    }
    let bus_name = match (args.dev_bus_name.as_deref(), args.service) {
        (Some(_), true) => anyhow::bail!("--dev-bus-name and --service are mutually exclusive"),
        (Some(name), false) => Some(name.to_string()),
        (None, true) => Some("org.freedesktop.Notifications".to_string()),
        (None, false) => None,
    };

    let ownership = startup::StartupOwnership::acquire(&paths)?;
    // Establish ownership before opening history: opening performs restart recovery.
    let bus_connection = if let Some(bus_name) = bus_name {
        let builder = zbus::connection::Builder::session()?.name(bus_name)?;
        Some(builder.build().await?)
    } else {
        None
    };
    let mut startup = daemon_startup(mode, paths.database.clone()).await?;
    if mode == DaemonMode::Service {
        startup.options = startup.options.with_link_opener(
            canopa_notify::PortalNotificationLinkOpener::connect_to_session().await,
        );
    }
    paths.import_legacy_history().await?;
    startup.options = startup
        .options
        .with_configuration_source(configuration_request(args.config, args.canopa_config));
    let daemon = Daemon::open(startup.options).await?;
    let control = control_adapters::ControlSocket::new(NotifyControl::new(daemon.clone()));
    let interactions = startup
        .banner_interactions
        .map(|receiver| banner_interactions::Forwarder::spawn(daemon.clone(), receiver));
    eprintln!(
        "level=info component=daemon event=startup mode={} socket={}",
        mode.as_str(),
        paths.socket.display()
    );

    let notifications = if let Some(connection) = &bus_connection {
        let notifications = attach_notification_server(connection.clone(), daemon.clone()).await?;
        if args.service {
            // Portal activation must see an installed interface/properties when
            // this secondary service name appears, after history and UI startup.
            connection
                .request_name(notification_server::PORTAL_BUS_NAME)
                .await?;
        }
        Some(notifications)
    } else {
        None
    };

    let shutdown = async {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    };
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            incoming = ownership.listener.accept() => {
                let (stream, _) = incoming?;
                control.accept(stream);
            }
            result = &mut shutdown => {
                result?;
                eprintln!("level=info component=daemon event=shutdown");
                break;
            }
        }
    }

    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        if let Some(notifications) = notifications {
            notifications.shutdown().await?;
        }
        if let Some(interactions) = interactions {
            interactions.shutdown().await?;
        }
        daemon.shutdown().await
    })
    .await
    .map_err(|_| anyhow::anyhow!("notification daemon shutdown timed out"))??;
    if let Some(connection) = bus_connection {
        tokio::time::timeout(std::time::Duration::from_secs(1), connection.close())
            .await
            .map_err(|_| anyhow::anyhow!("notification bus close timed out"))??;
    }
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

async fn daemon_startup(mode: DaemonMode, database: PathBuf) -> anyhow::Result<DaemonStartup> {
    match mode {
        DaemonMode::Headless => Ok(DaemonStartup {
            options: DaemonOptions::headless(database),
            banner_interactions: None,
        }),
        #[cfg(not(feature = "gui"))]
        DaemonMode::Service => anyhow::bail!(
            "service mode requires the gui Cargo feature; use --headless with this build"
        ),
        #[cfg(feature = "gui")]
        DaemonMode::Service => {
            let (interactions, banner_interactions) =
                mpsc::channel(banner_interactions::QUEUE_CAPACITY);
            Ok(graphical_daemon_startup(
                database,
                IcedLayerShellBannerPresenter::start(interactions).await?,
                Some(banner_interactions),
            ))
        }
    }
}

#[cfg(feature = "gui")]
fn graphical_daemon_startup(
    database: PathBuf,
    banner_presenter: impl canopa_notify::BannerPresenter + 'static,
    banner_interactions: Option<mpsc::Receiver<BannerInteraction>>,
) -> DaemonStartup {
    let options = DaemonOptions::graphical(database, banner_presenter)
        .with_activation_token_provider(WaylandActivationTokenProvider::connect_to_env());
    #[cfg(feature = "audio")]
    let options = options.with_sound_presenter(RodioSoundPresenter::start());
    DaemonStartup {
        options,
        banner_interactions,
    }
}

#[cfg(feature = "gui")]
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

#[cfg(not(feature = "gui"))]
fn ensure_wayland_session() -> anyhow::Result<()> {
    anyhow::bail!("service mode requires the gui Cargo feature; use --headless with this build")
}

async fn run_ctl(mut args: CtlArgs) -> anyhow::Result<()> {
    if let CtlCommand::Reload { path: Some(path) } = &mut args.command {
        *path = std::path::absolute(&*path)?;
    }
    if let CtlCommand::ReloadCanopa { path } = &mut args.command {
        *path = std::path::absolute(&*path)?;
    }
    control_adapters::run_ctl(
        &resolve_socket(args.socket)?,
        control_request_for_ctl(&args.command),
    )
    .await
}

fn run_panel_provider(args: PanelProviderArgs) -> anyhow::Result<()> {
    control_adapters::run_panel_provider(&resolve_socket(args.socket)?, args.protocol)
}

fn control_request_for_ctl(command: &CtlCommand) -> control_adapters::CtlRequest {
    match command {
        CtlCommand::Status { json: false } => control_adapters::CtlRequest::Status,
        CtlCommand::Status { json: true } => {
            control_adapters::CtlRequest::Command(canopa_notify::ControlCommand::Status)
        }
        CtlCommand::Active { json: true } => control_adapters::CtlRequest::ActiveJson,
        CtlCommand::Active { json: false } => control_adapters::CtlRequest::ActiveCount,
        CtlCommand::Dnd { state } => control_adapters::CtlRequest::Command(match state {
            DndCommand::Status => canopa_notify::ControlCommand::DoNotDisturb,
            DndCommand::On => canopa_notify::ControlCommand::ChangeDoNotDisturb {
                enabled: Some(true),
            },
            DndCommand::Off => canopa_notify::ControlCommand::ChangeDoNotDisturb {
                enabled: Some(false),
            },
            DndCommand::Toggle => {
                canopa_notify::ControlCommand::ChangeDoNotDisturb { enabled: None }
            }
        }),
        CtlCommand::Reload { path } => control_adapters::CtlRequest::Command(
            canopa_notify::ControlCommand::ReloadConfiguration { path: path.clone() },
        ),
        CtlCommand::ReloadCanopa { path } => control_adapters::CtlRequest::Command(
            canopa_notify::ControlCommand::ReloadConfigurationSource {
                source: canopa_config::Request::common(path),
            },
        ),
        CtlCommand::Clear => control_adapters::CtlRequest::Command(
            canopa_notify::ControlCommand::ClearActiveNotifications,
        ),
        CtlCommand::DeleteHistory => control_adapters::CtlRequest::Command(
            canopa_notify::ControlCommand::DeleteNotificationHistory,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_notify::SenderEvent;

    #[cfg(feature = "gui")]
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
        let (interactions, interaction_receiver) =
            mpsc::channel(banner_interactions::QUEUE_CAPACITY);
        let forwarder = banner_interactions::Forwarder::spawn(daemon.clone(), interaction_receiver);

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
            .try_send(BannerInteraction::InvokeAction {
                target: canopa_notify::BannerTarget {
                    notification_id: receipt.notification_id,
                    revision: 1,
                },
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
        forwarder.shutdown().await.unwrap();
        daemon.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn banner_action_interactions_forward_supplied_activation_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let (interactions, interaction_receiver) =
            mpsc::channel(banner_interactions::QUEUE_CAPACITY);
        let forwarder = banner_interactions::Forwarder::spawn(daemon.clone(), interaction_receiver);

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
            .try_send(BannerInteraction::InvokeAction {
                target: canopa_notify::BannerTarget {
                    notification_id: receipt.notification_id,
                    revision: 1,
                },
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
        forwarder.shutdown().await.unwrap();
        daemon.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn banner_interaction_preparation_has_a_bounded_worker_set() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct PausedTokens(Arc<AtomicUsize>);
        impl canopa_notify::ActivationTokenProvider for PausedTokens {
            fn available(&self) -> bool {
                true
            }
            fn activation_token(&self, _: String) -> canopa_notify::ActivationTokenFuture {
                self.0.fetch_add(1, Ordering::Relaxed);
                Box::pin(std::future::pending())
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let entered = Arc::new(AtomicUsize::new(0));
        let daemon = Daemon::open(
            DaemonOptions::headless(directory.path().join("history.db"))
                .with_activation_token_provider(PausedTokens(entered.clone())),
        )
        .await
        .unwrap();
        let receipt = daemon
            .receive_notification(canopa_notify::NotificationInput {
                summary: "Busy sender".into(),
                resident: true,
                actions: vec![canopa_notify::NotificationAction::new("reply", "Reply")],
                expire_timeout: canopa_notify::NotificationTimeout::Never,
                ..Default::default()
            })
            .await
            .unwrap();
        let (sender, receiver) = mpsc::channel(banner_interactions::QUEUE_CAPACITY);
        let forwarder = banner_interactions::Forwarder::spawn(daemon.clone(), receiver);
        let mut accepted = 0;
        for _ in 0..128 {
            accepted += usize::from(
                sender
                    .try_send(BannerInteraction::InvokeAction {
                        target: canopa_notify::BannerTarget {
                            notification_id: receipt.notification_id,
                            revision: 1,
                        },
                        action_key: "reply".into(),
                        activation_token: None,
                    })
                    .is_ok(),
            );
        }
        assert_eq!(accepted, banner_interactions::QUEUE_CAPACITY);
        // Token calls cannot finish before the runtime's 100ms deadline. Observe
        // actual concurrent external preparation, not an implementation task count.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let observed = entered.load(Ordering::Relaxed);
        assert!(
            observed > 0 && observed <= 4,
            "unbounded token preparation: {observed}"
        );
        forwarder.shutdown().await.unwrap();
        assert_eq!(entered.load(Ordering::Relaxed), accepted);
        assert!(sender.is_closed());
        daemon.shutdown().await.unwrap();
    }

    fn daemon_args(headless: bool, service: bool) -> DaemonArgs {
        DaemonArgs {
            headless,
            socket: Some(PathBuf::from("control.sock")),
            database: None,
            config: None,
            canopa_config: None,
            dev_bus_name: None,
            service,
        }
    }
}
