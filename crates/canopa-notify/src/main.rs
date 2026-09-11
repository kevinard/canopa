use anyhow::Context;
use canopa_notify::{
    BannerInteraction, Daemon, DaemonOptions, IcedLayerShellBannerPresenter, NotifyControl,
    RodioSoundPresenter, WaylandActivationTokenProvider,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::mpsc;

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
    config: Option<PathBuf>,
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
        Command::Ctl(args) => run_ctl(args).await,
        Command::PanelProvider(args) => run_panel_provider(args),
    }
}

async fn run_daemon(args: DaemonArgs) -> anyhow::Result<()> {
    let mode = daemon_mode(&args)?;
    let paths = startup::DaemonPaths::resolve(args.socket, args.database)?;
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
        Some(
            zbus::connection::Builder::session()?
                .name(bus_name)?
                .build()
                .await?,
        )
    } else {
        None
    };
    let mut startup = daemon_startup(mode, paths.database.clone()).await?;
    paths.import_legacy_history().await?;
    let configuration = startup::configuration_path(args.config)?;
    let reload_path = configuration.clone().unwrap_or(
        startup::xdg_home("XDG_CONFIG_HOME", ".config")?.join("canopa-notify/config.kdl"),
    );
    if let Some(configuration) = configuration {
        startup.options = startup.options.with_configuration_file(configuration);
    }
    let daemon = Daemon::open(startup.options).await?;
    let control = control_adapters::ControlSocket::new(
        NotifyControl::new(daemon.clone()).with_configuration_path(reload_path),
    );
    if let Some(interactions) = startup.banner_interactions {
        spawn_banner_interaction_forwarder(daemon.clone(), interactions)?;
    }
    eprintln!(
        "level=info component=daemon event=startup mode={} socket={}",
        mode.as_str(),
        paths.socket.display()
    );

    if let Some(connection) = &bus_connection {
        attach_notification_server(connection.clone(), daemon.clone()).await?;
    }

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
        DaemonMode::Service => {
            let (interactions, banner_interactions) = mpsc::channel();
            Ok(graphical_daemon_startup(
                database,
                IcedLayerShellBannerPresenter::start(interactions).await?,
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
                    let result = daemon.handle_banner_interaction(interaction).await;
                    if result.is_err() {
                        eprintln!(
                            "level=warn component=presentation event=banner_interaction_failed"
                        );
                    }
                });
            }
        })?;
    Ok(())
}

async fn run_ctl(mut args: CtlArgs) -> anyhow::Result<()> {
    if let CtlCommand::Reload { path: Some(path) } = &mut args.command {
        *path = std::path::absolute(&*path)?;
    }
    control_adapters::run_ctl(&args.socket, control_request_for_ctl(&args.command)).await
}

fn run_panel_provider(args: PanelProviderArgs) -> anyhow::Result<()> {
    control_adapters::run_panel_provider(&args.socket)
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
    }

    fn daemon_args(headless: bool, service: bool) -> DaemonArgs {
        DaemonArgs {
            headless,
            socket: PathBuf::from("control.sock"),
            database: None,
            config: None,
            dev_bus_name: None,
            service,
        }
    }
}
