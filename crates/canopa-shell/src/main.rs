/// The Shell's messages: the library's catalog, with a local `fl!` checked
/// against the same English file.
#[macro_use]
mod i18n {
    pub use canopa_shell::i18n::LOADER;

    macro_rules! fl {
        ($($args:tt)*) => {
            canopa_i18n::checked_fl!(crate::i18n::LOADER.get(), $($args)*)
        };
    }
}
mod app;
#[cfg(test)]
#[allow(dead_code)]
mod i18n_testing;
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
mod launcher;
mod modules;
mod niri;
mod notice;
mod session;
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
        cli::Command::PrintHelp => "help",
        cli::Command::ClipboardFrame(_) => "clipboard-frame",
        cli::Command::LauncherExtension(..) => "launcher-extension",
        cli::Command::AppScope(_) => "app-scope",
        cli::Command::HardwarePanel(_) => "hardware-panel",
        cli::Command::Reload => "reload",
        cli::Command::Lock => "lock",
        cli::Command::IpcCommand(_) => "ipc",
        cli::Command::OpenMenu(_) => "open-menu",
        cli::Command::ToggleMenu(_) => "toggle-menu",
        cli::Command::OpenTemplate(_) => "open-template",
        cli::Command::Dmenu(_) => "dmenu",
        cli::Command::DeviceKey(_) => "device-key",
        cli::Command::UsageError(_) => "usage-error",
    }
}

fn main() {
    canopa_i18n::init();
    // Keep the Pulse client name the audio server showed before the shared client.
    canopa_services::audio::set_client_name("canopa-shell");
    let command = cli::parse();
    if let cli::Command::ClipboardFrame(kind) = command {
        std::process::exit(i32::from(launcher::clipboard::write_frame(kind).is_err()));
    }
    // Runs once per launch before the application; keep it free of logging setup.
    if let cli::Command::AppScope(request) = command {
        std::process::exit(command::app_scope::run(request));
    }
    let diagnostics = diagnostics::init();
    diagnostics::log_startup(cli_mode_label(&command), &diagnostics);

    match command {
        cli::Command::ClipboardFrame(_) | cli::Command::AppScope(_) => unreachable!(),
        cli::Command::LauncherExtension(mode, template) => {
            if let Err(error) = launcher::bundled::run(&mode, template.as_deref()) {
                eprintln!("canopa-shell: {error}");
                std::process::exit(1);
            }
        }
        cli::Command::HardwarePanel(name) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = runtime.map_err(anyhow::Error::from).and_then(|runtime| {
                runtime.block_on(canopa_shell::hardware_panel::run(
                    &name,
                    tokio::io::BufReader::new(tokio::io::stdin()),
                    tokio::io::stdout(),
                ))
            });
            if result.is_err() {
                tracing::error!(
                    component = "hardware_panel",
                    "hardware provider session failed"
                );
                std::process::exit(1);
            }
        }
        cli::Command::PrintVersion => println!("{}", env!("CARGO_PKG_VERSION")),
        cli::Command::PrintHelp => print!("{}", cli::USAGE),
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
        cli::Command::OpenTemplate(name) => std::process::exit(run_template_cli(name)),
        cli::Command::Dmenu(options) => std::process::exit(run_dmenu_cli(options)),
        cli::Command::DeviceKey(request) => std::process::exit(run_device_key_cli(request)),
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

/// Run a device key in the Shell, which shows the result. Without a running
/// Shell the key still works, without a display, using the configured steps.
/// Missing services are an ordinary result: exit 1 with nothing printed, so
/// key repeats never fill the compositor's log.
fn run_device_key_cli(request: canopa_shell::device_keys::Request) -> i32 {
    use ipc::protocol::{IpcRequest, IpcResponse};
    match send_framed_request(
        &IpcRequest::DeviceKey(request),
        Some(std::time::Duration::from_secs(5)),
    ) {
        Ok(IpcResponse::Accepted) => 0,
        Ok(_) => 1,
        Err(error) if shell_unreachable(&error) => {
            let steps = config::load(&canopa_config::Request::Discover, false)
                .map(|(config, _)| config.osd.steps())
                .unwrap_or_default();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let Ok(runtime) = runtime else {
                return 1;
            };
            let keys = canopa_shell::device_keys::Keys::default();
            let outcome = runtime.block_on(keys.run(request, steps));
            i32::from(!outcome.succeeded())
        }
        Err(error) => {
            tracing::debug!(%error, "device key request failed");
            1
        }
    }
}

/// No Shell owns the control socket.
fn shell_unreachable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<std::io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        )
    })
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

