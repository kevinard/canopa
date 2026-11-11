use super::*;
use futures_util::StreamExt;

fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value.into().try_to_owned().unwrap()
}

struct Fixture {
    _temp: tempfile::TempDir,
    daemon: Daemon,
    server: zbus::Connection,
    proxy: zbus::Proxy<'static>,
    handle: super::super::NotificationServerHandle,
}

impl Fixture {
    async fn new(suffix: &str) -> Self {
        Self::with_options(suffix, canopa_notify::DaemonOptions::headless).await
    }

    async fn with_options(
        suffix: &str,
        options: impl FnOnce(std::path::PathBuf) -> canopa_notify::DaemonOptions,
    ) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(options(temp.path().join("history.db")))
            .await
            .unwrap();
        let name = format!(
            "org.canopa.NotificationPortal.p{}.{}",
            std::process::id(),
            suffix
        );
        let server = zbus::connection::Builder::session()
            .unwrap()
            .name(name.clone())
            .unwrap()
            .build()
            .await
            .unwrap();
        let handle = super::super::attach_notification_server(server.clone(), daemon.clone())
            .await
            .unwrap();
        let proxy = zbus::Proxy::new_owned(
            zbus::Connection::session().await.unwrap(),
            name,
            PATH,
            "org.freedesktop.impl.portal.Notification",
        )
        .await
        .unwrap();
        Self {
            _temp: temp,
            daemon,
            server,
            proxy,
            handle,
        }
    }

    async fn add(
        &self,
        app: &str,
        id: &str,
        values: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()> {
        self.proxy.call("AddNotification", &(app, id, values)).await
    }

    async fn remove(&self, app: &str, id: &str) {
        self.proxy
            .call::<_, _, ()>("RemoveNotification", &(app, id))
            .await
            .unwrap();
    }

    async fn active_items(&self) -> Vec<canopa_notify::NotificationCenterItem> {
        let control = canopa_notify::NotifyControl::new(self.daemon.clone());
        let canopa_notify::ControlReply::NotificationCenterProjection(projection) = control
            .execute(canopa_notify::ControlCommand::NotificationCenterProjection)
            .await
            .unwrap()
        else {
            panic!("unexpected Center reply");
        };
        projection
            .groups
            .into_iter()
            .flat_map(|group| group.items)
            .filter(|item| item.status == canopa_notify::NotificationStatus::Active)
            .collect()
    }

    async fn finish(self) {
        self.handle.shutdown().await.unwrap();
        self.daemon.shutdown().await.unwrap();
        self.server.close().await.unwrap();
    }
}

#[test]
fn plain_body_and_original_target_types_survive_intake() {
    let identity = Identity {
        app_id: "org.canopa.Test".into(),
        id: "one".into(),
    };
    let values = HashMap::from([
        ("title".into(), Value::from("Title").try_to_owned().unwrap()),
        (
            "body".into(),
            Value::from("literal <b>&body</b>").try_to_owned().unwrap(),
        ),
        (
            "default-action".into(),
            Value::from("app.open").try_to_owned().unwrap(),
        ),
        (
            "default-action-target".into(),
            Value::from(("payload", 7i32)).try_to_owned().unwrap(),
        ),
    ]);
    let (input, binding) = intake::prepare(identity, values).unwrap();
    assert_eq!(input.body, "literal <b>&body</b>");
    assert_eq!(
        input.body_format,
        canopa_notify::NotificationBodyFormat::Plain
    );
    assert_eq!(
        binding.actions["default"]
            .target
            .as_ref()
            .unwrap()
            .value_signature()
            .to_string(),
        "(si)"
    );
}

