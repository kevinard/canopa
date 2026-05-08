mod app;
mod cli;
mod command;
mod config;
mod dbus;
mod ipc;
mod modules;
mod niri;
mod surface;
mod theme;

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
    let path = match ipc::socket_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("niribar: IPC unavailable: {e}");
            std::process::exit(1);
        }
    };
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let _ = writeln!(stream, "{cmd}");
        }
        Err(e) => {
            eprintln!(
                "niribar: cannot connect to socket at {}: {e}",
                path.display()
            );
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

    // Extract default surface settings from the first bar (used when AllScreens is active).
    // In AllScreens mode there is always exactly one bar targeting "all" outputs.
    let anchor = config.bars[0].edge.anchor();
    let layer = config.bars[0].layer.layer_shell();
    let collapsed_width = config.bars[0].collapsed_width;
    // All dimensions passed to iced_layershell are in logical pixels (NFR11 / Story 7.2).
    // The compositor multiplies by each output's scale factor for physical rendering.
    let exclusive_zone = i32::try_from(collapsed_width).unwrap_or(48);
    // For horizontal bars (top/bottom) the fixed dimension is the height; for vertical it's width.
    let size = if config.bars[0].edge.is_horizontal() {
        Some((0, collapsed_width)) // logical height for horizontal bar
    } else {
        Some((collapsed_width, 0)) // logical width for vertical bar
    };

    // Use AllScreens only for the single-Bar-without-Output-Target case (Story 9.1).
    // This preserves non-Niri compositor compatibility while supporting multi-bar configs.
    // Any other configuration (multiple bars or output-specific targeting) requires manual mode
    // so each (bar, output) pair can be spawned individually.
    let use_allscreens = config.bars.len() == 1 && config.bars[0].targets_all_outputs();
    let (start_mode, allowed_outputs) = if use_allscreens {
        (StartMode::AllScreens, Vec::new())
    } else {
        match fetch_niri_outputs() {
            Some(all) => (StartMode::Background, all),
            None => {
                // Niri unavailable — warn and fall back to AllScreens single-bar mode.
                eprintln!(
                    "warning: niribar: multi-bar or output-targeted configuration requires Niri \
                     IPC but it is unavailable; bar will appear on all outputs in single-bar mode."
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
            layer,
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
        let entry: fn() = super::main;
        let _ = entry;
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
