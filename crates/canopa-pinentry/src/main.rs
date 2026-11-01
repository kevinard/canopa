use anyhow::Result;
use canopa_config::Request;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Canopa Wayland PIN and passphrase dialog")]
struct Cli {
    #[arg(long, global = true, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    #[arg(long, global = true)]
    defaults: bool,
    /// Compatibility metadata supplied by gpg-agent; Wayland uses its environment.
    #[arg(long, allow_hyphen_values = true)]
    display: Option<String>,
    #[command(subcommand)]
    command: Option<Operation>,
}
#[derive(Subcommand)]
enum Operation {
    CheckConfig {
        #[arg(long)]
        show_source: bool,
    },
    Preview {
        output: PathBuf,
    },
}
fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("canopa-pinentry: {error:#}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<()> {
    let request = if cli.defaults {
        Request::Defaults
    } else {
        cli.canopa_config
            .map(Request::common)
            .unwrap_or(Request::Discover)
    };
    let loaded = canopa_pinentry::config::load(&request);
    if let Some(Operation::CheckConfig { show_source }) = cli.command {
        return match loaded {
            Ok((_, document)) => {
                println!(
                    "{}",
                    serde_json::json!({"component":"pinentry","source":document.info()})
                );
                Ok(())
            }
            Err(error) => {
                if show_source {
                    eprintln!("{error:#}");
                }
                let diagnostic=error.downcast_ref::<canopa_config::Diagnostic>().map(serde_json::to_value).transpose()?.unwrap_or_else(||serde_json::json!({"code":"pinentry-schema","message":"invalid pinentry configuration"}));
                println!(
                    "{}",
                    serde_json::json!({"component":"pinentry","error":diagnostic})
                );
                Err(error)
            }
        };
    }
    let (settings, _) = loaded?;
    if let Some(Operation::Preview { output }) = cli.command {
        let controller = canopa_pinentry::controller::Controller::new(
            &settings,
            canopa_pinentry::protocol::Request {
                description: "Enter the passphrase to unlock your OpenPGP key.".into(),
                ..Default::default()
            },
        )?;
        let color = settings.color;
        controller
            .ui
            .render_at_scale(
                &settings,
                image::RgbImage::from_pixel(800, 720, image::Rgb([color.0, color.1, color.2])),
                1.,
            )?
            .0
            .save_with_format(output, image::ImageFormat::Png)?;
        return Ok(());
    }
    canopa_session_ui::secret::protect_process()?;
    // An unbuffered duplicated stdout keeps protocol data out of stdio buffers.
    use std::os::fd::AsFd;
    let output = std::fs::File::from(std::io::stdout().as_fd().try_clone_to_owned()?);
    canopa_pinentry::protocol::serve(
        std::io::stdin().lock(),
        output,
        settings.auth_timeout,
        |request| match canopa_pinentry::wayland::run(&settings, request) {
            Ok(response) => response,
            Err(_) => {
                eprintln!("canopa-pinentry: graphical dialog unavailable");
                canopa_pinentry::protocol::Response::Unavailable
            }
        },
    )?;
    Ok(())
}
