//! systemd-logind: session actions, their `Can*` answers, inhibitors, and the
//! session object (for example for `SetBrightness`).
//!
//! Requests use an uncached Manager proxy and bounded calls, as
//! `canopa-session` does. Actions are interactive, so polkit can ask for
//! authentication when other users are logged in or an inhibitor needs
//! overriding.
//!
//! ```no_run
//! use canopa_services::logind::{self, SessionAction};
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let capabilities = logind::capabilities(&system).await;
//! if capabilities.offers(SessionAction::Suspend) {
//!     // Show the button; on click:
//!     if let Err(failure) = logind::request(&system, SessionAction::Suspend).await {
//!         eprintln!("{failure}"); // Blocked(apps), NotAuthorized, Refused(message), ...
//!     }
//! }
//! let blockers = logind::blockers(&system, SessionAction::PowerOff).await?;
//! # let _ = blockers;
//! # Ok(()) }
//! ```
use crate::{Error, bus::StateStream};
use std::time::Duration;

pub const SERVICE: &str = "org.freedesktop.login1";
pub const MANAGER_PATH: &str = "/org/freedesktop/login1";
pub const MANAGER: &str = "org.freedesktop.login1.Manager";
pub const SESSION: &str = "org.freedesktop.login1.Session";
const DEADLINE: Duration = Duration::from_secs(2);
/// A polkit prompt waits for the person; leave time to type a password.
const AUTHORIZATION_DEADLINE: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionAction {
    Suspend,
    Hibernate,
    Reboot,
    PowerOff,
}

impl SessionAction {
    pub const ALL: [Self; 4] = [Self::Suspend, Self::Hibernate, Self::Reboot, Self::PowerOff];

    /// A stable identifier: `suspend`, `hibernate`, `reboot` or `power-off`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Reboot => "reboot",
            Self::PowerOff => "power-off",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    /// The Manager method that requests the action.
    pub fn method(self) -> &'static str {
        match self {
            Self::Suspend => "Suspend",
            Self::Hibernate => "Hibernate",
            Self::Reboot => "Reboot",
            Self::PowerOff => "PowerOff",
        }
    }

    /// The Manager method that answers whether the action is allowed.
    pub fn can_method(self) -> &'static str {
        match self {
            Self::Suspend => "CanSuspend",
            Self::Hibernate => "CanHibernate",
            Self::Reboot => "CanReboot",
            Self::PowerOff => "CanPowerOff",
        }
    }

    /// The logind inhibitor lock type that blocks this action.
    pub fn inhibited_by(self) -> &'static str {
        match self {
            Self::Suspend | Self::Hibernate => "sleep",
            Self::Reboot | Self::PowerOff => "shutdown",
        }
    }
}

/// A `Can*` answer from logind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Availability {
    /// Not asked yet, logind unreachable, or an answer this client does not know.
    #[default]
    Unknown,
    Yes,
    /// Allowed after polkit authentication.
    Challenge,
    No,
    /// The system does not support the action (for example hibernate without swap).
    NotApplicable,
}

impl Availability {
    pub fn parse(answer: &str) -> Self {
        match answer {
            "yes" => Self::Yes,
            "challenge" => Self::Challenge,
            "no" => Self::No,
            "na" => Self::NotApplicable,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub suspend: Availability,
    pub hibernate: Availability,
    pub reboot: Availability,
    pub power_off: Availability,
}

impl Capabilities {
    pub fn get(&self, action: SessionAction) -> Availability {
        match action {
            SessionAction::Suspend => self.suspend,
            SessionAction::Hibernate => self.hibernate,
            SessionAction::Reboot => self.reboot,
            SessionAction::PowerOff => self.power_off,
        }
    }

    fn set(&mut self, action: SessionAction, availability: Availability) {
        match action {
            SessionAction::Suspend => self.suspend = availability,
            SessionAction::Hibernate => self.hibernate = availability,
            SessionAction::Reboot => self.reboot = availability,
            SessionAction::PowerOff => self.power_off = availability,
        }
    }

    /// Whether Canopa offers the action. Hibernate needs a definite yes: it is
    /// commonly misconfigured and a failed resume loses the session. The others
    /// stay visible until logind says no, so an unreachable logind still reports
    /// its failure when the person tries.
    pub fn offers(&self, action: SessionAction) -> bool {
        match (action, self.get(action)) {
            (SessionAction::Hibernate, availability) => availability == Availability::Yes,
            (_, Availability::No | Availability::NotApplicable) => false,
            _ => true,
        }
    }
}

/// An application holding a blocking inhibitor lock. Text is bounded and has
/// no control characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inhibitor {
    pub who: String,
    pub why: String,
}

impl Inhibitor {
    /// "Firefox: Downloading files", or only the application without a reason.
    pub fn describe(&self) -> String {
        if self.why.trim().is_empty() {
            self.who.clone()
        } else {
            format!("{}: {}", self.who, self.why)
        }
    }
}

/// One `ListInhibitors` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InhibitorLock {
    /// Lock types, for example `["shutdown", "sleep"]`.
    pub what: Vec<String>,
    pub who: String,
    pub why: String,
    /// `block` or `delay`.
    pub mode: String,
    pub uid: u32,
    pub pid: u32,
}

