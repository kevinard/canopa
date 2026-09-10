#[path = "support/daemon.rs"]
mod daemon_support;

use canopa_notify::{
    NotificationAction, NotificationBodyContent, NotificationBodyRun, NotificationBodyStyle,
    NotificationInput, NotificationLimitExceeded, NotificationSound, NotificationTimeout,
    NotificationVisualReference,
};

#[tokio::test]
async fn malformed_tag_starts_do_not_swallow_later_content_or_quoted_attributes() {
    let daemon = daemon_support::open_headless_daemon().await;
    daemon
        .receive_notification(NotificationInput {
            body: "<<b>Visible</b>".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active[0].accepted_notification.body_content,
        NotificationBodyContent::from_text_runs(vec![NotificationBodyRun::styled(
            "Visible",
            NotificationBodyStyle {
                bold: true,
                ..Default::default()
            }
        )])
    );

    let receipt = daemon
        .receive_notification(NotificationInput {
            body: "<<<img src=\"/tmp/icon.png\" alt=\"A > B\"/>".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    let image = &snapshot
        .active
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap()
        .accepted_notification;
    assert_eq!(image.body, "A > B");
    assert_eq!(image.body_content.segments.len(), 1);
    assert!(matches!(
        image.body_content.segments[0],
        canopa_notify::NotificationBodySegment::Image { .. }
    ));
}

#[tokio::test]
async fn excessive_markup_depth_degrades_to_plain_text_without_losing_image_alternatives() {
    let daemon = daemon_support::open_headless_daemon().await;
    let body = format!(
        "<img src=\"/tmp/first.png\" alt=\"First \"/>{}Deep{}<img src=\"/tmp/last.png\" alt=\" last\"/>",
        "<b>".repeat(65),
        "</b>".repeat(65)
    );
    daemon
        .receive_notification(NotificationInput {
            body: body.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    let accepted = &snapshot.active[0].accepted_notification;
    assert_eq!(accepted.raw_body, body);
    assert_eq!(accepted.body, "First Deep last");
    assert_eq!(
        accepted.body_content,
        NotificationBodyContent::plain("First Deep last")
    );
}

#[tokio::test]
async fn rich_segment_budget_is_enforced_before_whitespace_normalization() {
    let daemon = daemon_support::open_headless_daemon().await;
    let body = format!("{}<b>Tail</b>", "<b> </b>\n".repeat(128));
    daemon
        .receive_notification(NotificationInput {
            body,
            ..Default::default()
        })
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active[0].accepted_notification.body_content,
        NotificationBodyContent::plain("Tail")
    );
}

#[tokio::test]
async fn markup_limits_preserve_rich_content_at_the_boundary_and_plain_fallback_after_it() {
    let daemon = daemon_support::open_headless_daemon().await;
    let cases = [
        (
            format!("{}Deep{}", "<b>".repeat(64), "</b>".repeat(64)),
            "Deep".into(),
            1,
            true,
        ),
        ("<b>x</b>y".repeat(128), "xy".repeat(128), 256, true),
        (
            "<img src=\"/tmp/a.png\" alt=\"a\"/>".repeat(257),
            "a".repeat(257),
            1,
            false,
        ),
    ];
    for (body, expected, segments, bold) in cases {
        let receipt = daemon
            .receive_notification(NotificationInput {
                body,
                ..Default::default()
            })
            .await
            .unwrap();
        let snapshot = daemon.snapshot().await.unwrap();
        let accepted = &snapshot
            .active
            .iter()
            .find(|thread| thread.id == receipt.notification_id)
            .unwrap()
            .accepted_notification;
        assert_eq!(accepted.body, expected);
        assert_eq!(accepted.body_content.segments.len(), segments);
        assert_eq!(accepted.body_content.text_runs()[0].style.bold, bold);
    }
}

#[tokio::test]
async fn adversarial_markup_keeps_safe_text_without_recursive_or_repeated_suffix_scans() {
    let daemon = daemon_support::open_headless_daemon().await;
    let cases = [
        (
            format!("{}<b>Tail</b>", "<".repeat(64_000)),
            "Tail".to_string(),
        ),
        (
            format!("{}&#x1F44D;", "&".repeat(64_000)),
            format!("{}👍", "&".repeat(64_000)),
        ),
        (format!("{}Visible", "\n".repeat(64_000)), "Visible".into()),
        (
            format!("{}Deep{}", "<b>".repeat(8192), "</b>".repeat(8192)),
            "Deep".into(),
        ),
        (
            "é &bogus; &#xD800; &#1114112; &#000000065; &amp;".into(),
            "é &bogus; &#xD800; &#1114112; A &".into(),
        ),
    ];
    for (body, expected) in cases {
        let receipt = daemon
            .receive_notification(NotificationInput {
                body,
                ..Default::default()
            })
            .await
            .unwrap();
        let snapshot = daemon.snapshot().await.unwrap();
        let accepted = &snapshot
            .active
            .iter()
            .find(|thread| thread.id == receipt.notification_id)
            .unwrap()
            .accepted_notification;
        assert_eq!(accepted.body, expected);
        assert!(accepted.body_content.segments.len() <= 256);
    }
}

#[tokio::test]
async fn exact_text_and_action_limits_preserve_utf8_content_and_every_action() {
    let daemon = daemon_support::open_headless_daemon().await;
    let input = NotificationInput {
        app_name: "n".repeat(4096),
        summary: "s".repeat(8192),
        body: "é".repeat(32_768),
        actions: (0..64)
            .map(|index| NotificationAction::new(format!("action-{index}"), "Label"))
            .collect(),
        ..Default::default()
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    let accepted = &snapshot.active[0].accepted_notification;
    assert_eq!(accepted.raw_body, input.body);
    assert_eq!(accepted.body, input.body);
    assert_eq!(accepted.summary, input.summary);
    assert_eq!(accepted.app_name, input.app_name);
    assert_eq!(accepted.actions, input.actions);
    let mut oversized = input;
    oversized.replaces_id = Some(receipt.notification_id);
    oversized.body.push('é');
    assert!(daemon.receive_notification(oversized).await.is_err());
    assert_eq!(daemon.snapshot().await.unwrap(), snapshot);
}

#[tokio::test]
async fn aggregate_text_limit_counts_fields_even_when_each_one_fits() {
    let daemon = daemon_support::open_headless_daemon().await;
    // 65,536 body bytes + 16 * (one-byte key + 4,095-byte label) = 131,072.
    let mut input = NotificationInput {
        body: "b".repeat(65_536),
        actions: ('a'..='p')
            .map(|key| NotificationAction::new(key.to_string(), "l".repeat(4095)))
            .collect(),
        ..Default::default()
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let before = daemon.snapshot().await.unwrap();
    input.replaces_id = Some(receipt.notification_id);
    input.actions[0].label.push('l');
    let error = daemon.receive_notification(input).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "notification aggregate text limit exceeded"
    );
    assert_eq!(daemon.snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn text_and_action_admission_limits_apply_before_policy_or_transient_receipt() {
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    let cases = [
        NotificationInput {
            summary: "x".repeat(8_193),
            ..Default::default()
        },
        NotificationInput {
            app_name: "x".repeat(4_097),
            ..Default::default()
        },
        NotificationInput {
            desktop_entry: Some("x".repeat(4_097)),
            ..Default::default()
        },
        NotificationInput {
            category: Some("x".repeat(4_097)),
            ..Default::default()
        },
        NotificationInput {
            app_icon: Some(NotificationVisualReference::IconName("x".repeat(4_097))),
            ..Default::default()
        },
        NotificationInput {
            image_reference: Some(NotificationVisualReference::LocalPath("x".repeat(4_097))),
            ..Default::default()
        },
        NotificationInput {
            sound: Some(NotificationSound::Name("x".repeat(4_097))),
            ..Default::default()
        },
        NotificationInput {
            actions: vec![NotificationAction::new("key", "Label"); 65],
            ..Default::default()
        },
        NotificationInput {
            actions: vec![NotificationAction::new("x".repeat(4_097), "Label")],
            ..Default::default()
        },
        NotificationInput {
            actions: vec![NotificationAction::new("key", "x".repeat(4_097))],
            ..Default::default()
        },
        NotificationInput {
            actions: vec![
                NotificationAction::new("key", "Label")
                    .with_icon(NotificationVisualReference::IconName("x".repeat(4_097))),
            ],
            ..Default::default()
        },
    ];
    for mut input in cases {
        input.transient = true;
        let error = daemon
            .receive_notification(input)
            .await
            .expect_err("oversized input accepted");
        assert!(error.downcast_ref::<NotificationLimitExceeded>().is_some());
        assert!(!error.to_string().contains("xxxx"));
    }
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
    assert!(daemon.sender_events().await.is_empty());
    assert!(daemon.delivery_decision_logs().await.is_empty());
}

#[tokio::test]
async fn oversized_body_rejects_replacement_without_mutating_the_live_notification() {
    let daemon = daemon_support::open_headless_daemon().await;
    let original = daemon
        .receive_notification(NotificationInput {
            app_name: "Limits".into(),
            summary: "Original".into(),
            expire_timeout: NotificationTimeout::Never,
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let before = daemon.snapshot().await.unwrap();
    let error = daemon
        .receive_notification(NotificationInput {
            app_name: "Limits".into(),
            replaces_id: Some(original.notification_id),
            body: "x".repeat(65_537),
            ..NotificationInput::default()
        })
        .await
        .expect_err("body exceeding 64 KiB was accepted");
    assert_eq!(error.to_string(), "notification body limit exceeded");
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert!(daemon.sender_events().await.is_empty());
    daemon
        .close_notification(original.notification_id)
        .await
        .unwrap();
}
