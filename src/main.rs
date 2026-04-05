mod app;
mod config;
mod dbus;
mod ipc;
mod modules;
mod niri;
mod surface;
mod theme;

use app::App;
use iced_layershell::build_pattern::application;
use iced_layershell::settings::{LayerShellSettings, Settings};

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if std::env::args().any(|a| a == "--toggle-expand") {
        send_ipc_command("toggle-expand");
        return;
    }
    if let Err(e) = try_main() {
        eprintln!("error: {e}");
        std::process::exit(1);
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
    let exclusive_zone = i32::try_from(collapsed_width).unwrap_or(48);

    application(
        move || App::new(config.clone()),
        App::namespace,
        App::update,
        App::view,
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone,
            size: Some((collapsed_width, 0)),
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