fn run_template_cli(name: String) -> i32 {
    use ipc::protocol::{IpcRequest, IpcResponse};

    match send_framed_request(
        &IpcRequest::OpenTemplate { name },
        Some(std::time::Duration::from_secs(2)),
    ) {
        Ok(IpcResponse::Accepted) => 0,
        Ok(IpcResponse::Rejected { reason }) => {
            eprintln!("canopa-shell: open-template request rejected: {reason}");
            1
        }
        Ok(_) => {
            eprintln!("canopa-shell: unexpected open-template response");
            1
        }
        Err(error) => {
            eprintln!("canopa-shell: open-template request failed: {error}");
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

/// The configuration the Shell starts with. An invalid selection never stops the
/// Shell: it starts with the embedded defaults (as `canopa-lock` does), keeps
/// watching the selected file, and reports the problem in the bar and Notify.
fn startup_configuration(
    request: &canopa_config::Request,
) -> Result<
    (
        config::Config,
        canopa_config::SourceInfo,
        canopa_config::Request,
        Option<config::notice::ConfigNotice>,
    ),
    AppError,
> {
    let (config, source, notice) = match config::load(request, true) {
        Ok((config, source)) => (config, source, None),
        Err(error) => {
            let notice = config::notice::ConfigNotice::from_error(&error, request);
            tracing::warn!(target: "canopa_shell::configuration", error = %error.safe_diagnostic(),
                "configuration rejected at startup; starting with the embedded defaults");
            eprintln!(
                "canopa-shell: {}; starting with the embedded defaults. Run canopa-shell --check-config --show-source for details",
                error.safe_diagnostic()
            );
            let (config, source) = config::load(&canopa_config::Request::Defaults, false)?;
            (config, source, Some(notice))
        }
    };
    // Discovery is repeated on every reload; explicit paths are resolved once.
    // A selection that cannot be resolved has nothing to watch.
    let watched = match request {
        canopa_config::Request::Discover => request.clone(),
        _ => request
            .resolve(canopa_config::Component::Shell)
            .unwrap_or_default(),
    };
    Ok((config, source, watched, notice))
}

fn try_main(request: &canopa_config::Request) -> Result<(), AppError> {
    // Load config fully before any surface is created (AR7).
    let (config, source, request, startup_error) = startup_configuration(request)?;
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

    daemon(
        move || {
            let (mut app, task) =
                App::boot_with_eligible_outputs(config.clone(), eligible_outputs.clone());
            app.configure_reload(request.clone(), source.clone(), startup_error.clone());
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
    fn invalid_configuration_starts_with_defaults_and_keeps_watching_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.kdl");
        let (defaults, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        for text in [
            "schema-version 1\nshell { bar \"main\" { edge \"nowhere\"; }; }\n",
            "schema-version 1\nshell {\n",
            "schema-version 7\n",
            "schema-version 1\nappearance { color-scheme \"plaid\"; }\n",
        ] {
            std::fs::write(&path, text).unwrap();
            let request = canopa_config::Request::common(&path);
            let (config, source, watched, notice) =
                super::startup_configuration(&request).expect("never exits on a config error");
            assert_eq!(config, defaults, "{text}");
            assert!(source.defaults);
            assert_eq!(watched, request);
            let notice = notice.expect("the error is reported");
            assert_eq!(notice.path.as_deref(), Some(path.as_path()));
        }
        // A missing explicit file is also reported instead of stopping the Shell.
        let missing = canopa_config::Request::common(directory.path().join("absent.kdl"));
        let (config, _, _, notice) = super::startup_configuration(&missing).unwrap();
        assert_eq!(config, defaults);
        assert!(notice.is_some());

        std::fs::write(
            &path,
            "schema-version 1\nshell { bar \"main\" { edge \"top\"; }; }\n",
        )
        .unwrap();
        let (config, source, _, notice) =
            super::startup_configuration(&canopa_config::Request::common(&path)).unwrap();
        assert_eq!(config.bars[0].edge, crate::config::Edge::Top);
        assert!(!source.defaults);
        assert!(notice.is_none());
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
