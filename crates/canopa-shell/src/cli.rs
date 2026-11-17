#[allow(clippy::enum_variant_names)]
pub enum Command {
    Run(ConfigOptions),
    CheckConfig(ConfigOptions),
    PrintVersion,
    /// `--help`/`-h`: print [`USAGE`] and exit 0.
    PrintHelp,
    ClipboardFrame(crate::launcher::clipboard::Kind),
    LauncherExtension(String, Option<String>),
    /// `--app-scope`: start an application in its own named systemd scope.
    AppScope(crate::command::app_scope::Request),
    HardwarePanel(String),
    Reload,
    /// `lock [--swaylock]`.
    Lock {
        swaylock: bool,
    },
    IpcCommand(String),
    OpenMenu(String),
    ToggleMenu(String),
    /// `--open-template NAME`: open a configured workspace template.
    OpenTemplate(String),
    Dmenu(DmenuOptions),
    /// `volume up`, `brightness down`, `media play-pause` and the other keys.
    DeviceKey(canopa_shell::device_keys::Request),
    UsageError(String),
}

#[derive(Debug, Clone)]
pub struct ConfigOptions {
    pub request: canopa_config::Request,
    pub show_source: bool,
}
impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            request: canopa_config::Request::Discover,
            show_source: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmenuOptions {
    pub prompt: Option<String>,
    pub delimiter: String,
    pub allow_custom: bool,
    pub entry_format: DmenuEntryFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmenuEntryFormat {
    Plain,
    JsonLines,
}

impl Default for DmenuOptions {
    fn default() -> Self {
        Self {
            prompt: None,
            delimiter: "\n".into(),
            allow_custom: true,
            entry_format: DmenuEntryFormat::Plain,
        }
    }
}

/// Printed by `--help`; keep it in step with the command table in `docs/shell.md`.
pub const USAGE: &str = "\
Usage: canopa-shell [OPTIONS]
       canopa-shell COMMAND

Run the Canopa Shell (one instance per session), or send a command to it.

Options for running the Shell:
  --canopa-config PATH   Use an explicit shared configuration file
  --config PATH          Use a legacy Shell configuration file
  --defaults             Use the embedded defaults
  --check-config         Validate the configuration, print one JSON line and exit
  --show-source          With --check-config, also print the error and its source

Commands:
  reload, --reload             Reload the running Shell's configuration
  --toggle-menu NAME           Open a named menu, or close it if open
  --open-menu NAME             Open a named menu
  --open-template NAME         Open a workspace template
  --toggle-panel               Toggle the bar's panel
  volume|microphone|brightness|keyboard-backlight|media ACTION [STEP]
                               Device keys
  --dmenu [-p PROMPT] [-d DELIM] [--json|--entry-format plain|jsonl] [--no-custom]
                               Choose from lines on stdin and print the choice
  lock                         Run Swaylock with the portal lock-screen image
  -V, --version                Print the version
  -h, --help                   Print this help

See docs/shell.md for details.
";

pub fn parse() -> Command {
    let mut args = std::env::args_os().skip(1).peekable();
    // Application arguments need not be UTF-8, so this mode parses them raw.
    if args
        .peek()
        .is_some_and(|word| word == crate::command::app_scope::FLAG)
    {
        args.next();
        return match crate::command::app_scope::parse(args) {
            Ok(request) => Command::AppScope(request),
            Err(message) => Command::UsageError(message),
        };
    }
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Command {
    let args = args.into_iter().collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|word| canopa_shell::device_keys::is_command(word))
    {
        return match canopa_shell::device_keys::parse(&args) {
            Ok(request) => Command::DeviceKey(request),
            Err(message) => Command::UsageError(message),
        };
    }
    let mut args = args.into_iter().peekable();
    let mut special_mode = None;
    let mut dmenu = false;
    let mut configuration = ConfigOptions::default();
    let mut check_config = false;
    let mut explicit_source = false;
    let mut dmenu_options = DmenuOptions::default();
    let mut custom_delimiter = false;
    let mut dmenu_option = false;
    let mut unknown_argument = None;
    let mut help = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => help = true,
            arg if matches!(
                arg.split('=').next(),
                Some("--open-menu" | "--toggle-menu" | "--open-template")
            ) =>
            {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                let (flag, inline_name) = arg
                    .split_once('=')
                    .map_or((arg, None), |(flag, name)| (flag, Some(name)));
                let what = if flag == "--open-template" {
                    "template"
                } else {
                    "menu"
                };
                let Some(name) = inline_name.map(str::to_owned).or_else(|| args.next()) else {
                    return Command::UsageError(format!("{flag} requires a {what} name"));
                };
                if name.is_empty() || (inline_name.is_none() && name.starts_with('-')) {
                    return Command::UsageError(format!("{flag} requires a {what} name"));
                }
                special_mode = Some(match flag {
                    "--toggle-menu" => Command::ToggleMenu(name),
                    "--open-template" => Command::OpenTemplate(name),
                    _ => Command::OpenMenu(name),
                });
            }
            "--version" | "-V" | "--toggle-panel" | "reload" | "--reload" | "lock" => {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                special_mode = Some(if arg == "lock" {
                    let swaylock = args.peek().is_some_and(|next| next == "--swaylock");
                    if swaylock {
                        args.next();
                    }
                    Command::Lock { swaylock }
                } else if matches!(arg.as_str(), "reload" | "--reload") {
                    Command::Reload
                } else if arg == "--toggle-panel" {
                    Command::IpcCommand("toggle-panel".into())
                } else {
                    Command::PrintVersion
                });
            }
            "--clipboard-frame" | "--clipboard-image-frame" => {
                let kind = if arg == "--clipboard-frame" {
                    crate::launcher::clipboard::Kind::Text
                } else {
                    crate::launcher::clipboard::Kind::Image
                };
                if special_mode
                    .replace(Command::ClipboardFrame(kind))
                    .is_some()
                {
                    return Command::UsageError("choose one command mode".into());
                }
            }
            "--launcher-extension" => {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                let Some(mode) = args.next() else {
                    return Command::UsageError("--launcher-extension requires a mode".into());
                };
                if !matches!(
                    mode.as_str(),
                    "web" | "web-search" | "date-time" | "date-time-search"
                ) {
                    return Command::UsageError("unknown launcher extension".into());
                }
                let template = if mode == "web-search" {
                    args.next()
                } else {
                    None
                };
                special_mode = Some(Command::LauncherExtension(mode, template));
            }
            "--hardware-panel" => {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                let Some(name) = args
                    .next()
                    .filter(|name| matches!(name.as_str(), "bluetooth" | "power"))
                else {
                    return Command::UsageError(
                        "--hardware-panel requires bluetooth or power".into(),
                    );
                };
                special_mode = Some(Command::HardwarePanel(name));
            }
            "--check-config" => check_config = true,
            "--show-source" => configuration.show_source = true,
            "--config" | "--canopa-config" | "--defaults" => {
                if explicit_source {
                    return Command::UsageError("choose one configuration source".into());
                }
                explicit_source = true;
                configuration.request = if arg == "--defaults" {
                    canopa_config::Request::Defaults
                } else {
                    let Some(path) = args.next() else {
                        return Command::UsageError(format!("{arg} requires a path"));
                    };
                    if arg == "--config" {
                        canopa_config::Request::legacy(path)
                    } else {
                        canopa_config::Request::common(path)
                    }
                };
            }
            "--dmenu" => dmenu = true,
            "-p" | "--prompt" => {
                dmenu_option = true;
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                dmenu_options.prompt = Some(value);
            }
            "-d" | "--delimiter" => {
                dmenu_option = true;
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                match crate::ipc::protocol::decode_delimiter(&value) {
                    Ok(delimiter) => {
                        dmenu_options.delimiter = delimiter;
                        custom_delimiter = true;
                    }
                    Err(err) => return Command::UsageError(err.to_string()),
                }
            }
            "--json" => {
                dmenu_option = true;
                dmenu_options.entry_format = DmenuEntryFormat::JsonLines;
            }
            "--entry-format" => {
                dmenu_option = true;
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                match value.as_str() {
                    "plain" => dmenu_options.entry_format = DmenuEntryFormat::Plain,
                    "jsonl" | "json-lines" => {
                        dmenu_options.entry_format = DmenuEntryFormat::JsonLines
                    }
                    _ => {
                        return Command::UsageError(format!(
                            "unknown dmenu entry format {value:?}"
                        ));
                    }
                }
            }
            "--no-custom" => {
                dmenu_option = true;
                dmenu_options.allow_custom = false;
            }
            _ => unknown_argument = Some(arg),
        }
    }
    if help {
        return Command::PrintHelp;
    }
    if let Some(command) = special_mode {
        if matches!(
            command,
            Command::OpenMenu(_)
                | Command::ToggleMenu(_)
                | Command::OpenTemplate(_)
                | Command::Reload
                | Command::Lock { .. }
                | Command::LauncherExtension(..)
                | Command::ClipboardFrame(_)
        ) {
            if dmenu_option {
                return Command::UsageError(
                    "IPC commands cannot be combined with dmenu options".into(),
                );
            }
            if let Some(arg) = unknown_argument {
                return Command::UsageError(format!("unexpected argument {arg:?}"));
            }
        }
        if dmenu || check_config || explicit_source || configuration.show_source {
            return Command::UsageError("choose one command mode".into());
        }
        return command;
    }
    if (configuration.show_source && !check_config) || (dmenu && (check_config || explicit_source))
    {
        return Command::UsageError("configuration checking options cannot be combined with other modes; --show-source requires --check-config".into());
    }
    // dmenu mode keeps ignoring words it does not know, as dmenu-style callers
    // pass flags of other launchers; starting the Shell never does.
    if let (false, Some(arg)) = (dmenu, unknown_argument) {
        return Command::UsageError(format!(
            "unexpected argument {arg:?}; run canopa-shell --help for usage"
        ));
    }
    if dmenu {
        if custom_delimiter && matches!(dmenu_options.entry_format, DmenuEntryFormat::JsonLines) {
            return Command::UsageError(
                "dmenu delimiter options cannot be combined with JSONL entry format".into(),
            );
        }
        Command::Dmenu(dmenu_options)
    } else {
        if check_config {
            Command::CheckConfig(configuration)
        } else {
            Command::Run(configuration)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_prints_usage_instead_of_starting_the_shell() {
        for args in [
            &["--help"][..],
            &["-h"],
            &["--defaults", "--help"],
            &["--dmenu", "-h"],
        ] {
            assert!(matches!(
                parse_args(args.iter().map(|word| word.to_string())),
                Command::PrintHelp
            ));
        }
        for flag in [
            "--check-config",
            "--toggle-menu",
            "--dmenu",
            "lock",
            "--version",
        ] {
            assert!(USAGE.contains(flag), "usage omits {flag}");
        }
    }

    #[test]
    fn unknown_arguments_never_start_the_shell() {
        for args in [
            &["--bogus"][..],
            &["--defaults", "stray"],
            &["--check-config", "--bogus"],
        ] {
            assert!(matches!(
                parse_args(args.iter().map(|word| word.to_string())),
                Command::UsageError(message) if message.contains("unexpected argument")
            ));
        }
        assert!(matches!(parse_args(Vec::<String>::new()), Command::Run(_)));
    }

    #[test]
    fn lock_is_a_standalone_command_without_configuration_or_dmenu_options() {
        assert!(matches!(
            parse_args(["lock".into()]),
            Command::Lock { swaylock: false }
        ));
        assert!(matches!(
            parse_args(["lock".into(), "--swaylock".into()]),
            Command::Lock { swaylock: true }
        ));
        for extra in [
            "--defaults",
            "--json",
            "--check-config",
            "--version",
            "unexpected",
        ] {
            assert!(matches!(
                parse_args(["lock".into(), extra.into()]),
                Command::UsageError(_)
            ));
        }
    }

    #[test]
    fn reload_is_a_standalone_command_for_the_running_selection() {
        for command in ["reload", "--reload"] {
            assert!(matches!(parse_args([command.to_string()]), Command::Reload));
            for extra in [
                "--defaults",
                "--check-config",
                "--toggle-panel",
                "--json",
                "unexpected",
            ] {
                assert!(matches!(
                    parse_args([command.to_string(), extra.to_string()]),
                    Command::UsageError(_)
                ));
            }
        }
    }

    #[test]
    fn check_mode_never_dispatches_ipc_with_conflicting_options() {
        for args in [
            ["--check-config", "--toggle-panel"],
            ["--toggle-panel", "--check-config"],
            ["--version", "--check-config"],
            ["--check-config", "--version"],
        ] {
            assert!(matches!(
                parse_args(args.map(str::to_string)),
                Command::UsageError(_)
            ));
        }
    }

    #[test]
    fn open_template_names_a_template() {
        assert!(matches!(
            parse_args(["--open-template", "dev"].map(str::to_string)),
            Command::OpenTemplate(name) if name == "dev"
        ));
        assert!(matches!(
            parse_args(["--open-template=dev"].map(str::to_string)),
            Command::OpenTemplate(name) if name == "dev"
        ));
        assert!(matches!(
            parse_args(["--open-template"].map(str::to_string)),
            Command::UsageError(message) if message.contains("template name")
        ));
    }

    #[test]
    fn device_keys_are_standalone_commands() {
        let parsed = parse_args(["volume", "up", "10"].map(str::to_string));
        assert!(matches!(
            parsed,
            Command::DeviceKey(canopa_shell::device_keys::Request {
                action: canopa_shell::device_keys::Action::VolumeUp,
                step: Some(10),
            })
        ));
        assert!(matches!(
            parse_args(["media", "next"].map(str::to_string)),
            Command::DeviceKey(_)
        ));
        for refused in [&["brightness"][..], &["volume", "up", "--defaults"]] {
            assert!(matches!(
                parse_args(refused.iter().map(|word| word.to_string())),
                Command::UsageError(_)
            ));
        }
        // Device words elsewhere are ordinary arguments.
        assert!(matches!(
            parse_args(["--dmenu", "volume"].map(str::to_string)),
            Command::Dmenu(_)
        ));
    }

    #[test]
    fn toggle_panel_emits_ipc_command() {
        match parse_args(["--toggle-panel".to_string()]) {
            Command::IpcCommand(command) => assert_eq!(command, "toggle-panel"),
            _ => panic!("expected IPC command"),
        }
    }

    #[test]
    fn dmenu_defaults_to_newline_delimiter_and_custom_values() {
        match parse_args(["--dmenu".to_string()]) {
            Command::Dmenu(options) => {
                assert_eq!(options.prompt, None);
                assert_eq!(options.delimiter, "\n");
                assert!(options.allow_custom);
            }
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_defaults_to_plain_entry_format() {
        match parse_args(["--dmenu".to_string()]) {
            Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::Plain),
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_accepts_json_entry_format_aliases() {
        match parse_args(["--dmenu".to_string(), "--json".to_string()]) {
            Command::Dmenu(options) => {
                assert_eq!(options.entry_format, DmenuEntryFormat::JsonLines)
            }
            _ => panic!("expected dmenu command"),
        }

        match parse_args([
            "--dmenu".to_string(),
            "--entry-format".to_string(),
            "jsonl".to_string(),
        ]) {
            Command::Dmenu(options) => {
                assert_eq!(options.entry_format, DmenuEntryFormat::JsonLines)
            }
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_accepts_plain_entry_format() {
        match parse_args([
            "--dmenu".to_string(),
            "--entry-format".to_string(),
            "plain".to_string(),
        ]) {
            Command::Dmenu(options) => assert_eq!(options.entry_format, DmenuEntryFormat::Plain),
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_rejects_json_entry_format_with_custom_delimiter() {
        match parse_args([
            "--dmenu".to_string(),
            "--json".to_string(),
            "--delimiter".to_string(),
            ",".to_string(),
        ]) {
            Command::UsageError(message) => assert!(message.contains("delimiter")),
            _ => panic!("expected usage error"),
        }
    }

    #[test]
    fn dmenu_accepts_prompt_aliases_and_no_custom() {
        match parse_args([
            "--dmenu".to_string(),
            "-p".to_string(),
            "Run".to_string(),
            "--no-custom".to_string(),
        ]) {
            Command::Dmenu(options) => {
                assert_eq!(options.prompt.as_deref(), Some("Run"));
                assert!(!options.allow_custom);
            }
            _ => panic!("expected dmenu command"),
        }

        match parse_args([
            "--dmenu".to_string(),
            "--prompt".to_string(),
            "Launch".to_string(),
        ]) {
            Command::Dmenu(options) => assert_eq!(options.prompt.as_deref(), Some("Launch")),
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_accepts_delimiter_aliases() {
        match parse_args([
            "--dmenu".to_string(),
            "--delimiter".to_string(),
            r"\0".to_string(),
        ]) {
            Command::Dmenu(options) => assert_eq!(options.delimiter, "\0"),
            _ => panic!("expected dmenu command"),
        }

        match parse_args(["--dmenu".to_string(), "-d".to_string(), ",".to_string()]) {
            Command::Dmenu(options) => assert_eq!(options.delimiter, ","),
            _ => panic!("expected dmenu command"),
        }
    }

    #[test]
    fn dmenu_reports_missing_option_value() {
        match parse_args(["--dmenu".to_string(), "--prompt".to_string()]) {
            Command::UsageError(message) => assert!(message.contains("--prompt")),
            _ => panic!("expected usage error"),
        }
    }
}
