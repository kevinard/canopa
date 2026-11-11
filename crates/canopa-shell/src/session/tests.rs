use super::*;
use canopa_services::testing::{
    TestBus,
    logind::{FakeLogind, inhibitor},
};

#[tokio::test]
async fn blocked_and_denied_requests_are_explained() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    let logind = FakeLogind {
        deny: Some("access"),
        inhibitors: vec![inhibitor(
            "shutdown:sleep",
            "Firefox",
            "Downloading files",
            "block",
        )],
        ..FakeLogind::default()
    };
    let server = logind.serve(&bus).await;
    let failure = request(&client, SessionAction::PowerOff).await.unwrap_err();
    let shown = notice(&failure, SessionAction::PowerOff);
    assert_eq!(shown.summary, "Couldn't power off");
    assert!(
        shown.body.contains("Firefox: Downloading files"),
        "{}",
        shown.body
    );
    assert!(!shown.persistent);
    server.close().await.unwrap();

    let _server = FakeLogind {
        deny: Some("auth"),
        inhibitors: Vec::new(),
        ..logind
    }
    .serve(&bus)
    .await;
    let failure = request(&client, SessionAction::Reboot).await.unwrap_err();
    assert!(
        notice(&failure, SessionAction::Reboot)
            .body
            .contains("not allowed to restart")
    );
}

#[test]
fn failure_notices_are_translated() {
    let english = crate::i18n_testing::english();
    let english_notice = notice(&Failure::TimedOut, SessionAction::Reboot);
    assert_eq!(english_notice.summary, "Couldn't restart");
    assert_eq!(
        english_notice.body,
        "The login manager did not answer in time."
    );
    drop(english);
    let _french = crate::i18n_testing::select("fr", false);
    let blocked = Failure::Blocked(vec![Inhibitor {
        who: "Firefox".into(),
        why: "Téléchargement".into(),
    }]);
    let french_notice = notice(&blocked, SessionAction::PowerOff);
    assert_eq!(french_notice.summary, "Impossible d’éteindre");
    assert_eq!(
        french_notice.body,
        "Bloqué par Firefox: Téléchargement. Terminez ou fermez cette application, puis réessayez."
    );
}

#[test]
fn config_names_round_trip_and_blocker_lists_are_bounded() {
    let _english = crate::i18n_testing::english();
    for action in SessionAction::ALL {
        assert_eq!(from_config_name(action.id()), Some(action));
    }
    assert_eq!(from_config_name("poweroff"), None);
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
