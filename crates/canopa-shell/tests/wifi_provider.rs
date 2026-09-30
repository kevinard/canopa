use std::sync::{Arc, Mutex};

use canopa_panel_protocol::v1::{ExternalPanelElement, ExternalPanelSeverity};
use canopa_shell::wifi_provider::{
    BackendCapabilities, BackendName, WifiBackendAvailability, WifiBackendError,
    WifiBackendErrorCategory, WifiConnectRequest, WifiControlBackend, WifiNetwork,
    WifiPanelProvider, WifiSecurity, WifiSnapshot, select_backend,
};

#[derive(Clone)]
struct FakeBackend {
    calls: Arc<Mutex<Vec<&'static str>>>,
    initial_snapshot: WifiSnapshot,
    scan_snapshot: WifiSnapshot,
    connect_result: Result<WifiSnapshot, WifiBackendError>,
    connect_requests: Arc<Mutex<Vec<WifiConnectRequest>>>,
}

type FakeBackendFixture = (
    FakeBackend,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<Vec<WifiConnectRequest>>>,
);

impl FakeBackend {
    fn new(snapshot: WifiSnapshot) -> (Self, Arc<Mutex<Vec<&'static str>>>) {
        Self::with_scan(snapshot.clone(), snapshot)
    }

    fn with_scan(
        initial_snapshot: WifiSnapshot,
        scan_snapshot: WifiSnapshot,
    ) -> (Self, Arc<Mutex<Vec<&'static str>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
                initial_snapshot: initial_snapshot.clone(),
                scan_snapshot: scan_snapshot.clone(),
                connect_result: Ok(scan_snapshot),
                connect_requests: Arc::new(Mutex::new(Vec::new())),
            },
            calls,
        )
    }

    fn with_connect(
        initial_snapshot: WifiSnapshot,
        scan_snapshot: WifiSnapshot,
        connect_snapshot: WifiSnapshot,
    ) -> FakeBackendFixture {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let connect_requests = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
                initial_snapshot,
                scan_snapshot,
                connect_result: Ok(connect_snapshot),
                connect_requests: connect_requests.clone(),
            },
            calls,
            connect_requests,
        )
    }

    fn with_connect_error(
        initial_snapshot: WifiSnapshot,
        scan_snapshot: WifiSnapshot,
        connect_error: WifiBackendError,
    ) -> FakeBackendFixture {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let connect_requests = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
                initial_snapshot,
                scan_snapshot,
                connect_result: Err(connect_error),
                connect_requests: connect_requests.clone(),
            },
            calls,
            connect_requests,
        )
    }
}

impl WifiControlBackend for FakeBackend {
    fn name(&self) -> BackendName {
        BackendName::Iwd
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            can_scan_while_connecting: false,
        }
    }

    fn initial_snapshot<'a>(
        &'a mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WifiSnapshot, WifiBackendError>> + Send + 'a>,
    > {
        self.calls.lock().unwrap().push("initial_snapshot");
        let snapshot = self.initial_snapshot.clone();
        Box::pin(async move { Ok(snapshot) })
    }

    fn scan<'a>(
        &'a mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WifiSnapshot, WifiBackendError>> + Send + 'a>,
    > {
        self.calls.lock().unwrap().push("scan");
        let snapshot = self.scan_snapshot.clone();
        Box::pin(async move { Ok(snapshot) })
    }

    fn connect<'a>(
        &'a mut self,
        request: WifiConnectRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<WifiSnapshot, WifiBackendError>> + Send + 'a>,
    > {
        self.calls.lock().unwrap().push("connect");
        self.connect_requests.lock().unwrap().push(request);
        let result = self.connect_result.clone();
        Box::pin(async move { result })
    }
}

