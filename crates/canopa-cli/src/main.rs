//! Product configuration operations delegate schema checks to the owning executables.
use canopa_config::{Component, Format, MAX_BYTES, Request};
use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(name = "canopa", version, about = "Canopa desktop tools")]
struct Cli {
    #[command(subcommand)]
    command: Operation,
}
#[derive(Subcommand)]
enum Operation {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}
#[derive(Subcommand)]
enum ConfigCommand {
    /// Check both owning schemas. With no file, use the normal discovery order.
    Check {
        /// Shared Canopa file (never interpreted as a legacy file).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Print local schema diagnostics, which may include configuration contents.
        #[arg(long)]
        show_source: bool,
    },
    /// Wrap explicit legacy sources, check both schemas, then create a new file.
    /// Omitting a component uses its embedded defaults. Existing files are never replaced.
    Migrate {
        #[arg(long)]
        shell: Option<PathBuf>,
        #[arg(long)]
        notifications: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("canopa: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Operation::Config {
            command: ConfigCommand::Check { file, show_source },
        } => {
            let request = file.map(Request::common).unwrap_or(Request::Discover);
            anyhow::ensure!(check(&request, show_source)?, "configuration check failed");
        }
        Operation::Config {
            command:
                ConfigCommand::Migrate {
                    shell,
                    notifications,
                    output,
                },
        } => {
            anyhow::ensure!(
                shell.is_some() || notifications.is_some(),
                "specify at least one legacy source"
            );
            // No source rewriting: wrapping preserves comments and relative path semantics.
            let mut text = String::from("schema-version 1\n");
            for (component, path) in [
                (Component::Shell, shell),
                (Component::Notifications, notifications),
            ] {
                if let Some(path) = path {
                    let source = canopa_config::read(&path)?;
                    // Parsing as legacy before wrapping rejects a file that could escape its block.
                    canopa_config::parse_kdl(&source)?;
                    text.push_str(component.namespace());
                    text.push_str(" {\n");
                    text.push_str(source.trim_start_matches('\u{feff}'));
                    text.push_str("\n}\n");
                }
            }
            anyhow::ensure!(
                text.len() <= MAX_BYTES,
                "combined configuration exceeds 1 MiB"
            );
            let mut snapshot = tempfile::NamedTempFile::new()?;
            snapshot.write_all(text.as_bytes())?;
            anyhow::ensure!(
                check(&Request::common(snapshot.path()), false)?,
                "migration rejected by a component schema"
            );
            install_new(&output, text.as_bytes())?;
            println!(
                "Created {}. Install this candidate at the shared configuration path (or select it explicitly), then restart Canopa Shell and reload Notify.",
                output.display()
            );
        }
    }
    Ok(())
}

/// Read each selected file once for this invocation. Both owners see the same common bytes.
fn check(request: &Request, show_source: bool) -> anyhow::Result<bool> {
    let mut snapshots = HashMap::new();
    let mut valid = true;
    for component in [Component::Shell, Component::Notifications] {
        let result = check_component(request, component, show_source, &mut snapshots);
        match result {
            Ok(report) => {
                valid &= report.get("error").is_none();
                println!("{report}");
            }
            Err(error) => {
                valid = false;
                // All shared intake errors are content-free. Operational errors don't include config text.
                let diagnostic = match error.downcast_ref::<canopa_config::Diagnostic>() {
                    Some(diagnostic) => serde_json::to_value(diagnostic)?,
                    None => serde_json::json!({"code": "validator", "message": error.to_string()}),
                };
                println!(
                    "{}",
                    serde_json::json!({"component": component, "error": diagnostic})
                );
            }
        }
    }
    Ok(valid)
}

struct ValidationSnapshot {
    main: tempfile::NamedTempFile,
    _theme_file: Option<tempfile::NamedTempFile>,
    fingerprint: String,
    theme: Option<canopa_config::ThemeSource>,
    combined_fingerprint: Option<String>,
}
impl ValidationSnapshot {
    fn capture(path: &Path, format: Format) -> anyhow::Result<Self> {
        let text = canopa_config::read(path)?;
        let theme = if format == Format::CanopaV1 {
            canopa_config::capture_theme(&text, Some(path))?
        } else {
            None
        };
        let mut theme_file = None;
        let mut captured_text = text.clone();
        if let Some(theme) = &theme {
            let mut file = tempfile::NamedTempFile::new()?;
            file.write_all(theme.text.as_bytes())?;
            captured_text = theme.snapshot_text(&text, file.path());
            theme_file = Some(file);
        }
        let mut main = tempfile::NamedTempFile::new()?;
        main.write_all(captured_text.as_bytes())?;
        Ok(Self {
            main,
            _theme_file: theme_file,
            fingerprint: canopa_config::fingerprint(&text),
            combined_fingerprint: theme.as_ref().map(|t| t.combined_fingerprint(&text)),
            theme,
        })
    }
}

