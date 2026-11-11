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

/// The Fluent selector value naming `action` in session messages.
fn action_id(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Suspend => "suspend",
        SessionAction::Hibernate => "hibernate",
        SessionAction::Reboot => "reboot",
        SessionAction::PowerOff => "power-off",
    }
}

/// The notice reporting a failed request.
pub(crate) fn notice(failure: &Failure, action: SessionAction) -> Notice {
    let action = action_id(action);
    let body = match failure {
        Failure::Unavailable => fl!("shell-session-logind-unavailable"),
        Failure::NotAuthorized => fl!("shell-session-not-authorized", action = action),
        Failure::Blocked(inhibitors) => fl!(
            "shell-session-blocked",
            blockers = blockers_sentence(inhibitors),
            count = inhibitors.len()
        ),
        // logind's own explanation.
        Failure::Refused(message) => message.clone(),
        Failure::TimedOut => fl!("shell-session-timed-out"),
    };
    Notice {
        summary: fl!("shell-session-failed", action = action),
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
        let more = (inhibitors.len() - BLOCKER_LIMIT) as u64;
        parts.push(fl!("shell-session-more-blockers", count = more));
    }
    parts.join("; ")
}

pub(crate) async fn system() -> Option<zbus::Connection> {
    canopa_services::bus::system().await.ok()
}

#[cfg(test)]
mod tests;
