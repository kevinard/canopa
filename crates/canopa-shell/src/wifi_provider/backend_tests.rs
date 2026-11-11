//! The panel over the shared Wi-Fi backends, against fake daemons on a private bus.
use super::*;
use canopa_services::network::{
    bundle::{IWD, NM},
    wifi::backends::RuntimeBackend,
};
use canopa_services::testing::{TestBus, wifi};

async fn update(changes: &mut tokio::sync::mpsc::Receiver<()>) {
    tokio::time::timeout(std::time::Duration::from_secs(3), changes.recv())
        .await
        .unwrap()
        .unwrap();
}

async fn select_first(provider: &mut WifiPanelProvider<RuntimeBackend>) {
    let selected = provider.snapshot.networks[0].id.clone();
    provider
        .handle_event(ProviderEvent::SelectableListEntryActivated {
            element_id: "networks".into(),
            entry_id: selected,
        })
        .await
        .unwrap();
    assert!(provider.credential_prompt.is_some());
}

#[tokio::test]
async fn open_credential_prompt_is_retired_when_selected_network_changes() {
    let bus = TestBus::new();
    let iwd = wifi::Iwd::serve(&bus).await;
    let backend = RuntimeBackend::with_connection(bus.connect().await)
        .await
        .unwrap();
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    select_first(&mut provider).await;
    provider
        .handle_event(ProviderEvent::TextInputChanged {
            element_id: "credential".into(),
            value: "test-passphrase".into(),
        })
        .await
        .unwrap();
    iwd.rename_network("Other").await;
    let document = provider.synchronize().await.unwrap();
    assert!(provider.credential_prompt.is_none());
    assert!(!format!("{document:?}").contains("test-passphrase"));
    assert!(
        provider
            .handle_event(ProviderEvent::ButtonActivated {
                element_id: "connect".into()
            })
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!iwd.agent_registered());
}

#[tokio::test]
async fn open_provider_tracks_the_status_bundle_and_retires_credentials() {
    let bus = TestBus::new();
    let iwd = wifi::Iwd::serve(&bus).await;
    let backend = RuntimeBackend::with_connection(bus.connect().await)
        .await
        .unwrap();
    let mut changes = backend.changes().await.unwrap();
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    assert_eq!(provider.backend_name(), BackendName::Iwd);
    select_first(&mut provider).await;
    let nm = wifi::NetworkManager::serve(&bus, 2, false).await;
    update(&mut changes).await;
    provider.synchronize().await.unwrap();
    assert!(provider.credential_prompt.is_none());
    assert_eq!(provider.backend_name(), BackendName::NetworkManager);
    nm.service.release_name(NM).await.unwrap();
    update(&mut changes).await;
    provider.synchronize().await.unwrap();
    assert_eq!(provider.backend_name(), BackendName::Iwd);
    iwd.service.release_name(IWD).await.unwrap();
    update(&mut changes).await;
    provider.synchronize().await.unwrap();
    assert_eq!(provider.backend_name(), BackendName::Unavailable);
    assert!(provider.snapshot.device_id.is_none());
    iwd.service.request_name(IWD).await.unwrap();
    update(&mut changes).await;
    provider.synchronize().await.unwrap();
    assert_eq!(provider.backend_name(), BackendName::Iwd);
}
