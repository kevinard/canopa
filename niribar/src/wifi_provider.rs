pub mod backends;
pub mod cleanup;
pub mod session;
pub mod transport;

use std::future::Future;
use std::pin::Pin;

use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, ExternalSelectableEntry,
    ProviderEvent,
};

pub type WifiBackendFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, WifiBackendError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendName {
    Iwd,
    NetworkManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WifiBackendAvailability {
    pub networkmanager: bool,
    pub iwd: bool,
}

pub fn select_backend(availability: WifiBackendAvailability) -> Option<BackendName> {
    if availability.networkmanager {
        Some(BackendName::NetworkManager)
    } else if availability.iwd {
        Some(BackendName::Iwd)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub can_scan_while_connecting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiBackendError {
    category: WifiBackendErrorCategory,
}

impl WifiBackendError {
    pub fn new(category: WifiBackendErrorCategory) -> Self {
        Self { category }
    }

    pub fn category(&self) -> WifiBackendErrorCategory {
        self.category
    }
}

impl std::fmt::Display for WifiBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.category {
            WifiBackendErrorCategory::NoControl => {
                "no supported Wi-Fi control backend is available"
            }
            WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed",
            WifiBackendErrorCategory::ConnectionFailed => "Wi-Fi connection failed",
            WifiBackendErrorCategory::CredentialRejected => "network credential was rejected",
            WifiBackendErrorCategory::UnsupportedNetwork => "Wi-Fi network type is unsupported",
            WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required",
        })
    }
}

impl std::error::Error for WifiBackendError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBackendErrorCategory {
    NoControl,
    ScanFailed,
    ConnectionFailed,
    CredentialRejected,
    UnsupportedNetwork,
    AuthorizationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurity {
    Open,
    Personal,
    Enterprise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub id: String,
    pub label: String,
    pub signal_strength: u8,
    pub security: WifiSecurity,
    pub known: bool,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiSnapshot {
    pub networks: Vec<WifiNetwork>,
    pub scanning: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WifiConnectRequest {
    pub network_id: Option<String>,
    pub label: String,
    pub credential: Option<String>,
    pub hidden: bool,
}

impl std::fmt::Debug for WifiConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WifiConnectRequest")
            .field("network_id", &self.network_id)
            .field("label", &self.label)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "[redacted]"),
            )
            .field("hidden", &self.hidden)
            .finish()
    }
}

pub trait WifiControlBackend {
    fn name(&self) -> BackendName;
    fn capabilities(&self) -> BackendCapabilities;
    fn initial_snapshot<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot>;
    fn scan<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot>;
    fn connect<'a>(
        &'a mut self,
        request: WifiConnectRequest,
    ) -> WifiBackendFuture<'a, WifiSnapshot>;
}

pub fn no_control_document() -> ExternalPanelDocument {
    ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Status {
            title: "Wi-Fi unavailable".into(),
            message: user_message_for_error(WifiBackendErrorCategory::NoControl).into(),
            severity: ExternalPanelSeverity::Error,
        }],
    }
}