#[tokio::test]
async fn iwd_provider_initial_content_reports_active_network_without_scanning() {
    let (backend, calls) = FakeBackend::new(WifiSnapshot {
        networks: vec![WifiNetwork {
            id: "iwd:/station/0/Home".into(),
            label: "Home".into(),
            signal_strength: 82,
            security: WifiSecurity::Personal,
            known: true,
            active: true,
        }],
        scanning: false,
    });
    let mut provider = WifiPanelProvider::new(backend);

    let document = provider.initial_content().await.unwrap();

    assert_eq!(*calls.lock().unwrap(), vec!["initial_snapshot"]);
    assert_eq!(
        document.elements,
        vec![
            ExternalPanelElement::Status {
                title: "Wi-Fi".into(),
                message: "Connected to Home".into(),
                severity: ExternalPanelSeverity::Info,
            },
            ExternalPanelElement::Button {
                id: "refresh".into(),
                label: "Refresh".into(),
            },
            ExternalPanelElement::SelectableList {
                id: "networks".into(),
                entries: vec![canopa_panel_protocol::v1::ExternalSelectableEntry {
                    id: "iwd:/station/0/Home".into(),
                    label: "Home".into(),
                    description: Some("known secured 82%".into()),
                    disabled: false,
                    active: true,
                    status: Some("active".into()),
                }],
                allow_custom: true,
            },
        ]
    );
}

#[tokio::test]
async fn iwd_provider_scan_collapses_and_orders_wifi_network_rows() {
    let (backend, calls) = FakeBackend::with_scan(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![
                WifiNetwork {
                    id: "ap:guest".into(),
                    label: "Guest".into(),
                    signal_strength: 30,
                    security: WifiSecurity::Open,
                    known: false,
                    active: false,
                },
                WifiNetwork {
                    id: "ap:home".into(),
                    label: "Home".into(),
                    signal_strength: 60,
                    security: WifiSecurity::Personal,
                    known: true,
                    active: true,
                },
                WifiNetwork {
                    id: "ap:office".into(),
                    label: "Office".into(),
                    signal_strength: 45,
                    security: WifiSecurity::Personal,
                    known: true,
                    active: false,
                },
                WifiNetwork {
                    id: "ap:cafe-low".into(),
                    label: "Cafe".into(),
                    signal_strength: 30,
                    security: WifiSecurity::Open,
                    known: false,
                    active: false,
                },
                WifiNetwork {
                    id: "ap:cafe-high".into(),
                    label: "Cafe".into(),
                    signal_strength: 70,
                    security: WifiSecurity::Open,
                    known: false,
                    active: false,
                },
                WifiNetwork {
                    id: "ap:airport".into(),
                    label: "Airport".into(),
                    signal_strength: 80,
                    security: WifiSecurity::Open,
                    known: false,
                    active: false,
                },
            ],
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);

    provider.initial_content().await.unwrap();
    let document = provider.refresh().await.unwrap();

    assert_eq!(*calls.lock().unwrap(), vec!["initial_snapshot", "scan"]);
    let ExternalPanelElement::SelectableList { entries, .. } = &document.elements[2] else {
        panic!("expected selectable list");
    };
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Home", "Office", "Airport", "Cafe", "Guest"]
    );
    let cafe = entries.iter().find(|entry| entry.label == "Cafe").unwrap();
    assert_eq!(cafe.id, "ap:cafe-high");
    assert_eq!(cafe.description.as_deref(), Some("open 70%"));
}

#[tokio::test]
async fn known_wifi_row_activation_streams_connecting_then_active_content() {
    let (backend, calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:home".into(),
                label: "Home".into(),
                signal_strength: 81,
                security: WifiSecurity::Personal,
                known: true,
                active: false,
            }],
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:home".into(),
                label: "Home".into(),
                signal_strength: 81,
                security: WifiSecurity::Personal,
                known: true,
                active: true,
            }],
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: "ap:home".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        *calls.lock().unwrap(),
        vec!["initial_snapshot", "scan", "connect"]
    );
    assert_eq!(
        requests.lock().unwrap()[0].network_id.as_deref(),
        Some("ap:home")
    );
    assert_eq!(requests.lock().unwrap()[0].credential, None);

    let ExternalPanelElement::SelectableList {
        entries: connecting,
        ..
    } = documents[0].elements.iter().find(|element| matches!(element, ExternalPanelElement::SelectableList { id, .. } if id == "networks")).expect("connecting view has network list")
    else {
        panic!("expected connecting list");
    };
    assert_eq!(connecting[0].status.as_deref(), Some("connecting"));
    assert!(connecting[0].disabled);
    assert!(!documents[0].elements.iter().any(
        |element| matches!(element, ExternalPanelElement::Button { id, .. } if id == "refresh")
    ));

    let ExternalPanelElement::SelectableList {
        entries: active, ..
    } = &documents[1].elements[2]
    else {
        panic!("expected active list");
    };
    assert_eq!(active[0].status.as_deref(), Some("active"));
    assert!(active[0].active);
}

