mod app;
#[cfg(test)]
mod test_bus;

mod cli;
mod command;
mod config;
mod dbus;
mod desktop_apps;
mod diagnostics;
mod external_panel;
mod focused_window_icons;
mod icon_theme;
mod ipc;
mod modules;
mod niri;
mod surface;
mod theme;
mod wallpaper;

use app::App;
use app::eligible_outputs::EligibleOutputState;
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

fn cli_mode_label(command: &cli::Command) -> &'static str {
    match command {
        cli::Command::Run(_) => "run",
        cli::Command::CheckConfig(_) => "check-config",
        cli::Command::PrintVersion => "version",
        cli::Command::Reload => "reload",
        cli::Command::Lock => "lock",
        cli::Command::IpcCommand(_) => "ipc",
        cli::Command::OpenMenu(_) => "open-menu",
        cli::Command::ToggleMenu(_) => "toggle-menu",
        cli::Command::Dmenu(_) => "dmenu",
        cli::Command::UsageError(_) => "usage-error",
    }
}

fn main() {
    let diagnostics = diagnostics::init();
    let command = cli::parse();
    diagnostics::log_startup(cli_mode_label(&command), &diagnostics);

    match command {
        cli::Command::PrintVersion => println!("{}", env!("CARGO_PKG_VERSION")),
        cli::Command::Reload => std::process::exit(run_reload_cli()),
        cli::Command::Lock => {
            if let Err(error) = wallpaper::lock() {
                eprintln!("canopa-shell: failed to start Swaylock: {error}");
                std::process::exit(1);
            }
        }
        cli::Command::IpcCommand(cmd) => send_ipc_command(&cmd),
        cli::Command::OpenMenu(name) => std::process::exit(run_menu_cli(name, false)),
        cli::Command::ToggleMenu(name) => std::process::exit(run_menu_cli(name, true)),
        cli::Command::Dmenu(options) => std::process::exit(run_dmenu_cli(options)),
        cli::Command::UsageError(message) => {
            eprintln!("canopa-shell: {message}");
            std::process::exit(DMENU_USAGE_EXIT);
        }
        cli::Command::CheckConfig(options) => std::process::exit(check_config(options)),
        cli::Command::Run(options) => {
            if let Err(e) = try_main(&options.request) {
                match e {
                    AppError::Config(config_error) => {
                        eprintln!(
                            "canopa-shell: {}; run canopa-shell --check-config --show-source for details",
                            config_error.safe_diagnostic()
                        );
                    }
                    AppError::Other(error) => eprintln!("error: {error}"),
                }
                std::process::exit(1);
            }
        }
    }
}

fn check_config(options: cli::ConfigOptions) -> i32 {
    match config::load(&options.request, options.show_source) {
        Ok((_, source)) => {
            println!(
                "{}",
                serde_json::json!({"component": "shell", "source": source})
            );
            0
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"component": "shell", "error": error.safe_diagnostic()})
            );
            if options.show_source {
                eprintln!("{:?}", miette::Report::new(error));
            }
            1
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
        ipc::protocol::IpcResponse::Rejected { .. } | ipc::protocol::IpcResponse::Accepted => {
            DMENU_REQUEST_FAILURE_EXIT
        }
    }
}

fn parse_dmenu_stdin(
    input: &str,
    options: &cli::DmenuOptions,
) -> Result<Vec<ipc::protocol::DmenuEntry>, ipc::protocol::ProtocolError> {
    match options.entry_format {
        cli::DmenuEntryFormat::Plain => {
            ipc::protocol::split_dmenu_entries_plain(input, &options.delimiter)
        }
        cli::DmenuEntryFormat::JsonLines => ipc::protocol::parse_dmenu_entries_jsonl(input),
    }
}