pub struct WifiPanelProvider<B> {
    backend: B,
    backend_name: BackendName,
    capabilities: BackendCapabilities,
    snapshot: WifiSnapshot,
    connecting_network_id: Option<String>,
    failed_network_id: Option<String>,
    credential_prompt: Option<CredentialPrompt>,
    progress: Option<tokio::sync::mpsc::UnboundedSender<ExternalPanelDocument>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CredentialPrompt {
    network: WifiNetwork,
    value: String,
    error: Option<String>,
    hidden: bool,
}

impl<B> WifiPanelProvider<B>
where
    B: WifiControlBackend,
{
    pub fn new(backend: B) -> Self {
        let backend_name = backend.name();
        let capabilities = backend.capabilities();
        Self {
            backend,
            backend_name,
            capabilities,
            snapshot: WifiSnapshot {
                networks: Vec::new(),
                scanning: true,
            },
            connecting_network_id: None,
            failed_network_id: None,
            credential_prompt: None,
            progress: None,
        }
    }

    pub fn stream_progress(
        &mut self,
        sender: tokio::sync::mpsc::UnboundedSender<ExternalPanelDocument>,
    ) {
        self.progress = Some(sender);
    }

    fn progress_documents(&self) -> Vec<ExternalPanelDocument> {
        let content = self.render();
        if let Some(sender) = &self.progress {
            let _ = sender.send(content);
            Vec::new()
        } else {
            vec![content]
        }
    }

    fn completed(
        &self,
        mut documents: Vec<ExternalPanelDocument>,
        content: ExternalPanelDocument,
    ) -> Vec<ExternalPanelDocument> {
        documents.push(content);
        documents
    }

    pub async fn synchronize(&mut self) -> Result<ExternalPanelDocument, WifiBackendError> {
        self.snapshot = normalize_snapshot(self.backend.initial_snapshot().await?);
        Ok(if self.credential_prompt.is_some() {
            self.render_credential_prompt()
        } else {
            self.render()
        })
    }

    pub async fn initial_content(&mut self) -> Result<ExternalPanelDocument, WifiBackendError> {
        self.snapshot = normalize_snapshot(self.backend.initial_snapshot().await?);
        Ok(self.render())
    }

    pub async fn refresh(&mut self) -> Result<ExternalPanelDocument, WifiBackendError> {
        self.snapshot.scanning = true;
        self.progress_documents();
        self.snapshot = normalize_snapshot(self.backend.scan().await?);
        Ok(self.render())
    }

    pub fn backend_name(&self) -> BackendName {
        self.backend_name
    }

    pub async fn handle_event(
        &mut self,
        event: ProviderEvent,
    ) -> Result<Vec<ExternalPanelDocument>, WifiBackendError> {
        match event {
            ProviderEvent::SelectableListEntryActivated {
                element_id,
                entry_id,
            } if element_id == "networks" => self.activate_network(entry_id).await,
            ProviderEvent::SelectableListCustomValueSubmitted { element_id, value }
                if element_id == "networks" =>
            {
                self.submit_custom_network_value(value).await
            }
            ProviderEvent::TextInputSubmitted { element_id, value }
                if element_id == "credential" =>
            {
                self.submit_credential(value).await
            }
            ProviderEvent::TextInputChanged { element_id, value } if element_id == "credential" => {
                if let Some(prompt) = &mut self.credential_prompt {
                    prompt.value = value;
                    prompt.error = None;
                    return Ok(vec![self.render_credential_prompt()]);
                }
                Ok(Vec::new())
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "connect" => {
                let value = self
                    .credential_prompt
                    .as_ref()
                    .map(|prompt| prompt.value.clone())
                    .unwrap_or_default();
                self.submit_credential(value).await
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "refresh" => {
                self.refresh().await.map(|document| vec![document])
            }
            _ => Ok(Vec::new()),
        }
    }

    async fn submit_custom_network_value(
        &mut self,
        value: String,
    ) -> Result<Vec<ExternalPanelDocument>, WifiBackendError> {
        let label = value.trim();
        if label.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(network) = self
            .snapshot
            .networks
            .iter()
            .find(|network| network.label == label)
        {
            return self.activate_network(network.id.clone()).await;
        }

        self.credential_prompt = Some(CredentialPrompt {
            network: WifiNetwork {
                id: format!("hidden:{label}"),
                label: label.to_string(),
                signal_strength: 0,
                security: WifiSecurity::Personal,
                known: false,
                active: false,
            },
            value: String::new(),
            error: None,
            hidden: true,
        });
        Ok(vec![self.render_credential_prompt()])
    }

    async fn activate_network(
        &mut self,
        entry_id: String,
    ) -> Result<Vec<ExternalPanelDocument>, WifiBackendError> {
        let Some(network) = self
            .snapshot
            .networks
            .iter()
            .find(|network| network.id == entry_id)
            .cloned()
        else {
            return Ok(Vec::new());
        };

        if network.active {
            return Ok(Vec::new());
        }
        if network.security == WifiSecurity::Enterprise && !network.known {
            self.failed_network_id = Some(network.id);
            return Ok(vec![
                self.render_error(WifiBackendErrorCategory::UnsupportedNetwork),
            ]);
        }
        if network.security == WifiSecurity::Personal && !network.known {
            self.credential_prompt = Some(CredentialPrompt {
                network,
                value: String::new(),
                error: None,
                hidden: false,
            });
            return Ok(vec![self.render_credential_prompt()]);
        }

        self.failed_network_id = None;
        self.connecting_network_id = Some(network.id.clone());
        let connecting = self.progress_documents();
        let request = WifiConnectRequest {
            network_id: Some(network.id.clone()),
            label: network.label.clone(),
            credential: None,
            hidden: false,
        };

        match self.backend.connect(request).await {
            Ok(snapshot) => {
                self.connecting_network_id = None;
                self.snapshot = normalize_snapshot(snapshot);
                Ok(self.completed(connecting, self.render()))
            }
            Err(err)
                if network.security == WifiSecurity::Personal
                    && err.category() == WifiBackendErrorCategory::CredentialRejected =>
            {
                self.connecting_network_id = None;
                self.credential_prompt = Some(CredentialPrompt {
                    network,
                    value: String::new(),
                    error: Some("Credential required".into()),
                    hidden: false,
                });
                Ok(self.completed(connecting, self.render_credential_prompt()))
            }
            Err(err) => {
                self.connecting_network_id = None;
                self.failed_network_id = Some(network.id);
                Ok(self.completed(connecting, self.render_error(err.category())))
            }
        }
    }

    async fn submit_credential(
        &mut self,
        credential: String,
    ) -> Result<Vec<ExternalPanelDocument>, WifiBackendError> {
        let Some(prompt) = self.credential_prompt.clone() else {
            return Ok(Vec::new());
        };

        self.credential_prompt = None;
        self.failed_network_id = None;
        if !self
            .snapshot
            .networks
            .iter()
            .any(|network| network.id == prompt.network.id)
        {
            self.snapshot.networks.push(prompt.network.clone());
        }
        self.connecting_network_id = Some(prompt.network.id.clone());
        let connecting = self.progress_documents();
        let request = WifiConnectRequest {
            network_id: if prompt.hidden {
                None
            } else {
                Some(prompt.network.id.clone())
            },
            label: prompt.network.label.clone(),
            credential: Some(credential),
            hidden: prompt.hidden,
        };

        match self.backend.connect(request).await {
            Ok(snapshot) => {
                self.connecting_network_id = None;
                self.snapshot = normalize_snapshot(snapshot);
                Ok(self.completed(connecting, self.render()))
            }
            Err(err) if err.category() == WifiBackendErrorCategory::CredentialRejected => {
                self.connecting_network_id = None;
                self.credential_prompt = Some(CredentialPrompt {
                    value: String::new(),
                    error: Some("Credential rejected".into()),
                    ..prompt
                });
                Ok(vec![self.render_credential_prompt()])
            }
            Err(err) => {
                self.connecting_network_id = None;
                self.credential_prompt = Some(CredentialPrompt {
                    value: String::new(),
                    error: Some(user_message_for_error(err.category()).into()),
                    ..prompt
                });
                Ok(self.completed(connecting, self.render_credential_prompt()))
            }
        }
    }

    fn render(&self) -> ExternalPanelDocument {
        let active = self.snapshot.networks.iter().find(|network| network.active);
        let connecting = self.connecting_network();
        let message = if let Some(connecting) = connecting {
            format!("Connecting to {}", connecting.label)
        } else if let Some(active) = active {
            format!("Connected to {}", active.label)
        } else if self.snapshot.scanning {
            "Scanning for Wi-Fi networks".to_string()
        } else {
            "Choose a Wi-Fi network".to_string()
        };

        let mut document = ExternalPanelDocument {
            elements: vec![
                ExternalPanelElement::Status {
                    title: "Wi-Fi".into(),
                    message,
                    severity: ExternalPanelSeverity::Info,
                },
                ExternalPanelElement::Button {
                    id: "refresh".into(),
                    label: refresh_label(self.capabilities, self.snapshot.scanning),
                },
                ExternalPanelElement::SelectableList {
                    id: "networks".into(),
                    entries: self
                        .snapshot
                        .networks
                        .iter()
                        .map(|network| self.render_entry(network))
                        .collect(),
                    allow_custom: true,
                },
            ],
        };
        if self.connecting_network_id.is_some() && !self.capabilities.can_scan_while_connecting {
            document.elements.retain(|element| !matches!(element, ExternalPanelElement::Button { id, .. } if id == "refresh"));
        }
        document
    }

    fn render_error(&self, category: WifiBackendErrorCategory) -> ExternalPanelDocument {
        let mut document = self.render();
        document.elements.insert(
            1,
            ExternalPanelElement::Status {
                title: "Wi-Fi problem".into(),
                message: user_message_for_error(category).into(),
                severity: ExternalPanelSeverity::Error,
            },
        );
        document
    }

    fn render_credential_prompt(&self) -> ExternalPanelDocument {
        let Some(prompt) = &self.credential_prompt else {
            return self.render();
        };
        ExternalPanelDocument {
            elements: vec![
                ExternalPanelElement::Status {
                    title: "Network Credential".into(),
                    message: format!("Enter credential for {}", prompt.network.label),
                    severity: ExternalPanelSeverity::Info,
                },
                ExternalPanelElement::TextInput {
                    id: "credential".into(),
                    label: "Network credential".into(),
                    value: prompt.value.clone(),
                    secret: true,
                    error: prompt.error.clone(),
                },
                ExternalPanelElement::Button {
                    id: "connect".into(),
                    label: "Connect".into(),
                },
            ],
        }
    }

    fn connecting_network(&self) -> Option<&WifiNetwork> {
        let connecting_id = self.connecting_network_id.as_deref()?;
        self.snapshot
            .networks
            .iter()
            .find(|network| network.id == connecting_id)
    }

    fn render_entry(&self, network: &WifiNetwork) -> ExternalSelectableEntry {
        let connecting = self.connecting_network_id.as_deref() == Some(network.id.as_str());
        let failed = self.failed_network_id.as_deref() == Some(network.id.as_str());
        ExternalSelectableEntry {
            id: network.id.clone(),
            label: network.label.clone(),
            description: Some(network_description(network)),
            disabled: self.connecting_network_id.is_some(),
            active: network.active,
            status: Some(if connecting {
                "connecting".into()
            } else if failed {
                "failed".into()
            } else if network.active {
                "active".into()
            } else if network.known {
                "known".into()
            } else {
                match network.security {
                    WifiSecurity::Open => "open".into(),
                    WifiSecurity::Personal | WifiSecurity::Enterprise => "secured".into(),
                }
            }),
        }
    }
}

fn refresh_label(capabilities: BackendCapabilities, scanning: bool) -> String {
    let _ = capabilities;
    if scanning {
        "Scanning".into()
    } else {
        "Refresh".into()
    }
}

fn normalize_snapshot(mut snapshot: WifiSnapshot) -> WifiSnapshot {
    let mut collapsed: std::collections::HashMap<String, WifiNetwork> =
        std::collections::HashMap::new();

    for network in snapshot.networks {
        match collapsed.get_mut(&network.label) {
            Some(current) if better_representative(&network, current) => *current = network,
            Some(_) => {}
            None => {
                collapsed.insert(network.label.clone(), network);
            }
        }
    }

    let mut networks: Vec<_> = collapsed.into_values().collect();
    networks.sort_by(compare_network_rows);
    snapshot.networks = networks;
    snapshot
}

fn better_representative(candidate: &WifiNetwork, current: &WifiNetwork) -> bool {
    candidate
        .active
        .cmp(&current.active)
        .then(candidate.known.cmp(&current.known))
        .then(candidate.signal_strength.cmp(&current.signal_strength))
        .then_with(|| current.id.cmp(&candidate.id))
        .is_gt()
}

fn compare_network_rows(left: &WifiNetwork, right: &WifiNetwork) -> std::cmp::Ordering {
    right
        .active
        .cmp(&left.active)
        .then(right.known.cmp(&left.known))
        .then(right.signal_strength.cmp(&left.signal_strength))
        .then(left.label.cmp(&right.label))
}

fn user_message_for_error(category: WifiBackendErrorCategory) -> &'static str {
    match category {
        WifiBackendErrorCategory::NoControl => "No supported Wi-Fi control backend is available.",
        WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed.",
        WifiBackendErrorCategory::ConnectionFailed => "Connection failed.",
        WifiBackendErrorCategory::CredentialRejected => "Network credential was rejected.",
        WifiBackendErrorCategory::UnsupportedNetwork => "This Wi-Fi network is not supported.",
        WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required.",
    }
}

fn network_description(network: &WifiNetwork) -> String {
    let mut parts = Vec::new();
    if network.known {
        parts.push("known");
    }
    parts.push(match network.security {
        WifiSecurity::Open => "open",
        WifiSecurity::Personal => "secured",
        WifiSecurity::Enterprise => "enterprise",
    });
    let signal = format!("{}%", network.signal_strength);
    parts.push(signal.as_str());
    parts.join(" ")
}
