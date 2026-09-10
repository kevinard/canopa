use super::*;
use canopa_notify::{
    ActivationTokenFuture, ActivationTokenProvider, DaemonOptions, NotificationSoundPlayback,
    NotificationSoundSettings, SoundPresenter,
};
use futures_util::StreamExt;
use std::collections::VecDeque;

struct ServerFixture {
    temp: tempfile::TempDir,
    daemon: Daemon,
    _server: zbus::Connection,
    proxy: zbus::Proxy<'static>,
}

impl ServerFixture {
    async fn new(suffix: &str, ids: NotificationSenderIds) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let bus_name = test_bus_name(suffix);
        let server = zbus::connection::Builder::session()
            .unwrap()
            .name(bus_name.clone())
            .unwrap()
            .build()
            .await
            .unwrap();
        attach_with_ids(server.clone(), daemon.clone(), ids)
            .await
            .unwrap();
        let proxy = zbus::Proxy::new_owned(
            zbus::Connection::session().await.unwrap(),
            bus_name,
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();
        Self {
            temp,
            daemon,
            _server: server,
            proxy,
        }
    }

    async fn notify(&self, replaces_id: u32, summary: &str) -> zbus::Result<u32> {
        self.proxy
            .call(
                "Notify",
                &(
                    "Test",
                    replaces_id,
                    "",
                    summary,
                    "",
                    Vec::<String>::new(),
                    HashMap::<String, OwnedValue>::new(),
                    0i32,
                ),
            )
            .await
    }

    async fn close(&self, id: u32) {
        self.proxy
            .call::<_, _, ()>("CloseNotification", &(id,))
            .await
            .unwrap();
    }
}

fn assert_dbus_error<T: std::fmt::Debug>(result: zbus::Result<T>, expected_name: &str) {
    let error = result.unwrap_err();
    assert!(
        matches!(&error, zbus::Error::MethodError(name, _, _) if name.as_str() == expected_name),
        "expected {expected_name}, got {error:?}"
    );
}

