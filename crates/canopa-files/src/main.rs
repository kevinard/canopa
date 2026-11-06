use clap::Parser;
use std::path::PathBuf;
mod copy_check;

#[derive(Parser)]
#[command(name = "canopa-files", version, about = "Canopa's local file manager")]
struct Arguments {
    /// Open a local file in Quick Look, owned by a new Files browser window.
    #[arg(long,conflicts_with_all=["paths","daemon","quit","check_config","internal_check_copy","internal_volume_monitor"])]
    quick_look: Option<PathBuf>,
    #[arg(long, hide = true, conflicts_with_all = ["paths", "config", "defaults", "check_config", "show_source", "daemon", "quit", "internal_volume_monitor"])]
    internal_check_copy: bool,
    #[arg(long, hide = true, conflicts_with_all = ["paths", "config", "defaults", "check_config", "show_source", "daemon", "quit"])]
    internal_volume_monitor: bool,
    /// Local folders or file:// URIs to open. Multiple locations become tabs.
    paths: Vec<PathBuf>,
    /// Stay resident without opening a browser; later launches use this runtime.
    #[arg(long, conflicts_with_all = ["paths", "quit", "check_config"])]
    daemon: bool,
    /// Exit when windows and accepted work finish, releasing resident mode.
    #[arg(long, conflicts_with_all = ["paths", "daemon", "check_config"])]
    quit: bool,
    /// Read this shared Canopa configuration.
    #[arg(long, alias = "canopa-config", conflicts_with = "defaults")]
    config: Option<PathBuf>,
    #[arg(long)]
    check_config: bool,
    #[arg(long)]
    defaults: bool,
    #[arg(long, requires = "check_config")]
    show_source: bool,
}

fn main() -> std::process::ExitCode {
    let arguments = Arguments::parse();
    if arguments.internal_check_copy {
        return match copy_check::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("canopa-files: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    if arguments.internal_volume_monitor {
        return match canopa_files::local_volumes::run_helper() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("canopa-files: local volume monitor: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let request = if arguments.defaults {
        canopa_config::Request::Defaults
    } else {
        arguments.config.clone().map_or(
            canopa_config::Request::Discover,
            canopa_config::Request::common,
        )
    };
    if arguments.check_config {
        return match canopa_files::configuration::ConfigurationSnapshot::load(&request) {
            Ok(snapshot) => {
                println!(
                    "{}",
                    serde_json::json!({"component": "files", "source": snapshot.source})
                );
                std::process::ExitCode::SUCCESS
            }
            Err(error) => {
                if arguments.show_source {
                    eprintln!("{error}");
                }
                println!(
                    "{}",
                    serde_json::json!({"component": "files", "error": error})
                );
                std::process::ExitCode::FAILURE
            }
        };
    }
    use canopa_files::activation::{Command, Configuration};
    let selection = if arguments.defaults {
        Some(Configuration::Defaults)
    } else {
        arguments.config.map(Configuration::File)
    };
    let command = if arguments.daemon {
        Command::Resident
    } else if arguments.quit {
        Command::Quit
    } else if let Some(path) = arguments.quick_look {
        Command::QuickLook(path)
    } else {
        Command::Open(arguments.paths)
    };
    match canopa_files::desktop::run(command, selection) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("canopa-files: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
