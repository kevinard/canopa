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
    _config: tempfile::TempDir,
}

/// A session-like bus without service directories: nothing installed on the
/// computer (xdg-permission-store, for example) is started on it.
const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

impl TestBus {
    /// Start a private bus. Requires `dbus-daemon`.
    pub fn new() -> Self {
        let config = tempfile::tempdir().expect("a temporary folder");
        let file = config.path().join("bus.conf");
        std::fs::write(&file, CONFIG).expect("a bus configuration");
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", file.display()))
            .args(["--nofork", "--print-address=1"])
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
            _config: config,
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

pub mod accounts;
#[cfg(feature = "audio")]
pub mod audio;
pub mod bluez;
pub mod cups_pk_helper;
pub mod geoclue;
pub mod hostname;
pub mod iwd;
pub mod locale;
pub mod logind;
pub mod mpris;
pub mod network;
pub mod permission_store;
pub mod power_profiles;
pub mod timedate;
pub mod upower;
pub mod wifi;
