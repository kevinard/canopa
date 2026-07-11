#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    AcceptedNotification, Daemon, DaemonOptions, DeliveryDecisionReason, DeliveryOutcome,
    DeliveryPolicy, DeliveryPolicyRule, LoadedDeliveryPolicyState, NotificationBodyContent,
    NotificationInput, Urgency, UserRule,
};
use notifications::{notification, notification_with_body};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Executor, SqliteConnection};

#[test]
fn delivery_policy_applies_user_rule_and_do_not_disturb_precedence() {
    let policy = DeliveryPolicy::new(
        LoadedDeliveryPolicyState::builder()
            .do_not_disturb(true)
            .user_rule(DeliveryPolicyRule::new(
                "Chat",
                None,
                DeliveryOutcome::Shown,
                "user:Chat:*",
            ))
            .user_rule(DeliveryPolicyRule::new(
                "Build",
                None,
                DeliveryOutcome::Dropped,
                "user:Build:*",
            ))
            .config_rule(DeliveryPolicyRule::new(
                "Chat",
                None,
                DeliveryOutcome::Dropped,
                "config-drop-chat",
            ))
            .build(),
    );

    let silenced_by_dnd = policy.decide(&accepted_notification("Chat", Urgency::Normal));
    assert_eq!(silenced_by_dnd.outcome, DeliveryOutcome::Silenced);
    assert_eq!(
        silenced_by_dnd.rule_reference.as_deref(),
        Some("do-not-disturb")
    );
    assert_eq!(
        silenced_by_dnd.log.reason,
        DeliveryDecisionReason::DoNotDisturb
    );

    let critical_user_allow = policy.decide(&accepted_notification("Chat", Urgency::Critical));
    assert_eq!(critical_user_allow.outcome, DeliveryOutcome::Shown);
    assert_eq!(
        critical_user_allow.rule_reference.as_deref(),
        Some("user:Chat:*")
    );
    assert_eq!(
        critical_user_allow.log.reason,
        DeliveryDecisionReason::UserRule
    );

    let explicit_drop = policy.decide(&accepted_notification("Build", Urgency::Critical));
    assert_eq!(explicit_drop.outcome, DeliveryOutcome::Dropped);
    assert_eq!(
        explicit_drop.rule_reference.as_deref(),
        Some("user:Build:*")
    );
    assert_eq!(explicit_drop.log.reason, DeliveryDecisionReason::UserRule);
}

#[test]
fn delivery_policy_owns_config_rule_matching_and_default_allow_diagnostics() {
    let policy = DeliveryPolicy::new(
        LoadedDeliveryPolicyState::builder()
            .config_rule(DeliveryPolicyRule::new(
                "Mail",
                Some(Urgency::Normal),
                DeliveryOutcome::Silenced,
                "config:silence-mail-normal",
            ))
            .build(),
    );

    let config_decision = policy.decide(&accepted_notification("Mail", Urgency::Normal));
    assert_eq!(config_decision.outcome, DeliveryOutcome::Silenced);
    assert_eq!(
        config_decision.rule_reference.as_deref(),
        Some("config:silence-mail-normal")
    );
    assert_eq!(
        config_decision.log.reason,
        DeliveryDecisionReason::ConfigRule
    );

    let default_decision = policy.decide(&accepted_notification("Unknown", Urgency::Normal));
    assert_eq!(default_decision.outcome, DeliveryOutcome::Shown);
    assert_eq!(default_decision.rule_reference, None);
    assert_eq!(
        default_decision.log.reason,
        DeliveryDecisionReason::DefaultAllow
    );
    assert_eq!(default_decision.log.app_identity, "Unknown");
    assert_eq!(default_decision.log.urgency, Urgency::Normal);
}

#[test]
fn delivery_policy_prefers_urgency_specific_user_rules() {
    let policy = DeliveryPolicy::new(
        LoadedDeliveryPolicyState::builder()
            .user_rule(DeliveryPolicyRule::new(
                "Chat",
                None,
                DeliveryOutcome::Silenced,
                "user:Chat:*",
            ))
            .user_rule(DeliveryPolicyRule::new(
                "Chat",
                Some(Urgency::Critical),
                DeliveryOutcome::Shown,
                "user:Chat:critical",
            ))
            .build(),
    );

    let critical = policy.decide(&accepted_notification("Chat", Urgency::Critical));
    assert_eq!(critical.outcome, DeliveryOutcome::Shown);
    assert_eq!(
        critical.rule_reference.as_deref(),
        Some("user:Chat:critical")
    );

    let normal = policy.decide(&accepted_notification("Chat", Urgency::Normal));
    assert_eq!(normal.outcome, DeliveryOutcome::Silenced);
    assert_eq!(normal.rule_reference.as_deref(), Some("user:Chat:*"));
}

