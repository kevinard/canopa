//! Private buses and fake services for tests, behind the `test-support`
//! feature. Each fixture owns its daemon; tests never touch the desktop's
//! buses or audio server. Fakes implement the subset of each interface that
//! the clients in this crate use.
//!
//! ```toml
//! [dev-dependencies]
//! canopa-services = { workspace = true, features = ["test-support"] }
//! ```
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

/// A private `dbus-daemon`, killed on drop. Fakes take a name on it with
/// [`zbus::Connection::request_name`]; clients connect with [`TestBus::connect`].
pub struct TestBus {
    child: Child,
    pub address: String,
}

impl TestBus {
    /// Start a private bus. Requires `dbus-daemon`.
    pub fn new() -> Self {
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

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Default for TestBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub mod logind;
pub mod power_profiles;
pub mod upower;
pub mod wifi;
pub mod bluez;
#[cfg(feature = "audio")]
pub mod audio;
pub mod mpris;