#[tokio::test]
async fn rejected_network_credential_keeps_prompt_and_clears_secret() {
    let secret = "wrong credential";
    let (backend, _calls, requests) = FakeBackend::with_connect_error(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:cafe".into(),
                label: "Cafe".into(),
                signal_strength: 64,
                security: WifiSecurity::Personal,
                known: false,
                active: false,
            }],
            scanning: false,
        },
        WifiBackendError::new(WifiBackendErrorCategory::CredentialRejected),
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();
    provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: "ap:cafe".into(),
            },
        )
        .await
        .unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::TextInputSubmitted {
                element_id: "credential".into(),
                value: secret.into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        requests.lock().unwrap()[0].credential.as_deref(),
        Some(secret)
    );
    assert_eq!(
        documents[0].elements,
        vec![
            ExternalPanelElement::Status {
                title: "Network Credential".into(),
                message: "Enter credential for Cafe".into(),
                severity: ExternalPanelSeverity::Info,
            },
            ExternalPanelElement::TextInput {
                id: "credential".into(),
                label: "Network credential".into(),
                value: String::new(),
                secret: true,
                error: Some("Credential rejected".into()),
            },
            ExternalPanelElement::Button {
                id: "connect".into(),
                label: "Connect".into(),
            },
        ]
    );
    assert!(!document_text(&documents[0]).contains(secret));
}

#[tokio::test]
async fn custom_list_value_starts_trimmed_hidden_network_prompt() {
    let (backend, calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListCustomValueSubmitted {
                element_id: "networks".into(),
                value: "  Hidden Cafe  ".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(*calls.lock().unwrap(), vec!["initial_snapshot", "scan"]);
    assert!(requests.lock().unwrap().is_empty());
    assert_eq!(
        documents[0].elements,
        vec![
            ExternalPanelElement::Status {
                title: "Network Credential".into(),
                message: "Enter credential for Hidden Cafe".into(),
                severity: ExternalPanelSeverity::Info,
            },
            ExternalPanelElement::TextInput {
                id: "credential".into(),
                label: "Network credential".into(),
                value: String::new(),
                secret: true,
                error: None,
            },
            ExternalPanelElement::Button {
                id: "connect".into(),
                label: "Connect".into(),
            },
        ]
    );
}

#[tokio::test]
async fn hidden_network_credential_submit_uses_hidden_connect_request() {
    let secret = "hidden secret";
    let (backend, _calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "known:hidden-cafe".into(),
                label: "Hidden Cafe".into(),
                signal_strength: 0,
                security: WifiSecurity::Personal,
                known: true,
                active: true,
            }],
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();
    provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListCustomValueSubmitted {
                element_id: "networks".into(),
                value: "Hidden Cafe".into(),
            },
        )
        .await
        .unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::TextInputSubmitted {
                element_id: "credential".into(),
                value: secret.into(),
            },
        )
        .await
        .unwrap();

    let request = &requests.lock().unwrap()[0];
    assert_eq!(request.network_id, None);
    assert_eq!(request.label, "Hidden Cafe");
    assert!(request.hidden);
    assert_eq!(request.credential.as_deref(), Some(secret));
    assert!(
        documents
            .iter()
            .all(|document| !document_text(document).contains(secret))
    );
    let ExternalPanelElement::SelectableList {
        entries: active, ..
    } = &documents[1].elements[2]
    else {
        panic!("expected active list");
    };
    assert_eq!(active[0].label, "Hidden Cafe");
    assert!(active[0].active);
}

#[tokio::test]
async fn unknown_enterprise_wifi_row_reports_unsupported_without_connecting() {
    let (backend, _calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:enterprise".into(),
                label: "Enterprise".into(),
                signal_strength: 90,
                security: WifiSecurity::Enterprise,
                known: false,
                active: false,
            }],
            scanning: false,
        },
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: "ap:enterprise".into(),
            },
        )
        .await
        .unwrap();

    assert!(requests.lock().unwrap().is_empty());
    assert!(matches!(
        &documents[0].elements[1],
        ExternalPanelElement::Status {
            severity: ExternalPanelSeverity::Error,
            message,
            ..
        } if message.contains("not supported")
    ));
}

