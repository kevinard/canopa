use iced::Task;
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use std::time::Duration;

use super::app_effects::AppEffect;
use super::external_panel_session::{ExternalPanelPanelIdentity, ExternalPanelSessionEffect};
#[cfg(test)]
pub(super) use super::panel_sizing::MENU_ROW_EXTENT;
use super::panel_sizing::{PanelPhysicalPlan, PanelSizingInput, PlannedDimension};
use super::{App, Message};

pub(super) fn panel_search_input_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-search-input-{window:?}"))
}

pub(super) fn panel_menu_scroll_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("panel-menu-scroll-{window:?}"))
}

const DEFAULT_MENU_VIEWPORT_HEIGHT: f32 = super::panel_sizing::MENU_ROW_EXTENT * 12.0;
const PANEL_SEARCH_FOCUS_RETRIES: u8 = 8;
const PANEL_SEARCH_FOCUS_RETRY_DELAY: Duration = Duration::from_millis(25);

pub(super) fn request_panel_search_focus() -> Task<Message> {
    Task::done(Message::Panel(
        super::messages::PanelEvent::FocusSearchInput {
            retries: PANEL_SEARCH_FOCUS_RETRIES,
        },
    ))
}

fn panel_origin_label(origin: &crate::surface::panel::PanelOrigin) -> &'static str {
    match origin {
        crate::surface::panel::PanelOrigin::Bar => "bar",
        crate::surface::panel::PanelOrigin::Module(_) => "module",
    }
}

fn panel_content_label(content: &crate::surface::panel::PanelContent) -> &'static str {
    match content {
        crate::surface::panel::PanelContent::BarPanel => "bar-panel",
        crate::surface::panel::PanelContent::ModulePanel(_) => "module-panel",
        crate::surface::panel::PanelContent::Menu(_) => "menu",
        crate::surface::panel::PanelContent::DmenuRequest => "dmenu-request",
        crate::surface::panel::PanelContent::ExternalProvider(_) => "external-provider",
    }
}

fn panel_window_settings(plan: PanelPhysicalPlan) -> NewLayerShellSettings {
    let anchor = panel_layer_anchor(&plan.edge, plan.width.fills(), plan.height.fills());
    let mut margin = panel_layer_margin(
        &plan.edge,
        plan.placement.layer_margin(&plan.edge),
        plan.width,
        plan.height,
    );
    match plan.edge {
        crate::config::Edge::Left => margin.3 += plan.edge_offset,
        crate::config::Edge::Right => margin.1 += plan.edge_offset,
        crate::config::Edge::Top => margin.0 += plan.edge_offset,
        crate::config::Edge::Bottom => margin.2 += plan.edge_offset,
    }

    NewLayerShellSettings {
        anchor,
        // Respect other reserved bars. Push-bar drawers overlap their own
        // bar's original reservation through the edge margin above.
        exclusive_zone: Some(0),
        size: Some(plan.layer_size()),
        margin: Some(margin),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
        output_option: plan
            .output_name
            .map(OutputOption::OutputName)
            .unwrap_or(OutputOption::Active),
        namespace: Some(App::namespace()),
        ..Default::default()
    }
}

fn panel_layer_anchor(edge: &crate::config::Edge, fill_width: bool, fill_height: bool) -> Anchor {
    match (edge, fill_width, fill_height) {
        (crate::config::Edge::Top, false, false) => Anchor::Top | Anchor::Left,
        (crate::config::Edge::Top, true, false) => Anchor::Top | Anchor::Left | Anchor::Right,
        (crate::config::Edge::Top, false, true) => Anchor::Top | Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Top, true, true) => {
            Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
        }
        (crate::config::Edge::Bottom, false, false) => Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Bottom, true, false) => Anchor::Bottom | Anchor::Left | Anchor::Right,
        (crate::config::Edge::Bottom, false, true) => Anchor::Top | Anchor::Bottom | Anchor::Left,
        (crate::config::Edge::Bottom, true, true) => {
            Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
        }
        (crate::config::Edge::Left, false, false) => Anchor::Left | Anchor::Top,
        (crate::config::Edge::Left, true, false) => Anchor::Left | Anchor::Right | Anchor::Top,
        (crate::config::Edge::Left, false, true) => Anchor::Left | Anchor::Top | Anchor::Bottom,
        (crate::config::Edge::Left, true, true) => {
            Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom
        }
        (crate::config::Edge::Right, false, false) => Anchor::Right | Anchor::Top,
        (crate::config::Edge::Right, true, false) => Anchor::Left | Anchor::Right | Anchor::Top,
        (crate::config::Edge::Right, false, true) => Anchor::Right | Anchor::Top | Anchor::Bottom,
        (crate::config::Edge::Right, true, true) => {
            Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom
        }
    }
}

fn panel_layer_margin(
    edge: &crate::config::Edge,
    margin: (i32, i32, i32, i32),
    width: PlannedDimension,
    height: PlannedDimension,
) -> (i32, i32, i32, i32) {
    let (top, mut right, mut bottom, mut left) =
        crate::surface::placement::usable_area_margin(edge, margin, 0);
    if width.fills() && matches!(edge, crate::config::Edge::Top | crate::config::Edge::Bottom) {
        right = 0;
        left = 0;
    }
    if height.fills() && matches!(edge, crate::config::Edge::Left | crate::config::Edge::Right) {
        bottom = 0;
    }
    (top, right, bottom, left)
}

impl App {
    #[cfg(test)]
    pub(super) fn set_external_panel_response_for_test(
        &mut self,
        provider_name: &str,
        response: Result<crate::surface::panel::ExternalPanelDocument, String>,
    ) {
        self.external_panel_responses_for_test
            .insert(provider_name.to_string(), response);
    }

    #[cfg(test)]
    pub(super) fn set_external_panel_event_response_for_test(
        &mut self,
        provider_name: &str,
        element_id: &str,
        response: crate::surface::panel::ExternalPanelDocument,
    ) {
        self.external_panel_event_responses_for_test.insert(
            (provider_name.to_string(), element_id.to_string()),
            response,
        );
    }

    #[cfg(test)]
    pub(super) fn set_pending_external_panel_session_for_test(&mut self, session_id: &str) {
        self.external_panel_session.set_pending_for_test(session_id);
    }

    #[cfg(test)]
    pub(super) fn active_external_panel_session_id_for_test(&self) -> Option<&str> {
        self.external_panel_session.active_session_id()
    }

