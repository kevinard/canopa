//! Suspend, hibernate, reboot and power off through systemd-logind.
//!
//! The requests follow `canopa-session`'s logind client: one system-bus proxy
//! without property caching, bounded calls, and errors reported to the caller.
//! Requests are interactive so polkit can ask for authentication (for example
//! when other users are logged in or an application blocks shutdown).
use std::time::Duration;

use crate::notice::Notice;

const SERVICE: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const DEADLINE: Duration = Duration::from_secs(2);
/// A polkit prompt waits for the person; leave time to type a password.
const AUTHORIZATION_DEADLINE: Duration = Duration::from_secs(300);
const BLOCKER_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionAction {
    Suspend,
    Hibernate,
    Reboot,
    PowerOff,
}

impl SessionAction {
    pub(crate) const ALL: [Self; 4] =
        [Self::Suspend, Self::Hibernate, Self::Reboot, Self::PowerOff];
    pub(crate) const CONFIG_NAMES: &'static str = "`suspend`, `hibernate`, `reboot` or `power-off`";

    pub(crate) fn from_config_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.config_name() == name)
    }

    pub(crate) fn config_name(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Reboot => "reboot",
            Self::PowerOff => "power-off",
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::Suspend => "Suspend",
            Self::Hibernate => "Hibernate",
            Self::Reboot => "Reboot",
            Self::PowerOff => "PowerOff",
        }
    }

    fn can_method(self) -> &'static str {
        match self {
            Self::Suspend => "CanSuspend",
            Self::Hibernate => "CanHibernate",
            Self::Reboot => "CanReboot",
            Self::PowerOff => "CanPowerOff",
        }
    }

    /// The logind inhibitor lock type that blocks this action.
    fn inhibited_by(self) -> &'static str {
        match self {
            Self::Suspend | Self::Hibernate => "sleep",
            Self::Reboot | Self::PowerOff => "shutdown",
        }
    }

    pub(crate) fn verb(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Reboot => "restart",
            Self::PowerOff => "power off",
        }
    }
}

/// A `Can*` answer from logind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Availability {
    /// Not asked yet, logind unreachable, or an answer this Shell does not know.
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
    fn parse(answer: &str) -> Self {
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
    pub(crate) suspend: Availability,
    pub(crate) hibernate: Availability,
    pub(crate) reboot: Availability,
    pub(crate) power_off: Availability,
}

impl Capabilities {
    pub(crate) fn get(&self, action: SessionAction) -> Availability {
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

    /// Whether the Shell shows the action. Hibernate needs a definite yes: it is
    /// commonly misconfigured and a failed resume loses the session. The others
    /// stay visible until logind says no, so an unreachable logind still reports
    /// its failure when the person tries.
    pub(crate) fn offers(&self, action: SessionAction) -> bool {
        match (action, self.get(action)) {
            (SessionAction::Hibernate, availability) => availability == Availability::Yes,
            (_, Availability::No | Availability::NotApplicable) => false,
            _ => true,
        }
    }
}

/// An application holding a blocking inhibitor lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inhibitor {
    pub(crate) who: String,
    pub(crate) why: String,
}

impl Inhibitor {
    pub(crate) fn describe(&self) -> String {
        if self.why.trim().is_empty() {
            self.who.clone()
        } else {
            format!("{}: {}", self.who, self.why)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The system bus or logind is not reachable.
    Unavailable,
    /// Polkit refused, or the person dismissed the authentication prompt.
    NotAuthorized,
    /// Applications hold block inhibitors.
    Blocked(Vec<Inhibitor>),
    /// logind refused for another reason; its message is kept.
    Refused(String),
    TimedOut,
}

impl Failure {
    pub(crate) fn notice(&self, action: SessionAction) -> Notice {
        let body = match self {
            Self::Unavailable => {
                "The login manager (systemd-logind) is not available on the system bus.".into()
            }
            Self::NotAuthorized => format!(
                "You are not allowed to {} now, or authentication was cancelled.",
                action.verb()
            ),
            Self::Blocked(inhibitors) => format!(
                "Blocked by {}. Finish or close {}, then try again.",
                blockers_sentence(inhibitors),
                if inhibitors.len() == 1 {
                    "that application"
                } else {
                    "those applications"
                }
            ),
            Self::Refused(message) => message.clone(),
            Self::TimedOut => "The login manager did not answer in time.".into(),
        };
        Notice {
            summary: format!("Couldn't {}", action.verb()),
            body,
            persistent: false,
            actions: Vec::new(),
        }
    }
}

/// "Firefox: Downloading files; Steam" with a bounded count.
pub(crate) fn blockers_sentence(inhibitors: &[Inhibitor]) -> String {
    let mut parts: Vec<_> = inhibitors
        .iter()
        .take(BLOCKER_LIMIT)
        .map(Inhibitor::describe)
        .collect();
    if inhibitors.len() > BLOCKER_LIMIT {
        parts.push(format!("{} more", inhibitors.len() - BLOCKER_LIMIT));
    }
    parts.join("; ")
}

pub(crate) async fn system() -> Option<zbus::Connection> {
    tokio::time::timeout(DEADLINE, zbus::Connection::system())
        .await
        .ok()
        .and_then(Result::ok)
}

async fn manager(connection: &zbus::Connection) -> zbus::Result<zbus::Proxy<'_>> {
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
pub(crate) async fn capabilities(connection: &zbus::Connection) -> Capabilities {
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

/// Applications holding a `block` inhibitor for this action's lock type.
pub(crate) async fn blockers(
    connection: &zbus::Connection,
    action: SessionAction,
) -> zbus::Result<Vec<Inhibitor>> {
    let manager = manager(connection).await?;
    let inhibitors: Vec<(String, String, String, String, u32, u32)> =
        tokio::time::timeout(DEADLINE, manager.call("ListInhibitors", &()))
            .await
            .map_err(|_| zbus::Error::Failure("logind did not answer".into()))??;
    Ok(inhibitors
        .into_iter()
        .filter(|(what, _, _, mode, _, _)| {
            mode == "block" && what.split(':').any(|lock| lock == action.inhibited_by())
        })
        .map(|(_, who, why, ..)| Inhibitor {
            who: crate::notice::sanitize(who.trim(), 60),
            why: crate::notice::sanitize(why.trim(), 120),
        })
        .collect())
}

/// Request the action. Success means logind accepted it, not that it finished.
pub(crate) async fn request(
    connection: &zbus::Connection,
    action: SessionAction,
) -> Result<(), Failure> {
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
    if matches!(
        name,
        "org.freedesktop.DBus.Error.AccessDenied"
            | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired"
            | "org.freedesktop.PolicyKit1.Error.NotAuthorized"
            | "org.freedesktop.PolicyKit1.Error.Cancelled"
    ) {
        return Err(Failure::NotAuthorized);
    }
    let message = description
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("The login manager refused the request.");
    Err(Failure::Refused(crate::notice::sanitize(message, 200)))
}

#[cfg(test)]
mod tests;
