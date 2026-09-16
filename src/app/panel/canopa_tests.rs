//! Exercise provider content and queued input through the application interface.
use super::*;
use crate::app::messages::{ExternalPanelEvent, SurfaceEvent};
use crate::surface::panel::{
    ExternalPanelContent, ExternalPanelDocument, ExternalPanelElement, PanelContent, PanelOrigin,
};
use niribar::external_panel::canopa::{CanopaSession, CanopaSnapshot};

async fn document(session: &mut CanopaSession) -> ExternalPanelDocument {
    let content = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": {
            "title": "Notifications", "error": null,
            "actions": [
                {"key": "first", "label": "First", "value": null},
                {"key": "second", "label": "Second", "value": null}
            ],
            "sections": []
        }
    });
    let mut bytes = serde_json::to_vec(&content).unwrap();
    bytes.push(b'\n');
    let snapshot = session
        .read_content(&mut tokio::io::BufReader::new(bytes.as_slice()))
        .await
        .unwrap();
    ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Canopa { snapshot }],
    }
}

fn open(content: ExternalPanelDocument) -> (App, iced::window::Id) {
    let (mut app, _task) = App::boot(crate::config::Config::default(), Vec::new());
    let origin = iced::window::Id::unique();
    app.surface_mut(origin);
    app.set_pending_external_panel_session_for_test("session-1");
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::HandshakeCompleted {
            session_id: "session-1".into(),
            origin_window: origin,
            origin: PanelOrigin::Bar,
            provider_name: "notifications".into(),
            anchor: None,
            session: None,
            content,
        },
    ));
    let window = app.foreground.active_panel_window().expect("open panel");
    (app, window)
}

fn displayed_content(app: &App) -> &ExternalPanelContent {
    let PanelContent::ExternalProvider(content) = &app
        .foreground
        .active_panel_tree()
        .expect("active panel")
        .content
    else {
        panic!("expected provider panel");
    };
    content
}

fn displayed(app: &App) -> &CanopaSnapshot {
    let content = displayed_content(app);
    let ExternalPanelElement::Canopa { snapshot } = &content.elements[0] else {
        panic!("expected Canopa content");
    };
    snapshot
}

#[tokio::test]
async fn queued_navigation_keeps_the_panel_window_and_delivers_both_keys() {
    let (mut app, window) = open(document(&mut CanopaSession::default()).await);
    // Both events were queued for the window that currently owns keyboard focus.
    // Do not look up a new window between events: the compositor cannot do that.
    for _ in 0..2 {
        let _ = app.update(Message::Surface(SurfaceEvent::FocusMovePrev(window)));
    }
    assert_eq!(displayed(&app).focused_label(), Some("First"));
    assert_eq!(app.foreground.active_panel_window(), Some(window));
    let _ = app.update(Message::Surface(SurfaceEvent::EscapePressed(window)));
    assert_eq!(app.foreground.active_panel_window(), None);
    assert_eq!(app.active_external_panel_session_id_for_test(), None);
}

#[tokio::test]
async fn replacement_clears_selection_without_interrupting_the_keyboard_window() {
    let mut session = CanopaSession::default();
    let (mut app, window) = open(document(&mut session).await);
    let _ = app.update(Message::Surface(SurfaceEvent::FocusMovePrev(window)));
    let old_id = displayed(&app).focused_action_id().unwrap().to_owned();
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderContentReceived {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            content: document(&mut session).await,
        },
    ));
    assert_eq!(displayed(&app).focused_action_id(), None);
    for _ in 0..2 {
        let _ = app.update(Message::Surface(SurfaceEvent::FocusMovePrev(window)));
    }
    assert_eq!(displayed(&app).focused_label(), Some("First"));
    assert_ne!(displayed(&app).focused_action_id(), Some(old_id.as_str()));
    assert_eq!(app.foreground.active_panel_window(), Some(window));
}