#[tokio::test]
async fn wire_ids_are_app_scoped_dropped_replacements_keep_the_old_target_and_withdrawal_retires() {
    let fixture = Fixture::new("Identity").await;
    assert_eq!(
        fixture.proxy.get_property::<u32>("version").await.unwrap(),
        2
    );
    assert!(
        bool::try_from(
            fixture
                .proxy
                .get_property::<HashMap<String, OwnedValue>>("SupportedOptions")
                .await
                .unwrap()[EXPIRY_OPTION]
                .try_clone()
                .unwrap()
        )
        .unwrap()
    );
    let mut signals = fixture.proxy.receive_signal("ActionInvoked").await.unwrap();
    let values = || {
        HashMap::from([
            ("title".into(), owned("Original")),
            ("body".into(), owned("Literal <b>body</b>")),
            ("default-action".into(), owned("chosen")),
            ("default-action-target".into(), owned(("original", 7i32))),
        ])
    };
    fixture.add("org.canopa.A", "same", values()).await.unwrap();
    fixture.add("org.canopa.B", "same", values()).await.unwrap();
    let before = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(before.active.len(), 2);
    let first = before
        .active
        .iter()
        .find(|thread| thread.accepted_notification.app_identity == "org.canopa.A")
        .unwrap();
    assert_eq!(first.accepted_notification.body, "Literal <b>body</b>");
    fixture
        .daemon
        .upsert_user_rule(canopa_notify::UserRule::for_app(
            "org.canopa.A",
            DeliveryOutcome::Dropped,
        ))
        .await
        .unwrap();
    fixture
        .add(
            "org.canopa.A",
            "same",
            HashMap::from([
                ("title".into(), owned("Dropped")),
                ("default-action".into(), owned("replacement")),
                ("default-action-target".into(), owned("wrong")),
            ]),
        )
        .await
        .unwrap();
    let after = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(
        after
            .active
            .iter()
            .find(|thread| thread.id == first.id)
            .unwrap()
            .revision,
        first.revision
    );
    fixture
        .daemon
        .invoke_action_with_activation_token(first.id, "default", Some("fresh-token".into()))
        .await
        .unwrap();
    let message = tokio::time::timeout(std::time::Duration::from_secs(3), signals.next())
        .await
        .unwrap()
        .unwrap();
    let (app, id, name, values): (String, String, String, Vec<OwnedValue>) =
        message.body().deserialize().unwrap();
    assert_eq!(
        (app.as_str(), id.as_str(), name.as_str()),
        ("org.canopa.A", "same", "chosen")
    );
    assert_eq!(
        <(String, i32)>::try_from(values[0].try_clone().unwrap()).unwrap(),
        ("original".into(), 7)
    );
    let platform = HashMap::<String, OwnedValue>::try_from(values[1].try_clone().unwrap()).unwrap();
    assert_eq!(
        <&str>::try_from(&platform["activation-token"]).unwrap(),
        "fresh-token"
    );
    fixture.remove("org.canopa.A", "same").await;
    let active = fixture.daemon.snapshot().await.unwrap().active;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].accepted_notification.app_identity, "org.canopa.B");
    fixture.remove("org.canopa.B", "same").await;
    fixture
        .add("org.canopa.B", "same", values_for("New lifetime"))
        .await
        .unwrap();
    assert_ne!(
        fixture.daemon.snapshot().await.unwrap().active[0].id,
        active[0].id
    );
    fixture.finish().await;
}

#[test]
fn request_expiry_is_bounded_transient_and_anchored_at_admission() {
    let identity = || Identity {
        app_id: "org.canopa.Question".into(),
        id: "one".into(),
    };
    let admitted = Instant::now() - std::time::Duration::from_secs(2);
    let (input, binding) = intake::prepare_at(
        identity(),
        HashMap::from([(EXPIRY_OPTION.into(), 1000u32.into())]),
        admitted,
    )
    .unwrap();
    assert!(input.transient);
    assert_eq!(
        input.expire_timeout,
        canopa_notify::NotificationTimeout::Never
    );
    assert_eq!(
        binding.deadline,
        Some(admitted + std::time::Duration::from_secs(1))
    );
    for invalid in [0u32.into(), 30_001u32.into(), owned("1000"), owned(-1i32)] {
        assert!(
            intake::prepare(identity(), HashMap::from([(EXPIRY_OPTION.into(), invalid)])).is_err()
        );
    }
}

#[test]
fn ordinary_portal_notifications_use_the_server_default_timeout_policy() {
    let identity = || Identity {
        app_id: "org.canopa.Chat".into(),
        id: "message".into(),
    };
    for (priority, urgency) in [
        (None, canopa_notify::Urgency::Normal),
        (Some("low"), canopa_notify::Urgency::Low),
        (Some("high"), canopa_notify::Urgency::Normal),
        (Some("urgent"), canopa_notify::Urgency::Critical),
    ] {
        let mut values = values_for("Message");
        if let Some(priority) = priority {
            values.insert("priority".into(), owned(priority));
        }
        let (input, binding) = intake::prepare(identity(), values).unwrap();
        assert_eq!(input.urgency, urgency);
        assert_eq!(
            input.expire_timeout,
            canopa_notify::NotificationTimeout::ServerDefault,
            "{priority:?} follows the ordinary banner timeout policy"
        );
        assert!(!input.transient);
        assert_eq!(binding.deadline, None);
    }
}

