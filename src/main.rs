mod app;
mod cli;
mod command;
mod config;
mod dbus;
mod desktop_apps;
mod icon_theme;
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
        cli::Command::Dmenu(options) => std::process::exit(run_dmenu_cli(options)),
        cli::Command::UsageError(message) => {
            eprintln!("niribar: {message}");
            std::process::exit(DMENU_USAGE_EXIT);
        }
        cli::Command::Run => {
            if let Err(e) = try_main() {
                match e {
                    AppError::Config(config_error) => {
                        eprintln!("{:?}", miette::Report::new(config_error));
                    }
                    AppError::Other(error) => eprintln!("error: {error}"),
                }
                std::process::exit(1);
            }
        }
    }
}

#[derive(Debug)]
enum AppError {
    Config(config::ConfigError),
    Other(anyhow::Error),
}

impl From<config::ConfigError> for AppError {
    fn from(value: config::ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value)
    }
}

const DMENU_USAGE_EXIT: i32 = 2;
const DMENU_REQUEST_FAILURE_EXIT: i32 = 3;

fn exit_code_for_dmenu_response(response: &ipc::protocol::IpcResponse) -> i32 {
    match response {
        ipc::protocol::IpcResponse::Selected { .. } => 0,
        ipc::protocol::IpcResponse::Cancelled => 1,
        ipc::protocol::IpcResponse::Rejected { .. } => DMENU_REQUEST_FAILURE_EXIT,
    }
}

fn run_dmenu_cli(options: cli::DmenuOptions) -> i32 {
    use std::io::{Read, Write};

    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("niribar: failed to read stdin: {err}");
        return DMENU_USAGE_EXIT;
    }

    let entries = match ipc::protocol::split_entries(&input, &options.delimiter) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("niribar: {err}");
            return DMENU_USAGE_EXIT;
        }
    };
    let request = ipc::protocol::DmenuRequest {
        prompt: options.prompt,
        entries,
        allow_custom: options.allow_custom,
    };
    if let Err(err) = ipc::protocol::validate_dmenu_request(&request) {
        eprintln!("niribar: {err}");
        return DMENU_USAGE_EXIT;
    }

    let response = match send_dmenu_request(&request) {
        Ok(response) => response,
        Err(err) => {
            eprintln!("niribar: dmenu request failed: {err}");
            return DMENU_REQUEST_FAILURE_EXIT;
        }
    };

    if let ipc::protocol::IpcResponse::Selected { value } = &response {
        println!("{value}");
        let _ = std::io::stdout().flush();
    } else if let ipc::protocol::IpcResponse::Rejected { reason } = &response {
        eprintln!("niribar: dmenu request rejected: {reason}");
    }

    exit_code_for_dmenu_response(&response)
}

fn send_dmenu_request(
    request: &ipc::protocol::DmenuRequest,
) -> anyhow::Result<ipc::protocol::IpcResponse> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let path = ipc::socket_path()?;
    let mut stream = UnixStream::connect(&path)?;
    let frame =
        ipc::protocol::encode_frame(&ipc::protocol::IpcRequest::DmenuSelection(request.clone()))?;
    stream.write_all(&frame)?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response_bytes = Vec::new();
    stream.read_to_end(&mut response_bytes)?;
    ipc::decode_response_frame(&response_bytes)
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

fn try_main() -> Result<(), AppError> {
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
    .map_err(|e| AppError::Other(anyhow::Error::from(e)))
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

    #[test]
    fn dmenu_response_exit_codes_are_stable() {
        assert_eq!(
            super::exit_code_for_dmenu_response(&crate::ipc::protocol::IpcResponse::Selected {
                value: "x".into()
            }),
            0
        );
        assert_eq!(
            super::exit_code_for_dmenu_response(&crate::ipc::protocol::IpcResponse::Cancelled),
            1
        );
        assert_eq!(super::DMENU_USAGE_EXIT, 2);
        assert_eq!(super::DMENU_REQUEST_FAILURE_EXIT, 3);
    }
}
