#[allow(clippy::enum_variant_names)]
pub enum Command {
    Run,
    PrintVersion,
    IpcCommand(String),
}

pub fn parse() -> Command {
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Command {
    for arg in args {
        match arg.as_str() {
            "--version" | "-V" => return Command::PrintVersion,
            "--toggle-panel" => return Command::IpcCommand("toggle-panel".into()),
            _ => {}
        }
    }
    Command::Run
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
}