#[tokio::test]
async fn portal_banners_hide_after_their_timeout_but_keep_center_actions_live() {
    let fixture = Fixture::with_options("BannerTimeout", |path| {
        canopa_notify::DaemonOptions::graphical(path, canopa_notify::NoopBannerPresenter)
            .with_notification_timeout_defaults(canopa_notify::NotificationTimeoutDefaults {
                low: Some(std::time::Duration::from_millis(50)),
                normal: Some(std::time::Duration::from_millis(50)),
                critical: None,
            })
    })
    .await;
    let mut normal = values_for("Hides by itself");
    normal.insert(
        "buttons".into(),
        owned(vec![HashMap::from([
            ("label".to_string(), Value::from("Archive")),
            ("action".to_string(), Value::from("archive")),
        ])]),
    );
    fixture
        .add("org.canopa.Chat", "normal", normal)
        .await
        .unwrap();
    let mut urgent = values_for("Stays on screen");
    urgent.insert("priority".into(), owned("urgent"));
    fixture
        .add("org.canopa.Chat", "urgent", urgent)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while fixture.daemon.visible_banners().await.len() != 1 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the normal portal banner never hid");
    assert_eq!(
        fixture.daemon.visible_banners().await[0].summary,
        "Stays on screen"
    );
    // Both remain live: the hidden one keeps its sender action in the Center.
    let items = fixture.active_items().await;
    let hidden = items
        .iter()
        .find(|item| item.summary == "Hides by itself")
        .unwrap();
    assert_eq!(hidden.actions[0].label, "Archive");
    // The same portal ID still updates the live notification in place.
    fixture
        .add("org.canopa.Chat", "normal", values_for("Updated quietly"))
        .await
        .unwrap();
    assert_eq!(fixture.daemon.snapshot().await.unwrap().active.len(), 2);
    assert!(
        fixture
            .daemon
            .visible_banners()
            .await
            .iter()
            .all(|banner| banner.summary != "Updated quietly"),
        "without show-as-new a hidden notification updates without a new banner"
    );
    let mut again = values_for("Shown as new");
    again.insert("display-hint".into(), owned(vec!["show-as-new"]));
    fixture
        .add("org.canopa.Chat", "normal", again)
        .await
        .unwrap();
    assert!(
        fixture
            .daemon
            .visible_banners()
            .await
            .iter()
            .any(|banner| banner.summary == "Shown as new")
    );
    fixture.remove("org.canopa.Chat", "urgent").await;
    fixture.remove("org.canopa.Chat", "normal").await;
    assert!(fixture.daemon.snapshot().await.unwrap().active.is_empty());
    fixture.finish().await;
}

#[test]
fn display_hints_and_button_purposes_map_to_presentation() {
    let identity = || Identity {
        app_id: "org.canopa.Chat".into(),
        id: "message".into(),
    };
    let with_hints = |hints: &[&str]| {
        let mut values = values_for("Hinted");
        let hints: Vec<String> = hints.iter().map(|hint| hint.to_string()).collect();
        values.insert("display-hint".into(), owned(hints));
        intake::prepare(identity(), values)
    };
    let (input, _) = intake::prepare(identity(), values_for("Plain")).unwrap();
    assert!(input.presentation.keep_after_banner && input.presentation.quiet_update);
    assert!(!input.presentation.center_only && input.presentation.lock_screen.is_none());

    let (input, _) = with_hints(&["transient"]).unwrap();
    assert!(input.transient && !input.presentation.keep_after_banner);
    let (input, _) = with_hints(&["tray", "persistent", "show-as-new"]).unwrap();
    assert!(input.presentation.center_only && input.resident && !input.presentation.quiet_update);
    let (input, _) = with_hints(&["hide-content-on-lock-screen"]).unwrap();
    assert_eq!(
        input.presentation.lock_screen,
        Some(canopa_notify::LockScreenVisibility::NameOnly)
    );
    // The 1.22 XML spelling is accepted too; hiding wins over hiding content.
    let (input, _) = with_hints(&["hide-content-on-lockscreen", "hide-on-lockscreen"]).unwrap();
    assert_eq!(
        input.presentation.lock_screen,
        Some(canopa_notify::LockScreenVisibility::Hidden)
    );
    let (input, _) = with_hints(&["unknown-future-hint"]).unwrap();
    assert!(input.presentation.keep_after_banner);
    assert!(with_hints(&["transient", "tray"]).is_err());

    let mut values = values_for("Message");
    values.insert("category".into(), owned("im.received"));
    values.insert(
        "buttons".into(),
        owned(vec![
            HashMap::from([
                ("purpose".to_string(), Value::from("im.reply-with-text")),
                ("action".to_string(), Value::from("reply")),
            ]),
            HashMap::from([
                ("purpose".to_string(), Value::from("call.decline")),
                ("action".to_string(), Value::from("app.decline")),
            ]),
            HashMap::from([
                ("purpose".to_string(), Value::from("x-unknown.purpose")),
                ("action".to_string(), Value::from("unknown")),
            ]),
            HashMap::from([
                ("purpose".to_string(), Value::from("x-unknown.purpose")),
                ("label".to_string(), Value::from("Labelled")),
                ("action".to_string(), Value::from("labelled")),
            ]),
        ]),
    );
    let (input, binding) = intake::prepare(identity(), values).unwrap();
    let labels: Vec<_> = input
        .actions
        .iter()
        .map(|a| (a.key.as_str(), a.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            ("default", ""),
            ("portal-button-0", "Reply"),
            ("portal-button-1", "Decline"),
            ("portal-button-3", "Labelled"),
        ],
        "an unlabelled unknown purpose is skipped; a labelled one degrades to a button"
    );
    assert_eq!(
        input.presentation.reply,
        Some(canopa_notify::ReplyRequest {
            action_key: "portal-button-0".into(),
            placeholder: None,
        })
    );
    assert_eq!(binding.actions["portal-button-0"].name, "reply");
}

