#[allow(clippy::enum_variant_names)]
pub enum Command {
    Run(ConfigOptions),
    CheckConfig(ConfigOptions),
    PrintVersion,
    Reload,
    IpcCommand(String),
    OpenMenu(String),
    ToggleMenu(String),
    Dmenu(DmenuOptions),
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

pub fn parse() -> Command {
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Command {
    let mut args = args.into_iter();
    let mut special_mode = None;
    let mut dmenu = false;
    let mut configuration = ConfigOptions::default();
    let mut check_config = false;
    let mut explicit_source = false;
    let mut dmenu_options = DmenuOptions::default();
    let mut custom_delimiter = false;
    let mut dmenu_option = false;
    let mut unknown_argument = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            arg if matches!(arg.split('=').next(), Some("--open-menu" | "--toggle-menu")) => {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                let (flag, inline_name) = arg
                    .split_once('=')
                    .map_or((arg, None), |(flag, name)| (flag, Some(name)));
                let Some(name) = inline_name.map(str::to_owned).or_else(|| args.next()) else {
                    return Command::UsageError(format!("{flag} requires a menu name"));
                };
                if name.is_empty() || (inline_name.is_none() && name.starts_with('-')) {
                    return Command::UsageError(format!("{flag} requires a menu name"));
                }
                special_mode = Some(if flag == "--toggle-menu" {
                    Command::ToggleMenu(name)
                } else {
                    Command::OpenMenu(name)
                });
            }
            "--version" | "-V" | "--toggle-panel" | "reload" | "--reload" => {
                if special_mode.is_some() {
                    return Command::UsageError("choose one command mode".into());
                }
                special_mode = Some(if matches!(arg.as_str(), "reload" | "--reload") {
                    Command::Reload
                } else if arg == "--toggle-panel" {
                    Command::IpcCommand("toggle-panel".into())
                } else {
                    Command::PrintVersion
                });
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
    if let Some(command) = special_mode {
        if matches!(
            command,
            Command::OpenMenu(_) | Command::ToggleMenu(_) | Command::Reload
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
