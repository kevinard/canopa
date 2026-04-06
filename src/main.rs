mod app;
mod cli;
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

    daemon(
        move || App::boot(config.clone()),
        App::namespace,
        App::update,
        App::view,
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone,
            size,
            start_mode: StartMode::AllScreens,
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
