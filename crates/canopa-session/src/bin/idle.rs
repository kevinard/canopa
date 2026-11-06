use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "canopa-idle",
    version,
    about = "Canopa Stasis integration and secure suspend coordination"
)]
struct Cli {
    #[arg(long, global = true, default_value = "stasis")]
    stasis: PathBuf,
    #[arg(long, global = true)]
    session: Option<String>,
    #[command(subcommand)]
    command: Operation,
}
#[derive(Subcommand)]
enum Operation {
    /// Run external Stasis after validating the selected session's lock coordinator.
    Run {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Create Canopa/Niri Rune defaults; an existing configuration is never overwritten.
    Init {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Refuse to request suspend until the compositor confirms locking.
    Suspend,
    Info {
        #[arg(long)]
        json: bool,
    },
    Watch,
    Pause {
        duration: Option<String>,
    },
    Resume,
    Toggle,
    Reload,
    Profile {
        name: String,
    },
}
fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("canopa-idle: {error:#}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<()> {
    use canopa_session::idle;
    let command = match cli.command {
        Operation::Init { output } => {
            return idle::initialize(&output.map(Ok).unwrap_or_else(idle::config_path)?);
        }
        Operation::Run { config } => {
            let config = config.map(Ok).unwrap_or_else(idle::config_path)?;
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(idle::run(&cli.stasis, &config, cli.session.as_deref()));
        }
        Operation::Suspend => {
            return tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(idle::suspend(cli.session.as_deref()));
        }
        Operation::Info { json } => {
            if json {
                vec!["info".into(), "--json".into()]
            } else {
                vec!["info".into()]
            }
        }
        Operation::Watch => vec!["watch".into()],
        Operation::Pause { duration } => {
            let mut args = vec!["pause".into()];
            if let Some(duration) = duration {
                args.push(duration.into());
            }
            args
        }
        Operation::Resume => vec!["resume".into()],
        Operation::Toggle => vec!["toggle-inhibit".into()],
        Operation::Reload => vec!["reload".into()],
        Operation::Profile { name } => vec!["profile".into(), name.into()],
    };
    idle::forward(&cli.stasis, command)
}