#[tokio::test]
async fn automatic_ids_skip_retired_caller_tokens() {
    let fixture = ServerFixture::new("RetiredAllocation", NotificationSenderIds::new()).await;
    assert_eq!(fixture.notify(2, "Caller token").await.unwrap(), 2);
    fixture.close(2).await;
    assert_eq!(fixture.notify(0, "Generated").await.unwrap(), 1);
    fixture.close(1).await;
    assert_eq!(fixture.notify(0, "Unrelated").await.unwrap(), 3);
    assert_dbus_error(
        fixture.notify(2, "Retired").await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    assert!(
        fixture
            .proxy
            .call::<_, _, ()>("CloseNotification", &(2u32,))
            .await
            .is_err()
    );
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(
        snapshot.active[0].accepted_notification.summary,
        "Unrelated"
    );
}

#[tokio::test]
async fn exhausted_id_space_fails_promptly_without_wrapping_or_blocking_live_replacement() {
    let fixture = ServerFixture::new("IdExhaustion", NotificationSenderIds::for_test(3, 2)).await;
    assert_eq!(fixture.notify(0, "First").await.unwrap(), 1);
    fixture.close(1).await;
    assert_eq!(fixture.notify(0, "Live").await.unwrap(), 2);
    assert_eq!(fixture.notify(0, "Last").await.unwrap(), 3);
    fixture.close(3).await;
    let before = fixture.daemon.snapshot().await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        fixture.notify(0, "Overflow"),
    )
    .await
    .expect("allocator did not terminate");
    assert_dbus_error(result, "org.freedesktop.DBus.Error.LimitsExceeded");
    assert_eq!(fixture.daemon.snapshot().await.unwrap(), before);
    assert_eq!(fixture.notify(2, "Still live").await.unwrap(), 2);
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(snapshot.active[0].revision, 2);
    fixture.close(2).await;
    assert_dbus_error(
        fixture.notify(2, "Retired").await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
}

#[tokio::test]
async fn sparse_id_budget_merges_ranges_and_keeps_automatic_allocation_available() {
    let fixture = ServerFixture::new("SparseIds", NotificationSenderIds::for_test(20, 2)).await;
    // Blank requests immediately retire their IDs, so these are not live-map limits.
    for id in [4, 6] {
        assert_eq!(fixture.notify(id, "").await.unwrap(), id);
    }
    assert_dbus_error(
        fixture.notify(9, "Rejected").await,
        "org.freedesktop.DBus.Error.LimitsExceeded",
    );
    assert!(fixture.daemon.snapshot().await.unwrap().history.is_empty());
    assert_eq!(fixture.notify(0, "").await.unwrap(), 1);
    // Joining two sparse intervals frees budget for a previously rejected token.
    assert_eq!(fixture.notify(5, "").await.unwrap(), 5);
    assert_eq!(fixture.notify(9, "").await.unwrap(), 9);
    assert_eq!(fixture.notify(3, "").await.unwrap(), 3);
    assert_eq!(fixture.notify(0, "").await.unwrap(), 2);
    assert_eq!(fixture.notify(0, "").await.unwrap(), 7);
    assert_eq!(fixture.notify(0, "").await.unwrap(), 8);
    for id in 1..=9 {
        assert_dbus_error(
            fixture.notify(id, "Retired").await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    assert_eq!(fixture.notify(0, "Next").await.unwrap(), 10);
}

#[tokio::test]
async fn upper_boundary_caller_ids_merge_without_overflow_or_reuse() {
    let fixture = ServerFixture::new("UpperIds", NotificationSenderIds::new()).await;
    assert_eq!(
        fixture.notify(u32::MAX, "Last token").await.unwrap(),
        u32::MAX
    );
    assert_eq!(
        fixture.notify(u32::MAX, "Live replacement").await.unwrap(),
        u32::MAX
    );
    fixture.close(u32::MAX).await;
    for id in [u32::MAX - 2, u32::MAX - 1] {
        assert_eq!(fixture.notify(id, "").await.unwrap(), id);
    }
    for id in [u32::MAX - 2, u32::MAX - 1, u32::MAX] {
        assert_dbus_error(
            fixture.notify(id, "Retired").await,
            "org.freedesktop.DBus.Error.InvalidArgs",
        );
    }
    assert_eq!(fixture.notify(0, "First automatic").await.unwrap(), 1);
}

#[tokio::test]
async fn failed_storage_receipts_do_not_consume_generated_or_caller_ids() {
    let fixture = ServerFixture::new("FailedIdReceipt", NotificationSenderIds::new()).await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(fixture.temp.path().join("history.db")),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_receipt BEFORE INSERT ON notification_threads BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END")
        .execute(&pool).await.unwrap();
    for id in [0, 9] {
        assert_dbus_error(
            fixture.notify(id, "Cannot commit").await,
            "org.freedesktop.DBus.Error.Failed",
        );
    }
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert!(snapshot.history.is_empty());
    sqlx::query("DROP TRIGGER fail_receipt")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(fixture.notify(9, "Retry token").await.unwrap(), 9);
    assert_eq!(fixture.notify(0, "Retry automatic").await.unwrap(), 1);
    assert_eq!(fixture.daemon.snapshot().await.unwrap().active.len(), 2);
    fixture.close(9).await;
    assert_dbus_error(
        fixture.notify(9, "Retired").await,
        "org.freedesktop.DBus.Error.InvalidArgs",
    );
    pool.close().await;
}

#[tokio::test]
async fn client_disconnect_preserves_session_global_live_replacement() {
    let fixture = ServerFixture::new("DisconnectedSender", NotificationSenderIds::new()).await;
    let id = fixture.notify(0, "Original sender").await.unwrap();
    let thread_id = fixture.daemon.snapshot().await.unwrap().active[0].id;
    let destination = fixture.proxy.destination().to_owned();
    fixture.proxy.connection().clone().close().await.unwrap();
    let proxy = zbus::Proxy::new_owned(
        zbus::Connection::session().await.unwrap(),
        destination,
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let replaced: u32 = proxy
        .call(
            "Notify",
            &(
                "Different claimed app",
                id,
                "",
                "Replacement",
                "",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                0i32,
            ),
        )
        .await
        .unwrap();
    assert_eq!(replaced, id);
    let snapshot = fixture.daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(snapshot.active[0].id, thread_id);
    assert_eq!(snapshot.active[0].revision, 2);
    assert_eq!(
        snapshot.active[0].accepted_notification.app_identity,
        "Different claimed app"
    );
    proxy
        .call::<_, _, ()>("CloseNotification", &(id,))
        .await
        .unwrap();
    assert!(fixture.daemon.snapshot().await.unwrap().active.is_empty());
}

#[derive(Clone, Debug)]
struct FakeActivationTokenProvider {
    tokens: Arc<Mutex<VecDeque<String>>>,
}

impl FakeActivationTokenProvider {
    fn new(tokens: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(tokens.into_iter().map(Into::into).collect())),
        }
    }
}

impl ActivationTokenProvider for FakeActivationTokenProvider {
    fn available(&self) -> bool {
        true
    }

    fn activation_token(&self, _app_identity: String) -> ActivationTokenFuture {
        let token = self.tokens.lock().unwrap().pop_front();
        Box::pin(async move { token })
    }
}

#[derive(Clone, Debug, Default)]
struct AvailableSoundPresenter;

impl SoundPresenter for AvailableSoundPresenter {
    fn apply_sound_settings(&self, _settings: NotificationSoundSettings) -> anyhow::Result<()> {
        Ok(())
    }

    fn play_notification_sound(&self, _playback: NotificationSoundPlayback) -> anyhow::Result<()> {
        Ok(())
    }

    fn stop_all_sounds(&self) {}

    fn supports_sound(&self) -> bool {
        true
    }
}

#[derive(Clone, Default)]
struct RecordingBannerPresenter {
    updates: Arc<Mutex<Vec<Vec<canopa_notify::BannerView>>>>,
}

impl RecordingBannerPresenter {
    fn displayed(&self, notification_id: canopa_notify::InternalNotificationId) -> bool {
        self.updates.lock().unwrap().iter().any(|banners| {
            banners
                .iter()
                .any(|banner| banner.notification_id == notification_id)
        })
    }
}

impl canopa_notify::BannerPresenter for RecordingBannerPresenter {
    fn replace_visible_banners(
        &self,
        banners: Vec<canopa_notify::BannerView>,
    ) -> anyhow::Result<()> {
        self.updates.lock().unwrap().push(banners);
        Ok(())
    }
}

#[tokio::test]
async fn dbus_bulk_clear_publishes_all_130_closures() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let bus_name = test_bus_name("BulkSignals");
    let _server = start_notification_server(bus_name.clone(), daemon.clone())
        .await
        .unwrap();
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closures = proxy.receive_signal("NotificationClosed").await.unwrap();
    let mut expected = std::collections::BTreeSet::new();
    for _ in 0..130 {
        let hints = HashMap::from([("transient".to_string(), OwnedValue::from(true))]);
        let id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Burst",
                    0u32,
                    "",
                    "Active",
                    "",
                    Vec::<String>::new(),
                    hints,
                    0i32,
                ),
            )
            .await
            .unwrap();
        expected.insert(id);
    }
    daemon.clear_active_notifications().await.unwrap();
    let mut received = std::collections::BTreeSet::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while received.len() < expected.len() {
            let signal = closures.next().await.unwrap();
            let (id, reason): (u32, u32) = signal.body().deserialize().unwrap();
            assert_eq!(reason, 2);
            assert!(received.insert(id), "duplicate closure");
        }
    })
    .await;
    assert_eq!(received, expected, "required sender signals were lost");
}

