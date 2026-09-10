#[derive(Debug, Default)]
pub(super) struct ExternalPanelSessionRuntime {
    pending_session_id: Option<String>,
    active: Option<ActiveExternalPanelSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExternalPanelPanelIdentity {
    pub(super) origin_window: iced::window::Id,
    pub(super) origin: crate::surface::panel::PanelOrigin,
    pub(super) provider_name: String,
}

#[derive(Debug, Clone)]
pub(super) enum ExternalPanelSessionEffect {
    OpenPanel {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::ExternalPanelContent,
        anchor: Option<iced::Rectangle>,
    },
    WatchProviderSession {
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
    },
    WatchProviderContent {
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
    },
    SendProviderEvent {
        session_id: String,
        provider_name: String,
        handle: crate::external_panel::ExternalPanelSessionHandle,
        event: crate::external_panel::protocol::ProviderEvent,
    },
}

#[derive(Debug)]
struct ActiveExternalPanelSession {
    session_id: String,
    provider_name: String,
    origin_window: iced::window::Id,
    origin: crate::surface::panel::PanelOrigin,
    anchor: Option<iced::Rectangle>,
    content: crate::surface::panel::ExternalPanelContent,
    handle: Option<crate::external_panel::ExternalPanelSessionHandle>,
}

impl Drop for ActiveExternalPanelSession {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.close();
        }
    }
}

impl ActiveExternalPanelSession {
    fn panel_identity(&self) -> ExternalPanelPanelIdentity {
        ExternalPanelPanelIdentity {
            origin_window: self.origin_window,
            origin: self.origin.clone(),
            provider_name: self.provider_name.clone(),
        }
    }

    fn matches_panel(&self, active_panel: Option<&ExternalPanelPanelIdentity>) -> bool {
        active_panel.is_some_and(|panel| panel == &self.panel_identity())
    }

