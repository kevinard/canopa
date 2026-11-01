use anyhow::Result;
use canopa_config::{Component, Request as ConfigRequest};
use canopa_session::{
    background::Fit,
    ipc::{self, Request},
    wayland,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "canopa-wallpaper",
    version,
    about = "Lightweight Wayland wallpaper daemon and live controls"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Run in the foreground. Configure startup images in Canopa's wallpaper block.
    Daemon {
        #[arg(long, conflicts_with = "defaults")]
        canopa_config: Option<PathBuf>,
        #[arg(long)]
        defaults: bool,
    },
    /// Replace one output's image, or the default and every output with --all.
    Set {
        image: PathBuf,
        #[arg(long, required_unless_present = "all", conflicts_with = "all")]
        output: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, value_enum, default_value = "fill")]
        fit: Fit,
    },
    /// List output names, dimensions and active image assignments as JSON.
    Status,
    /// Re-read startup configuration; invalid candidates preserve current assignments.
    Reload,
    Stop,
    /// Validate the wallpaper namespace without connecting to Wayland.
    CheckConfig {
        #[arg(long, conflicts_with = "defaults")]
        canopa_config: Option<PathBuf>,
        #[arg(long)]
        defaults: bool,
        #[arg(long)]
        show_source: bool,
    },
}
fn main() {
    if let Err(error) = run() {
        eprintln!("canopa-wallpaper: {error:#}");
        std::process::exit(1);
    }
}
fn request(path: Option<PathBuf>, defaults: bool) -> ConfigRequest {
    if defaults {
        ConfigRequest::Defaults
    } else {
        path.map(ConfigRequest::common)
            .unwrap_or(ConfigRequest::Discover)
    }
}
fn run() -> Result<()> {
    let command = Cli::parse().command;
    let operation = match command {
        Command::Daemon {
            canopa_config,
            defaults,
        } => return wayland::wallpaper(request(canopa_config, defaults)),
        Command::CheckConfig {
            canopa_config,
            defaults,
            show_source,
        } => {
            return wayland::check_config(
                request(canopa_config, defaults),
                Component::Wallpaper,
                show_source,
            );
        }
        Command::Set {
            image, output, fit, ..
        } => Request::Set {
            image: std::path::absolute(image)?,
            output,
            fit,
        },
        Command::Status => Request::Status {},
        Command::Reload => Request::Reload {},
        Command::Stop => Request::Stop {},
    };
    println!("{}", serde_json::to_string(&ipc::send(&operation)?)?);
    Ok(())
}