#[tokio::test]
async fn supported_options_advertise_categories_and_purposes() {
    let fixture = Fixture::new("Options").await;
    let options: HashMap<String, OwnedValue> = fixture
        .proxy
        .get_property("SupportedOptions")
        .await
        .unwrap();
    let strings = |key: &str| Vec::<String>::try_from(options[key].try_clone().unwrap()).unwrap();
    assert_eq!(strings("category"), intake::SUPPORTED_CATEGORIES);
    assert_eq!(strings("button-purpose"), intake::SUPPORTED_PURPOSES);
    assert!(bool::try_from(&options[EXPIRY_OPTION]).unwrap());
    fixture.finish().await;
}

#[tokio::test]
async fn tray_hint_delivers_quietly_and_transient_skips_history() {
    let fixture = Fixture::new("Hints").await;
    let mut tray = values_for("Tray only");
    tray.insert("display-hint".into(), owned(vec!["tray"]));
    fixture.add("org.canopa.Chat", "tray", tray).await.unwrap();
    let log = fixture.daemon.delivery_decision_logs().await.pop().unwrap();
    assert_eq!(
        log.delivery_outcome,
        canopa_notify::DeliveryOutcome::Silenced
    );
    assert_eq!(
        log.reason,
        canopa_notify::DeliveryDecisionReason::DisplayHint
    );
    let mut transient = values_for("Banner only");
    transient.insert("display-hint".into(), owned(vec!["transient"]));
    fixture
        .add("org.canopa.Chat", "transient", transient)
        .await
        .unwrap();
    let history = fixture.daemon.snapshot().await.unwrap().history;
    assert!(
        history
            .iter()
            .all(|thread| thread.accepted_notification.summary != "Banner only")
    );
    fixture.remove("org.canopa.Chat", "tray").await;
    fixture.remove("org.canopa.Chat", "transient").await;
    fixture.finish().await;
}

fn reply_values(action: &str) -> HashMap<String, OwnedValue> {
    let mut values = values_for("Alice");
    values.insert("category".into(), owned("im.received"));
    values.insert(
        "buttons".into(),
        owned(vec![HashMap::from([
            ("purpose".to_string(), Value::from("im.reply-with-text")),
            ("action".to_string(), Value::from(action.to_owned())),
            ("target".to_string(), Value::from("thread-7")),
        ])]),
    );
    values
}