    fn matches_session(
        &self,
        session_id: &str,
        provider_name: &str,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> bool {
        self.session_id == session_id
            && self.provider_name == provider_name
            && self.matches_panel(active_panel)
    }

    fn open_content_effect(&self) -> ExternalPanelSessionEffect {
        ExternalPanelSessionEffect::OpenPanel {
            origin_window: self.origin_window,
            origin: self.origin.clone(),
            content: self.content.clone(),
            anchor: self.anchor,
        }
    }

    fn watch_effects(&self) -> Vec<ExternalPanelSessionEffect> {
        let Some(handle) = self.handle.clone() else {
            return Vec::new();
        };
        vec![
            ExternalPanelSessionEffect::WatchProviderSession {
                session_id: self.session_id.clone(),
                provider_name: self.provider_name.clone(),
                handle: handle.clone(),
            },
            ExternalPanelSessionEffect::WatchProviderContent {
                session_id: self.session_id.clone(),
                provider_name: self.provider_name.clone(),
                handle,
            },
        ]
    }
}

impl ExternalPanelSessionRuntime {
    pub(super) fn canopa_focus_move(
        &mut self,
        forward: bool,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Option<Vec<ExternalPanelSessionEffect>> {
        let active = self.active.as_mut()?;
        if !active.matches_panel(active_panel) {
            return None;
        }
        let snapshot = active
            .content
            .elements
            .iter_mut()
            .find_map(|element| match element {
                crate::surface::panel::ExternalPanelElement::Canopa { snapshot } => Some(snapshot),
                _ => None,
            })?;
        snapshot.move_focus(forward);
        Some(vec![active.open_content_effect()])
    }

    pub(super) fn canopa_keyboard_activate(
        &self,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Option<Vec<ExternalPanelSessionEffect>> {
        let active = self.active.as_ref()?;
        if !active.matches_panel(active_panel) {
            return None;
        }
        let snapshot = active
            .content
            .elements
            .iter()
            .find_map(|element| match element {
                crate::surface::panel::ExternalPanelElement::Canopa { snapshot } => Some(snapshot),
                _ => None,
            })?;
        Some(
            snapshot
                .focused_action_id()
                .map(|id| {
                    self.button_activated(
                        active.provider_name.clone(),
                        id.to_string(),
                        active_panel,
                    )
                })
                .unwrap_or_default(),
        )
    }

    pub(super) fn begin_open_provider(&mut self, session_id: String) {
        self.close();
        self.pending_session_id = Some(session_id);
    }

    pub(super) fn handshake_completed(
        &mut self,
        session_id: String,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
        session: Option<crate::external_panel::ExternalPanelSessionHandle>,
        content: crate::surface::panel::ExternalPanelDocument,
    ) -> Vec<ExternalPanelSessionEffect> {
        if !self.pending_matches(&session_id) {
            return Vec::new();
        }
        self.pending_session_id = None;
        let content = crate::surface::panel::ExternalPanelContent::from_provider_document(
            provider_name.clone(),
            content,
        );
        let active = ActiveExternalPanelSession {
            session_id,
            provider_name,
            origin_window,
            origin,
            anchor,
            content,
            handle: session,
        };
        let mut effects = vec![active.open_content_effect()];
        effects.extend(active.watch_effects());
        self.active = Some(active);
        effects
    }

    pub(super) fn provider_failed(
        &mut self,
        session_id: String,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
        message: String,
    ) -> Vec<ExternalPanelSessionEffect> {
        if !self.pending_matches(&session_id) {
            return Vec::new();
        }
        self.pending_session_id = None;
        self.active = None;
        vec![ExternalPanelSessionEffect::OpenPanel {
            origin_window,
            origin,
            content: crate::surface::panel::ExternalPanelContent::failure(provider_name, message),
            anchor,
        }]
    }

    pub(super) fn provider_content_received(
        &mut self,
        session_id: String,
        provider_name: String,
        content: crate::surface::panel::ExternalPanelDocument,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if !active.matches_session(&session_id, &provider_name, active_panel) {
            return Vec::new();
        }
        active.content = crate::surface::panel::ExternalPanelContent::from_provider_document(
            provider_name,
            content,
        );
        let mut effects = vec![active.open_content_effect()];
        if let Some(handle) = active.handle.clone() {
            effects.push(ExternalPanelSessionEffect::WatchProviderContent {
                session_id: active.session_id.clone(),
                provider_name: active.provider_name.clone(),
                handle,
            });
        }
        effects
    }

    pub(super) fn provider_event_sent(
        &self,
        session_id: &str,
        provider_name: &str,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.matches_session(session_id, provider_name, active_panel))
    }

    pub(super) fn provider_session_failed(
        &mut self,
        session_id: String,
        provider_name: String,
        message: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let should_replace = self.active.as_ref().is_some_and(|active| {
            active.matches_session(&session_id, &provider_name, active_panel)
        });
        if !should_replace {
            return Vec::new();
        }
        let Some(mut active) = self.active.take() else {
            return Vec::new();
        };
        active.content =
            crate::surface::panel::ExternalPanelContent::failure(provider_name, message);
        vec![active.open_content_effect()]
    }

    pub(super) fn provider_session_ended(
        &mut self,
        session_id: &str,
        provider_name: &str,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> bool {
        let should_close = self
            .active
            .as_ref()
            .is_some_and(|active| active.matches_session(session_id, provider_name, active_panel));
        if should_close {
            self.active = None;
        }
        should_close
    }

    pub(super) fn replace_content_from_provider_response(
        &mut self,
        provider_name: String,
        content: crate::surface::panel::ExternalPanelDocument,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if active.provider_name != provider_name || !active.matches_panel(active_panel) {
            return Vec::new();
        }
        active.content = crate::surface::panel::ExternalPanelContent::from_provider_document(
            provider_name,
            content,
        );
        vec![active.open_content_effect()]
    }

    pub(super) fn button_activated(
        &self,
        provider_name: String,
        element_id: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        self.send_provider_event(
            provider_name,
            crate::external_panel::protocol::ProviderEvent::ButtonActivated { element_id },
            active_panel,
        )
    }

    pub(super) fn list_search_changed(
        &mut self,
        provider_name: String,
        list_id: String,
        query: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if active.provider_name != provider_name || !active.matches_panel(active_panel) {
            return Vec::new();
        }
        active.content = active.content.with_list_search(&list_id, query);
        vec![active.open_content_effect()]
    }

    pub(super) fn list_entry_activated(
        &self,
        provider_name: String,
        list_id: String,
        entry_id: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        self.send_provider_event(
            provider_name,
            crate::external_panel::protocol::ProviderEvent::SelectableListEntryActivated {
                element_id: list_id,
                entry_id,
            },
            active_panel,
        )
    }

    pub(super) fn list_custom_value_submitted(
        &self,
        provider_name: String,
        list_id: String,
        value: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        self.send_provider_event(
            provider_name,
            crate::external_panel::protocol::ProviderEvent::SelectableListCustomValueSubmitted {
                element_id: list_id,
                value,
            },
            active_panel,
        )
    }

    pub(super) fn text_input_changed(
        &mut self,
        provider_name: String,
        element_id: String,
        value: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if active.provider_name != provider_name || !active.matches_panel(active_panel) {
            return Vec::new();
        }
        active.content = active
            .content
            .with_text_input_value(&element_id, value.clone());
        let mut effects = vec![active.open_content_effect()];
        if let Some(handle) = active.handle.clone() {
            effects.push(ExternalPanelSessionEffect::SendProviderEvent {
                session_id: active.session_id.clone(),
                provider_name,
                handle,
                event: crate::external_panel::protocol::ProviderEvent::TextInputChanged {
                    element_id,
                    value,
                },
            });
        }
        effects
    }

    pub(super) fn text_input_submitted(
        &self,
        provider_name: String,
        element_id: String,
        value: String,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        self.send_provider_event(
            provider_name,
            crate::external_panel::protocol::ProviderEvent::TextInputSubmitted {
                element_id,
                value,
            },
            active_panel,
        )
    }

    pub(super) fn close(&mut self) -> bool {
        let had_pending = self.pending_session_id.take().is_some();
        let had_active = self.active.take().is_some();
        had_pending || had_active
    }

    pub(super) fn active_session_id(&self) -> Option<&str> {
        self.active
            .as_ref()
            .map(|session| session.session_id.as_str())
    }

    #[cfg(test)]
    pub(super) fn set_pending_for_test(&mut self, session_id: &str) {
        self.pending_session_id = Some(session_id.to_string());
    }

    fn pending_matches(&self, session_id: &str) -> bool {
        self.pending_session_id
            .as_deref()
            .is_some_and(|pending| pending == session_id)
    }

    fn send_provider_event(
        &self,
        provider_name: String,
        event: crate::external_panel::protocol::ProviderEvent,
        active_panel: Option<&ExternalPanelPanelIdentity>,
    ) -> Vec<ExternalPanelSessionEffect> {
        let Some(active) = self.active.as_ref() else {
            return Vec::new();
        };
        if active.provider_name != provider_name || !active.matches_panel(active_panel) {
            return Vec::new();
        }
        let Some(handle) = active.handle.clone() else {
            return Vec::new();
        };
        vec![ExternalPanelSessionEffect::SendProviderEvent {
            session_id: active.session_id.clone(),
            provider_name,
            handle,
            event,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_document(text: &str) -> crate::surface::panel::ExternalPanelDocument {
        crate::surface::panel::ExternalPanelDocument {
            elements: vec![crate::surface::panel::ExternalPanelElement::Text { text: text.into() }],
        }
    }

    fn selectable_document() -> crate::surface::panel::ExternalPanelDocument {
        crate::surface::panel::ExternalPanelDocument {
            elements: vec![
                crate::surface::panel::ExternalPanelElement::SelectableList {
                    id: "networks".into(),
                    entries: Vec::new(),
                    allow_custom: true,
                    search_query: String::new(),
                },
            ],
        }
    }

    fn text_input_document() -> crate::surface::panel::ExternalPanelDocument {
        crate::surface::panel::ExternalPanelDocument {
            elements: vec![crate::surface::panel::ExternalPanelElement::TextInput {
                id: "credential".into(),
                label: "Password".into(),
                value: String::new(),
                secret: true,
                error: None,
            }],
        }
    }

    fn active_panel(origin_window: iced::window::Id) -> ExternalPanelPanelIdentity {
        ExternalPanelPanelIdentity {
            origin_window,
            origin: crate::surface::panel::PanelOrigin::Bar,
            provider_name: "wifi".into(),
        }
    }

    fn complete_handshake(
        runtime: &mut ExternalPanelSessionRuntime,
        origin_window: iced::window::Id,
        document: crate::surface::panel::ExternalPanelDocument,
    ) -> Vec<ExternalPanelSessionEffect> {
        runtime.begin_open_provider("session-1".into());
        runtime.handshake_completed(
            "session-1".into(),
            origin_window,
            crate::surface::panel::PanelOrigin::Bar,
            "wifi".into(),
            None,
            None,
            document,
        )
    }

    #[test]
    fn stale_handshake_is_ignored_without_pending_session() {
        let mut runtime = ExternalPanelSessionRuntime::default();

        let effects = runtime.handshake_completed(
            "session-1".into(),
            iced::window::Id::unique(),
            crate::surface::panel::PanelOrigin::Bar,
            "wifi".into(),
            None,
            None,
            text_document("stale"),
        );

        assert!(effects.is_empty());
        assert_eq!(runtime.active_session_id(), None);
    }

    #[test]
    fn handshake_opens_content_and_stores_active_session() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();

        let effects = complete_handshake(&mut runtime, origin_window, text_document("ready"));

        assert_eq!(runtime.active_session_id(), Some("session-1"));
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            ExternalPanelSessionEffect::OpenPanel {
                origin_window: effect_window,
                content,
                ..
            } => {
                assert_eq!(*effect_window, origin_window);
                assert_eq!(content.provider_name, "wifi");
                assert_eq!(
                    content.elements,
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "ready".into(),
                    }]
                );
            }
            other => panic!("expected open panel effect, got {other:?}"),
        }
    }

    #[test]
    fn content_update_requires_session_provider_and_active_panel_identity() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();
        let _ = complete_handshake(&mut runtime, origin_window, text_document("initial"));
        let current_panel = active_panel(origin_window);

        let stale_session_effects = runtime.provider_content_received(
            "stale-session".into(),
            "wifi".into(),
            text_document("stale"),
            Some(&current_panel),
        );
        let stale_panel_effects = runtime.provider_content_received(
            "session-1".into(),
            "wifi".into(),
            text_document("stale"),
            Some(&active_panel(iced::window::Id::unique())),
        );
        let current_effects = runtime.provider_content_received(
            "session-1".into(),
            "wifi".into(),
            text_document("updated"),
            Some(&current_panel),
        );

        assert!(stale_session_effects.is_empty());
        assert!(stale_panel_effects.is_empty());
        assert_eq!(current_effects.len(), 1);
        match &current_effects[0] {
            ExternalPanelSessionEffect::OpenPanel { content, .. } => {
                assert_eq!(
                    content.elements,
                    vec![crate::surface::panel::ExternalPanelElement::Text {
                        text: "updated".into(),
                    }]
                );
            }
            other => panic!("expected open panel effect, got {other:?}"),
        }
    }

