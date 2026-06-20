use canopa_notify::{Daemon, DaemonOptions, DeliveryOutcome, NotificationInput, Urgency, UserRule};

#[tokio::test]
async fn do_not_disturb_silences_non_critical_notifications_but_allows_critical_banners() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let before_dnd = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Visible before DND".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    assert!(is_banner_visible(&daemon, before_dnd.notification_id).await);

    daemon.set_do_not_disturb(true).await.unwrap();
    assert!(daemon.do_not_disturb().await.unwrap());

    let normal = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Quiet please".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let critical = daemon
        .receive_notification(NotificationInput {
            app_name: "Battery".to_string(),
            summary: "Critical battery".to_string(),
            urgency: Urgency::Critical,
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    assert!(!is_banner_visible(&daemon, before_dnd.notification_id).await);
    assert_eq!(
        thread(&snapshot, normal.notification_id)
            .delivery_outcome
            .as_str(),
        "silenced"
    );
    assert!(!is_banner_visible(&daemon, normal.notification_id).await);
    assert_eq!(
        thread(&snapshot, critical.notification_id)
            .delivery_outcome
            .as_str(),
        "shown"
    );
    assert!(is_banner_visible(&daemon, critical.notification_id).await);

    drop(daemon);
    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    assert!(restarted.do_not_disturb().await.unwrap());
}

fn thread(
    snapshot: &canopa_notify::NotificationSnapshot,
    id: i64,
) -> &canopa_notify::NotificationThread {
    snapshot
        .history
        .iter()
        .find(|thread| thread.id == id)
        .unwrap()
}

async fn is_banner_visible(daemon: &Daemon, id: i64) -> bool {
    daemon
        .visible_banners()
        .await
        .iter()
        .any(|banner| banner.notification_id == id)
}

#[tokio::test]
async fn user_rules_take_precedence_over_config_rules_and_are_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let config = temp.path().join("notify.kdl");
    std::fs::write(&config, r#"rule "drop-chat" app="Chat" outcome="dropped""#).unwrap();

    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();

    let dropped = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Config drops this".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    assert_eq!(dropped.delivery_outcome.as_str(), "dropped");
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 0);

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Shown))
        .await
        .unwrap();
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    restarted.reload_configuration_file(&config).await.unwrap();
    let shown = restarted
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "User allows this".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(shown.delivery_outcome.as_str(), "shown");
    assert_eq!(restarted.user_rules().await.unwrap().len(), 1);
    assert_eq!(restarted.snapshot().await.unwrap().history.len(), 1);
}

#[tokio::test]
async fn silenced_and_dropped_user_rules_affect_active_notifications_and_replacements() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let original = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Original".to_string(),
            body: "Visible".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Silenced))
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id)
            .delivery_outcome
            .as_str(),
        "silenced"
    );
    assert!(!is_banner_visible(&daemon, original.notification_id).await);

    daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            replaces_id: Some(original.notification_id),
            summary: "Original".to_string(),
            body: "Silenced update".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id).body,
        "Silenced update"
    );

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Dropped))
        .await
        .unwrap();
    let dropped_replacement = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            replaces_id: Some(original.notification_id),
            summary: "Original".to_string(),
            body: "Should not overwrite".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(dropped_replacement.delivery_outcome.as_str(), "dropped");
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id).body,
        "Silenced update"
    );
}

#[tokio::test]
async fn delivery_decision_logs_redact_notification_content() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Silenced))
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Secret subject".to_string(),
            body: "Secret body".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let encoded_logs = serde_json::to_string(&daemon.delivery_decision_logs().await).unwrap();
    assert!(encoded_logs.contains("Chat"));
    assert!(encoded_logs.contains("silenced"));
    assert!(!encoded_logs.contains("Secret subject"));
    assert!(!encoded_logs.contains("Secret body"));
}

#[tokio::test]
async fn invalid_configuration_reload_keeps_previous_valid_rules() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let config = temp.path().join("notify.kdl");

    std::fs::write(
        &config,
        r#"rule "silence-chat" app="Chat" outcome="silenced""#,
    )
    .unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();

    std::fs::write(&config, "not-a-rule").unwrap();
    assert!(daemon.reload_configuration_file(&config).await.is_err());
    assert!(daemon.configuration_error().await.is_some());

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Still silenced".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome.as_str(), "silenced");
}
