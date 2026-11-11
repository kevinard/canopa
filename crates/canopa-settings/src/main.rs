use anyhow::Result;
use canopa_settings::{session::Paths, storage::Snapshot};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Canopa desktop settings",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    /// Open the settings window at this page.
    #[arg(long, value_enum)]
    page: Option<canopa_settings::Page>,
    /// Open Displays with a saved layout selected for review.
    #[arg(long, value_name = "NAME")]
    display_profile: Option<String>,
    #[command(subcommand)]
    command: Option<Operation>,
}
#[derive(Subcommand)]
enum Operation {
    /// Initialize a separate Canopa compositor configuration without replacing files.
    InitSession {
        #[arg(long)]
        directory: Option<PathBuf>,
        #[arg(long)]
        defaults_file: Option<PathBuf>,
    },
    /// Validate the Canopa compositor configuration without changing the session.
    Check {
        #[arg(long)]
        directory: Option<PathBuf>,
    },
    /// Print connected monitor information as JSON.
    Outputs,
    /// Apply a JSON input-settings document in the current Canopa session.
    ApplyInput {
        #[arg(long)]
        file: PathBuf,
    },
    #[command(hide = true)]
    DisplayGuard {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long, requires = "profile")]
        automatic: bool,
    },
    #[command(hide = true)]
    WatchDisplayProfiles,
}
fn main() {
    if let Err(error) = run() {
        eprintln!("canopa-settings: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    anyhow::ensure!(
        cli.page.is_none() || cli.command.is_none(),
        "--page is only available when opening the settings window"
    );
    anyhow::ensure!(
        cli.display_profile.is_none()
            || cli
                .page
                .is_none_or(|page| page == canopa_settings::Page::Displays),
        "--display-profile opens the Displays page"
    );
    match cli.command {
        Some(Operation::InitSession {
            directory,
            defaults_file,
        }) => {
            let paths = directory
                .map(|directory| Paths { directory })
                .map(Ok)
                .unwrap_or_else(Paths::discover)?;
            paths.initialize(defaults_file.as_deref())?;
            // Shortcut changes must never prevent logging in: keep the last
            // valid files and report why they were not updated.
            match canopa_settings::shortcuts::refresh(
                &paths,
                defaults_file.as_deref(),
                canopa_settings::niri::validate,
            ) {
                Ok(Some(change)) => eprintln!("canopa-settings: {change}"),
                Ok(None) => {}
                Err(error) => {
                    eprintln!("canopa-settings: keyboard shortcuts were not updated: {error:#}")
                }
            }
            canopa_settings::niri::validate(&paths.niri())?;
            println!("{}", paths.niri().display());
        }
        Some(Operation::Check { directory }) => {
            let paths = directory
                .map(|directory| Paths { directory })
                .map(Ok)
                .unwrap_or_else(Paths::discover)?;
            canopa_settings::niri::validate(&paths.niri())?;
        }
        Some(Operation::Outputs) => println!(
            "{}",
            serde_json::to_string(&canopa_settings::niri::Client::discover()?.outputs()?)?
        ),
        Some(Operation::ApplyInput { file }) => {
            let paths = Paths::discover()?;
            anyhow::ensure!(paths.active(), "input settings require the Canopa session");
            let value: canopa_settings::input::Input =
                serde_json::from_slice(&canopa_file::read_bounded(&file, 16384)?)?;
            let source = Snapshot::load(paths.input())?;
            canopa_settings::input::apply(&paths, &source, &value)?;
        }
        Some(Operation::DisplayGuard { profile, automatic }) => {
            if let Err(error) = canopa_settings::display::guard(profile.as_deref(), automatic) {
                let _ = writeln!(
                    std::io::stdout(),
                    "{}",
                    serde_json::json!({"state":"error", "message":format!("{error:#}")})
                );
                return Err(error);
            }
        }
        Some(Operation::WatchDisplayProfiles) => canopa_settings::profile_watch::run()?,
        None => {
            #[cfg(feature = "desktop")]
            canopa_settings::ui::run_with_profile(
                cli.page.unwrap_or(canopa_settings::Page::Displays),
                cli.display_profile,
            )?;
            #[cfg(not(feature = "desktop"))]
            anyhow::bail!("this build does not include the desktop interface");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_page_is_explicit_and_does_not_mix_with_operations() {
        assert_eq!(
            Cli::try_parse_from(["canopa-settings", "--page", "input"])
                .unwrap()
                .page,
            Some(canopa_settings::Page::Input)
        );
        assert!(Cli::try_parse_from(["canopa-settings", "--page", "unknown"]).is_err());
        assert!(Cli::try_parse_from(["canopa-settings", "--page", "power", "outputs"]).is_err());
    }
}
