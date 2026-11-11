pub mod session;
pub mod transport;

use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, ExternalSelectableEntry,
    ProviderEvent,
};

pub use canopa_services::network::wifi::{
    BackendCapabilities, BackendName, WifiBackendError, WifiBackendErrorCategory,
    WifiBackendFuture, WifiConnectRequest, WifiControlBackend, WifiNetwork, WifiSecurity,
    WifiSnapshot,
};

pub fn no_control_document() -> ExternalPanelDocument {
    ExternalPanelDocument {
        elements: vec![
            ExternalPanelElement::Status {
                title: fl!("shell-wifi-unavailable"),
                message: user_message_for_error(WifiBackendErrorCategory::NoControl),
                severity: ExternalPanelSeverity::Error,
            },
            ExternalPanelElement::Button {
                id: "refresh".into(),
                label: fl!("shell-wifi-check-again"),
            },
        ],
    }
}

pub struct WifiPanelProvider<B> {
    backend: B,
    capabilities: BackendCapabilities,
    snapshot: WifiSnapshot,
    connecting_network_id: Option<String>,
    failed_network_id: Option<String>,
    credential_prompt: Option<CredentialPrompt>,
    progress: Option<tokio::sync::mpsc::UnboundedSender<ExternalPanelDocument>>,
    /// Whether the radio is on; `None` when the backend cannot tell.
    radio: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CredentialPrompt {
    device_id: String,
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
        let capabilities = backend.capabilities();
        Self {
            backend,
            capabilities,
            snapshot: WifiSnapshot {
                device_id: None,
                networks: Vec::new(),
                scanning: true,
            },
            connecting_network_id: None,
            failed_network_id: None,
            credential_prompt: None,
            progress: None,
            radio: None,
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
        self.radio = self.backend.radio().await.unwrap_or(None);
        self.snapshot = match self.backend.initial_snapshot().await {
            Ok(snapshot) => normalize_snapshot(snapshot),
            Err(error) => {
                self.credential_prompt = None;
                self.snapshot = WifiSnapshot {
                    device_id: None,
                    networks: vec![],
                    scanning: false,
                };
                return Ok(self.render_error(error.category()));
            }
        };
        if self
            .credential_prompt
            .as_ref()
            .is_some_and(|prompt| !self.prompt_is_current(prompt))
        {
            self.credential_prompt = None;
            return Ok(self.render_error(WifiBackendErrorCategory::TargetChanged));
        }
        Ok(if self.credential_prompt.is_some() {
            self.render_credential_prompt()
        } else {
            self.render()
        })
    }

    pub async fn initial_content(&mut self) -> Result<ExternalPanelDocument, WifiBackendError> {
        self.radio = self.backend.radio().await.unwrap_or(None);
        self.snapshot = normalize_snapshot(self.backend.initial_snapshot().await?);
        Ok(self.render())
    }

    pub async fn refresh(&mut self) -> Result<ExternalPanelDocument, WifiBackendError> {
        self.credential_prompt = None;
        self.snapshot.scanning = true;
        self.progress_documents();
        self.snapshot = normalize_snapshot(self.backend.scan().await?);
        Ok(self.render())
    }

    pub fn backend_name(&self) -> BackendName {
        self.backend.name()
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
            ProviderEvent::ButtonActivated { element_id } if element_id == "radio" => {
                let enable = self.radio == Some(false);
                self.credential_prompt = None;
                Ok(vec![match self.backend.set_radio(enable).await {
                    Ok(snapshot) => {
                        self.radio = Some(enable);
                        self.snapshot = normalize_snapshot(snapshot);
                        self.render()
                    }
                    Err(error) => self.render_error(error.category()),
                }])
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "disconnect" => {
                let Some(device) = self.snapshot.device_id.clone() else {
                    return Ok(Vec::new());
                };
                Ok(vec![match self.backend.disconnect(device).await {
                    Ok(snapshot) => {
                        self.snapshot = normalize_snapshot(snapshot);
                        self.render()
                    }
                    Err(error) => self.render_error(error.category()),
                }])
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "forget" => {
                let (Some(device), Some(network)) =
                    (self.snapshot.device_id.clone(), self.forgettable().cloned())
                else {
                    return Ok(Vec::new());
                };
                let request = WifiConnectRequest {
                    device_id: device,
                    network_id: Some(network.id.clone()),
                    label: network.label.clone(),
                    credential: None,
                    hidden: false,
                };
                Ok(vec![match self.backend.forget(request).await {
                    Ok(snapshot) => {
                        self.failed_network_id = None;
                        self.snapshot = normalize_snapshot(snapshot);
                        self.render()
                    }
                    Err(error) => self.render_error(error.category()),
                }])
            }
            _ => Ok(Vec::new()),
        }
    }

    fn prompt_is_current(&self, prompt: &CredentialPrompt) -> bool {
        self.snapshot.device_id.as_deref() == Some(prompt.device_id.as_str())
            && (prompt.hidden
                || self
                    .snapshot
                    .networks
                    .iter()
                    .any(|network| network.id == prompt.network.id))
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

        let Some(device_id) = self.snapshot.device_id.clone() else {
            return Ok(vec![self.render_error(WifiBackendErrorCategory::NoControl)]);
        };
        self.credential_prompt = Some(CredentialPrompt {
            device_id,
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

        let Some(device_id) = self.snapshot.device_id.clone() else {
            return Ok(vec![self.render_error(WifiBackendErrorCategory::NoControl)]);
        };
        if network.active {
            return Ok(Vec::new());
        }
        if network.security == WifiSecurity::Enterprise && !network.known {
            self.failed_network_id = Some(network.id);
            let mut document = self.render();
            document.elements.insert(
                1,
                ExternalPanelElement::Status {
                    title: fl!("shell-wifi-set-up-in-settings"),
                    message: fl!("shell-wifi-enterprise-in-settings"),
                    severity: ExternalPanelSeverity::Info,
                },
            );
            return Ok(vec![document]);
        }
        if network.security == WifiSecurity::Personal && !network.known {
            self.credential_prompt = Some(CredentialPrompt {
                device_id: device_id.clone(),
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
            device_id: device_id.clone(),
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
                    device_id,
                    network,
                    value: String::new(),
                    error: Some(fl!("shell-wifi-credential-required")),
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

        if !self.prompt_is_current(&prompt) {
            self.credential_prompt = None;
            return Ok(vec![
                self.render_error(WifiBackendErrorCategory::TargetChanged),
            ]);
        }
        self.credential_prompt = None;
        self.failed_network_id = None;
        if prompt.hidden {
            self.snapshot.networks.push(prompt.network.clone());
        }
        self.connecting_network_id = Some(prompt.network.id.clone());
        let connecting = self.progress_documents();
        let request = WifiConnectRequest {
            device_id: prompt.device_id.clone(),
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
                    error: Some(fl!("shell-wifi-credential-rejected")),
                    ..prompt
                });
                Ok(vec![self.render_credential_prompt()])
            }
            Err(err) if err.category() == WifiBackendErrorCategory::TargetChanged => {
                self.connecting_network_id = None;
                Ok(self.completed(connecting, self.render_error(err.category())))
            }
            Err(err) => {
                self.connecting_network_id = None;
                self.credential_prompt = Some(CredentialPrompt {
                    value: String::new(),
                    error: Some(user_message_for_error(err.category())),
                    ..prompt
                });
                Ok(self.completed(connecting, self.render_credential_prompt()))
            }
        }
    }

    /// The network "Forget" applies to: the connected one, or a known one
    /// that just failed.
    fn forgettable(&self) -> Option<&WifiNetwork> {
        self.snapshot
            .networks
            .iter()
            .find(|network| network.active)
            .or_else(|| {
                let failed = self.failed_network_id.as_deref()?;
                self.snapshot
                    .networks
                    .iter()
                    .find(|network| network.id == failed && network.known)
            })
    }

    fn render(&self) -> ExternalPanelDocument {
        if self.backend.name() == BackendName::Unavailable {
            return no_control_document();
        }
        if self.radio == Some(false) {
            return ExternalPanelDocument {
                elements: vec![
                    ExternalPanelElement::Status {
                        title: fl!("shell-wifi-title"),
                        message: fl!("shell-wifi-off"),
                        severity: ExternalPanelSeverity::Info,
                    },
                    ExternalPanelElement::Button {
                        id: "radio".into(),
                        label: fl!("shell-wifi-turn-on"),
                    },
                ],
            };
        }
        let active = self.snapshot.networks.iter().find(|network| network.active);
        let connecting = self.connecting_network();
        let message = if let Some(connecting) = connecting {
            fl!("shell-wifi-connecting", network = connecting.label.clone())
        } else if let Some(active) = active {
            fl!("shell-wifi-connected", network = active.label.clone())
        } else if self.snapshot.scanning {
            fl!("shell-wifi-scanning-networks")
        } else {
            fl!("shell-wifi-choose")
        };

        let mut document = ExternalPanelDocument {
            elements: vec![
                ExternalPanelElement::Status {
                    title: fl!("shell-wifi-title"),
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
        // Backends that manage the radio also disconnect and forget.
        if self.connecting_network_id.is_none() && self.radio.is_some() {
            let mut buttons = Vec::new();
            if active.is_some() {
                buttons.push(("disconnect", fl!("shell-wifi-disconnect")));
            }
            if let Some(network) = self.forgettable() {
                buttons.push((
                    "forget",
                    fl!("shell-wifi-forget", network = network.label.clone()),
                ));
            }
            if self.radio == Some(true) {
                buttons.push(("radio", fl!("shell-wifi-turn-off")));
            }
            for (index, (id, label)) in buttons.into_iter().enumerate() {
                document.elements.insert(
                    2 + index,
                    ExternalPanelElement::Button {
                        id: id.into(),
                        label,
                    },
                );
            }
        }
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
                title: fl!("shell-wifi-problem"),
                message: user_message_for_error(category),
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
                    title: fl!("shell-wifi-credential-title"),
                    message: fl!(
                        "shell-wifi-credential-prompt",
                        network = prompt.network.label.clone()
                    ),
                    severity: ExternalPanelSeverity::Info,
                },
                ExternalPanelElement::TextInput {
                    id: "credential".into(),
                    label: fl!("shell-wifi-credential-label"),
                    value: prompt.value.clone(),
                    secret: true,
                    error: prompt.error.clone(),
                },
                ExternalPanelElement::Button {
                    id: "connect".into(),
                    label: fl!("shell-wifi-connect"),
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
        fl!("shell-wifi-scanning")
    } else {
        fl!("shell-wifi-refresh")
    }
}

fn normalize_snapshot(mut snapshot: WifiSnapshot) -> WifiSnapshot {
    let mut collapsed: std::collections::HashMap<String, WifiNetwork> =
        std::collections::HashMap::new();

    for network in snapshot.networks {
        match collapsed.get_mut(&network.id) {
            Some(current) if better_representative(&network, current) => *current = network,
            Some(_) => {}
            None => {
                collapsed.insert(network.id.clone(), network);
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
        .then(left.id.cmp(&right.id))
}

fn user_message_for_error(category: WifiBackendErrorCategory) -> String {
    match category {
        WifiBackendErrorCategory::TargetChanged => fl!("shell-wifi-error-target-changed"),
        WifiBackendErrorCategory::NoControl => fl!("shell-wifi-error-no-control"),
        WifiBackendErrorCategory::ScanFailed => fl!("shell-wifi-error-scan-failed"),
        WifiBackendErrorCategory::ConnectionFailed => fl!("shell-wifi-error-connection-failed"),
        WifiBackendErrorCategory::CredentialRejected => fl!("shell-wifi-error-credential-rejected"),
        WifiBackendErrorCategory::UnsupportedNetwork => fl!("shell-wifi-error-unsupported"),
        WifiBackendErrorCategory::AuthorizationRequired => {
            fl!("shell-wifi-error-authorization-required")
        }
    }
}

/// "known secured 70%": whether the network is saved, its security and signal.
fn network_description(network: &WifiNetwork) -> String {
    let security = match network.security {
        WifiSecurity::Open => "open",
        WifiSecurity::Personal => "secured",
        WifiSecurity::Enterprise => "enterprise",
    };
    fl!(
        "shell-wifi-network-description",
        known = if network.known { "yes" } else { "no" },
        security = security,
        signal = canopa_i18n::format::percent(f64::from(network.signal_strength))
    )
}

#[cfg(test)]
mod backend_tests;