/// Why a session action request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    /// The system bus or logind is not reachable.
    #[error("the login manager is not available")]
    Unavailable,
    /// Polkit refused, or the person dismissed the authentication prompt.
    #[error("the request is not authorized")]
    NotAuthorized,
    /// Applications hold block inhibitors.
    #[error("blocked by applications")]
    Blocked(Vec<Inhibitor>),
    /// logind refused for another reason; its bounded message is kept.
    #[error("{0}")]
    Refused(String),
    #[error("the login manager did not answer in time")]
    TimedOut,
}

pub(crate) async fn manager(connection: &zbus::Connection) -> zbus::Result<zbus::Proxy<'_>> {
    tokio::time::timeout(
        DEADLINE,
        zbus::proxy::Builder::new(connection)
            .destination(SERVICE)?
            .path(MANAGER_PATH)?
            .interface(MANAGER)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build(),
    )
    .await
    .map_err(|_| zbus::Error::Failure("logind proxy timed out".into()))?
}

/// Ask logind which actions it allows. Unanswered questions stay unknown.
pub async fn capabilities(connection: &zbus::Connection) -> Capabilities {
    let mut capabilities = Capabilities::default();
    let Ok(manager) = manager(connection).await else {
        return capabilities;
    };
    for action in SessionAction::ALL {
        let answer = tokio::time::timeout(
            DEADLINE,
            manager.call::<_, _, String>(action.can_method(), &()),
        )
        .await;
        if let Ok(Ok(answer)) = answer {
            capabilities.set(action, Availability::parse(&answer));
        }
    }
    capabilities
}

/// [`capabilities`] re-read whenever logind signals a change (sessions,
/// inhibitors, preparing for sleep), as a recovering stream.
pub fn watch_capabilities(connection: zbus::Connection) -> StateStream<Capabilities> {
    crate::bus::watch_service(connection, SERVICE, |connection| async move {
        Ok(capabilities(&connection).await)
    })
}

/// Every inhibitor lock logind knows, in any mode.
pub async fn list_inhibitors(connection: &zbus::Connection) -> Result<Vec<InhibitorLock>, Error> {
    let manager = manager(connection).await?;
    let inhibitors: Vec<(String, String, String, String, u32, u32)> =
        tokio::time::timeout(DEADLINE, manager.call("ListInhibitors", &())).await??;
    Ok(inhibitors
        .into_iter()
        .map(|(what, who, why, mode, uid, pid)| InhibitorLock {
            what: what.split(':').map(str::to_owned).collect(),
            who: crate::bus::bounded_text(who.trim(), 60),
            why: crate::bus::bounded_text(why.trim(), 120),
            mode,
            uid,
            pid,
        })
        .collect())
}

/// Applications holding a `block` inhibitor for this action's lock type.
pub async fn blockers(
    connection: &zbus::Connection,
    action: SessionAction,
) -> Result<Vec<Inhibitor>, Error> {
    Ok(list_inhibitors(connection)
        .await?
        .into_iter()
        .filter(|lock| {
            lock.mode == "block" && lock.what.iter().any(|what| what == action.inhibited_by())
        })
        .map(|lock| Inhibitor {
            who: lock.who,
            why: lock.why,
        })
        .collect())
}

