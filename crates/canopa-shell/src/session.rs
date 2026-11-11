//! Suspend, hibernate, reboot and power off through systemd-logind.
//!
//! The logind client lives in [`canopa_services::logind`]. This module keeps
//! the Shell's configuration names and its wording of failures.
use crate::notice::Notice;

#[cfg(test)]
pub use canopa_services::logind::Availability;
pub use canopa_services::logind::{
    Capabilities, Failure, Inhibitor, SessionAction, blockers, capabilities, request,
};

const BLOCKER_LIMIT: usize = 5;

/// The configuration names of `session "ACTION"`, for parse errors.
pub(crate) const CONFIG_NAMES: &str = "`suspend`, `hibernate`, `reboot` or `power-off`";

/// The action a `session "ACTION"` configuration names.
pub(crate) fn from_config_name(name: &str) -> Option<SessionAction> {
    SessionAction::from_id(name)
}

/// The verb in "Couldn't …" and "You are not allowed to … now".
pub(crate) fn verb(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Suspend => "suspend",
        SessionAction::Hibernate => "hibernate",
        SessionAction::Reboot => "restart",
        SessionAction::PowerOff => "power off",
    }
}

/// The notice reporting a failed request.
pub(crate) fn notice(failure: &Failure, action: SessionAction) -> Notice {
    let body = match failure {
        Failure::Unavailable => {
            "The login manager (systemd-logind) is not available on the system bus.".into()
        }
        Failure::NotAuthorized => format!(
            "You are not allowed to {} now, or authentication was cancelled.",
            verb(action)
        ),
        Failure::Blocked(inhibitors) => format!(
            "Blocked by {}. Finish or close {}, then try again.",
            blockers_sentence(inhibitors),
            if inhibitors.len() == 1 {
                "that application"
            } else {
                "those applications"
            }
        ),
        Failure::Refused(message) => message.clone(),
        Failure::TimedOut => "The login manager did not answer in time.".into(),
    };
    Notice {
        summary: format!("Couldn't {}", verb(action)),
        body,
        persistent: false,
        actions: Vec::new(),
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
    canopa_services::bus::system().await.ok()
}

#[cfg(test)]
mod tests;