async fn send_reply(fixture: &Fixture, summary: &str, text: &str) {
    let thread = fixture
        .daemon
        .snapshot()
        .await
        .unwrap()
        .active
        .into_iter()
        .find(|thread| thread.accepted_notification.summary == summary)
        .unwrap();
    fixture
        .daemon
        .handle_banner_interaction(canopa_notify::BannerInteraction::Reply {
            target: canopa_notify::BannerTarget {
                notification_id: thread.id,
                revision: thread.revision,
            },
            text: text.into(),
            activation_token: Some("reply-token".into()),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn inline_replies_reach_the_signal_and_exported_actions_after_their_target() {
    let fixture = Fixture::new("Reply").await;
    let mut signals = fixture.proxy.receive_signal("ActionInvoked").await.unwrap();
    fixture
        .add("org.canopa.Chat", "message", reply_values("reply"))
        .await
        .unwrap();
    // The Center offers "Reply…" for the live item.
    assert!(fixture.active_items().await[0].can_reply);
    assert!(
        fixture
            .daemon
            .handle_banner_interaction(canopa_notify::BannerInteraction::Reply {
                target: canopa_notify::BannerTarget {
                    notification_id: fixture.daemon.snapshot().await.unwrap().active[0].id,
                    revision: 1,
                },
                text: "   ".into(),
                activation_token: None,
            })
            .await
            .is_err(),
        "empty replies are rejected"
    );
    send_reply(&fixture, "Alice", "  On my way  ").await;
    let message = tokio::time::timeout(std::time::Duration::from_secs(3), signals.next())
        .await
        .unwrap()
        .unwrap();
    let (app, id, action, values): (String, String, String, Vec<OwnedValue>) =
        message.body().deserialize().unwrap();
    assert_eq!(
        (app.as_str(), id.as_str(), action.as_str()),
        ("org.canopa.Chat", "message", "reply")
    );
    assert_eq!(values.len(), 3, "target, platform data, then the response");
    assert_eq!(<&str>::try_from(&values[0]).unwrap(), "thread-7");
    let platform = HashMap::<String, OwnedValue>::try_from(values[1].try_clone().unwrap()).unwrap();
    assert_eq!(
        <&str>::try_from(&platform["activation-token"]).unwrap(),
        "reply-token"
    );
    assert_eq!(<&str>::try_from(&values[2]).unwrap(), "On my way");
    // Diagnostics never keep the text.
    assert!(
        fixture
            .daemon
            .sender_events()
            .await
            .iter()
            .all(|event| !matches!(
                event,
                SenderEvent::ReplyText { text, .. } if !text.is_empty()
            ))
    );
    let history = fixture.daemon.snapshot().await.unwrap().history;
    assert!(
        history
            .iter()
            .all(|thread| !thread.accepted_notification.summary.contains("On my way"))
    );

    // Exported action: ActivateAction(name, [target, response], platform).
    let name = format!("org.canopa.ReplyApp.p{}", std::process::id());
    let path = format!("/{}", name.replace('.', "/"));
    let (sent, mut calls) = tokio::sync::mpsc::unbounded_channel();
    let application = zbus::connection::Builder::session()
        .unwrap()
        .name(name.clone())
        .unwrap()
        .serve_at(path.as_str(), RecordingApplication(sent))
        .unwrap()
        .build()
        .await
        .unwrap();
    fixture
        .add(&name, "exported", reply_values("app.reply"))
        .await
        .unwrap();
    send_reply(&fixture, "Alice", "Exported").await;
    let (action, parameters, platform) =
        tokio::time::timeout(std::time::Duration::from_secs(3), calls.recv())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(action, "reply");
    let parameters: Vec<String> = parameters
        .into_iter()
        .map(|value| String::try_from(value).unwrap())
        .collect();
    assert_eq!(parameters, ["thread-7", "Exported"]);
    assert_eq!(
        <&str>::try_from(&platform["activation-token"]).unwrap(),
        "reply-token"
    );
    fixture.finish().await;
    application.close().await.unwrap();
}

#[tokio::test]
async fn a_question_expires_without_banner_display_or_its_sender_while_normal_notifications_survive()
 {
    let fixture = Fixture::new("RequestExpiry").await;
    fixture
        .add(
            "org.canopa.Question",
            "normal",
            values_for("Ordinary portal notification"),
        )
        .await
        .unwrap();
    let sender = zbus::Connection::session().await.unwrap();
    let question = zbus::Proxy::new(
        &sender,
        fixture.proxy.destination().clone(),
        PATH,
        "org.freedesktop.impl.portal.Notification",
    )
    .await
    .unwrap();
    let mut fields = values_for("Request question");
    fields.insert(EXPIRY_OPTION.into(), 1000u32.into());
    let _: () = question
        .call(
            "AddNotification",
            &("org.canopa.Question", "leased", fields),
        )
        .await
        .unwrap();
    sender.close().await.unwrap();
    let items = fixture.active_items().await;
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .find(|n| n.summary == "Request question")
            .unwrap()
            .transient
    );
    // A headless daemon never acknowledges displaying a banner. This expiry
    // must be independent of its ordinary display-based timeout mechanism.
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            if fixture.active_items().await.len() == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture.active_items().await[0].summary,
        "Ordinary portal notification"
    );
    fixture.finish().await;
}

#[tokio::test]
async fn replacing_a_question_retires_its_expiry_without_expiring_the_replacement() {
    let fixture = Fixture::new("ReplaceExpiry").await;
    let mut fields = values_for("Request question");
    fields.insert(EXPIRY_OPTION.into(), 700u32.into());
    fixture
        .add("org.canopa.Question", "one", fields)
        .await
        .unwrap();
    fixture
        .add(
            "org.canopa.Question",
            "one",
            values_for("Normal replacement"),
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    let items = fixture.active_items().await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].summary, "Normal replacement");
    // Retention is sticky: replacing a transient never archives the thread.
    assert!(items[0].transient);
    fixture.finish().await;
}

