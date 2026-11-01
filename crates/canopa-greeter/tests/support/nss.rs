use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn fixture(root: &Path) -> PathBuf {
    let library = root.join("nss-fixture.so");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/desktop-fixture/nss.c");
    assert!(
        Command::new("cc")
            .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-Werror"])
            .arg(source)
            .args(["-ldl", "-o"])
            .arg(&library)
            .status()
            .unwrap()
            .success()
    );
    library
}
pub fn configure(command: &mut assert_cmd::Command, root: &Path, library: &Path, mode: &str) {
    command
        .env("LD_PRELOAD", library)
        .env("CANOPA_NSS_FIXTURE", mode)
        .env("CANOPA_NSS_EVENTS", root.join("nss-events"))
        .env("GREETD_SOCK", "/must-not-be-inherited")
        .env("NOTIFY_SOCKET", "/must-not-be-inherited");
}
pub fn no_live_helpers(root: &Path) {
    let events = fs::read_to_string(root.join("nss-events")).unwrap();
    assert!(events.contains("begin"));
    for line in events.lines() {
        let pid = line.split_whitespace().next().unwrap();
        if let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
            // Grandchildren may await the test container's init reaper, but cannot run.
            assert_eq!(
                stat.rsplit_once(") ").unwrap().1.split_whitespace().next(),
                Some("Z"),
                "helper still running: {line}"
            );
            assert!(!line.ends_with("begin"), "owned helper was not reaped");
        }
    }
}
