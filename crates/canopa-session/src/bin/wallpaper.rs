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
    /// Preview images for selected/all monitors, then apply and save the selection.
    Pick {
        #[arg(num_args=0..=256)]
        images: Vec<PathBuf>,
    },
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
        /// Change this running daemon without saving the choice for the next login.
        #[arg(long)]
        temporary: bool,
    },
    /// Start an optional, bounded slideshow of PNG/JPEG images.
    Slideshow {
        directory: PathBuf,
        #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(10..=86400))]
        interval: u64,
        #[arg(long, required_unless_present = "all", conflicts_with = "all")]
        output: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, value_enum, default_value = "fill")]
        fit: Fit,
    },
    /// Stop the slideshow, retaining and saving its current wallpaper.
    StopSlideshow,
    /// Advance an active slideshow now.
    Next,
    /// List output names, dimensions and active image assignments as JSON.
    Status,
    /// Re-read the configuration, keeping saved per-output, --all and slideshow
    /// choices; an invalid candidate keeps the current wallpaper.
    Reload,
    /// Forget every saved choice (wallpaper.json) and stop the slideshow, returning
    /// to what the configuration says.
    Reset,
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
    canopa_i18n::init();
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
        Command::Pick { images } => {
            let settings = canopa_session::settings::Settings::load(
                &ConfigRequest::Discover,
                Component::Wallpaper,
            )?
            .0;
            return canopa_session::picker_wayland::run(settings, images);
        }
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
            image,
            output,
            fit,
            temporary,
            ..
        } => Request::Set {
            image: std::path::absolute(image)?,
            output,
            fit,
            persist: !temporary,
        },
        Command::Slideshow {
            directory,
            interval,
            output,
            fit,
            ..
        } => Request::SlideshowStart {
            directory: std::path::absolute(directory)?,
            interval,
            output,
            fit,
        },
        Command::StopSlideshow => Request::SlideshowStop {},
        Command::Next => Request::Next {},
        Command::Status => Request::Status {},
        Command::Reload => Request::Reload {},
        Command::Reset => Request::Reset {},
        Command::Stop => Request::Stop {},
    };
    println!("{}", serde_json::to_string(&ipc::send(&operation)?)?);
    Ok(())
}