    #[test]
    fn provider_session_failure_replaces_active_content_with_failure() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();
        let _ = complete_handshake(&mut runtime, origin_window, text_document("initial"));
        let active_panel = active_panel(origin_window);

        let effects = runtime.provider_session_failed(
            "session-1".into(),
            "wifi".into(),
            "boom".into(),
            Some(&active_panel),
        );

        assert_eq!(runtime.active_session_id(), None);
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            ExternalPanelSessionEffect::OpenPanel { content, .. } => {
                assert!(content.failure_text().contains("boom"));
            }
            other => panic!("expected open panel effect, got {other:?}"),
        }
    }

    #[test]
    fn list_search_changed_updates_local_echo() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();
        let _ = complete_handshake(&mut runtime, origin_window, selectable_document());
        let active_panel = active_panel(origin_window);

        let effects = runtime.list_search_changed(
            "wifi".into(),
            "networks".into(),
            "home".into(),
            Some(&active_panel),
        );

        match &effects[0] {
            ExternalPanelSessionEffect::OpenPanel { content, .. } => {
                assert_eq!(
                    content.elements,
                    vec![
                        crate::surface::panel::ExternalPanelElement::SelectableList {
                            id: "networks".into(),
                            entries: Vec::new(),
                            allow_custom: true,
                            search_query: "home".into(),
                        }
                    ]
                );
            }
            other => panic!("expected open panel effect, got {other:?}"),
        }
    }

    #[test]
    fn text_input_changed_updates_local_echo_before_provider_response() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();
        let _ = complete_handshake(&mut runtime, origin_window, text_input_document());
        let active_panel = active_panel(origin_window);

        let effects = runtime.text_input_changed(
            "wifi".into(),
            "credential".into(),
            "secret".into(),
            Some(&active_panel),
        );

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            ExternalPanelSessionEffect::OpenPanel { content, .. } => {
                assert_eq!(
                    content.elements,
                    vec![crate::surface::panel::ExternalPanelElement::TextInput {
                        id: "credential".into(),
                        label: "Password".into(),
                        value: "secret".into(),
                        secret: true,
                        error: None,
                    }]
                );
            }
            other => panic!("expected open panel effect, got {other:?}"),
        }
    }

    #[test]
    fn close_is_idempotent() {
        let origin_window = iced::window::Id::unique();
        let mut runtime = ExternalPanelSessionRuntime::default();
        let _ = complete_handshake(&mut runtime, origin_window, text_document("initial"));

        assert!(runtime.close());
        assert!(!runtime.close());
        assert_eq!(runtime.active_session_id(), None);
    }
}
