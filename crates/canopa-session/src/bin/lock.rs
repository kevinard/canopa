use anyhow::Result;
use canopa_config::{Component, Request};
use canopa_session::{auth, background, lock_ui::LockUi, settings::Settings, wayland};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "canopa-lock",
    version,
    about = "Canopa secure Wayland session locker"
)]
struct Cli {
    #[arg(long, global = true, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    #[arg(long, global = true)]
    defaults: bool,
    /// Write one byte to this inherited FD only after the compositor confirms the session is locked.
    #[arg(long)]
    ready_fd: Option<i32>,
    /// Logind session to report to (normally discovered automatically).
    #[arg(long, global = true)]
    session: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Listen for this graphical session's logind lock requests.
    Daemon,
    /// Request a managed lock and return only after compositor-confirmed readiness.
    Ensure {
        #[arg(long, default_value_t = 5000, value_parser = clap::value_parser!(u64).range(100..=30000))]
        timeout_ms: u64,
    },
    /// Report the session listener's managed locker state without requesting a lock.
    Status,
    #[command(hide = true)]
    InternalAuth {
        #[arg(long)]
        fd: i32,
    },
    /// Render a PNG without locking or starting authentication.
    Preview {
        output: PathBuf,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 800)]
        height: u32,
        #[arg(long, default_value = "preview")]
        monitor: String,
    },
    CheckConfig {
        #[arg(long)]
        show_source: bool,
    },
}
fn main() {
    canopa_i18n::init();
    if let Err(error) = run() {
        eprintln!("canopa-lock: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    if let Some(Command::InternalAuth { fd }) = cli.command {
        return auth::worker(fd);
    }
    let use_managed = cli.canopa_config.is_none()
        && cli.session.is_none()
        && !cli.defaults
        && cli.ready_fd.is_none()
        && cli.command.is_none();
    let request = if cli.defaults {
        Request::Defaults
    } else {
        cli.canopa_config
            .map(Request::common)
            .unwrap_or(Request::Discover)
    };
    if let Some(Command::CheckConfig { show_source }) = cli.command {
        return wayland::check_config(request, Component::Lockscreen, show_source);
    }
    if let Some(Command::Daemon) = cli.command {
        return canopa_session::logind::daemon(request, cli.session);
    }
    if matches!(
        cli.command,
        Some(Command::Ensure { .. }) | Some(Command::Status)
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        return runtime.block_on(async {
            if cli.session.is_some() {
                let target = canopa_session::logind::selected(cli.session.as_deref()).await?;
                anyhow::ensure!(
                    canopa_session::control::status().await?.session == target.id,
                    "lock listener belongs to another session"
                );
            }
            if let Some(Command::Ensure { timeout_ms }) = cli.command {
                canopa_session::control::ready(std::time::Duration::from_millis(timeout_ms)).await
            } else {
                println!(
                    "{}",
                    serde_json::to_string(&canopa_session::control::status().await?)?
                );
                Ok(())
            }
        });
    }
    if use_managed {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let attached = runtime.block_on(canopa_session::control::foreground());
        drop(runtime);
        match attached {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => return Err(error),
        }
    }
    let preview = matches!(cli.command, Some(Command::Preview { .. }));
    let settings = match Settings::load(&request, Component::Lockscreen) {
        Ok((settings, _)) => settings,
        // A configuration mistake must never turn the Lock action into a no-op,
        // just as an unavailable wallpaper falls back to the theme background.
        // Previews keep reporting the error to the person editing the file.
        Err(error) if !preview => {
            eprintln!(
                "canopa-lock: lockscreen configuration is invalid; locking with defaults: {error:#}"
            );
            Settings::load(&Request::Defaults, Component::Lockscreen)?.0
        }
        Err(error) => return Err(error),
    };
    if let Some(Command::Preview {
        output,
        width,
        height,
        monitor,
    }) = cli.command
    {
        background::dimensions(width, height)?;
        let name = auth::display_name().map_or_else(auth::username, Ok)?;
        let mut ui = LockUi::preview(&settings, name)?;
        canopa_session::lock_ui::configure(&mut ui);
        canopa_session::lock_ui::update_actions(&mut ui, false);
        ui.render(&settings, &monitor, width, height)?
            .save_with_format(output, image::ImageFormat::Png)?;
        return Ok(());
    }
    wayland::lock(settings, cli.ready_fd, cli.session)
}