/// Take an inhibitor lock. It lasts until the returned descriptor is closed.
/// `what` is a colon-separated list of lock types, `mode` is `block` or `delay`.
pub async fn inhibit(
    connection: &zbus::Connection,
    what: &str,
    who: &str,
    why: &str,
    mode: &str,
) -> Result<std::os::fd::OwnedFd, Error> {
    let manager = manager(connection).await?;
    let fd: zbus::zvariant::OwnedFd =
        tokio::time::timeout(DEADLINE, manager.call("Inhibit", &(what, who, why, mode))).await??;
    Ok(fd.into())
}

/// Request the action. Success means logind accepted it, not that it finished.
pub async fn request(connection: &zbus::Connection, action: SessionAction) -> Result<(), Failure> {
    let manager = manager(connection)
        .await
        .map_err(|_| Failure::Unavailable)?;
    let result = tokio::time::timeout(
        AUTHORIZATION_DEADLINE,
        manager.call_with_flags::<_, _, ()>(
            action.method(),
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            &(true,),
        ),
    )
    .await;
    let error = match result {
        Ok(Ok(_)) => return Ok(()),
        Ok(Err(error)) => error,
        Err(_) => return Err(Failure::TimedOut),
    };
    let zbus::Error::MethodError(name, description, _) = &error else {
        return Err(Failure::Unavailable);
    };
    let name = name.as_str();
    if matches!(
        name,
        "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.UnknownObject"
            | "org.freedesktop.DBus.Error.Disconnected"
    ) {
        return Err(Failure::Unavailable);
    }
    // logind reports an active block inhibitor as access denied or as its own
    // error, depending on the version. Name the blockers whenever they exist.
    let held = blockers(connection, action).await.unwrap_or_default();
    if !held.is_empty() {
        return Err(Failure::Blocked(held));
    }
    if Error::is_denied_name(name) {
        return Err(Failure::NotAuthorized);
    }
    let message = description
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("The login manager refused the request.");
    Err(Failure::Refused(crate::bus::bounded_text(message, 200)))
}

/// This process's login session.
pub struct Session {
    proxy: zbus::Proxy<'static>,
}

impl Session {
    /// Find this desktop's session: the one of `CANOPA_SESSION_PID` (the
    /// session launcher), else `XDG_SESSION_ID`, else logind's `auto` session.
    /// Only an active, local Wayland session is accepted.
    pub async fn find(connection: &zbus::Connection) -> Result<Self, Error> {
        let launcher = std::env::var("CANOPA_SESSION_PID")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|pid| *pid != 0);
        let session_id = std::env::var("XDG_SESSION_ID").ok();
        let path = if launcher.is_some() || session_id.is_some() {
            let manager = manager(connection).await?;
            if let Some(pid) = launcher {
                manager.call("GetSessionByPID", &(pid,)).await?
            } else {
                manager.call("GetSession", &(session_id.unwrap(),)).await?
            }
        } else {
            zbus::zvariant::OwnedObjectPath::try_from("/org/freedesktop/login1/session/auto")
                .map_err(zbus::Error::from)?
        };
        Self::at(connection, path).await
    }

    /// The session at `path`, if it is an active, local Wayland session.
    pub async fn at(
        connection: &zbus::Connection,
        path: zbus::zvariant::OwnedObjectPath,
    ) -> Result<Self, Error> {
        let proxy = zbus::Proxy::new_owned(
            connection.clone(),
            SERVICE.to_owned(),
            path,
            SESSION.to_owned(),
        )
        .await?;
        if !proxy.get_property::<bool>("Active").await?
            || proxy.get_property::<bool>("Remote").await?
        {
            return Err(Error::Invalid("An active local login is required."));
        }
        if proxy.get_property::<String>("Type").await? != "wayland" {
            return Err(Error::Invalid("A Wayland login is required."));
        }
        Ok(Self { proxy })
    }

    pub fn path(&self) -> &zbus::zvariant::ObjectPath<'_> {
        self.proxy.path()
    }

    /// Set a device's brightness through logind, which owns the permission:
    /// `subsystem` is `backlight` or `leds`, `name` the sysfs device name and
    /// `level` the raw value (see [`crate::backlight`]).
    pub async fn set_brightness(
        &self,
        subsystem: &str,
        name: &str,
        level: u32,
    ) -> Result<(), Error> {
        Ok(self
            .proxy
            .call::<_, _, ()>("SetBrightness", &(subsystem, name, level))
            .await?)
    }
}

#[cfg(test)]
mod tests;