fn check_component(
    request: &Request,
    component: Component,
    show_source: bool,
    snapshots: &mut HashMap<PathBuf, ValidationSnapshot>,
) -> anyhow::Result<serde_json::Value> {
    let resolved = request.resolve(component)?;
    let mut command = validator(component)?;
    let mut original_path = None;
    match &resolved {
        Request::File { path, format } => {
            if !snapshots.contains_key(path) {
                snapshots.insert(path.clone(), ValidationSnapshot::capture(path, *format)?);
            }
            let snapshot = &snapshots[path];
            original_path = Some(path);
            command
                .arg(match format {
                    Format::CanopaV1 => "--canopa-config",
                    Format::Legacy => "--config",
                })
                .arg(snapshot.main.path());
        }
        Request::Defaults => {
            command.arg("--defaults");
        }
        Request::Discover => unreachable!(),
    }
    if show_source {
        command.arg("--show-source");
    }
    let output = command
        .output()
        .map_err(|_| anyhow::anyhow!("component validator could not execute"))?;
    if show_source {
        if let Some(path) = original_path {
            eprintln!(
                "{}: {} (diagnostics refer to an immutable temporary copy)",
                component.namespace(),
                path.display()
            );
        }
        std::io::stderr().write_all(&output.stderr)?;
    }
    let mut report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| anyhow::anyhow!("component validator returned an invalid report"))?;
    anyhow::ensure!(
        report.get("component") == Some(&serde_json::to_value(component)?),
        "component validator returned the wrong schema owner"
    );
    anyhow::ensure!(
        output.status.success() == report.get("error").is_none(),
        "component validator returned an inconsistent result"
    );
    if let Some(path) = original_path {
        if let Some(source) = report.get_mut("source") {
            source["path"] = serde_json::to_value(path)?;
            let snapshot = &snapshots[path];
            source["fingerprint"] = serde_json::to_value(&snapshot.fingerprint)?;
            if let Some(theme) = &snapshot.theme {
                source["dependencies"] = serde_json::json!([{ "path": theme.path, "fingerprint": canopa_config::fingerprint(&theme.text) }]);
                source["combined_fingerprint"] =
                    serde_json::to_value(&snapshot.combined_fingerprint)?;
            }
        }
        report["path"] = serde_json::to_value(path)?;
    }
    Ok(report)
}

fn validator(component: Component) -> anyhow::Result<Command> {
    // Resolve sibling executables, keeping validation on the same installed release.
    let executable = std::env::current_exe()?;
    let sibling = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot find installation directory"))?
        .join(match component {
            Component::Shell => "canopa-shell",
            Component::Notifications => "canopa-notify",
        });
    anyhow::ensure!(
        sibling.is_file(),
        "install Canopa Shell and Canopa Notify beside the canopa executable"
    );
    let mut command = Command::new(sibling);
    command.arg(match component {
        Component::Shell => "--check-config",
        Component::Notifications => "check-config",
    });
    Ok(command)
}

fn install_new(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let path = std::path::absolute(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output needs a parent directory"))?;
    // Require an existing directory: the command only creates the requested file.
    let mut pending = tempfile::NamedTempFile::new_in(parent)?;
    pending.write_all(bytes)?;
    pending.as_file().sync_all()?;
    pending
        .persist_noclobber(&path)
        .map_err(|_| anyhow::anyhow!("output already exists or cannot be created"))?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn validation_snapshot_captures_main_and_theme_before_either_consumer_runs() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.kdl");
        let theme = directory.path().join("theme.kdl");
        let main = "schema-version 1\nappearance { theme \"theme.kdl\"; }\n";
        std::fs::write(&path, main).unwrap();
        std::fs::write(&theme, "palette { accent \"#123456\"; }\n").unwrap();
        let snapshot = ValidationSnapshot::capture(&path, Format::CanopaV1).unwrap();
        std::fs::write(&path, "invalid").unwrap();
        std::fs::remove_file(&theme).unwrap();
        for consumer in [Component::Shell, Component::Notifications] {
            let document =
                canopa_config::Document::load(&Request::common(snapshot.main.path()), consumer, "")
                    .unwrap();
            let accent = document.appearance().palette().accent;
            assert_eq!((accent.0, accent.1, accent.2), (18, 52, 86));
        }
        assert_eq!(snapshot.fingerprint, canopa_config::fingerprint(main));
        assert_eq!(snapshot.theme.as_ref().unwrap().path, theme);
        assert!(snapshot.combined_fingerprint.is_some());
    }

    #[test]
    fn migration_install_never_clobbers_and_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("config.kdl");
        install_new(&output, b"original").unwrap();
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(install_new(&output, b"replacement").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"original");
    }
}