#[tokio::test]
async fn expired_admitted_work_is_discarded_before_receipt() {
    let fixture = Fixture::new("ExpiredAdmission").await;
    let (service, handle) = PortalServer::start(fixture.daemon.clone(), Registry::default());
    let mut fields = values_for("Already ended");
    fields.insert(EXPIRY_OPTION.into(), 100u32.into());
    let (reply, result) = tokio::sync::oneshot::channel();
    service
        .admit(Operation::Add(
            Identity {
                app_id: "org.canopa.Question".into(),
                id: "one".into(),
            },
            fields,
            reply,
            Instant::now() - std::time::Duration::from_secs(1),
        ))
        .unwrap();
    result.await.unwrap().unwrap();
    assert!(fixture.daemon.snapshot().await.unwrap().active.is_empty());
    handle.shutdown().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
async fn queued_expiry_cannot_withdraw_a_replacement_binding() {
    let fixture = Fixture::new("QueuedExpiry").await;
    fixture
        .add(
            "org.canopa.Question",
            "one",
            values_for("Current replacement"),
        )
        .await
        .unwrap();
    let id = fixture.daemon.snapshot().await.unwrap().active[0].id;
    let registry = Registry::default();
    let binding = |deadline| {
        Arc::new(Binding {
            identity: Identity {
                app_id: "org.canopa.Question".into(),
                id: "one".into(),
            },
            actions: HashMap::new(),
            bytes: 1,
            deadline,
        })
    };
    let old = binding(Some(Instant::now() + std::time::Duration::from_secs(10)));
    registry.commit(old.clone(), id);
    registry.commit(binding(None), id);
    let (service, handle) = PortalServer::start(fixture.daemon.clone(), registry);
    service
        .admit(Operation::Expire(id, Arc::downgrade(&old)))
        .unwrap();
    handle.shutdown().await.unwrap();
    assert_eq!(
        fixture.daemon.snapshot().await.unwrap().active[0]
            .accepted_notification
            .summary,
        "Current replacement"
    );
    fixture.finish().await;
}

#[tokio::test]
async fn transport_shutdown_withdraws_admitted_questions_but_preserves_ordinary_notifications() {
    let fixture = Fixture::new("QuestionShutdown").await;
    fixture
        .add(
            "org.canopa.Question",
            "normal",
            values_for("Keep ordinary notification"),
        )
        .await
        .unwrap();
    let mut question = values_for("End question");
    question.insert(EXPIRY_OPTION.into(), 30_000u32.into());
    fixture
        .add("org.canopa.Question", "question", question)
        .await
        .unwrap();
    assert_eq!(fixture.active_items().await.len(), 2);
    let control = canopa_notify::NotifyControl::new(fixture.daemon.clone());
    fixture.handle.shutdown().await.unwrap();
    let canopa_notify::ControlReply::NotificationCenterProjection(center) = control
        .execute(canopa_notify::ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("unexpected Center reply");
    };
    assert_eq!(
        center
            .groups
            .iter()
            .map(|group| group.items.len())
            .sum::<usize>(),
        1
    );
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(
        snapshot.active[0].accepted_notification.summary,
        "Keep ordinary notification"
    );
    fixture.daemon.shutdown().await.unwrap();
    fixture.server.close().await.unwrap();
}

fn values_for(title: &str) -> HashMap<String, OwnedValue> {
    HashMap::from([("title".into(), Value::from(title).try_to_owned().unwrap())])
}

#[tokio::test]
async fn frozen_action_retains_its_target_and_token_after_replacement_and_close() {
    let fixture = Fixture::new("Frozen").await;
    let mut signals = fixture.proxy.receive_signal("ActionInvoked").await.unwrap();
    // Freeze at the same seam used synchronously by the authoritative publisher,
    // then deliberately hold emission while both replacement and closure occur.
    fixture
        .add("org.canopa.Frozen", "one", values_for("Identity source"))
        .await
        .unwrap();
    let id = fixture.daemon.snapshot().await.unwrap().active[0].id;
    let registry = Registry::default();
    let binding = |text: &'static str| {
        Arc::new(Binding {
            identity: Identity {
                app_id: "org.canopa.Frozen".into(),
                id: "one".into(),
            },
            actions: HashMap::from([(
                "default".into(),
                Action {
                    name: "chosen".into(),
                    target: Some(owned(text)),
                },
            )]),
            bytes: 64,
            deadline: None,
        })
    };
    registry.commit(binding("old"), id);
    assert!(
        registry
            .freeze(&SenderEvent::ActivationToken {
                notification_id: id,
                activation_token: "old-input".into()
            })
            .unwrap()
            .is_none()
    );
    let old = registry
        .freeze(&SenderEvent::ActionInvoked {
            notification_id: id,
            action_key: "default".into(),
        })
        .unwrap()
        .unwrap();
    registry.commit(binding("new"), id);
    let new = registry
        .freeze(&SenderEvent::ActionInvoked {
            notification_id: id,
            action_key: "default".into(),
        })
        .unwrap()
        .unwrap();
    registry.freeze(&SenderEvent::NotificationClosed {
        notification_id: id,
        reason: canopa_notify::SenderCloseReason::Closed,
    });
    assert!(registry.0.lock().unwrap().by_internal.is_empty());
    old.emit(&fixture.server).await.unwrap();
    new.emit(&fixture.server).await.unwrap();
    for (target, token) in [("old", Some("old-input")), ("new", None)] {
        let message = tokio::time::timeout(std::time::Duration::from_secs(3), signals.next())
            .await
            .unwrap()
            .unwrap();
        let (_, _, _, values): (String, String, String, Vec<OwnedValue>) =
            message.body().deserialize().unwrap();
        assert_eq!(<&str>::try_from(&values[0]).unwrap(), target);
        let platform =
            HashMap::<String, OwnedValue>::try_from(values[1].try_clone().unwrap()).unwrap();
        assert_eq!(
            platform
                .get("activation-token")
                .map(|value| <&str>::try_from(value).unwrap()),
            token
        );
    }
    fixture.finish().await;
}

