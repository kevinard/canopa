use super::*;
use std::sync::{Arc, Mutex};

/// A fake logind manager. Each action either succeeds, is denied by polkit, or
/// fails while an application holds a block inhibitor.
#[derive(Clone)]
pub(crate) struct FakeLogind {
    pub(crate) answers: [&'static str; 4],
    pub(crate) calls: Arc<Mutex<Vec<(String, bool)>>>,
    pub(crate) deny: Option<&'static str>,
    pub(crate) inhibitors: Vec<(String, String, String, String, u32, u32)>,
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
    fn list_inhibitors(&self) -> Vec<(String, String, String, String, u32, u32)> {
        self.inhibitors.clone()
    }
}

pub(crate) async fn serve(bus: &crate::test_bus::TestBus, logind: FakeLogind) -> zbus::Connection {
    let server = bus.connect().await;
    server
        .object_server()
        .at(MANAGER_PATH, logind)
        .await
        .unwrap();
    server.request_name(SERVICE).await.unwrap();
    server
}

fn inhibitor(
    what: &str,
    who: &str,
    why: &str,
    mode: &str,
) -> (String, String, String, String, u32, u32) {
    (what.into(), who.into(), why.into(), mode.into(), 1000, 42)
}

#[tokio::test]
async fn capabilities_follow_logind_answers_and_hibernate_needs_yes() {
    let bus = crate::test_bus::TestBus::new();
    let client = bus.connect().await;
    // Without logind every answer is unknown: hibernate hides, the rest stay.
    let unknown = capabilities(&client).await;
    assert_eq!(unknown, Capabilities::default());
    assert!(unknown.offers(SessionAction::Suspend));
    assert!(!unknown.offers(SessionAction::Hibernate));

    let _server = serve(
        &bus,
        FakeLogind {
            answers: ["yes", "challenge", "no", "na"],
            calls: Default::default(),
            deny: None,
            inhibitors: Vec::new(),
        },
    )
    .await;
    let answered = capabilities(&client).await;
    assert_eq!(
        answered,
        Capabilities {
            suspend: Availability::Yes,
            hibernate: Availability::Challenge,
            reboot: Availability::No,
            power_off: Availability::NotApplicable,
        }
    );
    assert!(answered.offers(SessionAction::Suspend));
    assert!(!answered.offers(SessionAction::Hibernate));
    assert!(!answered.offers(SessionAction::Reboot));
    assert!(!answered.offers(SessionAction::PowerOff));
    let challenge = Capabilities {
        reboot: Availability::Challenge,
        hibernate: Availability::Yes,
        ..answered
    };
    assert!(challenge.offers(SessionAction::Reboot));
    assert!(challenge.offers(SessionAction::Hibernate));
}

#[tokio::test]
async fn requests_are_interactive_and_failures_are_explained() {
    let bus = crate::test_bus::TestBus::new();
    let client = bus.connect().await;
    assert_eq!(
        request(&client, SessionAction::Suspend).await,
        Err(Failure::Unavailable)
    );

    let calls = Arc::new(Mutex::new(Vec::new()));
    let logind = FakeLogind {
        answers: ["yes"; 4],
        calls: calls.clone(),
        deny: None,
        inhibitors: vec![
            inhibitor("shutdown:sleep", "Firefox", "Downloading files", "block"),
            inhibitor("sleep", "Canopa Lock", "Lock before sleep", "delay"),
            inhibitor("idle", "mpv", "Playing", "block"),
        ],
    };
    let server = serve(&bus, logind.clone()).await;
    for action in SessionAction::ALL {
        assert_eq!(request(&client, action).await, Ok(()));
    }
    assert_eq!(
        *calls.lock().unwrap(),
        [
            ("Suspend".to_string(), true),
            ("Hibernate".to_string(), true),
            ("Reboot".to_string(), true),
            ("PowerOff".to_string(), true),
        ]
    );

    // Only block inhibitors of the action's lock type are named.
    assert_eq!(
        blockers(&client, SessionAction::Reboot).await.unwrap(),
        [Inhibitor {
            who: "Firefox".into(),
            why: "Downloading files".into()
        }]
    );
    assert_eq!(
        blockers(&client, SessionAction::Suspend)
            .await
            .unwrap()
            .len(),
        1
    );

    server.close().await.unwrap();
    let _server = serve(
        &bus,
        FakeLogind {
            deny: Some("access"),
            ..logind.clone()
        },
    )
    .await;
    let failure = request(&client, SessionAction::PowerOff).await.unwrap_err();
    assert_eq!(
        failure,
        Failure::Blocked(vec![Inhibitor {
            who: "Firefox".into(),
            why: "Downloading files".into()
        }])
    );
    let notice = failure.notice(SessionAction::PowerOff);
    assert_eq!(notice.summary, "Couldn't power off");
    assert!(
        notice.body.contains("Firefox: Downloading files"),
        "{}",
        notice.body
    );
    assert!(!notice.persistent);
}

#[tokio::test]
async fn denied_authorization_and_other_refusals_are_distinguished() {
    let bus = crate::test_bus::TestBus::new();
    let client = bus.connect().await;
    let base = FakeLogind {
        answers: ["yes"; 4],
        calls: Default::default(),
        deny: Some("auth"),
        inhibitors: Vec::new(),
    };
    let server = serve(&bus, base.clone()).await;
    let failure = request(&client, SessionAction::Reboot).await.unwrap_err();
    assert_eq!(failure, Failure::NotAuthorized);
    assert!(
        failure
            .notice(SessionAction::Reboot)
            .body
            .contains("not allowed to restart")
    );
    server.close().await.unwrap();

    let _server = serve(
        &bus,
        FakeLogind {
            deny: Some("There's already a shutdown or sleep operation in progress"),
            ..base
        },
    )
    .await;
    let failure = request(&client, SessionAction::Suspend).await.unwrap_err();
    assert_eq!(
        failure,
        Failure::Refused("There's already a shutdown or sleep operation in progress".into())
    );
}

#[test]
fn config_names_round_trip_and_blocker_lists_are_bounded() {
    for action in SessionAction::ALL {
        assert_eq!(
            SessionAction::from_config_name(action.config_name()),
            Some(action)
        );
    }
    assert_eq!(SessionAction::from_config_name("poweroff"), None);
    let many: Vec<_> = (0..7)
        .map(|index| Inhibitor {
            who: format!("app{index}"),
            why: String::new(),
        })
        .collect();
    assert_eq!(
        blockers_sentence(&many),
        "app0; app1; app2; app3; app4; 2 more"
    );
}
