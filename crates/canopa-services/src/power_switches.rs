//! The session's lid-close choice, through logind's own inhibitor.
//!
//! logind owns the lid switch and its system policy (`HandleLidSwitch` in
//! `logind.conf`). A session that wants another behaviour takes logind's
//! documented `handle-lid-switch` block inhibitor, which tells logind to leave
//! the switch to the session, and asks logind for the chosen action. When the
//! session ends, or the choice is [`LidAction::System`], the inhibitor is
//! released and logind's policy applies again. See
//! `docs/architecture/settings.md`.
//!
//! While docked (logind's `Docked`: an external display or a dock), closing
//! the lid does nothing, like logind's default `HandleLidSwitchDocked=ignore`.
//!
//! ```no_run
//! use canopa_services::power_switches::{LidAction, handle_lid};
//! # async fn session() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let (choice, policy) = tokio::sync::watch::channel(LidAction::Lock);
//! tokio::spawn(handle_lid(system, policy));
//! choice.send_replace(LidAction::System); // Release the switch to logind.
//! # Ok(()) }
//! ```
use crate::{
    ServiceState,
    logind::{self, SessionAction},
    upower,
};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::watch;

/// What closing the lid does in this session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LidAction {
    /// Leave the lid to logind's system policy (normally suspend).
    #[default]
    System,
    Suspend,
    Hibernate,
    PowerOff,
    /// Lock the screen and keep running.
    Lock,
    Nothing,
}

impl LidAction {
    pub const ALL: [Self; 6] = [
        Self::System,
        Self::Suspend,
        Self::Hibernate,
        Self::Lock,
        Self::Nothing,
        Self::PowerOff,
    ];

    /// A stable identifier for configuration files.
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::PowerOff => "power-off",
            Self::Lock => "lock",
            Self::Nothing => "nothing",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    fn session_action(self) -> Option<SessionAction> {
        match self {
            Self::Suspend => Some(SessionAction::Suspend),
            Self::Hibernate => Some(SessionAction::Hibernate),
            Self::PowerOff => Some(SessionAction::PowerOff),
            _ => None,
        }
    }
}

/// Without UPower, logind's `LidClosed` is read this often (it has no change signal).
const POLL: Duration = Duration::from_secs(1);
/// Wait this long before taking the inhibitor again after logind refused or left.
const RETRY: Duration = Duration::from_secs(5);

/// The lid state: UPower's `LidIsClosed` changes when UPower runs, otherwise
/// logind's `LidClosed` polled. `None` while neither answers.
fn lid_states(system: zbus::Connection) -> futures_util::stream::BoxStream<'static, Option<bool>> {
    let upower = upower::watch_power_supply(system.clone());
    let mut ticks = tokio::time::interval(POLL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    futures_util::stream::unfold(
        (upower, ticks, system, false),
        |(mut upower, mut ticks, system, mut upower_available)| async move {
            loop {
                tokio::select! {
                    state = upower.next() => match state? {
                        ServiceState::Available(supply) if supply.lid_present => {
                            upower_available = true;
                            return Some((Some(supply.lid_closed), (upower, ticks, system, upower_available)));
                        }
                        _ => upower_available = false,
                    },
                    _ = ticks.tick(), if !upower_available => {
                        let closed = logind::manager_flag(&system, "LidClosed").await.ok();
                        return Some((closed, (upower, ticks, system, upower_available)));
                    }
                }
            }
        },
    )
    .boxed()
}

/// Carry out `action` for a closed lid, unless the computer is docked.
async fn closed(system: &zbus::Connection, action: LidAction) {
    if logind::manager_flag(system, "Docked")
        .await
        .unwrap_or(false)
    {
        return;
    }
    if let Some(request) = action.session_action() {
        let _ = logind::request(system, request).await;
    } else if action == LidAction::Lock
        && let Ok(session) = logind::Session::find(system).await
    {
        let _ = session.lock().await;
    }
}

