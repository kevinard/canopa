#[allow(clippy::enum_variant_names)]
pub enum Command {
    Run,
    PrintVersion,
    IpcCommand(String),
    Dmenu(DmenuOptions),
    UsageError(String),
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
    let mut dmenu = false;
    let mut dmenu_options = DmenuOptions::default();
    let mut custom_delimiter = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => return Command::PrintVersion,
            "--toggle-panel" => return Command::IpcCommand("toggle-panel".into()),
            "--dmenu" => dmenu = true,
            "-p" | "--prompt" => {
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                dmenu_options.prompt = Some(value);
            }
            "-d" | "--delimiter" => {
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
            "--json" => dmenu_options.entry_format = DmenuEntryFormat::JsonLines,
            "--entry-format" => {
                let Some(value) = args.next() else {
                    return Command::UsageError(format!("{arg} requires a value"));
                };
                match value.as_str() {
                    "plain" => dmenu_options.entry_format = DmenuEntryFormat::Plain,
                    "jsonl" | "json-lines" => {
                        dmenu_options.entry_format = DmenuEntryFormat::JsonLines
                    }
                    _ => return Command::UsageError(format!("unknown dmenu entry format {value:?}")),
                }
            }
            "--no-custom" => dmenu_options.allow_custom = false,
            _ => {}
        }
    }
    if dmenu {
        if custom_delimiter && matches!(dmenu_options.entry_format, DmenuEntryFormat::JsonLines) {
            return Command::UsageError(
                "dmenu delimiter options cannot be combined with JSONL entry format".into(),
            );
        }
        Command::Dmenu(dmenu_options)
    } else {
        Command::Run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
