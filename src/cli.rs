pub enum Command {
    Run,
    PrintVersion,
    IpcCommand(String),
}

pub fn parse() -> Command {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => return Command::PrintVersion,
            "--toggle-expand" => return Command::IpcCommand("toggle-expand".into()),
            _ => {}
        }
    }
    Command::Run
}
