//! A fake logind Manager and session.
use super::TestBus;
use crate::logind::{MANAGER_PATH, SERVICE};
use std::sync::{Arc, Mutex};
use zbus::zvariant::OwnedObjectPath;

/// The path every session lookup answers.
pub const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";

/// One `ListInhibitors` entry: what, who, why, mode, uid, pid.
pub type InhibitorEntry = (String, String, String, String, u32, u32);

/// A fake logind. Each action either succeeds or fails as `deny` says:
/// `"access"` (denied, as for a block inhibitor), `"auth"` (interactive
/// authorization required) or another text (a plain failure with that message).
#[derive(Clone)]
pub struct FakeLogind {
    /// Answers to `CanSuspend`, `CanHibernate`, `CanReboot`, `CanPowerOff`.
    pub answers: [&'static str; 4],
    /// Requested methods with their `interactive` argument.
    pub calls: Arc<Mutex<Vec<(String, bool)>>>,
    pub deny: Option<&'static str>,
    pub inhibitors: Vec<InhibitorEntry>,
    /// `SetBrightness` calls on the session.
    pub brightness: Arc<Mutex<Vec<(String, String, u32)>>>,
    /// The session's `Type`; Canopa requires `wayland`.
    pub session_type: &'static str,
}

impl Default for FakeLogind {
    fn default() -> Self {
        Self {
            answers: ["yes"; 4],
            calls: Arc::default(),
            deny: None,
            inhibitors: Vec::new(),
            brightness: Arc::default(),
            session_type: "wayland",
        }
    }
}

/// An inhibitor entry for [`FakeLogind::inhibitors`].
pub fn inhibitor(what: &str, who: &str, why: &str, mode: &str) -> InhibitorEntry {
    (what.into(), who.into(), why.into(), mode.into(), 1000, 42)
}

impl FakeLogind {
    fn record(&self, method: &str, interactive: bool) -> zbus::fdo::Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push((method.into(), interactive));
        match self.deny {
            Some("access") => Err(zbus::fdo::Error::AccessDenied(
                "Operation denied due to active block inhibitor".into(),
            )),
            Some("auth") => Err(zbus::fdo::Error::InteractiveAuthorizationRequired(
                "Interactive authentication required.".into(),
            )),
            Some(other) => Err(zbus::fdo::Error::Failed(other.into())),
            None => Ok(()),
        }
    }

    /// Serve the Manager and the session and own `org.freedesktop.login1`.
    /// Close the returned connection to simulate logind leaving.
    pub async fn serve(&self, bus: &TestBus) -> zbus::Connection {
        let server = bus.connect().await;
        server
            .object_server()
            .at(MANAGER_PATH, self.clone())
            .await
            .unwrap();
        server
            .object_server()
            .at(SESSION_PATH, FakeSession(self.clone()))
            .await
            .unwrap();
        server.request_name(SERVICE).await.unwrap();
        server
    }
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl FakeLogind {
    fn can_suspend(&self) -> String {
        self.answers[0].into()
    }
    fn can_hibernate(&self) -> String {
        self.answers[1].into()
    }
    fn can_reboot(&self) -> String {
        self.answers[2].into()
    }
    fn can_power_off(&self) -> String {
        self.answers[3].into()
    }
    fn suspend(&self, interactive: bool) -> zbus::fdo::Result<()> {
        self.record("Suspend", interactive)
    }
    fn hibernate(&self, interactive: bool) -> zbus::fdo::Result<()> {
        self.record("Hibernate", interactive)
    }
    fn reboot(&self, interactive: bool) -> zbus::fdo::Result<()> {
        self.record("Reboot", interactive)
    }
    fn power_off(&self, interactive: bool) -> zbus::fdo::Result<()> {
        self.record("PowerOff", interactive)
    }
    fn list_inhibitors(&self) -> Vec<InhibitorEntry> {
        self.inhibitors.clone()
    }
    fn get_session(&self, _id: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(SESSION_PATH).unwrap()
    }
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, _pid: u32) -> OwnedObjectPath {
        OwnedObjectPath::try_from(SESSION_PATH).unwrap()
    }
}

struct FakeSession(FakeLogind);

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl FakeSession {
    #[zbus(property)]
    fn active(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn remote(&self) -> bool {
        false
    }
    #[zbus(property, name = "Type")]
    fn session_type(&self) -> &str {
        self.0.session_type
    }
    fn set_brightness(&self, subsystem: &str, name: &str, level: u32) {
        self.0
            .brightness
            .lock()
            .unwrap()
            .push((subsystem.into(), name.into(), level));
    }
}