#[test]
fn descriptor_reads_are_sealed_bounded_and_independent_of_shared_seek_position() {
    use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, memfd_create};
    use std::io::Write;
    let mut file =
        std::fs::File::from(memfd_create("canopa-portal-test", MemfdFlags::ALLOW_SEALING).unwrap());
    file.write_all(b"RIFFxxxxWAVEfixture").unwrap();
    let payload = || {
        let owned_fd: std::os::fd::OwnedFd = file.try_clone().unwrap().into();
        let fd = zbus::zvariant::Fd::from(owned_fd);
        HashMap::from([(
            "sound".into(),
            Value::from(("file-descriptor", Value::from(fd)))
                .try_to_owned()
                .unwrap(),
        )])
    };
    let identity = || Identity {
        app_id: "org.canopa.Sound".into(),
        id: "sound".into(),
    };
    assert!(intake::prepare(identity(), payload()).is_err());
    fcntl_add_seals(
        &file,
        SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW,
    )
    .unwrap();
    let (input, _) = intake::prepare(identity(), payload()).unwrap();
    drop(file);
    assert_eq!(
        input.sound,
        Some(canopa_notify::NotificationSound::Encoded(
            b"RIFFxxxxWAVEfixture".to_vec()
        ))
    );
}

#[test]
fn malformed_resource_shapes_and_descriptor_targets_are_rejected_without_panicking() {
    let identity = || Identity {
        app_id: "org.canopa.Invalid".into(),
        id: "one".into(),
    };
    for value in [
        owned(("file-descriptor",)),
        owned(("themed", "not-a-variant")),
        owned(("themed", Value::from(vec!["icon"]), 7u32)),
        owned(("bytes", Value::from(vec![1u8, 2, 3]))),
    ] {
        assert!(intake::prepare(identity(), HashMap::from([("icon".into(), value)])).is_err());
    }
    let fd = rustix::fs::memfd_create("target", rustix::fs::MemfdFlags::ALLOW_SEALING).unwrap();
    assert!(
        intake::prepare(
            identity(),
            HashMap::from([(
                "default-action-target".into(),
                owned(zbus::zvariant::Fd::from(fd))
            )])
        )
        .is_err()
    );
}

type ApplicationCall = (String, Vec<OwnedValue>, HashMap<String, OwnedValue>);
struct RecordingApplication(tokio::sync::mpsc::UnboundedSender<ApplicationCall>);

#[zbus::interface(name = "org.freedesktop.Application")]
impl RecordingApplication {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        self.0
            .send((String::new(), Vec::new(), platform_data))
            .unwrap();
    }

    fn activate_action(
        &self,
        action_name: String,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        self.0
            .send((action_name, parameter, platform_data))
            .unwrap();
    }
}