/// Follow `policy` for as long as it has a sender. While the choice is not
/// [`LidAction::System`], hold the `handle-lid-switch` inhibitor and act each
/// time the lid closes. A lid that is already closed is not acted on until it
/// opens and closes again. When logind restarts, the inhibitor is taken again.
pub async fn handle_lid(system: zbus::Connection, mut policy: watch::Receiver<LidAction>) {
    loop {
        let action = *policy.borrow_and_update();
        if action == LidAction::System {
            if policy.changed().await.is_err() {
                return;
            }
            continue;
        }
        let inhibitor = logind::inhibit(
            &system,
            "handle-lid-switch",
            "Canopa",
            &fl!("services-lid-inhibitor-reason"),
            "block",
        )
        .await;
        let Ok(_inhibitor) = inhibitor else {
            tokio::select! {
                changed = policy.changed() => if changed.is_err() { return },
                _ = tokio::time::sleep(RETRY) => {}
            }
            continue;
        };
        let owners = async {
            let dbus = zbus::fdo::DBusProxy::new(&system).await.ok()?;
            dbus.receive_name_owner_changed_with_args(&[(0, logind::SERVICE)])
                .await
                .ok()
        }
        .await;
        let Some(mut owners) = owners else {
            tokio::time::sleep(RETRY).await;
            continue;
        };
        let mut lid = lid_states(system.clone());
        let mut last = None;
        loop {
            tokio::select! {
                changed = policy.changed() => {
                    if changed.is_err() { return }
                    break;
                }
                // A new logind does not know the old inhibitor.
                _ = owners.next() => break,
                state = lid.next() => {
                    let Some(state) = state else { break };
                    if let Some(now) = state {
                        if last == Some(false) && now {
                            closed(&system, action).await;
                        }
                        last = Some(now);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, logind::FakeLogind, upower::FakeUPower};
    use std::sync::atomic::Ordering;

    async fn until(mut condition: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("condition within 5 s");
    }

    #[test]
    fn ids_round_trip() {
        for action in LidAction::ALL {
            assert_eq!(LidAction::from_id(action.id()), Some(action));
        }
        assert_eq!(LidAction::from_id("other"), None);
    }

    #[tokio::test]
    async fn the_lid_follows_the_choice_and_the_system_choice_releases_it() {
        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _logind = logind.serve(&bus).await;
        let upower = FakeUPower::serve(&bus).await;
        let (choice, policy) = watch::channel(LidAction::System);
        let handler = tokio::spawn(handle_lid(bus.connect().await, policy));
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(logind.held().is_empty(), "the system choice holds nothing");

        choice.send_replace(LidAction::Suspend);
        until(|| logind.held() == ["handle-lid-switch"]).await;
        // Give the handler its first lid reading (open).
        tokio::time::sleep(Duration::from_millis(200)).await;
        upower.set_lid(true).await;
        until(|| logind.requested() == ["Suspend"]).await;

        // Docked: closing again does nothing.
        upower.set_lid(false).await;
        logind.docked.store(true, Ordering::SeqCst);
        upower.set_lid(true).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(logind.requested(), ["Suspend"]);
        logind.docked.store(false, Ordering::SeqCst);

        choice.send_replace(LidAction::Lock);
        upower.set_lid(false).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        upower.set_lid(true).await;
        until(|| logind.locks.load(Ordering::SeqCst) == 1).await;

        choice.send_replace(LidAction::System);
        until(|| logind.held().is_empty()).await;
        drop(choice);
        tokio::time::timeout(Duration::from_secs(3), handler)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn without_upower_logind_is_polled() {
        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _logind = logind.serve(&bus).await;
        let (_choice, policy) = watch::channel(LidAction::Hibernate);
        tokio::spawn(handle_lid(bus.connect().await, policy));
        until(|| logind.held() == ["handle-lid-switch"]).await;
        tokio::time::sleep(POLL * 2).await;
        logind.lid_closed.store(true, Ordering::SeqCst);
        until(|| logind.requested() == ["Hibernate"]).await;
    }
}