    pub(super) fn handle_external_panel_event(
        &mut self,
        event: super::messages::ExternalPanelEvent,
    ) -> Task<Message> {
        match event {
            super::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id,
                origin_window,
                origin,
                provider_name,
                anchor,
                session,
                content,
            } => {
                let effects = self.external_panel_session.handshake_completed(
                    session_id.clone(),
                    origin_window,
                    origin,
                    provider_name.clone(),
                    anchor,
                    (session, content),
                );
                if effects.is_empty() {
                    tracing::debug!(
                        target: "canopa_shell::external_panel",
                        provider = provider_name.as_str(),
                        session = session_id.as_str(),
                        "ignoring stale external panel handshake"
                    );
                }
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ProviderFailed {
                session_id,
                origin_window,
                origin,
                provider_name,
                anchor,
                message,
            } => {
                let effects = self.external_panel_session.provider_failed(
                    session_id.clone(),
                    origin_window,
                    origin,
                    provider_name.clone(),
                    anchor,
                    message,
                );
                if effects.is_empty() {
                    tracing::debug!(
                        target: "canopa_shell::external_panel",
                        provider = provider_name.as_str(),
                        session = session_id.as_str(),
                        "ignoring stale external panel provider failure"
                    );
                }
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ProviderContentReceived {
                session_id,
                provider_name,
                content,
            } => {
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.provider_content_received(
                    session_id.clone(),
                    provider_name.clone(),
                    content,
                    active_panel.as_ref(),
                );
                if effects.is_empty() {
                    tracing::debug!(
                        target: "canopa_shell::external_panel",
                        provider = provider_name.as_str(),
                        session = session_id.as_str(),
                        "ignoring stale external panel content"
                    );
                }
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ProviderEventSent {
                session_id,
                provider_name,
            } => {
                let active_panel = self.active_external_panel_identity();
                if !self.external_panel_session.provider_event_sent(
                    &session_id,
                    &provider_name,
                    active_panel.as_ref(),
                ) {
                    tracing::debug!(
                        target: "canopa_shell::external_panel",
                        provider = provider_name.as_str(),
                        session = session_id.as_str(),
                        "ignoring stale external panel event write completion"
                    );
                }
                Task::none()
            }
            super::messages::ExternalPanelEvent::ProviderSessionFailed {
                session_id,
                provider_name,
                message,
            } => {
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.provider_session_failed(
                    session_id.clone(),
                    provider_name.clone(),
                    message.clone(),
                    active_panel.as_ref(),
                );
                if effects.is_empty() {
                    tracing::debug!(
                        target: "canopa_shell::external_panel",
                        provider = provider_name.as_str(),
                        session = session_id.as_str(),
                        "ignoring stale external panel session failure"
                    );
                    return Task::none();
                }
                tracing::error!(
                    target: "canopa_shell::external_panel",
                    provider = provider_name.as_str(),
                    session = session_id.as_str(),
                    error = message.as_str(),
                    "external panel provider session failed"
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ProviderSessionEnded {
                session_id,
                provider_name,
            } => {
                let active_panel = self.active_external_panel_identity();
                self.external_panel_session.provider_session_ended(
                    &session_id,
                    &provider_name,
                    active_panel.as_ref(),
                );
                Task::none()
            }
            super::messages::ExternalPanelEvent::ButtonActivated {
                provider_name,
                element_id,
            } => {
                #[cfg(test)]
                if let Some(content) = self
                    .external_panel_event_responses_for_test
                    .get(&(provider_name.clone(), element_id.clone()))
                    .cloned()
                {
                    let active_panel = self.active_external_panel_identity();
                    let effects = self
                        .external_panel_session
                        .replace_content_from_provider_response(
                            provider_name,
                            content,
                            active_panel.as_ref(),
                        );
                    return self.apply_external_panel_session_effects(effects);
                }
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.button_activated(
                    provider_name,
                    element_id,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ListSearchChanged {
                provider_name,
                list_id,
                query,
            } => {
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.list_search_changed(
                    provider_name,
                    list_id,
                    query,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ListEntryActivated {
                provider_name,
                list_id,
                entry_id,
            } => {
                #[cfg(test)]
                if let Some(content) = self
                    .external_panel_event_responses_for_test
                    .get(&(provider_name.clone(), entry_id.clone()))
                    .cloned()
                {
                    let active_panel = self.active_external_panel_identity();
                    let effects = self
                        .external_panel_session
                        .replace_content_from_provider_response(
                            provider_name,
                            content,
                            active_panel.as_ref(),
                        );
                    return self.apply_external_panel_session_effects(effects);
                }
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.list_entry_activated(
                    provider_name,
                    list_id,
                    entry_id,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::ListCustomValueSubmitted {
                provider_name,
                list_id,
                value,
            } => {
                #[cfg(test)]
                if let Some(content) = self
                    .external_panel_event_responses_for_test
                    .get(&(provider_name.clone(), value.clone()))
                    .cloned()
                {
                    let active_panel = self.active_external_panel_identity();
                    let effects = self
                        .external_panel_session
                        .replace_content_from_provider_response(
                            provider_name,
                            content,
                            active_panel.as_ref(),
                        );
                    return self.apply_external_panel_session_effects(effects);
                }
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.list_custom_value_submitted(
                    provider_name,
                    list_id,
                    value,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::TextInputChanged {
                provider_name,
                element_id,
                value,
            } => {
                #[cfg(test)]
                if let Some(content) = self
                    .external_panel_event_responses_for_test
                    .get(&(provider_name.clone(), element_id.clone()))
                    .cloned()
                {
                    let active_panel = self.active_external_panel_identity();
                    let effects = self
                        .external_panel_session
                        .replace_content_from_provider_response(
                            provider_name,
                            content,
                            active_panel.as_ref(),
                        );
                    return self.apply_external_panel_session_effects(effects);
                }
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.text_input_changed(
                    provider_name,
                    element_id,
                    value,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
            super::messages::ExternalPanelEvent::TextInputSubmitted {
                provider_name,
                element_id,
                value,
            } => {
                #[cfg(test)]
                if let Some(content) = self
                    .external_panel_event_responses_for_test
                    .get(&(provider_name.clone(), element_id.clone()))
                    .cloned()
                {
                    let active_panel = self.active_external_panel_identity();
                    let effects = self
                        .external_panel_session
                        .replace_content_from_provider_response(
                            provider_name,
                            content,
                            active_panel.as_ref(),
                        );
                    return self.apply_external_panel_session_effects(effects);
                }
                let active_panel = self.active_external_panel_identity();
                let effects = self.external_panel_session.text_input_submitted(
                    provider_name,
                    element_id,
                    value,
                    active_panel.as_ref(),
                );
                self.apply_external_panel_session_effects(effects)
            }
        }
    }

    pub(super) fn close_external_panel_session_for_replacement(&mut self) {
        self.external_panel_session.close();
    }

    fn active_panel_is_external_provider(&self) -> bool {
        self.foreground.active_panel_tree().is_some_and(|tree| {
            matches!(
                tree.content,
                crate::surface::panel::PanelContent::ExternalProvider(_)
            )
        })
    }

    pub(super) fn panel_window_is_external_provider(&self, window: iced::window::Id) -> bool {
        self.foreground
            .panel_tree_for_window(window)
            .is_some_and(|tree| {
                matches!(
                    tree.content,
                    crate::surface::panel::PanelContent::ExternalProvider(_)
                )
            })
    }

    pub(super) fn active_external_panel_identity(&self) -> Option<ExternalPanelPanelIdentity> {
        let tree = self.foreground.active_panel_tree()?;
        let crate::surface::panel::PanelContent::ExternalProvider(content) = &tree.content else {
            return None;
        };
        Some(ExternalPanelPanelIdentity {
            origin_window: tree.origin_window,
            origin: tree.origin.clone(),
            provider_name: content.provider_name.clone(),
        })
    }

    pub(super) fn apply_external_panel_session_effects(
        &mut self,
        effects: Vec<ExternalPanelSessionEffect>,
    ) -> Task<Message> {
        Task::batch(
            effects
                .into_iter()
                .map(|effect| self.apply_external_panel_session_effect(effect)),
        )
    }

    fn apply_external_panel_session_effect(
        &mut self,
        effect: ExternalPanelSessionEffect,
    ) -> Task<Message> {
        match effect {
            ExternalPanelSessionEffect::OpenPanel {
                origin_window,
                origin,
                content,
                anchor,
            } => self.open_panel_internal(
                origin_window,
                origin,
                crate::surface::panel::PanelContent::ExternalProvider(content),
                anchor,
                true,
            ),
            ExternalPanelSessionEffect::UpdatePanelContent { identity, content } => {
                let effects = self.foreground.handle(
                    super::foreground::ForegroundIntent::UpdateExternalPanelContent {
                        identity,
                        content,
                    },
                );
                Task::batch(self.apply_foreground_effects(effects))
            }
            ExternalPanelSessionEffect::WatchProviderSession {
                session_id,
                provider_name,
                handle,
            } => self.watch_external_panel_session(session_id, provider_name, handle),
            ExternalPanelSessionEffect::WatchProviderContent {
                session_id,
                provider_name,
                handle,
            } => self.watch_external_panel_content(session_id, provider_name, handle),
            ExternalPanelSessionEffect::SendProviderEvent {
                session_id,
                provider_name,
                handle,
                event,
            } => self.send_external_panel_event(session_id, provider_name, handle, event),
        }
    }

    fn send_external_panel_event(
        &self,
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
        event: crate::external_panel::protocol::ProviderEvent,
    ) -> Task<Message> {
        use iced_layershell::actions::{
            ActivationTokenCallback, LayerShellCustomAction, LayerShellCustomActionWithId,
        };
        let (request, token) = if handle.requests_activation(&event)
            && let Some(window) = self.foreground.active_panel_window()
        {
            let (complete, received) = tokio::sync::oneshot::channel();
            let complete = std::sync::Arc::new(std::sync::Mutex::new(Some(complete)));
            let callback = ActivationTokenCallback::new(move |token| {
                if let Some(complete) = complete.lock().unwrap().take() {
                    let _ = complete.send(token);
                }
            });
            (
                Task::done(Message::LayerShell(LayerShellCustomActionWithId::new(
                    Some(window),
                    LayerShellCustomAction::RequestActivationToken {
                        app_id: String::new(),
                        callback,
                    },
                ))),
                Some((received, std::time::Instant::now())),
            )
        } else {
            (Task::none(), None)
        };
        let dispatch = Task::perform(
            async move {
                let token = if let Some((received, started)) = token {
                    tokio::time::timeout(std::time::Duration::from_secs(1), received)
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .flatten()
                        .filter(|token| {
                            started.elapsed() < std::time::Duration::from_secs(1)
                                && !token.is_empty()
                                && token.len()
                                    <= canopa_panel_protocol::v5::MAX_ACTIVATION_TOKEN_BYTES
                                && !token.contains('\0')
                        })
                } else {
                    None
                };
                match tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    if token.is_some() {
                        handle.send_event_with_activation(event, token).await
                    } else {
                        handle.send_event(event).await
                    }
                })
                .await
                {
                    Ok(Ok(())) => super::messages::ExternalPanelEvent::ProviderEventSent {
                        session_id,
                        provider_name,
                    },
                    Ok(Err(err)) => super::messages::ExternalPanelEvent::ProviderSessionFailed {
                        session_id,
                        provider_name,
                        message: err.to_string(),
                    },
                    Err(_) => super::messages::ExternalPanelEvent::ProviderSessionFailed {
                        session_id,
                        provider_name: provider_name.clone(),
                        message: format!("provider {provider_name:?} event timed out"),
                    },
                }
            },
            Message::ExternalPanel,
        );
        Task::batch([request, dispatch])
    }

    fn watch_external_panel_session(
        &self,
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
    ) -> Task<Message> {
        Task::perform(
            async move {
                match handle.wait_for_exit().await {
                    Ok(Some(message)) => {
                        super::messages::ExternalPanelEvent::ProviderSessionFailed {
                            session_id,
                            provider_name,
                            message,
                        }
                    }
                    Ok(None) => super::messages::ExternalPanelEvent::ProviderSessionEnded {
                        session_id,
                        provider_name,
                    },
                    Err(err) => super::messages::ExternalPanelEvent::ProviderSessionFailed {
                        session_id,
                        provider_name,
                        message: err.to_string(),
                    },
                }
            },
            Message::ExternalPanel,
        )
    }

    fn watch_external_panel_content(
        &self,
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
    ) -> Task<Message> {
        Task::perform(
            async move {
                match handle.read_next_content().await {
                    Ok(content) => super::messages::ExternalPanelEvent::ProviderContentReceived {
                        session_id,
                        provider_name,
                        content,
                    },
                    Err(err) => super::messages::ExternalPanelEvent::ProviderSessionFailed {
                        session_id,
                        provider_name,
                        message: err.to_string(),
                    },
                }
            },
            Message::ExternalPanel,
        )
    }

    pub(super) fn suspend_bar_keyboard_interactivity(&mut self) -> Vec<Task<Message>> {
        let ids: Vec<iced::window::Id> = self.surfaces.keys().copied().collect();
        ids.into_iter()
            .map(|id| {
                Task::done(Message::KeyboardInteractivityChange {
                    id,
                    keyboard_interactivity: KeyboardInteractivity::None,
                })
            })
            .collect()
    }

    pub(super) fn dispatch_panel_action(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    ) -> Task<Message> {
        match action {
            crate::config::PanelAction::Command(command) => {
                Task::batch(self.apply_app_effects(vec![AppEffect::RunConfiguredCommand(command)]))
            }
            crate::config::PanelAction::Launch(argv) => {
                Task::batch(self.apply_app_effects(vec![AppEffect::LaunchApplication(argv)]))
            }
            crate::config::PanelAction::Session(action) => self.request_session_action(action),
            crate::config::PanelAction::OpenProvider(provider_name) => {
                self.open_provider_panel(origin_window, origin, provider_name, None)
            }
            action => match panel_event_for_action(origin_window, origin, action, None) {
                Some(event) => self.handle_panel_event(event),
                None => Task::none(),
            },
        }
    }

    fn open_provider_panel(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
    ) -> Task<Message> {
        let session_id = crate::external_panel::next_session_id();
        self.external_panel_session.begin_open_provider(
            session_id.clone(),
            super::external_panel_session::ExternalPanelPanelIdentity {
                origin_window,
                origin: origin.clone(),
                provider_name: provider_name.clone(),
            },
        );

        #[cfg(test)]
        if let Some(response) = self
            .external_panel_responses_for_test
            .get(&provider_name)
            .cloned()
        {
            let event = match response {
                Ok(content) => super::messages::ExternalPanelEvent::HandshakeCompleted {
                    session_id,
                    origin_window,
                    origin,
                    provider_name,
                    anchor,
                    session: None,
                    content,
                },
                Err(message) => super::messages::ExternalPanelEvent::ProviderFailed {
                    session_id,
                    origin_window,
                    origin,
                    provider_name,
                    anchor,
                    message,
                },
            };
            return self.handle_external_panel_event(event);
        }

        let provider_catalog =
            crate::external_panel::PanelProviderCatalog::new(&self.config.panel_providers);
        let provider_protocol = provider_catalog.protocol(&provider_name);
        let provider_command = match provider_catalog.resolve(&provider_name) {
            Ok(command) => command,
            Err(crate::external_panel::PanelProviderResolutionError::NotConfigured) => {
                tracing::error!(
                    target: "canopa_shell::external_panel",
                    provider = provider_name.as_str(),
                    "external panel provider is not configured"
                );
                return self.handle_external_panel_event(
                    super::messages::ExternalPanelEvent::ProviderFailed {
                        session_id,
                        origin_window,
                        origin,
                        provider_name: provider_name.clone(),
                        anchor,
                        message: format!("provider {provider_name:?} is not configured"),
                    },
                );
            }
            Err(crate::external_panel::PanelProviderResolutionError::BuiltinUnavailable) => {
                tracing::error!(
                    target: "canopa_shell::external_panel",
                    provider = provider_name.as_str(),
                    "external panel built-in provider has no launch command"
                );
                return self.handle_external_panel_event(
                    super::messages::ExternalPanelEvent::ProviderFailed {
                        session_id,
                        origin_window,
                        origin,
                        provider_name: provider_name.clone(),
                        anchor,
                        message: format!("provider {provider_name:?} is not available"),
                    },
                );
            }
        };

        let process = match crate::external_panel::spawn_provider_process(&provider_command) {
            Ok(process) => process,
            Err(err) => {
                tracing::error!(
                    target: "canopa_shell::external_panel",
                    provider = provider_name.as_str(),
                    error = %err,
                    "external panel provider launch failed"
                );
                return self.handle_external_panel_event(
                    super::messages::ExternalPanelEvent::ProviderFailed {
                        session_id,
                        origin_window,
                        origin,
                        provider_name: provider_name.clone(),
                        anchor,
                        message: format!("could not launch provider {provider_name:?}: {err}"),
                    },
                );
            }
        };

        let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
        let handshake = crate::external_panel::protocol::HostHandshake {
            protocol_version: crate::external_panel::protocol::PROTOCOL_VERSION,
            session_id: session_id.clone(),
            provider_name: provider_name.clone(),
            origin: crate::external_panel::protocol::ProviderOrigin {
                bar_name: origin_surface.bar_name,
                output_name: origin_surface.output_name,
                origin_kind: panel_origin_label(&origin).into(),
            },
        };
        Task::perform(
            async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    process.start_session_for_protocol(handshake, provider_protocol),
                )
                .await
                {
                    Ok(Ok(session)) => super::messages::ExternalPanelEvent::HandshakeCompleted {
                        session_id,
                        origin_window,
                        origin,
                        provider_name,
                        anchor,
                        session: Some(session.handle),
                        content: session.content,
                    },
                    Ok(Err(err)) => super::messages::ExternalPanelEvent::ProviderFailed {
                        session_id,
                        origin_window,
                        origin,
                        provider_name: provider_name.clone(),
                        anchor,
                        message: err.to_string(),
                    },
                    Err(_) => super::messages::ExternalPanelEvent::ProviderFailed {
                        session_id,
                        origin_window,
                        origin,
                        provider_name: provider_name.clone(),
                        anchor,
                        message: format!("provider {provider_name:?} handshake timed out"),
                    },
                }
            },
            Message::ExternalPanel,
        )
    }

    pub(super) fn close_active_panel(&mut self) -> Vec<Task<Message>> {
        self.close_external_panel_session_for_replacement();
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::ClosePanel);
        self.apply_foreground_effects(effects)
    }

    pub(super) fn handle_panel_focus_gained(&mut self, window: iced::window::Id) -> Task<Message> {
        if self.foreground.is_menu_panel_window(window) {
            let effects = self
                .foreground
                .handle(super::foreground::ForegroundIntent::Panel(
                    super::foreground::PanelInteraction::RequestSearchFocus,
                ));
            return Task::batch(self.apply_foreground_effects(effects));
        }
        Task::none()
    }

    pub(super) fn handle_panel_event(
        &mut self,
        event: super::messages::PanelEvent,
    ) -> Task<Message> {
        use super::messages::PanelEvent;

        match event {
            PanelEvent::MotionReady(window) => iced::window::scale_factor(window)
                .map(move |scale| Message::Panel(PanelEvent::MotionScale { window, scale })),
            PanelEvent::MotionScale { window, scale } => {
                self.foreground.panel_motion_ready(window, scale);
                Task::none()
            }
            PanelEvent::MotionFinished { window, closing } => {
                let effects = self.foreground.finish_panel_motion(window, closing);
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::IconsLoaded(loaded) => {
                self.panel_icons.complete(loaded);
                Task::none()
            }
            PanelEvent::Launcher(event) => self.handle_launcher_event(event),
            PanelEvent::FocusLauncher(target) => {
                if self
                    .foreground
                    .launcher()
                    .is_some_and(|active| active.accepts(&target))
                {
                    let menu = self.foreground.active_panel_menu_render_state();
                    if let Some(position) = menu.entries.iter().position(|entry| matches!(&entry.target, super::panel_menu_session::PanelMenuEntryTarget::Launcher(current) if current.identity() == target.identity())) {
                        return self.handle_panel_event(PanelEvent::FocusIndex(position));
                    }
                }
                Task::none()
            }
            PanelEvent::LauncherAction(action) => {
                let menu = self.foreground.active_panel_menu_render_state();
                if let Some(position) = menu.focused_index
                    && let Some(entry) = menu.entries.get(position)
                    && let super::panel_menu_session::PanelMenuEntryTarget::Launcher(target) =
                        &entry.target
                {
                    return self
                        .handle_panel_event(PanelEvent::ActivateLauncher(target.clone(), action));
                }
                Task::none()
            }
            PanelEvent::LauncherSearchChanged { session, query } => {
                if self
                    .foreground
                    .launcher()
                    .is_some_and(|active| active.id == session)
                {
                    self.handle_panel_event(PanelEvent::SearchChanged(query))
                } else {
                    Task::none()
                }
            }
            PanelEvent::LauncherSetQuery { session, query } => self.set_launcher_input(
                session,
                super::foreground::PanelInteraction::SearchChanged(query),
            ),
            PanelEvent::LauncherSelectScope { session, scope } => self.set_launcher_input(
                session,
                super::foreground::PanelInteraction::LauncherScope(scope),
            ),
            PanelEvent::LauncherActions(target) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::LauncherActions(target),
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::ActivateLauncher(target, action) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::ActivateLauncher(target, action),
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::InstalledAppsLoaded { generation, apps } => {
                let effects =
                    self.foreground
                        .installed_apps_loaded(generation, apps, &self.config.menus);
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            } => self.open_panel(origin_window, origin, content, anchor),
            PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            } => {
                self.close_external_panel_session_for_replacement();
                let refresh = self.refresh_session_state_for(&content);
                let request = self.panel_open_request(origin_window, origin, content, anchor);
                let module_detail_sessions = self.module_detail_sessions_for_request(&request);
                let menus = super::session_actions::offered_menus(
                    &self.config.menus,
                    &self.session.capabilities,
                );
                let context =
                    super::foreground::PanelOpenContext::new(&menus, module_detail_sessions);
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::TogglePanel { request, context });
                drop(menus);
                Task::batch(
                    self.apply_foreground_effects(effects)
                        .into_iter()
                        .chain([refresh]),
                )
            }
            PanelEvent::OpenProvider {
                origin_window,
                origin,
                provider_name,
                anchor,
            } => self.open_provider_panel(origin_window, origin, provider_name, anchor),
            PanelEvent::Dismiss(window) => {
                if self.foreground.active_panel_window() == Some(window) {
                    Task::batch(self.close_active_panel())
                } else {
                    Task::none()
                }
            }
            PanelEvent::Close => Task::batch(self.close_active_panel()),
            PanelEvent::SearchChanged(query) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::SearchChanged(query),
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::MenuScrolled { y, viewport_height } => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::MenuScrolled { y, viewport_height },
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::MenuScrollApplied => Task::none(),
            PanelEvent::FocusMoveNext => self.move_panel_focus(1),
            PanelEvent::FocusMovePrev => self.move_panel_focus(-1),
            PanelEvent::FocusIndex(index) => {
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::FocusIndex(index),
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
            PanelEvent::FocusSearchInput { retries } => self.focus_panel_search_input(retries),
            PanelEvent::Activate => self.activate_panel_entry(None),
            PanelEvent::ActivateIndex(index) => self.activate_panel_entry(Some(index)),
            PanelEvent::ModuleInteraction {
                window,
                target,
                event,
            } => self.interact_panel_module(window, target, event),
            PanelEvent::Back => {
                if self.foreground.panel_return_to_bar()
                    && let Some(tree) = self.foreground.active_panel_tree().cloned()
                {
                    return self.open_panel(
                        tree.origin_window,
                        tree.origin,
                        crate::surface::panel::PanelContent::BarPanel,
                        tree.anchor,
                    );
                }
                let effects = self
                    .foreground
                    .handle(super::foreground::ForegroundIntent::Panel(
                        super::foreground::PanelInteraction::Back,
                    ));
                Task::batch(self.apply_foreground_effects(effects))
            }
        }
    }

    fn open_panel(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> Task<Message> {
        self.open_panel_internal(origin_window, origin, content, anchor, false)
    }

    fn open_panel_internal(
        &mut self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
        preserve_external_session: bool,
    ) -> Task<Message> {
        if !preserve_external_session {
            self.close_external_panel_session_for_replacement();
        }
        let refresh = self.refresh_session_state_for(&content);
        let request = self.panel_open_request(origin_window, origin, content, anchor);
        let module_detail_sessions = self.module_detail_sessions_for_request(&request);
        let menus =
            super::session_actions::offered_menus(&self.config.menus, &self.session.capabilities);
        let context = super::foreground::PanelOpenContext::new(&menus, module_detail_sessions);
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::OpenPanel { request, context });
        drop(menus);
        Task::batch(
            self.apply_foreground_effects(effects)
                .into_iter()
                .chain([refresh]),
        )
    }

    pub(super) fn panel_open_request(
        &self,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    ) -> super::foreground::PanelForegroundRequest {
        let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
        super::foreground::PanelForegroundRequest {
            origin_window,
            origin,
            content,
            anchor,
            origin_bar_name: origin_surface.bar_name,
            origin_output_name: origin_surface.output_name,
            screen_size: origin_surface.screen_size,
        }
    }

    #[cfg(test)]
    pub(super) fn prepare_panel_for_test(
        &mut self,
        request: super::foreground::PanelForegroundRequest,
    ) -> Vec<super::foreground::ForegroundEffect> {
        let module_detail_sessions = self.module_detail_sessions_for_request(&request);
        let context = super::foreground::PanelOpenContext::new(
            self.config.menus.as_slice(),
            module_detail_sessions,
        );
        self.foreground
            .handle(super::foreground::ForegroundIntent::OpenPanel { request, context })
    }

    pub(super) fn module_detail_sessions_for_request(
        &self,
        request: &super::foreground::PanelForegroundRequest,
    ) -> std::collections::HashMap<crate::modules::ModuleId, crate::modules::ModuleDetailSession>
    {
        self.panel_module_layout_for_request(request)
            .into_iter()
            .filter_map(|entry| {
                self.module_detail_session_for(&entry.id)
                    .map(|session| (entry.id, session))
            })
            .collect()
    }

    fn module_detail_session_for(
        &self,
        id: &crate::modules::ModuleId,
    ) -> Option<crate::modules::ModuleDetailSession> {
        self.modules.create_module_detail_session(id)
    }

    pub(super) fn panel_module_layout_for_request(
        &self,
        request: &super::foreground::PanelForegroundRequest,
    ) -> Vec<crate::surface::panel::PanelModuleEntry> {
        self.panel_module_layout_for_content(
            request.origin_window,
            &request.origin,
            &request.content,
        )
    }

    pub(super) fn panel_module_layout_for_content(
        &self,
        origin_window: iced::window::Id,
        origin: &crate::surface::panel::PanelOrigin,
        content: &crate::surface::panel::PanelContent,
    ) -> Vec<crate::surface::panel::PanelModuleEntry> {
        match content {
            crate::surface::panel::PanelContent::BarPanel => {
                let origin_surface = self.surface(origin_window).cloned().unwrap_or_default();
                let bar = Self::bar_for_surface_name(&self.config.bars, &origin_surface.bar_name);
                self.effective_panel_module_layout_for_bar(bar)
                    .into_iter()
                    .filter(|(_, id)| {
                        self.module_offered(id)
                            && self.modules.kind_for(id).is_some_and(|kind| {
                                self.modules.has_bar_panel_content(
                                    id,
                                    &self.module_render_state(kind, id, &origin_surface),
                                )
                            })
                    })
                    .map(|(zone, id)| {
                        let focused = self.foreground.focused_panel_module().map_or(
                            origin == &crate::surface::panel::PanelOrigin::Module(id.clone()),
                            |focused| focused == &id,
                        );
                        crate::surface::panel::PanelModuleEntry { id, zone, focused }
                    })
                    .collect()
            }
            crate::surface::panel::PanelContent::ModulePanel(id) => {
                let surface = self.surface(origin_window).cloned().unwrap_or_default();
                if !self.module_has_content_on_surface(
                    id,
                    &surface,
                    crate::config::FormatMode::Expanded,
                ) {
                    return Vec::new();
                }
                vec![crate::surface::panel::PanelModuleEntry {
                    id: id.clone(),
                    zone: crate::modules::Zone::Center,
                    focused: origin == &crate::surface::panel::PanelOrigin::Module(id.clone()),
                }]
            }
            crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest
            | crate::surface::panel::PanelContent::ExternalProvider(_) => Vec::new(),
        }
    }

    fn module_detail_preferred_content_size(
        &self,
        id: &crate::modules::ModuleId,
        bar: &crate::config::BarConfig,
        origin_surface: &crate::surface::SurfaceState,
    ) -> Option<crate::surface::panel::PanelPreferredSize> {
        let kind = self.modules.kind_for(id)?;
        let state = self.module_render_state(kind, id, origin_surface);
        self.modules
            .module_detail_preferred_size(super::module_runtime::ModuleDetailSizeRequest {
                id: id.clone(),
                style: &bar.style,
                session: self.foreground.active_panel_module_detail_session(id),
                state_override: Some(state),
            })
    }

    pub(super) fn open_panel_window(
        &mut self,
        tree: crate::surface::panel::PanelTree,
    ) -> Task<Message> {
        let origin_surface = self
            .surface(tree.origin_window)
            .cloned()
            .unwrap_or_default();
        let bar = Self::bar_for_surface_name(&self.config.bars, &tree.origin_bar_name);
        let panel_module_layout =
            self.panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content);
        let menu = self.foreground.active_panel_menu_render_state();
        let plan = super::panel_sizing::panel_physical_plan(PanelSizingInput {
            bar,
            ui: self.panel_ui(&tree),
            menu_row_extent: self.panel_menu_metrics(&tree).extent,
            tree: &tree,
            // Layer-shell places this panel relative to the usable area.
            // Its origin already includes bars from this and other processes.
            surface_origin: crate::surface::placement::Point::default(),
            panel_module_layout: &panel_module_layout,
            menu,
            module_detail_preferred_size: |id| {
                self.module_detail_preferred_content_size(id, bar, &origin_surface)
            },
        });
        let drawer_width = if bar.drawer_pushes_bar() && bar.uses_drawer_for(&tree.content) {
            plan.width.value()
        } else {
            0
        };
        let edge = bar.edge.clone();
        let animation = if self.config.appearance.reduced_motion() {
            crate::config::PanelAnimationConfig {
                open_ms: 0.0,
                close_ms: 0.0,
            }
        } else {
            bar.panel.animation.clone()
        };
        let settings = panel_window_settings(plan);
        let resize_bar = self.set_drawer_bar_width(tree.origin_window, drawer_width);
        let (panel_window, open_task) = Message::layershell_open(settings);
        self.surface_mut(tree.origin_window).tooltip_pending = None;
        tracing::debug!(
            target: "canopa_shell::panel",
            origin = panel_origin_label(&tree.origin),
            content = panel_content_label(&tree.content),
            origin_bar = tree.origin_bar_name.as_str(),
            origin_output = tree.origin_output_name.as_str(),
            has_anchor = tree.anchor.is_some(),
            ?panel_window,
            "opening panel window"
        );

        let mut tasks = vec![resize_bar];
        self.foreground
            .handle(super::foreground::ForegroundIntent::PanelWindowOpened {
                window: panel_window,
            });
        self.foreground
            .configure_panel_motion(panel_window, &edge, animation);

        let open_sequence = open_task.chain(iced::window::gain_focus(panel_window));
        tasks.push(open_sequence);
        Task::batch(tasks)
    }

    #[cfg(test)]
    pub(super) fn load_installed_apps_for_panel(
        &self,
    ) -> Vec<crate::desktop_apps::DesktopApplication> {
        #[cfg(test)]
        if let Some(apps) = self.foreground.installed_apps_for_test() {
            return apps.clone();
        }
        crate::desktop_apps::discover_installed_applications().unwrap_or_default()
    }

    pub(super) fn handle_panel_escape_pressed(&mut self) -> Task<Message> {
        if self.active_panel_is_external_provider() {
            self.close_external_panel_session_for_replacement();
        }
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::Panel(
                super::foreground::PanelInteraction::Escape,
            ));
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn move_panel_focus(&mut self, delta: isize) -> Task<Message> {
        if let Some(tree) = self.foreground.active_panel_tree()
            && matches!(tree.content, crate::surface::panel::PanelContent::BarPanel)
        {
            let entries: Vec<_> = self
                .panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content)
                .into_iter()
                .filter(|entry| {
                    self.modules
                        .bar_panel_activation(&entry.id, tree.origin_window)
                        .is_some()
                })
                .collect();
            if !entries.is_empty() {
                let index = self
                    .foreground
                    .focused_panel_module()
                    .and_then(|id| entries.iter().position(|entry| &entry.id == id));
                let next = index.map_or(if delta < 0 { entries.len() - 1 } else { 0 }, |index| {
                    (index as isize + delta).rem_euclid(entries.len() as isize) as usize
                });
                self.foreground.focus_panel_module(entries[next].id.clone());
            }
            return Task::none();
        }
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::PanelFocusMove {
                delta,
                scroll: super::foreground::PanelMenuScrollContext {
                    row_extent: self.active_menu_metrics().extent,
                    default_viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
                },
            });
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn activate_panel_entry(&mut self, index: Option<usize>) -> Task<Message> {
        if let Some(tree) = self.foreground.active_panel_tree()
            && matches!(tree.content, crate::surface::panel::PanelContent::BarPanel)
        {
            let target = index
                .and_then(|index| {
                    self.panel_module_layout_for_content(
                        tree.origin_window,
                        &tree.origin,
                        &tree.content,
                    )
                    .get(index)
                    .map(|entry| entry.id.clone())
                })
                .or_else(|| self.foreground.focused_panel_module().cloned());
            if let Some(target) = target
                && let Some(window) = self.foreground.active_panel_window()
                && let Some(event) = self.modules.bar_panel_activation(&target, window)
            {
                return self.interact_panel_module(window, target, event);
            }
            return Task::none();
        }
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::Panel(
                super::foreground::PanelInteraction::Activate(index),
            ));
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn interact_panel_module(
        &mut self,
        window: iced::window::Id,
        target: crate::modules::ModuleId,
        event: crate::modules::InteractionEvent,
    ) -> Task<Message> {
        let Some(tree) = self.foreground.active_panel_tree().cloned() else {
            return Task::none();
        };
        if self.foreground.active_panel_window() != Some(window)
            || !matches!(tree.content, crate::surface::panel::PanelContent::BarPanel)
            || !self
                .panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content)
                .iter()
                .any(|entry| entry.id == target)
        {
            return Task::none();
        }
        self.foreground.focus_panel_module(target.clone());
        let trigger = match event {
            crate::modules::InteractionEvent::Clicked => Some(crate::config::ActionTrigger::Click),
            crate::modules::InteractionEvent::RightClicked(_) => {
                Some(crate::config::ActionTrigger::RightClick)
            }
            _ => None,
        };
        if let Some(action) = trigger
            .and_then(|trigger| self.modules.action_for(&target, &trigger))
            .cloned()
        {
            // Use the active Panel's origin and anchor: this Module need not be on the Bar.
            let action_origin = if matches!(
                action,
                crate::config::PanelAction::OpenPanel | crate::config::PanelAction::TogglePanel
            ) {
                crate::surface::panel::PanelOrigin::Module(target.clone())
            } else {
                tree.origin.clone()
            };
            if let Some(event) = panel_event_for_action(
                tree.origin_window,
                action_origin.clone(),
                action.clone(),
                tree.anchor,
            ) {
                return self.handle_panel_event(event);
            }
            return self.dispatch_panel_action(tree.origin_window, action_origin, action);
        }
        self.modules
            .module_owned_interaction(&target, event, tree.origin_window)
            .map_or_else(Task::none, Task::done)
    }

    fn set_launcher_input(
        &mut self,
        session: u64,
        interaction: super::foreground::PanelInteraction,
    ) -> Task<Message> {
        if self
            .foreground
            .launcher()
            .is_none_or(|active| active.id != session)
        {
            return Task::none();
        }
        let Some(window) = self.foreground.active_panel_window() else {
            return Task::none();
        };
        let id = panel_search_input_id(window);
        let effects = self
            .foreground
            .handle(super::foreground::ForegroundIntent::Panel(interaction));
        let mut tasks = self.apply_foreground_effects(effects);
        tasks.push(iced::widget::operation::focus(id.clone()));
        tasks.push(iced::widget::operation::move_cursor_to_end(id));
        Task::batch(tasks)
    }

    fn focus_panel_search_input(&mut self, retries: u8) -> Task<Message> {
        if !self.foreground.panel_search_focus_pending() {
            return Task::none();
        }
        let Some(panel_window) = self.foreground.active_panel_window() else {
            self.foreground
                .handle(super::foreground::ForegroundIntent::Panel(
                    super::foreground::PanelInteraction::SearchFocusSettled,
                ));
            return Task::none();
        };
        let search_input_id = panel_search_input_id(panel_window);
        let mut tasks = vec![iced::widget::operation::focus(search_input_id)];
        if retries > 0 {
            tasks.push(Task::perform(
                async {
                    tokio::time::sleep(PANEL_SEARCH_FOCUS_RETRY_DELAY).await;
                },
                move |_| {
                    Message::Panel(super::messages::PanelEvent::FocusSearchInput {
                        retries: retries - 1,
                    })
                },
            ));
        } else {
            self.foreground
                .handle(super::foreground::ForegroundIntent::Panel(
                    super::foreground::PanelInteraction::SearchFocusSettled,
                ));
        }
        Task::batch(tasks)
    }
}

impl App {
    pub(super) fn handle_dmenu_selection_request(
        &mut self,
        request: crate::ipc::protocol::DmenuRequest,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        self.close_external_panel_session_for_replacement();
        let panel_request = self.default_panel_origin_window().map(|origin_window| {
            self.panel_open_request(
                origin_window,
                crate::surface::panel::PanelOrigin::Bar,
                crate::surface::panel::PanelContent::DmenuRequest,
                None,
            )
        });
        let effects =
            self.foreground
                .handle(super::foreground::ForegroundIntent::BeginDmenuSelection {
                    request,
                    responder,
                    panel_request,
                });
        Task::batch(self.apply_foreground_effects(effects))
    }

    fn default_panel_origin_window(&self) -> Option<iced::window::Id> {
        self.surfaces.keys().copied().next()
    }

    pub(super) fn handle_toggle_menu_request(
        &mut self,
        name: String,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        if let Some(effects) = self.foreground.close_panel_menu_if_open(&name) {
            super::app_effects::send_ipc_response(
                responder,
                crate::ipc::protocol::IpcResponse::Accepted,
            );
            return Task::batch(self.apply_foreground_effects(effects));
        }
        self.handle_open_menu_request(name, responder)
    }

    pub(super) fn handle_open_menu_request(
        &mut self,
        name: String,
        responder: super::messages::IpcResponder,
    ) -> Task<Message> {
        use crate::ipc::protocol::IpcResponse;

        let origin = if !self.config.menus.iter().any(|menu| menu.name == name) {
            Err(format!("unknown menu {name:?}"))
        } else {
            self.surfaces
                .select_global_panel_surface(self.niri.state().focused_output())
                .ok_or_else(|| "no bar surface is available".to_string())
        };
        let (task, response) = match origin {
            Ok(origin_window) => (
                self.handle_panel_event(super::messages::PanelEvent::Open {
                    origin_window,
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu(name),
                    anchor: None,
                }),
                IpcResponse::Accepted,
            ),
            Err(reason) => (Task::none(), IpcResponse::Rejected { reason }),
        };
        super::app_effects::send_ipc_response(responder, response);
        task
    }
}

fn content_for_origin(
    origin: &crate::surface::panel::PanelOrigin,
) -> crate::surface::panel::PanelContent {
    match origin {
        crate::surface::panel::PanelOrigin::Bar => crate::surface::panel::PanelContent::BarPanel,
        crate::surface::panel::PanelOrigin::Module(id) => {
            crate::surface::panel::PanelContent::ModulePanel(id.clone())
        }
    }
}

pub(super) fn panel_action_needs_anchor(action: &crate::config::PanelAction) -> bool {
    matches!(
        action,
        crate::config::PanelAction::OpenPanel
            | crate::config::PanelAction::TogglePanel
            | crate::config::PanelAction::OpenBarPanel
            | crate::config::PanelAction::ToggleBarPanel
            | crate::config::PanelAction::OpenMenu(_)
            | crate::config::PanelAction::ToggleMenu(_)
            | crate::config::PanelAction::OpenProvider(_)
    )
}

pub(super) fn panel_event_for_action(
    origin_window: iced::window::Id,
    origin: crate::surface::panel::PanelOrigin,
    action: crate::config::PanelAction,
    anchor: Option<iced::Rectangle>,
) -> Option<super::messages::PanelEvent> {
    match action {
        crate::config::PanelAction::OpenPanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Open {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::TogglePanel => {
            let content = content_for_origin(&origin);
            Some(super::messages::PanelEvent::Toggle {
                origin_window,
                origin,
                content,
                anchor,
            })
        }
        crate::config::PanelAction::OpenBarPanel => Some(super::messages::PanelEvent::Open {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor,
        }),
        crate::config::PanelAction::ToggleBarPanel => Some(super::messages::PanelEvent::Toggle {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor,
        }),
        crate::config::PanelAction::ClosePanel => Some(super::messages::PanelEvent::Close),
        crate::config::PanelAction::OpenMenu(name) => Some(super::messages::PanelEvent::Open {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::ToggleMenu(name) => Some(super::messages::PanelEvent::Toggle {
            origin_window,
            origin,
            content: crate::surface::panel::PanelContent::Menu(name),
            anchor,
        }),
        crate::config::PanelAction::OpenProvider(provider_name) => {
            Some(super::messages::PanelEvent::OpenProvider {
                origin_window,
                origin,
                provider_name,
                anchor,
            })
        }
        crate::config::PanelAction::Command(_)
        | crate::config::PanelAction::Launch(_)
        | crate::config::PanelAction::Session(_) => None,
    }
}

#[cfg(test)]
mod canopa_tests;

#[cfg(test)]
mod tests {
    mod ipc_tests;
    mod scroll_tests;

    use super::super::panel_menu_session::{
        PanelMenuEntryTarget as MenuEntryTarget, dmenu_entry_views, search_menu_entries,
    };
    use super::*;

    fn panel_position_for_test(
        edge: crate::config::Edge,
        bar_size: u32,
        panel_size: (u32, u32),
        anchor: Option<iced::Rectangle>,
        screen_size: (u32, u32),
        surface_origin: (i32, i32),
    ) -> (i32, i32) {
        let placement = crate::surface::placement::centered_position(
            &edge,
            anchor,
            crate::surface::placement::Size {
                width: panel_size.0,
                height: panel_size.1,
            },
            crate::surface::placement::Size {
                width: screen_size.0,
                height: screen_size.1,
            },
            crate::surface::placement::Point {
                x: surface_origin.0,
                y: surface_origin.1,
            },
            bar_size,
            0,
        );
        (placement.position.x, placement.position.y)
    }

    fn active_menu_entries(app: &App) -> Vec<super::super::panel_menu_session::PanelMenuEntry> {
        app.foreground.active_panel_menu_entries_for_test()
    }

    fn panel_plan_for_test(
        edge: crate::config::Edge,
        width: PlannedDimension,
        height: PlannedDimension,
        position: (i32, i32),
        screen: (u32, u32),
    ) -> PanelPhysicalPlan {
        PanelPhysicalPlan {
            edge,
            edge_offset: 0,
            width,
            height,
            placement: super::super::panel_sizing::PanelPlacement {
                position: crate::surface::placement::Point {
                    x: position.0,
                    y: position.1,
                },
                size: crate::surface::placement::Size {
                    width: width.value(),
                    height: height.value(),
                },
                screen: crate::surface::placement::Size {
                    width: screen.0,
                    height: screen.1,
                },
            },
            output_name: Some("DP-1".into()),
        }
    }

    #[test]
    fn dmenu_entries_use_input_as_label_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
        assert!(matches!(entries[0].target, MenuEntryTarget::DmenuValue(_)));
    }

    #[test]
    fn dmenu_search_filters_entries_case_insensitively() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Firefox"),
                crate::ipc::protocol::DmenuEntry::plain("Kitty"),
            ],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "fire");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "fire");
    }

    #[test]
    fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: true,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert_eq!(entries[0].label, "kitty");
        assert!(matches!(
            entries[0].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_no_custom_does_not_create_custom_entry() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: false,
        };
        let entries = dmenu_entry_views(&request, "kitty");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_rich_entries_use_label_icon_keywords_and_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Firefox".into()),
                    value: Some("org.mozilla.firefox".into()),
                    icon: Some("firefox".into()),
                    keywords: vec!["browser".into(), "web".into()],
                },
                crate::ipc::protocol::DmenuEntry {
                    label: Some("Kitty".into()),
                    value: Some("kitty".into()),
                    icon: None,
                    keywords: Vec::new(),
                },
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "browser");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[0].icon.as_deref(), Some("firefox"));
        assert_eq!(
            entries[0].target,
            MenuEntryTarget::DmenuValue("org.mozilla.firefox".into())
        );
        assert_eq!(entries[1].label, "browser");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_search_uses_fuzzy_ranking() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("Telemetry Renderer Module"),
                crate::ipc::protocol::DmenuEntry::plain("Terminal"),
            ],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "term");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Terminal", "Telemetry Renderer Module"]
        );
    }

    #[test]
    fn dmenu_custom_value_appears_after_fuzzy_matches() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("Firefox")],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, "firfox");

        assert_eq!(entries[0].label, "Firefox");
        assert_eq!(entries[1].label, "firfox");
        assert!(matches!(
            entries[1].target,
            MenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn dmenu_blank_query_preserves_duplicate_source_order() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
                crate::ipc::protocol::DmenuEntry::plain("kitty"),
                crate::ipc::protocol::DmenuEntry::plain("firefox"),
            ],
            allow_custom: true,
        };

        let entries = dmenu_entry_views(&request, " ");

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
    }

    #[test]
    fn dmenu_rich_search_does_not_match_opaque_value_when_label_exists() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: Some("Firefox".into()),
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert!(entries.is_empty());
    }

    #[test]
    fn dmenu_value_only_rich_entry_matches_and_displays_value() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: None,
            entries: vec![crate::ipc::protocol::DmenuEntry {
                label: None,
                value: Some("org.mozilla.firefox".into()),
                icon: None,
                keywords: Vec::new(),
            }],
            allow_custom: false,
        };

        let entries = dmenu_entry_views(&request, "mozilla");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "org.mozilla.firefox");
    }

    #[test]
    fn dmenu_request_rejects_when_another_request_is_pending() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (active_tx, _active_rx) = tokio::sync::oneshot::channel();
        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("active")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(active_tx),
        });
        assert!(app.foreground.has_dmenu_responder_for_test());
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("x")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        let response = rx.try_recv().unwrap();
        assert!(matches!(
            response,
            crate::ipc::protocol::IpcResponse::Rejected { .. }
        ));
    }

    #[test]
    fn dmenu_request_keeps_responder_for_open_selection() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, _rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        assert!(app.foreground.has_dmenu_responder_for_test());
        assert_eq!(active_menu_entries(&app)[0].label, "firefox");
    }

    #[test]
    fn open_provider_action_shows_launch_failure_panel() {
        let mut config = crate::config::Config::default();
        config.panel_providers = vec![crate::config::PanelProviderConfig {
            protocol: Default::default(),
            name: "wifi".into(),
            command: crate::config::ConfiguredCommand::argv_for_test(
                "/definitely-missing-canopa-shell-provider",
                Vec::new(),
            ),
        }];
        let (mut app, _task) = crate::app::App::boot(config, Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.dispatch_panel_action(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            crate::config::PanelAction::OpenProvider("wifi".into()),
        );

        let panel = app
            .foreground
            .active_panel_tree()
            .expect("provider failure panel should open");
        match &panel.content {
            crate::surface::panel::PanelContent::ExternalProvider(content) => {
                assert_eq!(content.provider_name, "wifi");
                assert!(
                    content.failure_text().contains("could not launch"),
                    "got: {:?}",
                    content.failure_text()
                );
            }
            other => panic!("expected external provider panel, got {other:?}"),
        }
    }

    #[test]
    fn external_provider_handshake_content_opens_panel() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        app.set_pending_external_panel_session_for_test("session-1");

        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "session-1".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Wi-Fi ready".into(),
                    }],
                },
            },
        ));

        let panel = app
            .foreground
            .active_panel_tree()
            .expect("provider content panel should open");
        match &panel.content {
            crate::surface::panel::PanelContent::ExternalProvider(content) => {
                assert_eq!(content.provider_name, "wifi");
                assert_eq!(
                    content.elements(),
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Wi-Fi ready".into(),
                    }]
                );
            }
            other => panic!("expected external provider panel, got {other:?}"),
        }
    }

    #[test]
    fn stale_external_provider_handshake_after_replacement_is_ignored() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));

        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "stale-session".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "stale".into(),
                    }],
                },
            },
        ));

        let panel = app.foreground.active_panel_tree().expect("active panel");
        assert_eq!(panel.content, crate::surface::panel::PanelContent::BarPanel);
    }

    #[test]
    fn stale_external_provider_content_replacement_is_ignored() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        app.set_pending_external_panel_session_for_test("session-1");
        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "session-1".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "initial".into(),
                    }],
                },
            },
        ));

        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::ProviderContentReceived {
                session_id: "stale-session".into(),
                provider_name: "wifi".into(),
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "stale".into(),
                    }],
                },
            },
        ));

        let panel = app.foreground.active_panel_tree().expect("provider panel");
        match &panel.content {
            crate::surface::panel::PanelContent::ExternalProvider(content) => {
                assert_eq!(
                    content.elements(),
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "initial".into(),
                    }]
                );
            }
            other => panic!("expected external provider panel, got {other:?}"),
        }
    }

    #[test]
    fn opening_normal_panel_clears_active_external_provider_session() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        app.set_pending_external_panel_session_for_test("session-1");
        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "session-1".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "initial".into(),
                    }],
                },
            },
        ));
        assert_eq!(
            app.active_external_panel_session_id_for_test(),
            Some("session-1")
        );

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));

        assert_eq!(app.active_external_panel_session_id_for_test(), None);
        let panel = app.foreground.active_panel_tree().expect("active panel");
        assert_eq!(panel.content, crate::surface::panel::PanelContent::BarPanel);
    }

    #[test]
    fn dmenu_selection_request_clears_active_external_provider_session() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        app.set_pending_external_panel_session_for_test("session-1");
        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "session-1".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "initial".into(),
                    }],
                },
            },
        ));
        assert_eq!(
            app.active_external_panel_session_id_for_test(),
            Some("session-1")
        );
        let (tx, _rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });

        assert_eq!(app.active_external_panel_session_id_for_test(), None);
    }

    #[test]
    fn open_provider_action_applies_successful_provider_handshake_content() {
        let mut config = crate::config::Config::default();
        config.panel_providers = vec![crate::config::PanelProviderConfig {
            protocol: Default::default(),
            name: "wifi".into(),
            command: crate::config::ConfiguredCommand::argv_for_test(
                "unused-test-provider",
                Vec::new(),
            ),
        }];
        let (mut app, _task) = crate::app::App::boot(config, Vec::new());
        app.set_external_panel_response_for_test(
            "wifi",
            Ok(crate::surface::panel::ExternalPanelDocument::Elements {
                elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                    text: "Wi-Fi ready".into(),
                }],
            }),
        );
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.dispatch_panel_action(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            crate::config::PanelAction::OpenProvider("wifi".into()),
        );

        let panel = app
            .foreground
            .active_panel_tree()
            .expect("provider panel should open");
        match &panel.content {
            crate::surface::panel::PanelContent::ExternalProvider(content) => {
                assert_eq!(content.provider_name, "wifi");
                assert_eq!(
                    content.elements(),
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Wi-Fi ready".into(),
                    }]
                );
            }
            other => panic!("expected external provider panel, got {other:?}"),
        }
    }

    #[test]
    fn repeated_module_click_closes_its_provider_panel() {
        for (module, provider) in [("network", "wifi"), ("notifications", "notify")] {
            let mut config = crate::config::Config::default();
            config.bars[0].modules.end = vec![module.into()];
            if module == "notifications" {
                config.modules.push(crate::config::ModuleConfig {
                    name: module.into(),
                    module_type: "button".into(),
                    params: crate::config::ModuleParams::Button(crate::config::ButtonConfig {
                        label: "Notifications".into(),
                        icon: "N".into(),
                        format: Default::default(),
                        content_rotation: Default::default(),
                        tooltip: Default::default(),
                    }),
                    actions: crate::config::ModuleActionsConfig {
                        bindings: vec![crate::config::ActionBinding {
                            trigger: crate::config::ActionTrigger::Click,
                            action: crate::config::PanelAction::OpenProvider(provider.into()),
                        }],
                    },
                });
            }
            let (mut app, _) = App::boot(config, vec![]);
            let window = iced::window::Id::unique();
            app.surface_mut(window);
            let id = crate::modules::ModuleId::new(module);
            app.set_external_panel_response_for_test(
                provider,
                Ok(crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Ready".into(),
                    }],
                }),
            );
            // The first click resolves widget bounds before delivering this event.
            let _ = app.handle_panel_event(super::super::messages::PanelEvent::OpenProvider {
                origin_window: window,
                origin: crate::surface::panel::PanelOrigin::Module(id.clone()),
                provider_name: provider.into(),
                anchor: None,
            });
            assert!(app.foreground.active_panel_tree().is_some());
            // The same shared module on another output must not dismiss this panel.
            let other_window = iced::window::Id::unique();
            app.surface_mut(other_window);
            let _ = app.handle_module_runtime_event(
                super::super::messages::ModuleRuntimeEvent::ModuleClick(id.clone(), other_window),
            );
            assert!(app.foreground.active_panel_tree().is_some());
            let _ = app.handle_module_runtime_event(
                super::super::messages::ModuleRuntimeEvent::ModuleClick(id, window),
            );
            assert!(
                app.foreground.active_panel_tree().is_none(),
                "second {module} click must close the panel"
            );
            assert!(app.active_external_panel_session_id_for_test().is_none());
        }
    }

    #[test]
    fn repeated_module_click_cancels_pending_provider_and_ignores_late_handshake() {
        let mut config = crate::config::Config::default();
        config.bars[0].modules.end = vec!["network".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let window = iced::window::Id::unique();
        app.surface_mut(window);
        let id = crate::modules::ModuleId::new("network");
        let origin = crate::surface::panel::PanelOrigin::Module(id.clone());
        let identity = super::super::external_panel_session::ExternalPanelPanelIdentity {
            origin_window: window,
            origin: origin.clone(),
            provider_name: "wifi".into(),
        };
        app.external_panel_session
            .begin_open_provider("opening".into(), identity.clone());
        let _ = app.handle_module_runtime_event(
            super::super::messages::ModuleRuntimeEvent::ModuleClick(id, window),
        );
        assert!(!app.external_panel_session.pending_matches_panel(&identity));
        let _ = app.handle_external_panel_event(
            super::super::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "opening".into(),
                origin_window: window,
                origin,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![],
                },
            },
        );
        assert!(app.foreground.active_panel_tree().is_none());
        assert!(app.active_external_panel_session_id_for_test().is_none());
    }

    #[test]
    fn external_provider_button_activation_applies_full_content_replacement() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        app.set_external_panel_event_response_for_test(
            "wifi",
            "scan",
            crate::surface::panel::ExternalPanelDocument::Elements {
                elements: vec![crate::surface::panel::ExternalPanelElement::Text {
                    text: "Scan complete".into(),
                }],
            },
        );
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        app.set_pending_external_panel_session_for_test("session-1");
        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::HandshakeCompleted {
                session_id: "session-1".into(),
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                provider_name: "wifi".into(),
                anchor: None,
                session: None,
                content: crate::surface::panel::ExternalPanelDocument::Elements {
                    elements: vec![crate::surface::panel::ExternalPanelElement::Button {
                        id: "scan".into(),
                        label: "Scan".into(),
                    }],
                },
            },
        ));

        let _ = app.update(Message::ExternalPanel(
            crate::app::messages::ExternalPanelEvent::ButtonActivated {
                provider_name: "wifi".into(),
                element_id: "scan".into(),
            },
        ));

        let panel = app
            .foreground
            .active_panel_tree()
            .expect("provider panel should stay open");
        match &panel.content {
            crate::surface::panel::PanelContent::ExternalProvider(content) => {
                assert_eq!(
                    content.elements(),
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "Scan complete".into(),
                    }]
                );
            }
            other => panic!("expected external provider panel, got {other:?}"),
        }
    }

    #[test]
    fn activating_dmenu_selection_sends_selected_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });
        let _ = app.update(Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Selected {
                value: "firefox".into()
            }
        );
        assert!(!app.foreground.has_dmenu_responder_for_test());
    }

    #[test]
    fn closing_dmenu_panel_sends_cancelled_response() {
        let (mut app, _task) = crate::app::App::boot(crate::config::Config::default(), Vec::new());
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let (tx, mut rx) = tokio::sync::oneshot::channel();

        let _ = app.handle_ipc_event(crate::app::messages::IpcEvent::DmenuSelection {
            request: crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            responder: crate::app::messages::ipc_responder(tx),
        });
        let panel = app
            .foreground
            .active_panel_window()
            .expect("dmenu panel window");

        let _ = app.handle_surface_event(crate::app::messages::SurfaceEvent::WindowClosed(panel));

        assert_eq!(
            rx.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Cancelled
        );
        assert!(!app.foreground.has_dmenu_responder_for_test());
    }

    #[test]
    fn opening_panel_closes_existing_panel() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
    }

    #[test]
    fn opening_panel_clears_origin_tooltip_pending() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin).tooltip_pending =
            Some((crate::modules::ModuleId::new("clock"), 0.0));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                "clock",
            )),
            content: crate::surface::panel::PanelContent::Menu("main".into()),
            anchor: None,
        }));

        assert!(app.surface(origin).unwrap().tooltip_pending.is_none());
    }

    #[test]
    fn closing_panel_removes_panel_metadata() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Close));

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
    }

    #[test]
    fn outside_dismissal_closes_current_panel_and_ignores_replaced_windows() {
        use crate::app::messages::{PanelEvent, SurfaceEvent};
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let open = Message::Panel(PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        });
        let _ = app.update(open.clone());
        let old_panel = app.foreground.active_panel_window().unwrap();
        let old_backdrop = app.foreground.role_snapshot().roles_for_test_dismissal();
        let _ = app.update(open);
        let current = app.foreground.active_panel_window().unwrap();
        let _ = app.update(Message::Panel(PanelEvent::Dismiss(old_panel)));
        let _ = app.update(Message::Surface(SurfaceEvent::PointerPressed(old_backdrop)));
        assert_eq!(app.foreground.active_panel_window(), Some(current));
        let current_backdrop = app.foreground.role_snapshot().roles_for_test_dismissal();
        let _ = app.update(Message::Surface(SurfaceEvent::PointerPressed(
            current_backdrop,
        )));
        assert_eq!(app.foreground.active_panel_window(), None);
        assert!(!app.foreground.is_dismissal_window(current_backdrop));
    }

    #[test]
    fn left_edge_panel_opens_right_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 0.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert_eq!(position.0, 48);
        assert!(position.1 >= 0);
    }

    #[test]
    fn right_edge_panel_opens_left_of_bar_and_aligns_to_anchor_y() {
        let position = panel_position_for_test(
            crate::config::Edge::Right,
            48,
            (360, 600),
            Some(iced::Rectangle {
                x: 1872.0,
                y: 100.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (1872, 0),
        );

        assert_eq!(position.0, 1512);
        assert!(position.1 >= 0);
    }

    #[test]
    fn panel_window_uses_on_demand_keyboard_interactivity() {
        let settings = panel_window_settings(panel_plan_for_test(
            crate::config::Edge::Top,
            PlannedDimension::Exact(360),
            PlannedDimension::Exact(600),
            (0, 48),
            (1920, 1080),
        ));

        assert_eq!(
            settings.keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn panel_window_respects_other_bars_reserved_space() {
        for (edge, position) in [
            (crate::config::Edge::Top, (100, 48)),
            (crate::config::Edge::Bottom, (100, 852)),
            (crate::config::Edge::Left, (48, 100)),
            (crate::config::Edge::Right, (1622, 100)),
        ] {
            let settings = panel_window_settings(panel_plan_for_test(
                edge.clone(),
                PlannedDimension::Exact(250),
                PlannedDimension::Exact(180),
                position,
                (1920, 1080),
            ));
            assert_eq!(settings.exclusive_zone, Some(0), "{edge:?}");
            let (top, right, bottom, left) = settings.margin.unwrap();
            let distance = match edge {
                crate::config::Edge::Top => top,
                crate::config::Edge::Bottom => bottom,
                crate::config::Edge::Left => left,
                crate::config::Edge::Right => right,
            };
            assert_eq!(distance, 0, "the compositor already accounts for all bars");
        }
    }

    #[test]
    fn panel_window_settings_keep_layer_shell_focusable_for_search_input() {
        let settings = panel_window_settings(panel_plan_for_test(
            crate::config::Edge::Top,
            PlannedDimension::Fill { span: 1920 },
            PlannedDimension::Exact(400),
            (0, 48),
            (1920, 48),
        ));

        assert_eq!(settings.size, Some((0, 400)));
        assert!(settings.anchor.contains(Anchor::Left));
        assert!(settings.anchor.contains(Anchor::Right));
        assert_eq!(
            settings.keyboard_interactivity,
            KeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn bottom_panel_window_uses_layer_margin_above_bar() {
        let settings = panel_window_settings(panel_plan_for_test(
            crate::config::Edge::Bottom,
            PlannedDimension::Fill { span: 1920 },
            PlannedDimension::Exact(240),
            (0, -240),
            (1920, 48),
        ));

        assert_eq!(settings.size, Some((0, 240)));
        assert_eq!(settings.margin, Some((0, 0, 0, 0)));
    }

    #[test]
    fn left_edge_panel_menu_fills_edge_span_height() {
        let settings = panel_window_settings(panel_plan_for_test(
            crate::config::Edge::Left,
            PlannedDimension::Exact(320),
            PlannedDimension::Fill { span: 1080 },
            (48, 0),
            (48, 1080),
        ));

        assert_eq!(settings.size, Some((320, 0)));
        assert!(settings.anchor.contains(Anchor::Top));
        assert!(settings.anchor.contains(Anchor::Bottom));
        assert_eq!(settings.margin, Some((0, 0, 0, 0)));
    }

    #[test]
    fn clock_module_panel_uses_module_preferred_size() {
        let mut config = crate::config::Config::default();
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        {
            let surface = app.surface_mut(origin);
            surface.bar_name = "main".into();
            surface.output_name = "DP-1".into();
            surface.screen_size = (1920, 48);
        }
        let tree = crate::surface::panel::PanelTree {
            origin: crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                "clock",
            )),
            content: crate::surface::panel::PanelContent::ModulePanel(
                crate::modules::ModuleId::new("clock"),
            ),
            origin_window: origin,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size: (1920, 48),
        };
        let bar = app.config.bars[0].clone();
        let origin_surface = app.surface(origin).cloned().unwrap();
        let panel_module_layout =
            app.panel_module_layout_for_content(tree.origin_window, &tree.origin, &tree.content);

        let plan = super::super::panel_sizing::panel_physical_plan(PanelSizingInput {
            bar: &bar,
            ui: crate::theme::ui(canopa_theme::UiKind::Panel),
            menu_row_extent: 45.0,
            tree: &tree,
            surface_origin: crate::surface::placement::Point::default(),
            panel_module_layout: &panel_module_layout,
            menu: app.foreground.active_panel_menu_render_state(),
            module_detail_preferred_size: |id| {
                app.module_detail_preferred_content_size(id, &bar, &origin_surface)
            },
        });

        assert_eq!(plan.width, PlannedDimension::Exact(372));
        assert_eq!(plan.height, PlannedDimension::Exact(566));
        assert_eq!(panel_window_settings(plan).size, Some((372, 566)));
    }

    #[test]
    fn top_edge_panel_opens_below_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Top,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 0.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 32);
    }

    #[test]
    fn bottom_edge_panel_opens_above_bar_and_aligns_to_anchor_x() {
        let position = panel_position_for_test(
            crate::config::Edge::Bottom,
            32,
            (360, 600),
            Some(iced::Rectangle {
                x: 100.0,
                y: 1048.0,
                width: 48.0,
                height: 32.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert!(position.0 >= 0);
        assert_eq!(position.1, 448);
    }

    #[test]
    fn panel_position_top_centers_on_anchor_and_clamps() {
        let position = panel_position_for_test(
            crate::config::Edge::Top,
            48,
            (300, 600),
            Some(iced::Rectangle {
                x: 480.0,
                y: 4.0,
                width: 40.0,
                height: 40.0,
            }),
            (1920, 1080),
            (0, 0),
        );

        assert_eq!(position, (350, 48));
    }

    #[test]
    fn left_edge_bar_panel_starts_at_vertical_bar_top_when_top_bar_reserves_space() {
        let position = panel_position_for_test(
            crate::config::Edge::Left,
            48,
            (360, 600),
            None,
            (48, 1048),
            (0, 32),
        );

        assert_eq!(position, (48, 32));
    }

    #[test]
    fn bar_panel_scoped_action_opens_the_activated_modules_details() {
        use crate::modules::{InteractionEvent, ModuleId};
        use crate::surface::panel::{PanelContent, PanelOrigin};
        let (mut config, _) =
            crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        config.bars[0].panel.modules.start = vec!["clock".into()];
        config.bars[0].panel.modules.center.clear();
        config.bars[0].panel.modules.end.clear();
        let (mut app, _) = App::boot(config, vec![]);
        crate::app::tests::ready_clock(&mut app);
        let origin_window = iced::window::Id::unique();
        app.surface_mut(origin_window);
        let _ = app.open_panel(
            origin_window,
            PanelOrigin::Module(ModuleId::new("system")),
            PanelContent::BarPanel,
            None,
        );
        let window = app.foreground.active_panel_window().unwrap();
        let id = ModuleId::new("clock");
        let _ = app.interact_panel_module(window, id.clone(), InteractionEvent::Clicked);
        let tree = app.foreground.active_panel_tree().unwrap();
        assert_eq!(tree.content, PanelContent::ModulePanel(id.clone()));
        assert_eq!(tree.origin, PanelOrigin::Module(id));
        assert_eq!(tree.origin_window, origin_window);
    }

    #[test]
    fn quick_settings_controls_survive_unavailable_sources_and_return_from_providers() {
        use crate::modules::{InteractionEvent, ModuleId, ModuleState};
        use crate::surface::panel::{PanelContent, PanelOrigin};
        let (config, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        let (mut app, _) = App::boot(config, vec![]);
        let origin_window = iced::window::Id::unique();
        app.surface_mut(origin_window);
        for name in ["network", "volume"] {
            app.modules
                .set_state(ModuleId::new(name), ModuleState::Unavailable);
        }
        for name in ["bluetooth", "power-and-brightness"] {
            app.modules.set_state(
                ModuleId::new(name),
                ModuleState::Button(crate::modules::button::ButtonState),
            );
        }
        let origin = PanelOrigin::Module(ModuleId::new("system"));
        let anchor = Some(iced::Rectangle {
            x: 10.0,
            y: 680.0,
            width: 44.0,
            height: 44.0,
        });
        let _ = app.open_panel(
            origin_window,
            origin.clone(),
            PanelContent::BarPanel,
            anchor,
        );
        let mut panel_window = app.foreground.active_panel_window().unwrap();
        let layout =
            app.panel_module_layout_for_content(origin_window, &origin, &PanelContent::BarPanel);
        let names: Vec<_> = layout.iter().map(|entry| entry.id.display_name()).collect();
        assert_eq!(
            names,
            [
                "network",
                "volume",
                "notifications",
                "idle-inhibitor",
                "bluetooth",
                "power-and-brightness"
            ]
        );
        assert!(
            !app.bar_render_model(origin_window)
                .end
                .iter()
                .any(|entry| entry.id == ModuleId::new("volume"))
        );

        for (name, provider) in [
            ("network", "wifi"),
            ("volume", "audio"),
            ("bluetooth", "bluetooth"),
            ("power-and-brightness", "power"),
        ] {
            app.set_external_panel_response_for_test(provider, Err("Backend unavailable".into()));
            let _ = app.interact_panel_module(
                panel_window,
                ModuleId::new(name),
                InteractionEvent::Clicked,
            );
            let tree = app.foreground.active_panel_tree().unwrap();
            let PanelContent::ExternalProvider(content) = &tree.content else {
                panic!("provider panel");
            };
            assert_eq!(content.provider_name, provider);
            assert_eq!(tree.origin_window, origin_window);
            assert_eq!(tree.origin, origin);
            assert_eq!(tree.anchor, anchor);
            assert!(app.foreground.panel_return_to_bar());
            let _ = app.handle_panel_event(super::super::messages::PanelEvent::Back);
            assert_eq!(
                app.foreground.active_panel_tree().unwrap().content,
                PanelContent::BarPanel
            );
            assert!(!app.foreground.panel_return_to_bar());
            panel_window = app.foreground.active_panel_window().unwrap();
        }
        // Events from a replaced Panel and Modules outside this Panel are ignored.
        let _ = app.interact_panel_module(
            iced::window::Id::unique(),
            ModuleId::new("volume"),
            InteractionEvent::Clicked,
        );
        let _ = app.interact_panel_module(
            panel_window,
            ModuleId::new("clock"),
            InteractionEvent::Clicked,
        );
        assert_eq!(
            app.foreground.active_panel_tree().unwrap().content,
            PanelContent::BarPanel
        );

        let _ = app.move_panel_focus(1);
        assert_eq!(
            app.foreground.focused_panel_module(),
            Some(&ModuleId::new("network"))
        );
        let _ = app.move_panel_focus(1);
        assert_eq!(
            app.foreground.focused_panel_module(),
            Some(&ModuleId::new("volume"))
        );
        let _ = app.activate_panel_entry(None);
        assert!(matches!(
            app.foreground.active_panel_tree().unwrap().content,
            PanelContent::ExternalProvider(_)
        ));
        let _ = app.close_active_panel();
        assert!(!app.foreground.panel_return_to_bar());
    }

    #[test]
    fn bundled_controls_button_opens_its_bar_panel_and_repeated_click_dismisses() {
        let (config, _) = crate::config::load(&canopa_config::Request::Defaults, false).unwrap();
        assert_eq!(
            config.bars[0].panel.title.as_deref(),
            Some("Quick settings")
        );
        let (mut app, _) = App::boot(config, vec![]);
        let origin_window = iced::window::Id::unique();
        app.surface_mut(origin_window);
        let id = crate::modules::ModuleId::new("system");
        let action = app
            .modules
            .action_for(&id, &crate::config::ActionTrigger::Click)
            .cloned()
            .expect("bundled controls action");
        let origin = crate::surface::panel::PanelOrigin::Module(id.clone());
        let anchor = iced::Rectangle {
            x: 10.0,
            y: 680.0,
            width: 44.0,
            height: 44.0,
        };
        let event =
            panel_event_for_action(origin_window, origin.clone(), action.clone(), Some(anchor))
                .expect("panel action");
        let _ = app.handle_panel_event(event);

        let tree = app.foreground.active_panel_tree().expect("controls panel");
        assert_eq!(tree.content, crate::surface::panel::PanelContent::BarPanel);
        assert_eq!(tree.origin, origin);
        assert_eq!(tree.origin_window, origin_window);
        assert_eq!(tree.anchor, Some(anchor));
        assert!(
            app.foreground
                .module_action_targets_active_panel(origin_window, &id, &action)
        );
        assert!(!app.foreground.module_action_targets_active_panel(
            iced::window::Id::unique(),
            &id,
            &action,
        ));

        let _ = app.dispatch_module_interaction(
            &id,
            crate::modules::InteractionEvent::Clicked,
            origin_window,
        );
        assert!(app.foreground.active_panel_tree().is_none());
    }

    #[test]
    fn configured_module_open_panel_action_opens_panel() {
        let mut config = crate::config::Config {
            modules: vec![crate::config::ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: crate::config::ModuleParams::Button(crate::config::ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    format: crate::config::FormatConfig::default(),
                    content_rotation: crate::config::ContentRotation::None,
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::OpenPanel,
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let id = crate::modules::ModuleId::new("button");
        let action = app
            .modules
            .action_for(&id, &crate::config::ActionTrigger::Click)
            .cloned()
            .expect("configured click action");

        let _ = app.dispatch_panel_action(
            origin,
            crate::surface::panel::PanelOrigin::Module(id),
            action,
        );

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
    }

    #[test]
    fn panel_event_for_module_open_panel_carries_anchor() {
        let anchor = iced::Rectangle {
            x: 8.0,
            y: 16.0,
            width: 24.0,
            height: 32.0,
        };
        let event = panel_event_for_action(
            iced::window::Id::unique(),
            crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new("button")),
            crate::config::PanelAction::OpenPanel,
            Some(anchor),
        )
        .expect("panel event");

        match event {
            crate::app::messages::PanelEvent::Open {
                origin,
                content,
                anchor: event_anchor,
                ..
            } => {
                assert_eq!(
                    origin,
                    crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new(
                        "button"
                    ))
                );
                assert_eq!(
                    content,
                    crate::surface::panel::PanelContent::ModulePanel(
                        crate::modules::ModuleId::new("button")
                    )
                );
                assert_eq!(event_anchor, Some(anchor));
            }
            other => panic!("expected open event, got {other:?}"),
        }
    }

    #[test]
    fn panel_event_for_module_open_provider_carries_anchor() {
        let anchor = iced::Rectangle {
            x: 8.0,
            y: 16.0,
            width: 24.0,
            height: 32.0,
        };
        let event = panel_event_for_action(
            iced::window::Id::unique(),
            crate::surface::panel::PanelOrigin::Module(crate::modules::ModuleId::new("network")),
            crate::config::PanelAction::OpenProvider("wifi".into()),
            Some(anchor),
        );

        assert!(
            event.is_some(),
            "module-scoped open-provider action should survive anchor lookup"
        );
    }

    fn menu_with_apps_browser() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: Some("applications-other".into()),
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: Some("firefox".into()),
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("firefox"),
                    )),
                    children: vec![],
                }],
            }],
        }
    }

    fn menu_with_two_entries() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Terminal".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
                crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                },
            ],
        }
    }

    fn menu_with_entries(count: usize) -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: None,
            items: (0..count)
                .map(|index| crate::config::MenuItemConfig {
                    label: format!("Entry {index}"),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![],
                })
                .collect(),
        }
    }

    fn config_with_menu(menu: crate::config::MenuConfig) -> crate::config::Config {
        crate::config::Config {
            menus: vec![menu],
            ..crate::config::Config::default()
        }
    }

    fn prepare_panel_content(
        app: &mut App,
        origin: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            content,
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            super::super::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
    }

    fn register_panel_with_content(
        app: &mut App,
        panel: iced::window::Id,
        origin: iced::window::Id,
        content: crate::surface::panel::PanelContent,
    ) {
        prepare_panel_content(app, origin, content);
        app.register_panel_popup(panel, origin);
    }

    fn register_menu_panel(app: &mut App, panel: iced::window::Id, origin: iced::window::Id) {
        if app.foreground.panel_menu_snapshot_for_test().is_none() {
            prepare_panel_content(
                app,
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            );
        }
        app.register_panel_popup(panel, origin);
    }

    fn set_panel_search(app: &mut App, query: &str) {
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::SearchChanged(query.into()),
            ));
    }

    fn set_panel_focus(app: &mut App, index: usize) {
        if app.foreground.panel_menu_snapshot_for_test().is_none() {
            let origin = iced::window::Id::unique();
            prepare_panel_content(
                app,
                origin,
                crate::surface::panel::PanelContent::Menu("main".into()),
            );
        }
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::FocusIndex(index),
            ));
    }

    fn panel_focus(app: &App) -> Option<usize> {
        app.foreground.focused_panel_index_for_test()
    }

    #[test]
    fn menu_search_flattens_nested_matches_with_breadcrumb() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[0].path, vec![0, 0]);
        assert_eq!(results[0].icon, Some("firefox".into()));
    }

    #[test]
    fn menu_search_matches_keywords_case_insensitively() {
        let menu = menu_with_apps_browser();
        let results = search_menu_entries(&menu, "WEB");

        assert_eq!(results[0].label, "Apps / Browser");
    }

    #[test]
    fn menu_search_ranks_label_matches_before_keyword_matches() {
        let mut menu = menu_with_apps_browser();
        menu.items.push(crate::config::MenuItemConfig {
            label: "Settings".into(),
            icon: None,
            keywords: vec!["browser".into()],
            action: Some(crate::config::PanelAction::Command(
                crate::config::ConfiguredCommand::shell_for_test("settings"),
            )),
            children: vec![],
        });

        let results = search_menu_entries(&menu, "browser");

        assert_eq!(results[0].label, "Apps / Browser");
        assert_eq!(results[1].label, "Settings");
    }

    #[test]
    fn menu_search_uses_ordered_fuzzy_matching() {
        let menu = menu_with_apps_browser();

        let results = search_menu_entries(&menu, "brsr");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Apps / Browser");
        assert!(search_menu_entries(&menu, "rb").is_empty());
    }

    #[test]
    fn menu_search_matches_breadcrumb_as_lowest_priority() {
        let menu = crate::config::MenuConfig {
            name: "main".into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                crate::config::MenuItemConfig {
                    label: "Apps".into(),
                    icon: None,
                    keywords: vec![],
                    action: None,
                    children: vec![crate::config::MenuItemConfig {
                        label: "Firefox".into(),
                        icon: None,
                        keywords: vec![],
                        action: Some(crate::config::PanelAction::Command(
                            crate::config::ConfiguredCommand::shell_for_test("firefox"),
                        )),
                        children: vec![],
                    }],
                },
                crate::config::MenuItemConfig {
                    label: "App Settings".into(),
                    icon: None,
                    keywords: vec![],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::ConfiguredCommand::shell_for_test("settings"),
                    )),
                    children: vec![],
                },
            ],
        };

        let results = search_menu_entries(&menu, "app");

        assert_eq!(
            results
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["App Settings", "Apps", "Apps / Firefox"]
        );
    }

    #[test]
    fn menu_search_requires_every_query_token() {
        let menu = menu_with_apps_browser();

        assert_eq!(search_menu_entries(&menu, "apps browser").len(), 1);
        assert!(search_menu_entries(&menu, "apps terminal").is_empty());
    }

    fn installed_app(name: &str, id: &str) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            desktop_file: "fixture.desktop".into(),
            dbus_activatable: false,
            ..Default::default()
        }
    }

    fn app_with_installed_apps_menu(apps: Vec<crate::desktop_apps::DesktopApplication>) -> App {
        let config = config_with_menu(crate::config::MenuConfig {
            name: "apps".into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: Some(crate::config::MenuSourceConfig {
                kind: crate::config::MenuSourceKind::InstalledApps,
            }),
            items: vec![],
        });
        let (mut app, _) = App::boot(config, vec![]);
        app.foreground.set_installed_apps_for_test(apps);
        app
    }

    #[test]
    fn installed_apps_hover_render_path_does_not_rebuild_large_entry_list() {
        let apps = (0..5_000)
            .map(|index| installed_app(&format!("App {index:04}"), &format!("app-{index}.desktop")))
            .collect::<Vec<_>>();
        let mut app = app_with_installed_apps_menu(apps);
        open_apps_menu_panel(&mut app);
        let panel_window = app.foreground.active_panel_window().expect("panel window");

        let started = std::time::Instant::now();
        for index in 0..100 {
            let _ = app.update(Message::Panel(
                crate::app::messages::PanelEvent::FocusIndex(index),
            ));
            let model = app
                .panel_render_model(panel_window)
                .expect("panel render model");
            assert_eq!(model.menu_entries.len(), 5_000);
            assert_eq!(model.focused_index, Some(index));
        }

        assert!(
            started.elapsed() < std::time::Duration::from_millis(25),
            "hover render path rebuilt too much installed-app menu state: {:?}",
            started.elapsed()
        );
    }

    fn open_apps_menu_panel(app: &mut App) {
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));
    }

    #[test]
    fn toggle_installed_apps_menu_loads_entries() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);

        let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Toggle {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::Menu("apps".into()),
            anchor: None,
        }));

        assert_eq!(
            active_menu_entries(&app)
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha"]
        );
    }

    #[test]
    fn installed_apps_menu_entries_use_open_snapshot() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Alpha", "alpha.desktop"),
            installed_app("Zed", "zed.desktop"),
        ]);

        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Zed"]
        );
    }

    #[test]
    fn installed_apps_search_matches_keywords_and_comment() {
        let mut browser = installed_app("Browser", "browser.desktop");
        browser.comment = Some("Browse the web".into());
        browser.keywords = vec!["internet".into()];
        let mut app =
            app_with_installed_apps_menu(vec![browser, installed_app("Editor", "editor.desktop")]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "internet");

        let entries = active_menu_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Browser");
    }

    #[test]
    fn installed_apps_search_ranks_visible_label_before_metadata() {
        let mut settings = installed_app("Settings", "settings.desktop");
        settings.keywords = vec!["network".into()];
        let mut network = installed_app("Network Connections", "network.desktop");
        network.comment = Some("Configure links".into());
        let mut app = app_with_installed_apps_menu(vec![settings, network]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "net");

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Network Connections", "Settings"]
        );
    }

    #[test]
    fn installed_apps_search_matches_tokens_across_fields() {
        let mut code = installed_app("Visual Studio Code", "code.desktop");
        code.keywords = vec!["editor".into()];
        let mut app = app_with_installed_apps_menu(vec![code]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "code editor");

        let entries = active_menu_entries(&app);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Visual Studio Code");
    }

    #[test]
    fn installed_apps_search_preserves_source_order_for_blank_query() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Zed", "zed.desktop"),
            installed_app("Alpha", "alpha.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        set_panel_search(&mut app, "   ");

        let entries = active_menu_entries(&app);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Zed", "Alpha"]
        );
    }

    #[test]
    fn installed_apps_entries_keep_named_and_absolute_icons() {
        // Absolute `Icon=` paths are valid desktop-entry icons since the shared
        // icon lookup (ADR 0138); the launcher resolves them like theme names.
        let mut alpha = installed_app("Alpha", "alpha.desktop");
        alpha.icon = Some("alpha-icon".into());
        let mut beta = installed_app("Beta", "beta.desktop");
        beta.icon = Some("/usr/share/icons/beta.png".into());
        let mut app = app_with_installed_apps_menu(vec![alpha, beta]);
        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);
        assert_eq!(entries[0].icon, Some("alpha-icon".into()));
        assert_eq!(entries[1].icon, Some("/usr/share/icons/beta.png".into()));
    }

    #[test]
    fn activating_installed_app_closes_panel() {
        let mut app = app_with_installed_apps_menu(vec![installed_app("Alpha", "alpha.desktop")]);
        open_apps_menu_panel(&mut app);
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );

        let _ = app.update(Message::Panel(
            crate::app::messages::PanelEvent::ActivateIndex(0),
        ));

        assert_eq!(
            app.foreground.launched_installed_app_ids_for_test(),
            ["alpha.desktop"]
        );
        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            0
        );
    }

    #[test]
    fn duplicate_installed_app_names_are_disambiguated() {
        let mut app = app_with_installed_apps_menu(vec![
            installed_app("Terminal", "org.gnome.Terminal.desktop"),
            installed_app("Terminal", "com.raggesilver.BlackBox.desktop"),
        ]);
        open_apps_menu_panel(&mut app);

        let entries = active_menu_entries(&app);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].label.contains("Terminal"));
        assert!(entries[1].label.contains("Terminal"));
        assert_ne!(entries[0].label, entries[1].label);
    }

    #[test]
    fn installed_apps_menu_can_be_empty() {
        let mut app = app_with_installed_apps_menu(vec![]);
        open_apps_menu_panel(&mut app);

        assert_eq!(
            usize::from(app.foreground.active_panel_window().is_some()),
            1
        );
        assert!(active_menu_entries(&app).is_empty());
    }

    #[test]
    fn arrow_down_on_panel_window_moves_menu_focus_to_next_entry() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 0);
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
    }

    #[test]
    fn arrow_down_on_panel_window_does_not_scroll_when_focused_entry_is_visible() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 0);
        app.foreground.reset_panel_menu_scroll_for_test();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(1));
        assert_eq!(app.foreground.panel_menu_scroll_y_for_test(), 0.0);
    }

    #[test]
    fn arrow_down_on_panel_window_scrolls_only_enough_to_reveal_focused_entry() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 40);
        app.foreground.reset_panel_menu_scroll_for_test();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMoveNext(panel));

        assert_eq!(panel_focus(&app), Some(41));
        assert_eq!(
            app.foreground.panel_menu_scroll_y_for_test(),
            42.0 * app.active_menu_metrics().extent - DEFAULT_MENU_VIEWPORT_HEIGHT
        );
    }

    #[test]
    fn arrow_up_on_panel_window_scrolls_only_when_focused_entry_is_above_view() {
        let config = config_with_menu(menu_with_entries(100));
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        set_panel_focus(&mut app, 10);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::MenuScrolled {
                    y: 10.0 * app.active_menu_metrics().extent,
                    viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
                },
            ));
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::FocusMovePrev(panel));

        assert_eq!(panel_focus(&app), Some(9));
        assert_eq!(
            app.foreground.panel_menu_scroll_y_for_test(),
            9.0 * app.active_menu_metrics().extent
        );
    }

    #[test]
    fn focus_gained_on_panel_window_does_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ =
            app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusGained(panel));

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
    }

    #[test]
    fn focus_search_input_event_initializes_menu_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::RequestSearchFocus,
            ));

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusSearchInput {
            retries: 0,
        });

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
        assert!(!app.foreground.panel_search_focus_pending());
    }

    #[test]
    fn back_from_submenu_requests_search_refocus() {
        let config = config_with_menu(menu_with_apps_browser());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::SearchFocusSettled,
            ));
        let _ = app.handle_panel_event(super::super::messages::PanelEvent::ActivateIndex(0));

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::Back);

        assert_eq!(active_menu_entries(&app)[0].label, "Apps");
        assert!(app.foreground.panel_search_focus_pending());
    }

    #[test]
    fn hover_focus_cancels_pending_search_focus_retries() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::RequestSearchFocus,
            ));

        let _ = app.handle_panel_event(super::super::messages::PanelEvent::FocusIndex(1));

        assert_eq!(panel_focus(&app), Some(1));
        assert!(!app.foreground.panel_search_focus_pending());
    }

    #[test]
    fn window_sized_on_panel_window_routes_to_panel_focus_without_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
        assert_eq!(panel_focus(&app), Some(0));
    }

    #[test]
    fn close_active_panel_closes_active_foreground_panel() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let tracked_panel = iced::window::Id::unique();
        app.register_panel_popup(tracked_panel, origin);

        let tasks = app.close_active_panel();

        assert!(!tasks.is_empty());
        assert!(!app.foreground.is_panel_window(tracked_panel));
    }

    #[test]
    fn stale_events_from_closing_panel_window_do_not_create_bar_surface_state() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        register_menu_panel(&mut app, panel, origin);

        let _ = app.close_active_panel();
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::BarFocusLost(panel));
        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowSized(
            panel, 360, 600,
        ));

        assert!(app.surface(panel).is_none());
    }

    #[test]
    fn replacing_panel_ignores_stale_old_window_close() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin).bar_name = "main".into();
        app.surface_mut(origin).output_name = "DP-1".into();
        app.surface_mut(origin).screen_size = (1920, 1080);

        let first_window = iced::window::Id::unique();
        register_panel_with_content(
            &mut app,
            first_window,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let second_window = iced::window::Id::unique();
        register_panel_with_content(
            &mut app,
            second_window,
            origin,
            crate::surface::panel::PanelContent::Menu("main".into()),
        );

        let _ = app.handle_surface_event(super::super::messages::SurfaceEvent::WindowClosed(
            first_window,
        ));

        assert_eq!(app.foreground.active_panel_window(), Some(second_window));
        assert!(matches!(
            app.foreground.active_panel_tree().map(|tree| &tree.content),
            Some(crate::surface::panel::PanelContent::Menu(name)) if name == "main"
        ));
        assert!(!app.foreground.is_panel_window(first_window));
        assert!(app.foreground.is_panel_window(second_window));
    }

    #[test]
    fn escape_clears_search_before_closing_panel() {
        let mut panel = super::super::panel_runtime::PanelRuntime::default();
        let menus = [menu_with_two_entries()];
        panel.handle(
            super::super::panel_runtime::PanelRuntimeIntent::PrepareOpen {
                request: super::super::panel_runtime::PanelOpenRequest {
                    origin_window: iced::window::Id::unique(),
                    origin: crate::surface::panel::PanelOrigin::Bar,
                    content: crate::surface::panel::PanelContent::Menu("main".into()),
                    anchor: None,
                    origin_bar_name: "main".into(),
                    origin_output_name: "DP-1".into(),
                    screen_size: (1920, 1080),
                },
                catalog: super::super::panel_runtime::PanelMenuCatalog::new(&menus),
                module_detail_sessions: std::collections::HashMap::new(),
            },
        );
        panel.interact(super::super::panel_runtime::PanelInteraction::SearchChanged("term".into()));

        assert!(matches!(
            panel
                .interact(super::super::panel_runtime::PanelInteraction::Escape)
                .as_slice(),
            [super::super::panel_runtime::PanelRuntimeEffect::RequestSearchFocus]
        ));
        assert_eq!(
            panel.panel_menu_snapshot_for_test().unwrap().search_query,
            ""
        );
    }

    #[test]
    fn escape_that_clears_search_requests_search_refocus() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surface_mut(origin);
        register_menu_panel(&mut app, panel, origin);
        set_panel_search(&mut app, "term");
        app.foreground
            .handle(super::super::foreground::ForegroundIntent::Panel(
                super::super::foreground::PanelInteraction::SearchFocusSettled,
            ));

        let _ = app.handle_panel_escape_pressed();

        assert_eq!(
            app.foreground
                .panel_menu_snapshot_for_test()
                .unwrap()
                .search_query,
            ""
        );
        assert!(app.foreground.panel_search_focus_pending());
        assert!(app.foreground.is_panel_window(panel));
    }

    #[test]
    fn escape_without_first_escape_behavior_closes_panel_through_foreground() {
        let config = config_with_menu(menu_with_two_entries());
        let (mut app, _) = App::boot(config, vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        app.surface_mut(origin);
        register_panel_with_content(
            &mut app,
            panel,
            origin,
            crate::surface::panel::PanelContent::BarPanel,
        );

        let _ = app.handle_panel_escape_pressed();

        assert!(!app.foreground.is_panel_window(panel));
    }

    #[test]
    fn panel_search_input_id_is_stable_for_focus_task() {
        let first = iced::window::Id::unique();
        let second = iced::window::Id::unique();

        assert_eq!(panel_search_input_id(first), panel_search_input_id(first));
        assert_ne!(panel_search_input_id(first), panel_search_input_id(second));
    }
}
