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
    let (settings, _) = Settings::load(&request, Component::Lockscreen)?;
    if let Some(Command::Preview {
        output,
        width,
        height,
        monitor,
    }) = cli.command
    {
        background::dimensions(width, height)?;
        let ui = LockUi::preview(&settings, auth::username()?)?;
        ui.render(&settings, &monitor, width, height)?
            .save_with_format(output, image::ImageFormat::Png)?;
        return Ok(());
    }
    wayland::lock(settings, cli.ready_fd, cli.session)
}