#[tokio::test]
async fn exported_and_default_activation_use_standard_application_methods_with_typed_parameters() {
    let fixture = Fixture::new("Application").await;
    let name = format!("org.canopa.Callback.p{}", std::process::id());
    let path = format!("/{}", name.replace('.', "/"));
    let (sent, mut calls) = tokio::sync::mpsc::unbounded_channel();
    let application = zbus::connection::Builder::session()
        .unwrap()
        .name(name.clone())
        .unwrap()
        .serve_at(path.as_str(), RecordingApplication(sent))
        .unwrap()
        .build()
        .await
        .unwrap();
    fixture
        .add(
            &name,
            "exported",
            HashMap::from([
                ("title".into(), owned("Callback")),
                ("default-action".into(), owned("app.open")),
                ("default-action-target".into(), owned(("typed", 42i32))),
            ]),
        )
        .await
        .unwrap();
    let id = fixture.daemon.snapshot().await.unwrap().active[0].id;
    fixture
        .daemon
        .invoke_action_with_activation_token(id, "default", Some("input-token".into()))
        .await
        .unwrap();
    let (action, parameters, platform) =
        tokio::time::timeout(std::time::Duration::from_secs(3), calls.recv())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(action, "open");
    assert_eq!(parameters.len(), 1);
    assert_eq!(
        <(String, i32)>::try_from(parameters.into_iter().next().unwrap()).unwrap(),
        ("typed".into(), 42)
    );
    assert_eq!(
        <&str>::try_from(&platform["activation-token"]).unwrap(),
        "input-token"
    );
    fixture
        .add(&name, "implicit", values_for("Open the application"))
        .await
        .unwrap();
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    let id = snapshot
        .active
        .iter()
        .find(|thread| thread.accepted_notification.summary == "Open the application")
        .unwrap()
        .id;
    fixture
        .daemon
        .invoke_action_with_activation_token(id, "default", None)
        .await
        .unwrap();
    let (action, parameters, platform) =
        tokio::time::timeout(std::time::Duration::from_secs(3), calls.recv())
            .await
            .unwrap()
            .unwrap();
    assert!(action.is_empty() && parameters.is_empty() && platform.is_empty());
    fixture
        .add(
            "org.canopa.MissingApplication",
            "missing",
            values_for("Unavailable application"),
        )
        .await
        .unwrap();
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    let id = snapshot
        .active
        .iter()
        .find(|thread| thread.accepted_notification.summary == "Unavailable application")
        .unwrap()
        .id;
    fixture
        .daemon
        .invoke_action_with_activation_token(id, "default", Some("unused-token".into()))
        .await
        .unwrap();
    // A failed activation is consumed, not retained for another application's click.
    fixture
        .add(
            &name,
            "after-failure",
            values_for("Later application activation"),
        )
        .await
        .unwrap();
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    let id = snapshot
        .active
        .iter()
        .find(|thread| thread.accepted_notification.summary == "Later application activation")
        .unwrap()
        .id;
    fixture
        .daemon
        .invoke_action_with_activation_token(id, "default", None)
        .await
        .unwrap();
    let (_, _, platform) = tokio::time::timeout(std::time::Duration::from_secs(3), calls.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(platform.is_empty());
    assert!(fixture.daemon.sender_publication_error().is_none());
    fixture.finish().await;
    application.close().await.unwrap();
}

#[tokio::test]
async fn admitted_add_survives_reply_loss_and_shutdown_drains_before_stopping() {
    let fixture = Fixture::new("Drain").await;
    let (service, handle) = PortalServer::start(fixture.daemon.clone(), Registry::default());
    let (reply, result) = tokio::sync::oneshot::channel();
    drop(result);
    service
        .admit(Operation::Add(
            Identity {
                app_id: "org.canopa.Detached".into(),
                id: "one".into(),
            },
            values_for("Kept after caller loss"),
            reply,
            Instant::now(),
        ))
        .unwrap();
    handle.shutdown().await.unwrap();
    assert_eq!(
        fixture.daemon.snapshot().await.unwrap().active[0]
            .accepted_notification
            .summary,
        "Kept after caller loss"
    );
    let (reply, _) = tokio::sync::oneshot::channel();
    assert!(
        service
            .admit(Operation::Remove(
                Identity {
                    app_id: "org.canopa.Detached".into(),
                    id: "one".into()
                },
                reply
            ))
            .is_err()
    );
    fixture.finish().await;
}