#[tokio::test]
async fn notification_server_emits_activation_token_before_action_invoked() {
    let temp = tempfile::tempdir().unwrap();
    let bus_name = test_bus_name("ActivationToken");
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_activation_token_provider(FakeActivationTokenProvider::new(["token-1"])),
    )
    .await
    .unwrap();
    let _connection = start_notification_server(bus_name.clone(), daemon.clone())
        .await
        .unwrap();
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();
    assert_eq!(
        capabilities,
        vec![
            "actions",
            "action-icons",
            "body",
            "body-markup",
            "icon-static",
            "activation-token",
        ]
    );
    let mut token_signals = proxy.receive_signal("ActivationToken").await.unwrap();
    let mut action_signals = proxy.receive_signal("ActionInvoked").await.unwrap();

    let sender_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Calendar",
                0u32,
                "",
                "Meeting",
                "Starting soon",
                vec!["open".to_string(), "Open".to_string()],
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();
    let notification_id = daemon.snapshot().await.unwrap().active[0].id;

    daemon.invoke_action(notification_id, "open").await.unwrap();

    let token_signal =
        tokio::time::timeout(std::time::Duration::from_secs(1), token_signals.next())
            .await
            .unwrap()
            .unwrap();
    let (token_id, activation_token): (u32, String) = token_signal.body().deserialize().unwrap();
    assert_eq!(token_id, sender_id);
    assert_eq!(activation_token, "token-1");

    let action_signal =
        tokio::time::timeout(std::time::Duration::from_secs(1), action_signals.next())
            .await
            .unwrap()
            .unwrap();
    let (action_id, action_key): (u32, String) = action_signal.body().deserialize().unwrap();
    assert_eq!(action_id, sender_id);
    assert_eq!(action_key, "open");
}

