mod app;
mod cli;
mod config;
mod dbus;
mod ipc;
mod modules;
mod niri;
mod surface;
mod theme;

use std::collections::HashSet;

use app::App;
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

fn main() {
    match cli::parse() {
        cli::Command::PrintVersion => println!("{}", env!("CARGO_PKG_VERSION")),
        cli::Command::IpcCommand(cmd) => send_ipc_command(&cmd),
        cli::Command::Run => {
            if let Err(e) = try_main() {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn send_ipc_command(cmd: &str) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let path = ipc::socket_path();
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let _ = writeln!(stream, "{cmd}");
        }
        Err(e) => {
            eprintln!("niribar: cannot connect to socket at {}: {e}", path.display());
            std::process::exit(1);
        }
    }
}

/// Synchronously query Niri IPC for connected output names.
/// Returns `None` if Niri is not running or the IPC call fails.
fn fetch_niri_outputs() -> Option<Vec<String>> {
    let mut socket = niri_ipc::socket::Socket::connect().ok()?;
    match socket.send(niri_ipc::Request::Outputs).ok()? {
        Ok(niri_ipc::Response::Outputs(map)) => Some(map.into_keys().collect()),
        _ => None,
    }
}

fn try_main() -> anyhow::Result<()> {
    // Load config fully before any surface is created (AR7).
    let config = config::load()?;

    // Extract values needed for surface settings before config is moved into the closure.
    let anchor = config.bar.edge.anchor();
    let collapsed_width = config.bar.collapsed_width;
    // All dimensions passed to iced_layershell are in logical pixels (NFR11 / Story 7.2).
    // The compositor multiplies by each output's scale factor for physical rendering.
    let exclusive_zone = i32::try_from(collapsed_width).unwrap_or(48);
    // For horizontal bars (top/bottom) the fixed dimension is the height; for vertical it's width.
    let size = if config.bar.edge.is_horizontal() {
        Some((0, collapsed_width)) // logical height for horizontal bar
    } else {
        Some((collapsed_width, 0)) // logical width for vertical bar
    };

    // Determine which outputs to show the bar on (Story 7.3).
    // If no outputs have enabled=false, skip Niri query and keep AllScreens (zero behavior change).
    let excluded: HashSet<String> = config
        .outputs
        .iter()
        .filter(|o| !o.enabled)
        .map(|o| o.name.clone())
        .collect();

    // Also require manual per-output mode when any output has width or modules overrides,
    // because AllScreens mode never assigns output_name to surfaces — making effective_bar_for("")
    // always return global defaults regardless of per-output config.
    let has_per_output_overrides = config
        .outputs
        .iter()
        .any(|o| o.collapsed_width.is_some() || o.expanded_width.is_some() || o.modules.is_some());

    let (start_mode, allowed_outputs) = if excluded.is_empty() && !has_per_output_overrides {
        // No exclusions or per-output overrides — use AllScreens as before (Story 7.1 behavior).
        (StartMode::AllScreens, Vec::new())
    } else {
        match fetch_niri_outputs() {
            Some(all) => {
                let allowed: Vec<String> =
                    all.into_iter().filter(|n| !excluded.contains(n)).collect();
                (StartMode::Background, allowed)
            }
            None => {
                // Niri unavailable — warn and fall back to AllScreens (exclusions ignored).
                eprintln!(
                    "warning: niribar: output exclusion configured but Niri IPC unavailable; \
                     bar will appear on all outputs."
                );
                (StartMode::AllScreens, Vec::new())
            }
        }
    };

    daemon(
        move || App::boot(config.clone(), allowed_outputs.clone()),
        App::namespace,
        App::update,
        App::view,
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone,
            size,
            start_mode,
            ..Default::default()
        },
        ..Default::default()
    })
    .style(App::style)
    .subscription(App::subscription)
    .run()
    .map_err(anyhow::Error::from)
}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_entry_point_compiles() {
        // Smoke test: if this test runs, the binary entry point compiled
        assert!(true);
    }

    #[test]
    fn version_string_is_non_empty() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }

    #[test]
    fn version_string_matches_cargo_toml() {
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
    }
}
