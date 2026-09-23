//! Each transport test owns a private bus; never touch the user's desktop bus.
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

pub(crate) struct TestBus {
    child: Child,
    address: String,
}

impl TestBus {
    pub(crate) fn new() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("dbus-daemon installed");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(
            !address.trim().is_empty(),
            "private D-Bus daemon must start"
        );
        Self {
            child,
            address: address.trim().into(),
        }
    }
    pub(crate) async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}
impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
