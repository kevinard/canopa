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
    /// The name typed into the "Join other network" prompt, while it is open.
    hidden_prompt: Option<String>,
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
            hidden_prompt: None,
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
            // The rich panel joins a hidden network through a prompt of its own.
            ProviderEvent::ButtonActivated { element_id } if element_id == "join-hidden" => {
                self.credential_prompt = None;
                self.hidden_prompt = Some(String::new());
                Ok(vec![self.render_hidden_prompt()])
            }
            ProviderEvent::TextInputChanged { element_id, value }
                if element_id == "hidden-name" =>
            {
                if let Some(name) = &mut self.hidden_prompt {
                    *name = value;
                }
                Ok(Vec::new())
            }
            ProviderEvent::TextInputSubmitted { element_id, value }
                if element_id == "hidden-name" =>
            {
                self.hidden_prompt = None;
                self.submit_custom_network_value(value).await
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "join" => {
                let value = self.hidden_prompt.take().unwrap_or_default();
                self.submit_custom_network_value(value).await
            }
            ProviderEvent::ButtonActivated { element_id } if element_id == "cancel" => {
                self.hidden_prompt = None;
                self.credential_prompt = None;
                Ok(vec![self.render()])
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
                    id: "cancel".into(),
                    label: fl!("shell-wifi-cancel"),
                },
                ExternalPanelElement::Button {
                    id: "connect".into(),
                    label: fl!("shell-wifi-connect"),
                },
            ],
        }
    }

    /// The prompt for a hidden network's name.
    fn render_hidden_prompt(&self) -> ExternalPanelDocument {
        ExternalPanelDocument {
            elements: vec![
                ExternalPanelElement::Status {
                    title: fl!("shell-wifi-join-other"),
                    message: fl!("shell-wifi-join-other-prompt"),
                    severity: ExternalPanelSeverity::Info,
                },
                ExternalPanelElement::TextInput {
                    id: "hidden-name".into(),
                    label: fl!("shell-wifi-network-name"),
                    value: self.hidden_prompt.clone().unwrap_or_default(),
                    secret: false,
                    error: None,
                },
                ExternalPanelElement::Button {
                    id: "cancel".into(),
                    label: fl!("shell-wifi-cancel"),
                },
                ExternalPanelElement::Button {
                    id: "join".into(),
                    label: fl!("shell-wifi-join"),
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
            // A badge only where the row's state matters: the description
            // already says whether it is saved, secured and how strong.
            status: if connecting {
                Some(fl!("shell-wifi-status-connecting"))
            } else if failed {
                Some(fl!("shell-wifi-status-failed"))
            } else if network.active {
                Some(fl!("shell-wifi-status-connected"))
            } else {
                None
            },
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
/// "Saved · Secured · Signal 82%": what a network row says under its name.
fn network_description(network: &WifiNetwork) -> String {
    let security = match network.security {
        WifiSecurity::Open => fl!("shell-wifi-security-open"),
        WifiSecurity::Personal => fl!("shell-wifi-security-secured"),
        WifiSecurity::Enterprise => fl!("shell-wifi-security-enterprise"),
    };
    let signal = fl!(
        "shell-wifi-signal",
        signal = canopa_i18n::format::percent(f64::from(network.signal_strength))
    );
    let mut parts = Vec::new();
    if network.known {
        parts.push(fl!("shell-wifi-saved"));
    }
    parts.push(security);
    parts.push(signal);
    parts.join(" · ")
}

#[cfg(test)]
mod backend_tests;

/// The panel in Canopa's rich format, for hosts that speak it, built from a
/// form `document` of this provider: `None` keeps the form, for prompts that
/// need a text field and for a backend that cannot be controlled.
///
/// The header switches the radio and refreshes; the network in use leads with
/// Disconnect and Forget, and the others follow under **Networks**, each with
/// Connect. Joining a hidden network has its own prompt.
pub fn rich_view(document: &ExternalPanelDocument) -> Option<canopa_panel_protocol::PanelContent> {
    use canopa_panel_protocol::{PanelAction, PanelContent, PanelItem, PanelSection};

    let elements = &document.elements;
    if elements
        .iter()
        .any(|element| matches!(element, ExternalPanelElement::TextInput { .. }))
    {
        return None;
    }
    let button = |id: &str| {
        elements.iter().find_map(|element| match element {
            ExternalPanelElement::Button { id: found, label } if found == id => Some(label.clone()),
            _ => None,
        })
    };
    let radio = button("radio");
    // No radio switch and no list: the backend cannot be controlled.
    let list = elements.iter().find_map(|element| match element {
        ExternalPanelElement::SelectableList { id, entries, .. } if id == "networks" => {
            Some(entries)
        }
        _ => None,
    });
    if radio.is_none() && list.is_none() {
        return None;
    }
    let radio_on = radio
        .as_deref()
        .is_none_or(|label| label == fl!("shell-wifi-turn-off"));

    let item = |id: String, title: String| PanelItem {
        id,
        title,
        attribution: None,
        body: None,
        styled_body: None,
        rich_body: None,
        metadata: Vec::new(),
        actions: Vec::new(),
        slider: None,
    };
    let section = |id: &str, title: String, items: Vec<PanelItem>| PanelSection {
        id: id.into(),
        title,
        icon: None,
        items,
        controls: Vec::new(),
    };

    let mut content = PanelContent::new(fl!("shell-wifi-title"));
    content.error = elements.iter().find_map(|element| match element {
        ExternalPanelElement::Status {
            message,
            severity: ExternalPanelSeverity::Error,
            ..
        } => Some(message.clone()),
        _ => None,
    });
    if radio.is_some() {
        content.actions.push(
            PanelAction::new("radio", fl!("shell-wifi-title"))
                .pressed(radio_on)
                .secondary(),
        );
    }
    if let Some(label) = button("refresh") {
        content.actions.push(
            PanelAction::new("refresh", label)
                .secondary()
                .symbol("view-refresh-symbolic"),
        );
    }
    if !radio_on {
        let mut off = item("off".into(), fl!("shell-wifi-off"));
        off.body = Some(fl!("shell-wifi-off-hint"));
        content
            .sections
            .push(section("status", String::new(), vec![off]));
        return Some(content);
    }
    content.actions.push(PanelAction::new(
        "join-hidden",
        fl!("shell-wifi-join-other-action"),
    ));

    let entries = list.map(Vec::as_slice).unwrap_or_default();
    let failed = fl!("shell-wifi-status-failed");
    // Forget applies to the network that just failed, else the one in use.
    let forget_target = entries
        .iter()
        .find(|entry| entry.status.as_deref() == Some(failed.as_str()))
        .or_else(|| entries.iter().find(|entry| entry.active))
        .map(|entry| entry.id.clone());
    let forget = button("forget");
    let disconnect = button("disconnect");
    let row = |entry: &ExternalSelectableEntry| {
        let mut row = item(entry.id.clone(), entry.label.clone());
        row.body = Some(
            entry
                .status
                .iter()
                .chain(entry.description.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join(" · "),
        );
        if entry.active {
            if let Some(label) = &disconnect {
                row.actions
                    .push(PanelAction::new("disconnect", label.clone()));
            }
        } else if !entry.disabled {
            row.actions
                .push(PanelAction::new("connect", fl!("shell-wifi-connect")));
        }
        if forget.is_some() && forget_target.as_deref() == Some(entry.id.as_str()) {
            row.actions
                .push(PanelAction::new("forget", fl!("shell-wifi-forget-action")).destructive());
        }
        row
    };
    let current: Vec<_> = entries
        .iter()
        .filter(|entry| entry.active || entry.status.is_some())
        .map(row)
        .collect();
    let others: Vec<_> = entries
        .iter()
        .filter(|entry| !entry.active && entry.status.is_none())
        .map(row)
        .collect();
    if !current.is_empty() {
        content
            .sections
            .push(section("current", String::new(), current));
    }
    if others.is_empty() && content.sections.is_empty() {
        // Nothing found yet: say what is happening.
        let message = elements.iter().find_map(|element| match element {
            ExternalPanelElement::Status {
                message,
                severity: ExternalPanelSeverity::Info,
                ..
            } => Some(message.clone()),
            _ => None,
        });
        let status = item(
            "status".into(),
            message.unwrap_or_else(|| fl!("shell-wifi-choose")),
        );
        content
            .sections
            .push(section("status", String::new(), vec![status]));
    } else if !others.is_empty() {
        content
            .sections
            .push(section("networks", fl!("shell-wifi-networks"), others));
    }
    Some(content)
}

#[cfg(test)]
mod rich_tests {
    use super::*;

    fn entry(id: &str, active: bool, status: Option<&str>) -> ExternalSelectableEntry {
        ExternalSelectableEntry {
            id: id.into(),
            label: id.into(),
            description: Some("Secured · Signal 80%".into()),
            disabled: false,
            active,
            status: status.map(Into::into),
        }
    }

    fn list(
        entries: Vec<ExternalSelectableEntry>,
        buttons: &[(&str, String)],
    ) -> ExternalPanelDocument {
        let mut elements = vec![ExternalPanelElement::Status {
            title: fl!("shell-wifi-title"),
            message: "Connected to Home".into(),
            severity: ExternalPanelSeverity::Info,
        }];
        elements.extend(
            buttons
                .iter()
                .map(|(id, label)| ExternalPanelElement::Button {
                    id: (*id).into(),
                    label: label.clone(),
                }),
        );
        elements.push(ExternalPanelElement::SelectableList {
            id: "networks".into(),
            entries,
            allow_custom: true,
        });
        ExternalPanelDocument { elements }
    }

    #[test]
    fn the_network_in_use_leads_and_the_others_follow_under_networks() {
        let document = list(
            vec![
                entry("Home", true, Some(&fl!("shell-wifi-status-connected"))),
                entry("Cafe", false, None),
            ],
            &[
                ("refresh", fl!("shell-wifi-refresh")),
                ("disconnect", fl!("shell-wifi-disconnect")),
                ("forget", fl!("shell-wifi-forget", network = "Home")),
                ("radio", fl!("shell-wifi-turn-off")),
            ],
        );
        let content = rich_view(&document).expect("a list is rich");
        let keys: Vec<_> = content.actions.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["radio", "refresh", "join-hidden"]);
        assert_eq!(content.actions[0].pressed, Some(true));
        assert_eq!(content.sections[0].items[0].title, "Home");
        let home: Vec<_> = content.sections[0].items[0]
            .actions
            .iter()
            .map(|a| a.key.as_str())
            .collect();
        assert_eq!(home, ["disconnect", "forget"]);
        assert_eq!(content.sections[1].title, fl!("shell-wifi-networks"));
        assert_eq!(content.sections[1].items[0].actions[0].key, "connect");
    }

    #[test]
    fn prompts_keep_their_form_and_a_radio_that_is_off_says_so() {
        let mut prompt = list(Vec::new(), &[]);
        prompt.elements.push(ExternalPanelElement::TextInput {
            id: "credential".into(),
            label: "Password".into(),
            value: String::new(),
            secret: true,
            error: None,
        });
        assert!(rich_view(&prompt).is_none());
        assert!(rich_view(&no_control_document()).is_none());

        let off = ExternalPanelDocument {
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
        let content = rich_view(&off).expect("a radio is rich");
        assert_eq!(content.actions[0].pressed, Some(false));
        assert_eq!(content.sections[0].items[0].title, fl!("shell-wifi-off"));
    }
}