#[tokio::test]
async fn notification_server_expires_notification_after_banner_display() {
    let temp = tempfile::tempdir().unwrap();
    let bus_name = test_bus_name("DisplayedTimeout");
    let presenter = RecordingBannerPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let _server_connection = start_notification_server(bus_name.clone(), daemon.clone())
        .await
        .unwrap();
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let sender_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Calendar",
                0u32,
                "",
                "Displayed timeout",
                "Starting soon",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                100i32,
            ),
        )
        .await
        .unwrap();
    let internal_id = daemon.snapshot().await.unwrap().active[0].id;
    assert!(presenter.displayed(internal_id));

    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!((closed_id, reason), (sender_id, 1));
    assert!(daemon.visible_banners().await.is_empty());

    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert!(snapshot.history.iter().any(|thread| {
        thread.id == internal_id && thread.status == canopa_notify::NotificationStatus::Expired
    }));
    let close_result: zbus::Result<()> = proxy.call("CloseNotification", &(sender_id,)).await;
    assert!(close_result.is_err());
}

#[tokio::test]
async fn notification_server_advertises_sound_when_runtime_supports_sound() {
    let temp = tempfile::tempdir().unwrap();
    let bus_name = test_bus_name("SoundCapability");
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            canopa_notify::NoopBannerPresenter,
        )
        .with_sound_presenter(AvailableSoundPresenter),
    )
    .await
    .unwrap();
    let _connection = start_notification_server(bus_name.clone(), daemon)
        .await
        .unwrap();
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();

    assert_eq!(
        capabilities,
        vec![
            "actions",
            "action-icons",
            "body",
            "body-markup",
            "icon-static",
            "sound",
        ]
    );
}

#[tokio::test]
async fn capability_classification_is_exact_for_each_runtime_mode() {
    let temp = tempfile::tempdir().unwrap();
    let headless = Daemon::open(DaemonOptions::headless(temp.path().join("headless.db")))
        .await
        .unwrap();
    let graphical = Daemon::open(DaemonOptions::graphical(
        temp.path().join("graphical.db"),
        canopa_notify::NoopBannerPresenter,
    ))
    .await
    .unwrap();
    let sound_enabled = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("sound.db"),
            canopa_notify::NoopBannerPresenter,
        )
        .with_sound_presenter(AvailableSoundPresenter),
    )
    .await
    .unwrap();
    let activation_token_capable = Daemon::open(
        DaemonOptions::headless(temp.path().join("activation.db"))
            .with_activation_token_provider(FakeActivationTokenProvider::new(["token"])),
    )
    .await
    .unwrap();
    let base = CapabilityAdvertisement {
        specification: vec![
            "actions",
            "action-icons",
            "body",
            "body-markup",
            "icon-static",
        ],
        extensions: vec![],
    };

    assert_eq!(capability_advertisement(&headless), base);
    assert_eq!(capability_advertisement(&graphical), base);
    assert_eq!(
        capability_advertisement(&sound_enabled),
        CapabilityAdvertisement {
            specification: vec![
                "actions",
                "action-icons",
                "body",
                "body-markup",
                "icon-static",
                "sound",
            ],
            extensions: vec![],
        }
    );
    assert_eq!(
        capability_advertisement(&activation_token_capable),
        CapabilityAdvertisement {
            specification: base.specification,
            extensions: vec!["activation-token"],
        }
    );
}

fn test_bus_name(suffix: &str) -> String {
    format!("org.canopa.NotifyUnitTest{}{}", std::process::id(), suffix)
}