fn run_dmenu_cli(options: cli::DmenuOptions) -> i32 {
    use std::io::{Read, Write};

    let mut input = String::new();
    if let Err(err) = std::io::stdin()
        .take((ipc::protocol::MAX_FRAME_BYTES + 1) as u64)
        .read_to_string(&mut input)
    {
        eprintln!("canopa-shell: failed to read stdin: {err}");
        return DMENU_USAGE_EXIT;
    }

    if input.len() > ipc::protocol::MAX_FRAME_BYTES {
        eprintln!("canopa-shell: dmenu input exceeds 1 MiB");
        return DMENU_USAGE_EXIT;
    }
    let entries = match parse_dmenu_stdin(&input, &options) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("canopa-shell: {err}");
            return DMENU_USAGE_EXIT;
        }
    };
    let request = ipc::protocol::DmenuRequest {
        prompt: options.prompt,
        entries,
        allow_custom: options.allow_custom,
    };
    if let Err(err) = ipc::protocol::validate_dmenu_request(&request) {
        eprintln!("canopa-shell: {err}");
        return DMENU_USAGE_EXIT;
    }

    let response =
        match send_framed_request(&ipc::protocol::IpcRequest::DmenuSelection(request), None) {
            Ok(response) => response,
            Err(err) => {
                eprintln!("canopa-shell: dmenu request failed: {err}");
                return DMENU_REQUEST_FAILURE_EXIT;
            }
        };

    if let ipc::protocol::IpcResponse::Selected { value } = &response {
        println!("{value}");
        let _ = std::io::stdout().flush();
    } else if let ipc::protocol::IpcResponse::Rejected { reason } = &response {
        eprintln!("canopa-shell: dmenu request rejected: {reason}");
    }

    exit_code_for_dmenu_response(&response)
}

fn run_reload_cli() -> i32 {
    match send_framed_request(
        &ipc::protocol::IpcRequest::Reload,
        Some(std::time::Duration::from_secs(10)),
    ) {
        Ok(ipc::protocol::IpcResponse::Accepted) => {
            println!("configuration reloaded");
            0
        }
        Ok(ipc::protocol::IpcResponse::Rejected { reason }) => {
            eprintln!("canopa-shell: reload rejected: {reason}");
            1
        }
        Ok(_) => {
            eprintln!("canopa-shell: unexpected reload response");
            1
        }
        Err(error) => {
            eprintln!("canopa-shell: reload failed: {error}");
            1
        }
    }
}

fn run_menu_cli(name: String, toggle: bool) -> i32 {
    use ipc::protocol::{IpcRequest, IpcResponse};

    let (operation, request) = if toggle {
        ("toggle-menu", IpcRequest::ToggleMenu { name })
    } else {
        ("open-menu", IpcRequest::OpenMenu { name })
    };
    match send_framed_request(&request, Some(std::time::Duration::from_secs(2))) {
        Ok(IpcResponse::Accepted) => 0,
        Ok(IpcResponse::Rejected { reason }) => {
            eprintln!("canopa-shell: {operation} request rejected: {reason}");
            1
        }
        Ok(_) => {
            eprintln!("canopa-shell: unexpected {operation} response");
            1
        }
        Err(error) => {
            eprintln!("canopa-shell: {operation} request failed: {error}");
            1
        }
    }
}

fn send_framed_request(
    request: &ipc::protocol::IpcRequest,
    response_timeout: Option<std::time::Duration>,
) -> anyhow::Result<ipc::protocol::IpcResponse> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let path = ipc::socket_path()?;
    let mut stream = UnixStream::connect(&path)?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.set_read_timeout(response_timeout)?;
    let frame = ipc::protocol::encode_frame(request)?;
    stream.write_all(&frame)?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut response_bytes = Vec::new();
    stream
        .take((ipc::protocol::MAX_FRAME_BYTES + ipc::protocol::FRAMED_MAGIC.len() + 9) as u64)
        .read_to_end(&mut response_bytes)?;
    ipc::decode_response_frame(&response_bytes)
}

