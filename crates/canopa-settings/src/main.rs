use anyhow::Result;
use canopa_settings::{Page, registry, session::Paths, storage::Snapshot};
use clap::{
    Parser, Subcommand,
    builder::{PossibleValue, PossibleValuesParser, TypedValueParser},
};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Canopa desktop settings",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    /// Open the settings window at this page. A running window switches to it.
    #[arg(value_name = "PAGE", value_parser = page_parser())]
    target: Option<Page>,
    /// The same as PAGE.
    #[arg(long, value_name = "PAGE", value_parser = page_parser(), conflicts_with = "target")]
    page: Option<Page>,
    /// Open Displays with a saved layout selected for review.
    #[arg(long, value_name = "NAME")]
    display_profile: Option<String>,
    #[command(subcommand)]
    command: Option<Operation>,
}
/// Page ids from the registry; hidden pages are accepted but not listed.
fn page_parser() -> impl TypedValueParser<Value = Page> {
    PossibleValuesParser::new(
        registry::PAGES
            .iter()
            .map(|entry| PossibleValue::new(entry.id).hide(entry.hidden)),
    )
    .map(|id| Page::from_id(&id).expect("the parser lists registry ids"))
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
    /// Print the visible pages as JSON, in sidebar order, for launchers.
    Pages,
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
    /// Label each display with the number Displays gives it, reading the
    /// labels from standard input until it closes.
    #[command(hide = true)]
    IdentifyDisplays,
}
/// `{"pages":[{"id","title","description","icon","keywords"}]}`, the same
/// registry entries the sidebar, search and D-Bus `Pages` use, in the
/// selected language; keywords also list the English ones.
fn pages_json() -> serde_json::Value {
    let pages: Vec<_> = registry::navigation()
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "title": entry.title(),
                "description": entry.description(),
                "icon": entry.icon,
                // The Shell's launcher accepts up to 32 keywords a page.
                "keywords": entry.search_keywords(32),
            })
        })
        .collect();
    serde_json::json!({ "pages": pages })
}
fn main() {
    canopa_i18n::init();
    if let Err(error) = run() {
        eprintln!("canopa-settings: {error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let mut cli = Cli::parse();
    let page = cli.target.take().or(cli.page);
    anyhow::ensure!(
        page.is_none() || cli.command.is_none(),
        "a page is only available when opening the settings window"
    );
    anyhow::ensure!(
        cli.display_profile.is_none() || page.is_none_or(|page| page == Page::Displays),
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
            // Keep the previous compositor appearance if regeneration fails.
            if let Err(error) =
                canopa_settings::titlebar::appearance(&paths).and_then(|appearance| {
                    canopa_settings::titlebar::refresh(
                        &paths,
                        &appearance,
                        canopa_settings::niri::validate,
                    )
                })
            {
                eprintln!("canopa-settings: the compositor appearance was not updated: {error:#}");
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
        Some(Operation::Pages) => println!("{}", pages_json()),
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
        Some(Operation::IdentifyDisplays) => {
            #[cfg(feature = "desktop")]
            canopa_settings::identify::run()?;
            #[cfg(not(feature = "desktop"))]
            anyhow::bail!("this build does not include the desktop interface");
        }
        None => {
            #[cfg(feature = "desktop")]
            {
                use canopa_settings::instance::{Claim, Request, claim};
                let request = Request {
                    // A display profile is reviewed on Displays.
                    page: page.or(cli.display_profile.as_ref().map(|_| Page::Displays)),
                    display_profile: cli.display_profile,
                };
                // One window per session: a running one shows the page instead.
                let instance = match claim(&request) {
                    Ok(Claim::Forwarded) => return Ok(()),
                    Ok(Claim::Primary(instance)) => Some(instance),
                    Err(error) => {
                        eprintln!("canopa-settings: opening a separate window: {error:#}");
                        None
                    }
                };
                canopa_settings::ui::run(request, instance)?;
            }
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
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("canopa-settings").chain(args.iter().copied()))
        };
        assert_eq!(parse(&["--page", "input"]).unwrap().page, Some(Page::Input));
        assert_eq!(parse(&["power"]).unwrap().target, Some(Page::Power));
        assert_eq!(
            parse(&["widgets", "--display-profile", "Desk"])
                .unwrap()
                .target,
            Some(Page::Widgets)
        );
        assert!(parse(&["--page", "unknown"]).is_err());
        assert!(parse(&["unknown"]).is_err());
        assert!(parse(&["power", "--page", "input"]).is_err());
        assert!(parse(&["--page", "power", "outputs"]).is_err());
        // Operations remain subcommands, not page ids.
        assert!(matches!(
            parse(&["outputs"]).unwrap().command,
            Some(Operation::Outputs)
        ));
        assert!(parse(&["check", "power"]).is_err());
        assert!(matches!(
            parse(&["pages"]).unwrap().command,
            Some(Operation::Pages)
        ));
    }

    #[test]
    fn pages_list_the_visible_registry_in_sidebar_order() {
        let json = pages_json();
        let pages = json["pages"].as_array().unwrap();
        let ids: Vec<_> = pages
            .iter()
            .map(|page| page["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, registry::ids().collect::<Vec<_>>());
        let displays = pages.iter().find(|page| page["id"] == "displays").unwrap();
        assert_eq!(displays["title"], Page::Displays.title());
        assert!(
            displays["keywords"]
                .as_array()
                .is_some_and(|k| !k.is_empty())
        );
    }
}
