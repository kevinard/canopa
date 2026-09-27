#![allow(dead_code)]
#[path = "../../../../src/command.rs"]
mod command;
#[path = "../../../../src/desktop_apps.rs"]
mod desktop_apps;

// Only satisfy command.rs's configuration interface. Parsing and execution
// below use the original production modules directly.
mod config {
    pub enum ConfiguredCommandSpec {
        Shell(String),
        Argv { program: String, args: Vec<String> },
    }
    pub struct ConfiguredCommand;
    impl ConfiguredCommand {
        pub fn timeout_ms(&self) -> u64 { 3000 }
        pub fn spec(&self) -> &ConfiguredCommandSpec { unreachable!() }
    }
}

fn main() {
    let parsed = command::desktop_exec_to_argv("example --title %c %i --desktop-file %k").unwrap();
    println!("desktop field expansion: {:?}", parsed);
    let entry = |id: &str, body: &str| desktop_apps::RawDesktopEntry {
        id: id.into(), body: body.into(),
    };
    let apps = desktop_apps::applications_from_entries(vec![
        entry("org.example.DBus.desktop", "[Desktop Entry]\nType=Application\nName=DBus App\nDBusActivatable=true\n"),
        entry("org.example.Locale.desktop", "[Desktop Entry]\nType=Application\nName=English name\nName[fr]=Nom français\nExec=true\n"),
    ], &desktop_apps::DesktopEnvironment::unknown());
    println!("discovered apps: {:?}", apps.iter().map(|app| (&app.id, &app.name)).collect::<Vec<_>>());
    let path = format!("/tmp/niribar-shell-probe-child-{}", std::process::id());
    let cmd = format!("echo $$ > {path}; exec sleep 30");
    command::spawn_command_with_timeout_fire_and_forget(
        command::CommandSpec::Shell(&cmd),
        std::time::Duration::from_millis(3000),
    ).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let pid = std::fs::read_to_string(&path).unwrap();
    let proc_path = format!("/proc/{}", pid.trim());
    println!("configured launcher process alive at 100ms: {}", std::path::Path::new(&proc_path).exists());
    std::thread::sleep(std::time::Duration::from_millis(3300));
    println!("configured launcher process alive at 3400ms: {}", std::path::Path::new(&proc_path).exists());
    std::fs::remove_file(path).unwrap();
}