fn send_ipc_command(cmd: &str) {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    let path = match ipc::socket_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("canopa-shell: IPC unavailable: {e}");
            std::process::exit(1);
        }
    };
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            let _ = writeln!(stream, "{cmd}");
        }
        Err(e) => {
            eprintln!(
                "canopa-shell: cannot connect to socket at {}: {e}",
                path.display()
            );
            std::process::exit(1);
        }
    }
}

fn try_main(request: &canopa_config::Request) -> Result<(), AppError> {
    // Load config fully before any surface is created (AR7).
    let (config, source) = config::load(request, false)?;
    theme::install(config.appearance.clone());
    tracing::info!(target: "canopa_shell::configuration", source = ?source, "configuration installed; automatic reload enabled");
    tracing::debug!(
        target: "canopa_shell::runtime",
        bar_count = config.bars.len(),
        configured_module_count = config.modules.len(),
        menu_count = config.menus.len(),
        "configuration loaded"
    );

    // Extract default layer-shell settings from the first bar. Normal Bar Surfaces are opened
    // after the Wayland Output Adapter reports named Outputs.
    let anchor = config.bars[0].edge.anchor();
    let layer = config.bars[0].layer.layer_shell();
    // All dimensions passed to iced_layershell are in logical pixels (NFR11 / Story 7.2).
    // The compositor multiplies by each output's scale factor for physical rendering.
    let exclusive_zone = crate::surface::bar_geometry::BarGeometry::reservation(&config.bars[0]);
    // For horizontal bars (top/bottom) the fixed dimension is the height; for vertical it's width.
    let size = Some(crate::surface::bar_geometry::BarGeometry::host_size(
        &config.bars[0],
        0,
    ));

    let start_mode = StartMode::Background;
    let eligible_outputs = EligibleOutputState::Unknown;
    tracing::debug!(
        target: "canopa_shell::runtime",
        "waiting for Wayland Output Snapshot before opening Bar Surfaces"
    );

    let request = match request {
        canopa_config::Request::Discover => request.clone(),
        _ => request
            .resolve(canopa_config::Component::Shell)
            .map_err(config::ConfigError::Intake)?,
    };
    daemon(
        move || {
            let (mut app, task) =
                App::boot_with_eligible_outputs(config.clone(), eligible_outputs.clone());
            app.configure_reload(request.clone(), source.clone());
            (app, task)
        },
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
    .theme(|_: &App, _| theme::current())
    .style(App::style)
    .subscription(App::subscription)
    .run()
    .map_err(|e| AppError::Other(anyhow::Error::from(e)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_dmenu_stdin_uses_plain_format_by_default() {
        let options = crate::cli::DmenuOptions::default();
        let entries = super::parse_dmenu_stdin("firefox\nkitty\n", &options).unwrap();

        assert_eq!(
            entries,
            vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
            ]
        );
    }

    #[test]
    fn parse_dmenu_stdin_uses_json_lines_format() {
        let options = crate::cli::DmenuOptions {
            entry_format: crate::cli::DmenuEntryFormat::JsonLines,
            ..Default::default()
        };

        let entries = super::parse_dmenu_stdin(
            r#"{"label":"Firefox","value":"org.mozilla.firefox","icon":"firefox"}"#,
            &options,
        )
        .unwrap();

        assert_eq!(entries[0].display_label(), "Firefox");
        assert_eq!(entries[0].selected_value(), "org.mozilla.firefox");
    }

    #[test]
    fn binary_entry_point_compiles() {
        // Smoke test: if this test runs, the binary entry point compiled
        let entry: fn() = super::main;
        let _ = entry;
    }

    #[test]
    fn cli_mode_label_identifies_cli_modes() {
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::Run(Default::default())),
            "run"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::PrintVersion),
            "version"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::IpcCommand("toggle-panel".into())),
            "ipc"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::Dmenu(
                crate::cli::DmenuOptions::default()
            )),
            "dmenu"
        );
        assert_eq!(
            super::cli_mode_label(&crate::cli::Command::UsageError("bad flag".into())),
            "usage-error"
        );
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
