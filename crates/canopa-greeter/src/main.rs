use anyhow::{Result, ensure};
use canopa_config::Request;
use canopa_greeter::{
    compositor::{self, Compositor, Renderer},
    config::Config,
    controller::Controller,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(version, about = "Canopa graphical greetd login interface")]
struct Cli {
    /// Explicit shared-schema system/development configuration. Default: /etc/canopa/greeter.kdl.
    #[arg(long, global = true, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    #[arg(long, global = true)]
    defaults: bool,
    /// Private benchmark endpoint: notify once the first surface frame callback arrives.
    #[arg(long, global = true, hide = true)]
    ready_socket: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Operation>,
}
#[derive(Subcommand)]
enum Operation {
    /// List configured and discovered users without starting the greeter.
    ListUsers {
        #[arg(long, value_enum)]
        source: Option<canopa_greeter::accounts::Source>,
    },
    #[command(hide = true)]
    InternalNssUsers {
        #[command(flatten)]
        policy: canopa_greeter::nss::Policy,
    },
    #[command(hide = true)]
    InternalAvatar {
        #[arg(required = true, num_args = 1..=3)]
        paths: Vec<PathBuf>,
    },
    /// List resolved desktop session IDs, labels, commands and source files as JSON.
    ListSessions,
    /// Run the greeter inside a dedicated compositor. Configure this in greetd's command.
    Session {
        #[arg(long, value_enum)]
        compositor: Compositor,
        /// Override the compositor executable (development or nonstandard installations).
        #[arg(long)]
        compositor_bin: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "auto")]
        renderer: Renderer,
        /// Additional compositor arguments, passed literally before its program separator.
        #[arg(long, allow_hyphen_values = true)]
        compositor_arg: Vec<std::ffi::OsString>,
        /// Show the exact argv without starting a compositor or contacting greetd.
        #[arg(long)]
        print_command: bool,
    },
    CheckConfig {
        #[arg(long)]
        show_source: bool,
    },
    Preview {
        output: PathBuf,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 800)]
        height: u32,
    },
}
fn main() {
    if let Err(error) = run() {
        eprintln!("canopa-greeter: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Some(Operation::InternalAvatar { ref paths }) = cli.command {
        if let Some(image) = canopa_greeter::avatars::helper(paths)? {
            use std::io::Write;
            std::io::stdout().lock().write_all(image.as_raw())?;
        }
        return Ok(());
    }
    if let Some(Operation::InternalNssUsers { ref policy }) = cli.command {
        let accounts = canopa_greeter::nss::helper(policy)?;
        println!("{}", serde_json::to_string(&accounts)?);
        return Ok(());
    }
    let explicit = cli.canopa_config.is_some();
    let request = if cli.defaults {
        Request::Defaults
    } else {
        cli.canopa_config
            .clone()
            .map(Request::common)
            .unwrap_or_else(Config::system_request)
    };
    let loaded = Config::load(&request);
    if let Some(Operation::CheckConfig { show_source }) = cli.command {
        return match loaded {
            Ok((_, document)) => {
                println!(
                    "{}",
                    serde_json::json!({"component":"greeter","source":document.info()})
                );
                Ok(())
            }
            Err(error) => {
                if show_source {
                    eprintln!("{error:#}");
                }
                let diagnostic=error.downcast_ref::<canopa_config::Diagnostic>().map(serde_json::to_value).transpose()?.unwrap_or_else(||serde_json::json!({"code":"greeter-schema","message":"invalid greeter configuration"}));
                println!(
                    "{}",
                    serde_json::json!({"component":"greeter","error":diagnostic})
                );
                Err(error)
            }
        };
    }
    if let Some(Operation::Session {
        compositor,
        compositor_bin,
        renderer,
        compositor_arg,
        print_command,
    }) = cli.command
    {
        use std::os::unix::process::CommandExt;
        let mut arguments = Vec::new();
        if cli.defaults {
            arguments.push("--defaults".into());
        } else if let Some(path) = cli.canopa_config {
            arguments.extend(["--canopa-config".into(), path.into_os_string()]);
        }
        if let Some(path) = cli.ready_socket {
            arguments.extend(["--ready-socket".into(), path.into_os_string()]);
        }
        let mut command = compositor::command(
            compositor,
            compositor_bin.as_deref(),
            renderer,
            &compositor_arg,
            &std::env::current_exe()?,
            &arguments,
        );
        if print_command {
            println!(
                "{}",
                serde_json::json!({
                    "program": command.get_program().to_string_lossy(),
                    "argv": command.get_args().map(|a| a.to_string_lossy()).collect::<Vec<_>>(),
                    "env": command.get_envs().map(|(k,v)| (k.to_string_lossy(),v.map(|v|v.to_string_lossy()))).collect::<Vec<_>>()
                })
            );
            return Ok(());
        }
        // Validate deployment/configuration before starting any compositor.
        let (_, document) = loaded?;
        check_launch(explicit, &document)?;
        ensure!(
            std::env::var_os("GREETD_SOCK").is_some(),
            "greetd must provide GREETD_SOCK"
        );
        return Err(command.exec().into());
    }
    let (mut config, document) = loaded?;
    if let Some(Operation::ListUsers { source }) = cli.command {
        let users = canopa_greeter::accounts::list(
            &config.users,
            config.discover_users,
            source.unwrap_or(config.user_source),
            &config.nss,
            config.avatars_enabled(),
        );
        println!("{}", serde_json::to_string_pretty(&users)?);
        return Ok(());
    }
    if !matches!(
        cli.command,
        Some(Operation::Preview { .. }) | Some(Operation::ListSessions)
    ) {
        check_launch(explicit, &document)?;
    }
    config.resolve_sessions(!explicit);
    if let Some(Operation::ListSessions) = cli.command {
        println!("{}", serde_json::to_string_pretty(&config.sessions)?);
        return Ok(());
    }
    if let Some(Operation::Preview {
        output,
        width,
        height,
    }) = cli.command
    {
        canopa_session_ui::background::dimensions(width, height)?;
        let mut controller = Controller::new(&config)?;
        if config.avatars_enabled() {
            controller.ui.set_user_avatar(
                canopa_greeter::avatars::preview(&config.avatars, &controller.user)?
                    .map(std::sync::Arc::new),
            );
        }
        controller
            .ui
            .render(&config.view, "preview", width, height)?
            .save_with_format(output, image::ImageFormat::Png)?;
        return Ok(());
    }
    canopa_session_ui::secret::protect_process()?;
    let socket = std::env::var_os("GREETD_SOCK")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("greetd must provide GREETD_SOCK"))?;
    canopa_greeter::wayland::run(config, socket, cli.ready_socket)
}
fn check_launch(explicit: bool, document: &canopa_config::Document) -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } != 0,
        "run the greeter as a dedicated unprivileged account"
    );
    if !explicit && let Some(path) = &document.info().path {
        canopa_greeter::config::check_system_source(path)?;
    }
    Ok(())
}