#[tokio::test]
async fn stale_updates_do_not_change_selection_or_reopen_a_closed_panel() {
    let mut session = CanopaSession::default();
    let (mut app, window) = open(document(&mut session).await);
    let _ = app.update(Message::Surface(SurfaceEvent::FocusMovePrev(window)));
    let selected = displayed(&app).focused_action_id().unwrap().to_owned();
    for (session_id, provider_name) in [("old-session", "notifications"), ("session-1", "other")] {
        let _ = app.update(Message::ExternalPanel(
            ExternalPanelEvent::ProviderContentReceived {
                session_id: session_id.into(),
                provider_name: provider_name.into(),
                content: document(&mut session).await,
            },
        ));
        assert_eq!(displayed(&app).focused_action_id(), Some(selected.as_str()));
        assert_eq!(app.foreground.active_panel_window(), Some(window));
    }
    let _ = app.update(Message::Surface(SurfaceEvent::EscapePressed(window)));
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderContentReceived {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            content: document(&mut session).await,
        },
    ));
    assert_eq!(app.foreground.active_panel_window(), None);
}

#[tokio::test]
async fn provider_failure_remains_in_the_same_keyboard_dismissible_window() {
    let (mut app, window) = open(document(&mut CanopaSession::default()).await);
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderSessionFailed {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            message: "fixture disconnected".into(),
        },
    ));
    assert!(
        displayed_content(&app)
            .failure_text()
            .contains("fixture disconnected")
    );
    assert_eq!(app.foreground.active_panel_window(), Some(window));
    assert_eq!(app.active_external_panel_session_id_for_test(), None);
    let _ = app.update(Message::Surface(SurfaceEvent::EscapePressed(window)));
    assert_eq!(app.foreground.active_panel_window(), None);
}

#[test]
fn v1_text_and_search_echo_retain_existing_reopen_behavior() {
    let (mut app, mut window) = open(ExternalPanelDocument {
        elements: vec![
            ExternalPanelElement::TextInput {
                id: "entry".into(),
                label: "Name".into(),
                value: String::new(),
                secret: false,
                error: None,
            },
            ExternalPanelElement::SelectableList {
                id: "list".into(),
                entries: Vec::new(),
                allow_custom: true,
                search_query: String::new(),
            },
        ],
    });
    for value in ["a", "ab", "abc"] {
        let _ = app.update(Message::ExternalPanel(
            ExternalPanelEvent::TextInputChanged {
                provider_name: "notifications".into(),
                element_id: "entry".into(),
                value: value.into(),
            },
        ));
        let _ = app.update(Message::ExternalPanel(
            ExternalPanelEvent::ListSearchChanged {
                provider_name: "notifications".into(),
                list_id: "list".into(),
                query: value.into(),
            },
        ));
        assert_ne!(app.foreground.active_panel_window(), Some(window));
        window = app.foreground.active_panel_window().unwrap();
        assert!(matches!(&displayed_content(&app).elements[0],
            ExternalPanelElement::TextInput { value: actual, .. } if actual == value));
        assert!(matches!(&displayed_content(&app).elements[1],
            ExternalPanelElement::SelectableList { search_query, .. } if search_query == value));
    }
}

#[test]
fn v1_button_replacement_retires_the_window_with_the_old_press_state() {
    let button = |id: &str| ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Button {
            id: id.into(),
            label: "Action".into(),
        }],
    };
    let (mut app, window) = open(button("old"));
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderContentReceived {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            content: button("new"),
        },
    ));
    assert_ne!(app.foreground.active_panel_window(), Some(window));
    assert_eq!(displayed_content(&app).elements, button("new").elements);
}

#[tokio::test]
async fn accessibility_focus_uses_the_current_opaque_action_without_recreating_the_window() {
    let mut session = CanopaSession::default();
    let (mut app, window) = open(document(&mut session).await);
    let snapshot = displayed(&app);
    let id = snapshot.action_id(
        canopa_panel_protocol::InteractionTarget::Content,
        &snapshot.content.actions[1],
    );
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "notifications".into(),
        action_id: id.clone(),
    });
    assert_eq!(displayed(&app).focused_label(), Some("Second"));
    assert_eq!(app.foreground.active_panel_window(), Some(window));
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderContentReceived {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            content: document(&mut session).await,
        },
    ));
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "notifications".into(),
        action_id: id,
    });
    assert_eq!(displayed(&app).focused_action_id(), None);
    let snapshot = displayed(&app);
    let id = snapshot.action_id(
        canopa_panel_protocol::InteractionTarget::Content,
        &snapshot.content.actions[0],
    );
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "other".into(),
        action_id: id.clone(),
    });
    assert_eq!(displayed(&app).focused_action_id(), None);
    let _ = app.update(Message::Surface(SurfaceEvent::EscapePressed(window)));
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "notifications".into(),
        action_id: id,
    });
    assert_eq!(app.foreground.active_panel_window(), None);
}
