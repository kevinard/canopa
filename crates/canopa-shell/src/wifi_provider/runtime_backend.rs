//! Live control selection from the same integration policy as network status.
use super::*;
use crate::network_integration::{Bundle, Kind, Owner, Watch};

#[derive(Clone, Copy, Default)]
enum Policy {
    #[default]
    Auto,
    Disabled,
    Only(Kind),
}
impl Policy {
    fn selected(self, bundle: Bundle) -> Option<Owner> {
        bundle.control.filter(|owner| match self {
            Self::Auto => true,
            Self::Disabled => false,
            Self::Only(kind) => kind == owner.kind,
        })
    }
}

pub async fn choose_backend() -> Option<RuntimeBackend> {
    let conn = zbus::Connection::system().await.ok()?;
    let policy = match std::env::var("CANOPA_WIFI_PROVIDER_BACKEND")
        .or_else(|_| std::env::var("NIRIBAR_WIFI_PROVIDER_BACKEND"))
        .as_deref()
    {
        Ok("none") => Policy::Disabled,
        Ok("networkmanager") => Policy::Only(Kind::NetworkManager),
        Ok("iwd") => Policy::Only(Kind::Iwd),
        _ => Policy::Auto,
    };
    RuntimeBackend::new(conn, policy).await.ok()
}

pub struct RuntimeBackend {
    conn: zbus::Connection,
    watch: Watch,
    policy: Policy,
    selected: Option<Owner>,
    adapter: Option<Adapter>,
    cleanup: cleanup::Cleanup,
}
impl RuntimeBackend {
    async fn new(conn: zbus::Connection, policy: Policy) -> zbus::Result<Self> {
        let watch = Watch::new(&conn).await?;
        Ok(Self {
            conn,
            watch,
            policy,
            selected: None,
            adapter: None,
            cleanup: cleanup::Cleanup::default(),
        })
    }
    pub fn cleanup(&self) -> cleanup::Cleanup {
        self.cleanup.clone()
    }

    async fn reconcile(&mut self) -> Result<(), WifiBackendError> {
        let selected = self.policy.selected(
            Bundle::resolve(&self.conn)
                .await
                .map_err(connection_error)?,
        );
        if selected == self.selected {
            return Ok(());
        }
        self.adapter = None;
        self.selected = None;
        let Some(owner) = selected else {
            return Ok(());
        };
        let adapter = match owner.kind {
            Kind::NetworkManager => Adapter::NetworkManager(NetworkManagerBackend {
                owner: owner.unique.clone(),
                targets: Mutex::default(),
                conn: self.conn.clone(),
                cleanup: self.cleanup.clone(),
            }),
            Kind::Iwd => Adapter::Iwd(IwdBackend {
                owner: owner.unique.clone(),
                targets: Mutex::default(),
                conn: self.conn.clone(),
                cleanup: self.cleanup.clone(),
            }),
            Kind::Networkd => unreachable!("networkd has no Wi-Fi controls"),
        };
        self.selected = Some(owner);
        self.adapter = Some(adapter);
        Ok(())
    }

    async fn operate(
        &mut self,
        scan: bool,
        request: Option<WifiConnectRequest>,
    ) -> Result<WifiSnapshot, WifiBackendError> {
        self.reconcile().await?;
        let Some(adapter) = self.adapter.as_mut() else {
            return if request.is_some() {
                Err(identity::changed())
            } else {
                Ok(WifiSnapshot {
                    device_id: None,
                    networks: vec![],
                    scanning: false,
                })
            };
        };
        let selected = self.selected.clone();
        let policy = self.policy;
        let mut watch = self.watch.clone();
        let work = async {
            if let Some(request) = request {
                adapter.connect(request).await
            } else if scan {
                adapter.scan().await
            } else {
                adapter.initial_snapshot().await
            }
        };
        tokio::pin!(work);
        loop {
            tokio::select! {
                biased;
                change = watch.changed() => {
                    if change.is_err() || policy.selected(watch.current()) != selected { return Err(identity::changed()); }
                }
                result = &mut work => return result,
            }
        }
    }

    pub async fn changes(&self) -> zbus::Result<tokio::sync::mpsc::Receiver<()>> {
        let mut signals = futures_util::stream::SelectAll::new();
        for service in [
            crate::network_integration::NM,
            crate::network_integration::IWD,
        ] {
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(service)?
                .build();
            signals.push(zbus::MessageStream::for_match_rule(rule, &self.conn, Some(64)).await?);
        }
        let mut watch = self.watch.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tx.closed() => break,
                    change = watch.changed() => { if change.is_err() { break; } let _ = tx.try_send(()); }
                    signal = signals.next() => { if !matches!(signal, Some(Ok(_))) { break; } let _ = tx.try_send(()); }
                }
            }
        });
        Ok(rx)
    }
}
impl WifiControlBackend for RuntimeBackend {
    fn name(&self) -> BackendName {
        self.adapter
            .as_ref()
            .map_or(BackendName::Unavailable, WifiControlBackend::name)
    }
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            can_scan_while_connecting: false,
        }
    }
    fn initial_snapshot(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
        Box::pin(self.operate(false, None))
    }
    fn scan(&mut self) -> WifiBackendFuture<'_, WifiSnapshot> {
        Box::pin(self.operate(true, None))
    }
    fn connect(&mut self, request: WifiConnectRequest) -> WifiBackendFuture<'_, WifiSnapshot> {
        Box::pin(self.operate(false, Some(request)))
    }
}

