use super::*;
use crate::{
    ServiceState,
    testing::{
        TestBus,
        logind::{FakeLogind, inhibitor},
    },
};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn capabilities_follow_logind_answers_and_hibernate_needs_yes() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    // Without logind every answer is unknown: hibernate hides, the rest stay.
    let unknown = capabilities(&client).await;
    assert_eq!(unknown, Capabilities::default());
    assert!(unknown.offers(SessionAction::Suspend));
    assert!(!unknown.offers(SessionAction::Hibernate));

    let _server = FakeLogind {
        answers: ["yes", "challenge", "no", "na"],
        ..FakeLogind::default()
    }
    .serve(&bus)
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
    let bus = TestBus::new();
    let client = bus.connect().await;
    assert_eq!(
        request(&client, SessionAction::Suspend).await,
        Err(Failure::Unavailable)
    );

    let calls = Arc::new(Mutex::new(Vec::new()));
    let logind = FakeLogind {
        calls: calls.clone(),
        inhibitors: vec![
            inhibitor("shutdown:sleep", "Firefox", "Downloading files", "block"),
            inhibitor("sleep", "Canopa Lock", "Lock before sleep", "delay"),
            inhibitor("idle", "mpv", "Playing", "block"),
        ],
        ..FakeLogind::default()
    };
    let server = logind.serve(&bus).await;
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
    let all = list_inhibitors(&client).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].what, ["shutdown", "sleep"]);
    assert_eq!((all[1].mode.as_str(), all[1].pid), ("delay", 42));

    server.close().await.unwrap();
    let _server = FakeLogind {
        deny: Some("access"),
        ..logind.clone()
    }
    .serve(&bus)
    .await;
    let failure = request(&client, SessionAction::PowerOff).await.unwrap_err();
    assert_eq!(
        failure,
        Failure::Blocked(vec![Inhibitor {
            who: "Firefox".into(),
            why: "Downloading files".into()
        }])
    );
}

#[tokio::test]
async fn denied_authorization_and_other_refusals_are_distinguished() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    let base = FakeLogind {
        deny: Some("auth"),
        ..FakeLogind::default()
    };
    let server = base.serve(&bus).await;
    let failure = request(&client, SessionAction::Reboot).await.unwrap_err();
    assert_eq!(failure, Failure::NotAuthorized);
    server.close().await.unwrap();

    let _server = FakeLogind {
        deny: Some("There's already a shutdown or sleep operation in progress"),
        ..base
    }
    .serve(&bus)
    .await;
    let failure = request(&client, SessionAction::Suspend).await.unwrap_err();
    assert_eq!(
        failure,
        Failure::Refused("There's already a shutdown or sleep operation in progress".into())
    );
}

#[tokio::test]
async fn session_lookup_requires_a_local_wayland_login_and_capabilities_are_watched() {
    use futures_util::StreamExt;
    let bus = TestBus::new();
    let client = bus.connect().await;
    assert!(Session::find(&client).await.is_err());
    let mut states = watch_capabilities(client.clone());
    let next = async |states: &mut crate::bus::StateStream<Capabilities>| {
        tokio::time::timeout(std::time::Duration::from_secs(3), states.next())
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(next(&mut states).await, ServiceState::Unavailable);
    let server = FakeLogind {
        session_type: "x11",
        ..FakeLogind::default()
    }
    .serve(&bus)
    .await;
    assert!(matches!(
        next(&mut states).await,
        ServiceState::Available(Capabilities {
            suspend: Availability::Yes,
            ..
        })
    ));
    assert_eq!(
        Session::find(&client).await.err(),
        Some(Error::Invalid("A Wayland login is required."))
    );
    server.close().await.unwrap();
    assert_eq!(next(&mut states).await, ServiceState::Unavailable);
    let _server = FakeLogind::default().serve(&bus).await;
    let session = Session::find(&client).await.unwrap();
    assert_eq!(
        session.path().as_str(),
        crate::testing::logind::SESSION_PATH
    );
}

#[test]
fn identifiers_round_trip() {
    for action in SessionAction::ALL {
        assert_eq!(SessionAction::from_id(action.id()), Some(action));
    }
    assert_eq!(SessionAction::from_id("poweroff"), None);
}
