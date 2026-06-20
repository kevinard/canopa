use canopa_notify::{
    AcceptedNotification, Daemon, DaemonOptions, DeliveryDecisionReason, DeliveryOutcome,
    DeliveryPolicy, DeliveryPolicyRule, LoadedDeliveryPolicyState, NotificationInput, Urgency,
    UserRule,
};

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

fn accepted_notification(app_identity: &str, urgency: Urgency) -> AcceptedNotification {
    AcceptedNotification {
        app_identity: app_identity.to_string(),
        app_name: app_identity.to_string(),
        replaces_id: None,
        summary: "Policy test".to_string(),
        body: String::new(),
        raw_body: String::new(),
        urgency,
        category: None,
        progress: None,
        has_image_payload: false,
        actions: Vec::new(),
        resident: false,
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
        thread(&snapshot, original.notification_id)
            .accepted_notification
            .body,
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
        thread(&snapshot, original.notification_id)
            .accepted_notification
            .body,
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
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Still silenced".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome.as_str(), "silenced");
}