pub(super) async fn verify_control(
    conn: &zbus::Connection,
    kind: Kind,
    expected: &str,
) -> Result<(), WifiBackendError> {
    if Bundle::resolve(conn)
        .await
        .map_err(|_| identity::changed())?
        .control
        .is_some_and(|owner| owner.kind == kind && owner.unique == expected)
    {
        Ok(())
    } else {
        Err(identity::changed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi_provider::backends::tests::{IwdFixture, NmFixture};

    async fn update(changes: &mut tokio::sync::mpsc::Receiver<()>) {
        tokio::time::timeout(std::time::Duration::from_secs(3), changes.recv())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn open_provider_tracks_the_status_bundle_and_retires_credentials() {
        let fixture = IwdFixture::new().await;
        let client = fixture.bus.connect().await;
        let backend = RuntimeBackend::new(client.clone(), Policy::Auto)
            .await
            .unwrap();
        let mut changes = backend.changes().await.unwrap();
        let mut provider = WifiPanelProvider::new(backend);
        provider.initial_content().await.unwrap();
        assert_eq!(provider.backend_name(), BackendName::Iwd);
        let old = provider.snapshot.networks[0].id.clone();
        provider
            .handle_event(ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: old,
            })
            .await
            .unwrap();
        assert!(provider.credential_prompt.is_some());
        let nm = NmFixture::on_bus(&fixture.bus, 2, false).await;
        update(&mut changes).await;
        provider.synchronize().await.unwrap();
        assert!(provider.credential_prompt.is_none());
        assert_eq!(provider.backend_name(), BackendName::NetworkManager);
        let bundle = Bundle::resolve(&client).await.unwrap();
        assert_eq!(bundle.status, bundle.control);
        assert_eq!(
            bundle.control.unwrap().unique,
            nm._service.unique_name().unwrap().as_str()
        );
        let mut restricted = RuntimeBackend::new(client.clone(), Policy::Only(Kind::Iwd))
            .await
            .unwrap();
        assert!(
            restricted
                .initial_snapshot()
                .await
                .unwrap()
                .device_id
                .is_none()
        );
        let mut new_panel = RuntimeBackend::new(client.clone(), Policy::Auto)
            .await
            .unwrap();
        assert!(
            new_panel
                .initial_snapshot()
                .await
                .unwrap()
                .device_id
                .is_some()
        );
        assert_eq!(new_panel.name(), BackendName::NetworkManager);
        nm._service
            .release_name(crate::network_integration::NM)
            .await
            .unwrap();
        update(&mut changes).await;
        provider.synchronize().await.unwrap();
        assert_eq!(provider.backend_name(), BackendName::Iwd);
        fixture
            .service
            .release_name(crate::network_integration::IWD)
            .await
            .unwrap();
        update(&mut changes).await;
        provider.synchronize().await.unwrap();
        assert_eq!(provider.backend_name(), BackendName::Unavailable);
        assert!(provider.snapshot.device_id.is_none());
        fixture
            .service
            .request_name(crate::network_integration::IWD)
            .await
            .unwrap();
        update(&mut changes).await;
        provider.synchronize().await.unwrap();
        assert_eq!(provider.backend_name(), BackendName::Iwd);
    }

    #[tokio::test]
    async fn ownership_change_cancels_pending_activation_and_cleans_original_daemon() {
        let fixture = NmFixture::new(1, false).await;
        let conn = fixture._bus.as_ref().unwrap().connect().await;
        let mut backend = RuntimeBackend::new(conn, Policy::Auto).await.unwrap();
        let snapshot = backend.initial_snapshot().await.unwrap();
        let request = super::super::tests::selected_request(snapshot, Some("test-passphrase"));
        let cleanup = backend.cleanup();
        let work = backend.connect(request);
        tokio::pin!(work);
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            tokio::select! {
                result = &mut work => panic!("activation ended early: {result:?}"),
                _ = async { while !fixture.observed.load(std::sync::atomic::Ordering::SeqCst) { tokio::task::yield_now().await; } } => {}
            }
            fixture._service.release_name(crate::network_integration::NM).await.unwrap();
            assert_eq!(work.await.unwrap_err().category(), WifiBackendErrorCategory::TargetChanged);
        }).await.unwrap();
        cleanup.finish().await;
        assert!(fixture.deleted.load(std::sync::atomic::Ordering::SeqCst));
        assert!(
            fixture
                .deactivated
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }
}