#[test]
fn delivery_policy_prefers_category_then_urgency_specific_user_rules() {
    let policy = DeliveryPolicy::new(
        LoadedDeliveryPolicyState::builder()
            .user_rule(DeliveryPolicyRule::new(
                "Chat",
                None,
                DeliveryOutcome::Dropped,
                "app-any",
            ))
            .user_rule(DeliveryPolicyRule::new(
                "Chat",
                Some(Urgency::Normal),
                DeliveryOutcome::Silenced,
                "app-normal",
            ))
            .user_rule(DeliveryPolicyRule::for_category(
                "Chat",
                "im.received",
                None,
                DeliveryOutcome::Shown,
                "category-any",
            ))
            .user_rule(DeliveryPolicyRule::for_category(
                "Chat",
                "im.received",
                Some(Urgency::Critical),
                DeliveryOutcome::Silenced,
                "category-critical",
            ))
            .build(),
    );

    let mut notification = accepted_notification("Chat", Urgency::Critical);
    notification.category = Some("im.received".to_string());
    assert_eq!(
        policy.decide(&notification).rule_reference.as_deref(),
        Some("category-critical")
    );
    notification.urgency = Urgency::Normal;
    assert_eq!(
        policy.decide(&notification).rule_reference.as_deref(),
        Some("category-any")
    );
    notification.category = Some("transfer.complete".to_string());
    assert_eq!(
        policy.decide(&notification).rule_reference.as_deref(),
        Some("app-normal")
    );
    notification.urgency = Urgency::Low;
    assert_eq!(
        policy.decide(&notification).rule_reference.as_deref(),
        Some("app-any")
    );
}

#[tokio::test]
async fn do_not_disturb_silences_non_critical_notifications_but_allows_critical_banners() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let before_dnd = daemon
        .receive_notification(notification("Chat", "Visible before DND"))
        .await
        .unwrap();
    assert!(is_banner_visible(&daemon, before_dnd.notification_id).await);

    daemon.set_do_not_disturb(true).await.unwrap();
    assert!(daemon.do_not_disturb().await.unwrap());

    let normal = daemon
        .receive_notification(notification("Chat", "Quiet please"))
        .await
        .unwrap();
    let critical = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification("Battery", "Critical battery")
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, before_dnd.notification_id)
            .delivery_outcome
            .as_str(),
        "shown"
    );
    assert!(is_banner_visible(&daemon, before_dnd.notification_id).await);
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

fn accepted_notification(app_identity: &str, urgency: Urgency) -> AcceptedNotification {
    AcceptedNotification {
        app_identity: app_identity.to_string(),
        app_name: app_identity.to_string(),
        replaces_id: None,
        app_icon: None,
        summary: "Policy test".to_string(),
        body: String::new(),
        body_content: NotificationBodyContent::default(),
        raw_body: String::new(),
        urgency,
        category: None,
        progress: None,
        image_reference: None,
        has_image_payload: false,
        actions: Vec::new(),
        resident: false,
        transient: false,
    }
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
        .receive_notification(notification("Chat", "Config drops this"))
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
        .receive_notification(notification("Chat", "User allows this"))
        .await
        .unwrap();

    assert_eq!(shown.delivery_outcome.as_str(), "shown");
    assert_eq!(restarted.user_rules().await.unwrap().len(), 1);
    assert_eq!(restarted.snapshot().await.unwrap().history.len(), 1);
}

#[tokio::test]
async fn category_user_rules_persist_and_apply_to_new_and_replacement_requests() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    daemon
        .upsert_user_rule(
            UserRule::for_app_category("Chat", "im.received", DeliveryOutcome::Silenced).unwrap(),
        )
        .await
        .unwrap();

    let original = daemon
        .receive_notification(NotificationInput {
            category: Some("im.received".to_string()),
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();
    assert_eq!(original.delivery_outcome, DeliveryOutcome::Silenced);
    let unrelated = daemon
        .receive_notification(notification("Chat", "No category"))
        .await
        .unwrap();
    assert_eq!(unrelated.delivery_outcome, DeliveryOutcome::Shown);

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(original.notification_id),
            category: Some("transfer.complete".to_string()),
            ..notification("Chat", "Changed category")
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id).delivery_outcome,
        DeliveryOutcome::Shown
    );
    assert_eq!(
        thread(&snapshot, original.notification_id)
            .accepted_notification
            .category
            .as_deref(),
        Some("transfer.complete")
    );
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let rules = restarted.user_rules().await.unwrap();
    assert_eq!(rules[0].category.as_deref(), Some("im.received"));
    assert_eq!(rules[0].outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn user_rule_changes_leave_existing_threads_unchanged_and_apply_to_replacements() {
    let daemon = daemon_support::open_headless_daemon().await;

    let original = daemon
        .receive_notification(notification_with_body("Chat", "Original", "Visible"))
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
        "shown"
    );
    assert!(is_banner_visible(&daemon, original.notification_id).await);

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(original.notification_id),
            ..notification_with_body("Chat", "Original", "Silenced update")
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id)
            .accepted_notification
            .body,
        "Silenced update"
    );
    assert_eq!(
        thread(&snapshot, original.notification_id)
            .delivery_outcome
            .as_str(),
        "silenced"
    );
    assert!(!is_banner_visible(&daemon, original.notification_id).await);

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Dropped))
        .await
        .unwrap();
    let dropped_replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(original.notification_id),
            ..notification_with_body("Chat", "Original", "Should not overwrite")
        })
        .await
        .unwrap();

    assert_eq!(dropped_replacement.delivery_outcome.as_str(), "dropped");
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, original.notification_id)
            .accepted_notification
            .body,
        "Silenced update"
    );
}