#[tokio::test]
async fn unknown_secured_wifi_row_activation_shows_secret_credential_prompt() {
    let (backend, calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:cafe".into(),
                label: "Cafe".into(),
                signal_strength: 64,
                security: WifiSecurity::Personal,
                known: false,
                active: false,
            }],
            scanning: false,
        },
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: "ap:cafe".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(*calls.lock().unwrap(), vec!["initial_snapshot", "scan"]);
    assert!(requests.lock().unwrap().is_empty());
    assert_eq!(
        documents[0].elements,
        vec![
            ExternalPanelElement::Status {
                title: "Network Credential".into(),
                message: "Enter credential for Cafe".into(),
                severity: ExternalPanelSeverity::Info,
            },
            ExternalPanelElement::TextInput {
                id: "credential".into(),
                label: "Network credential".into(),
                value: String::new(),
                secret: true,
                error: None,
            },
            ExternalPanelElement::Button {
                id: "connect".into(),
                label: "Connect".into(),
            },
        ]
    );
}

#[tokio::test]
async fn submitted_network_credential_connects_without_rendering_the_secret() {
    let secret = "correct horse battery staple";
    let (backend, _calls, requests) = FakeBackend::with_connect(
        WifiSnapshot {
            networks: Vec::new(),
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:cafe".into(),
                label: "Cafe".into(),
                signal_strength: 64,
                security: WifiSecurity::Personal,
                known: false,
                active: false,
            }],
            scanning: false,
        },
        WifiSnapshot {
            networks: vec![WifiNetwork {
                id: "ap:cafe".into(),
                label: "Cafe".into(),
                signal_strength: 64,
                security: WifiSecurity::Personal,
                known: true,
                active: true,
            }],
            scanning: false,
        },
    );
    let mut provider = WifiPanelProvider::new(backend);
    provider.initial_content().await.unwrap();
    provider.refresh().await.unwrap();
    provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::SelectableListEntryActivated {
                element_id: "networks".into(),
                entry_id: "ap:cafe".into(),
            },
        )
        .await
        .unwrap();

    let documents = provider
        .handle_event(
            canopa_panel_protocol::v1::ProviderEvent::TextInputSubmitted {
                element_id: "credential".into(),
                value: secret.into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        requests.lock().unwrap()[0].credential.as_deref(),
        Some(secret)
    );
    assert!(
        documents
            .iter()
            .all(|document| !document_text(document).contains(secret)),
        "secret should not appear in rendered provider content"
    );
    let ExternalPanelElement::SelectableList {
        entries: active, ..
    } = &documents[1].elements[2]
    else {
        panic!("expected active list");
    };
    assert_eq!(active[0].status.as_deref(), Some("active"));
    assert!(active[0].active);
}

fn document_text(document: &canopa_panel_protocol::v1::ExternalPanelDocument) -> String {
    document
        .elements
        .iter()
        .map(|element| match element {
            ExternalPanelElement::Text { text } => text.clone(),
            ExternalPanelElement::Button { label, .. } => label.clone(),
            ExternalPanelElement::SelectableList { entries, .. } => entries
                .iter()
                .map(|entry| {
                    format!(
                        "{} {} {}",
                        entry.label,
                        entry.description.clone().unwrap_or_default(),
                        entry.status.clone().unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
            ExternalPanelElement::TextInput {
                label,
                value,
                error,
                ..
            } => format!("{} {} {}", label, value, error.clone().unwrap_or_default()),
            ExternalPanelElement::Status { title, message, .. } => {
                format!("{title} {message}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn backend_selection_prefers_networkmanager_then_iwd_then_no_control() {
    assert_eq!(
        select_backend(WifiBackendAvailability {
            networkmanager: true,
            iwd: true,
        }),
        Some(BackendName::NetworkManager)
    );
    assert_eq!(
        select_backend(WifiBackendAvailability {
            networkmanager: false,
            iwd: true,
        }),
        Some(BackendName::Iwd)
    );
    assert_eq!(
        select_backend(WifiBackendAvailability {
            networkmanager: false,
            iwd: false,
        }),
        None
    );
}
