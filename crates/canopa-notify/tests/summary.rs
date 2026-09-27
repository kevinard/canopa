use canopa_notify::{Daemon, DaemonOptions, NotificationInput, NotifyControl};

#[tokio::test]
async fn summary_tracks_attention_independently_of_sender_lifetime_and_coalesces() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let mut subscription = NotifyControl::new(daemon.clone()).subscribe_notification_state();
    let initial = subscription.next_summary().await.unwrap().unwrap();
    assert_eq!(initial.requires_attention, 0);
    assert!(!initial.do_not_disturb);

    let retained = daemon
        .receive_notification(NotificationInput {
            app_name: "Summary fixture".into(),
            summary: "Retained".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_name: "Summary fixture".into(),
            summary: "Transient".into(),
            transient: true,
            ..Default::default()
        })
        .await
        .unwrap();
    daemon
        .expire_notification(retained.notification_id)
        .await
        .unwrap();
    daemon.change_do_not_disturb(Some(true)).await.unwrap();
    let latest = subscription.next_summary().await.unwrap().unwrap();
    assert_eq!(latest.instance, initial.instance);
    assert_eq!(
        latest.requires_attention, 1,
        "expiry preserves attention; transient is excluded"
    );
    assert!(
        latest.do_not_disturb,
        "slow readers receive the latest state"
    );
    daemon.shutdown().await.unwrap();
    drop(subscription);
    drop(daemon);

    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let mut subscription = NotifyControl::new(daemon.clone()).subscribe_notification_state();
    let restarted = subscription.next_summary().await.unwrap().unwrap();
    assert_ne!(restarted.instance, initial.instance);
    assert_eq!(restarted.requires_attention, 1);
    assert!(restarted.do_not_disturb);
    daemon.delete_notification_history().await.unwrap();
    assert_eq!(
        subscription
            .next_summary()
            .await
            .unwrap()
            .unwrap()
            .requires_attention,
        0
    );
    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Summary fixture".into(),
            summary: "Dismiss me".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        subscription
            .next_summary()
            .await
            .unwrap()
            .unwrap()
            .requires_attention,
        1
    );
    daemon
        .dismiss_notification(receipt.notification_id)
        .await
        .unwrap();
    assert_eq!(
        subscription
            .next_summary()
            .await
            .unwrap()
            .unwrap()
            .requires_attention,
        0
    );
    daemon.shutdown().await.unwrap();
}