#[tokio::test]
async fn loaded_policy_state_is_not_reloaded_from_sqlite_per_request() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.set_do_not_disturb(true).await.unwrap();
    seed.upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Dropped))
        .await
        .unwrap();
    drop(seed);

    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let options = SqliteConnectOptions::new().filename(&database);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    connection
        .execute("UPDATE daemon_metadata SET value = 'false' WHERE key = 'do_not_disturb'")
        .await
        .unwrap();
    connection.execute("DELETE FROM user_rules").await.unwrap();

    assert!(daemon.do_not_disturb().await.unwrap());
    assert_eq!(daemon.user_rules().await.unwrap().len(), 1);

    let dropped = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification("Chat", "Still dropped by loaded rule")
        })
        .await
        .unwrap();
    assert_eq!(dropped.delivery_outcome, DeliveryOutcome::Dropped);

    let silenced = daemon
        .receive_notification(notification("Unknown", "Still silenced by loaded DND"))
        .await
        .unwrap();
    assert_eq!(silenced.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn configuration_rule_changes_apply_only_to_subsequent_requests() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let config = temp.path().join("notify.kdl");
    let existing = daemon
        .receive_notification(notification("Chat", "Already visible"))
        .await
        .unwrap();

    std::fs::write(
        &config,
        r#"rule "silence-chat" app="Chat" outcome="silenced""#,
    )
    .unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        thread(&snapshot, existing.notification_id).delivery_outcome,
        DeliveryOutcome::Shown
    );
    assert!(is_banner_visible(&daemon, existing.notification_id).await);

    let subsequent = daemon
        .receive_notification(notification("Chat", "Now silenced"))
        .await
        .unwrap();
    assert_eq!(subsequent.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn configuration_category_rules_cover_standard_vendor_missing_and_malformed_values() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let config = temp.path().join("notify.kdl");
    std::fs::write(
        &config,
        r#"
        rule "drop-messages" app="Chat" category="im.received" outcome="drop"
        rule "allow-vendor" app="Chat" category="x-vendor.attention" outcome="allow"
        rule "fallback" app="Chat" outcome="silence"
        "#,
    )
    .unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();

    let standard = daemon
        .receive_notification(NotificationInput {
            category: Some("im.received".to_string()),
            ..notification("Chat", "Standard")
        })
        .await
        .unwrap();
    let vendor = daemon
        .receive_notification(NotificationInput {
            category: Some("x-vendor.attention".to_string()),
            ..notification("Chat", "Vendor")
        })
        .await
        .unwrap();
    let missing = daemon
        .receive_notification(notification("Chat", "Missing"))
        .await
        .unwrap();
    let malformed = daemon
        .receive_notification(NotificationInput {
            category: Some("Bad Category".to_string()),
            ..notification("Chat", "Malformed")
        })
        .await
        .unwrap();

    assert_eq!(standard.delivery_outcome, DeliveryOutcome::Dropped);
    assert_eq!(vendor.delivery_outcome, DeliveryOutcome::Shown);
    assert_eq!(missing.delivery_outcome, DeliveryOutcome::Silenced);
    assert_eq!(malformed.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn delivery_decision_logs_redact_notification_content() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Silenced))
        .await
        .unwrap();
    daemon
        .receive_notification(notification_with_body(
            "Chat",
            "Secret subject",
            "Secret body",
        ))
        .await
        .unwrap();

    let logs = daemon.delivery_decision_logs().await;
    assert_eq!(logs[0].reason, DeliveryDecisionReason::UserRule);

    let encoded_logs = serde_json::to_string(&logs).unwrap();
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
        .receive_notification(notification("Chat", "Still silenced"))
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome.as_str(), "silenced");
}
