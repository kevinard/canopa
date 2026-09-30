//! Exercise provider content and queued input through the application interface.
use super::*;
use crate::app::messages::{ExternalPanelEvent, SurfaceEvent};
use crate::surface::panel::{
    ExternalPanelContent, ExternalPanelDocument, ExternalPanelElement, PanelContent, PanelOrigin,
};
use canopa_shell::external_panel::canopa::{CanopaSession, CanopaSnapshot};

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
    ExternalPanelDocument::Canopa(Box::new(snapshot))
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
    content.document.canopa().expect("expected Canopa content")
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
async fn replacement_preserves_selection_with_a_fresh_action_and_the_same_keyboard_window() {
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
    assert_eq!(displayed(&app).focused_label(), Some("Second"));
    assert_ne!(displayed(&app).focused_action_id(), Some(old_id.as_str()));
    let _ = app.update(Message::Surface(SurfaceEvent::FocusMovePrev(window)));
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
fn v1_text_and_search_echo_keep_the_focused_window() {
    let (mut app, window) = open(ExternalPanelDocument::Elements {
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
        assert_eq!(app.foreground.active_panel_window(), Some(window));
        assert!(matches!(&displayed_content(&app).elements()[0],
            ExternalPanelElement::TextInput { value: actual, .. } if actual == value));
        assert_eq!(displayed_content(&app).list_search("list"), value);
    }
}

#[test]
fn v1_background_updates_keep_the_existing_panel_window() {
    let button = |id: &str| ExternalPanelDocument::Elements {
        elements: vec![ExternalPanelElement::Button {
            id: id.into(),
            label: "Action".into(),
        }],
    };
    let (mut app, window) = open(button("old"));
    for revision in 0..20 {
        let content = button(&format!("new-{revision}"));
        let _ = app.update(Message::ExternalPanel(
            ExternalPanelEvent::ProviderContentReceived {
                session_id: "session-1".into(),
                provider_name: "notifications".into(),
                content: content.clone(),
            },
        ));
        assert_eq!(app.foreground.active_panel_window(), Some(window));
        assert_eq!(displayed_content(&app).elements(), content.elements());
    }
}

#[test]
fn provider_replacement_during_entry_reveals_new_controls_in_the_same_window() {
    use crate::surface::panel_motion::Presentation;
    let document = |id: &str| ExternalPanelDocument::Elements {
        elements: vec![ExternalPanelElement::Button {
            id: id.into(),
            label: id.into(),
        }],
    };
    let (mut app, window) = open(document("old"));
    app.foreground.configure_panel_motion(
        window,
        &crate::config::Edge::Left,
        crate::config::PanelAnimationConfig::default(),
    );
    app.foreground.panel_motion_ready(window, 1.0);
    assert!(matches!(
        app.foreground.panel_presentation(window),
        Presentation::Animated(_)
    ));
    provider_update(&mut app, document("new"));
    assert_eq!(app.foreground.active_panel_window(), Some(window));
    assert_eq!(
        app.foreground.panel_presentation(window),
        Presentation::Still
    );
    assert!(
        matches!(&displayed_content(&app).elements()[0], ExternalPanelElement::Button { id, .. } if id == "new")
    );
}

fn widget_event(
    app: &App,
    window: iced::window::Id,
    state: &mut iced::advanced::widget::Tree,
    renderer: &iced::Renderer,
    event: iced::Event,
) -> Vec<Message> {
    let mut view = app.view(window);
    state.diff(view.as_widget());
    let viewport = iced::Rectangle::with_size(iced::Size::new(360.0, 400.0));
    let node = view.as_widget_mut().layout(
        state,
        renderer,
        &iced::advanced::layout::Limits::new(iced::Size::ZERO, viewport.size()),
    );
    let mut messages = Vec::new();
    view.as_widget_mut().update(
        state,
        &event,
        iced::advanced::Layout::new(&node),
        iced::mouse::Cursor::Available(iced::Point::new(30.0, 85.0)),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut iced::advanced::Shell::new(&mut messages),
        &viewport,
    );
    messages
}

fn provider_update(app: &mut App, content: ExternalPanelDocument) {
    let _ = app.update(Message::ExternalPanel(
        ExternalPanelEvent::ProviderContentReceived {
            session_id: "session-1".into(),
            provider_name: "notifications".into(),
            content,
        },
    ));
}

#[tokio::test]
async fn v1_replacement_cancels_an_old_button_press_without_recreating_the_window() {
    use iced::advanced::renderer::Headless;
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let button = |id: &str| ExternalPanelDocument::Elements {
        elements: vec![ExternalPanelElement::Button {
            id: id.into(),
            label: "Action".into(),
        }],
    };
    let (mut app, window) = open(button("old"));
    let mut state = iced::advanced::widget::Tree::empty();
    let press = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
    let release = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
        iced::mouse::Button::Left,
    ));
    assert!(widget_event(&app, window, &mut state, &renderer, press.clone()).is_empty());
    provider_update(&mut app, button("new"));
    assert_eq!(app.foreground.active_panel_window(), Some(window));
    assert!(
        widget_event(&app, window, &mut state, &renderer, release.clone()).is_empty(),
        "a held click must not activate the replacement button"
    );
    assert!(widget_event(&app, window, &mut state, &renderer, press).is_empty());
    assert!(
        matches!(widget_event(&app, window, &mut state, &renderer, release).as_slice(),
        [Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { element_id, .. })] if element_id == "new")
    );
}

#[tokio::test]
async fn v1_typing_survives_echo_and_background_updates_but_not_a_different_field() {
    use iced::advanced::renderer::Headless;
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let input = |id: &str, value: &str| ExternalPanelDocument::Elements {
        elements: vec![ExternalPanelElement::TextInput {
            id: id.into(),
            label: "Name".into(),
            value: value.into(),
            secret: false,
            error: None,
        }],
    };
    let (mut app, window) = open(input("name", ""));
    let mut state = iced::advanced::widget::Tree::empty();
    for event in [
        iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
        iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
    ] {
        let _ = widget_event(
            &app,
            window,
            &mut state,
            &renderer,
            iced::Event::Mouse(event),
        );
    }
    let key = |value: &str| {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(value.into()),
            modified_key: iced::keyboard::Key::Character(value.into()),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::KeyA),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: Some(value.into()),
            repeat: false,
        })
    };
    for (character, value) in [("a", "a"), ("b", "ab"), ("c", "abc")] {
        let messages = widget_event(&app, window, &mut state, &renderer, key(character));
        assert!(
            matches!(messages.as_slice(), [Message::ExternalPanel(ExternalPanelEvent::TextInputChanged { value: actual, .. })] if actual == value)
        );
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(app.foreground.active_panel_window(), Some(window));
        provider_update(&mut app, input("name", value));
        assert_eq!(app.foreground.active_panel_window(), Some(window));
    }
    provider_update(&mut app, input("different-field", ""));
    assert!(
        widget_event(&app, window, &mut state, &renderer, key("d")).is_empty(),
        "keyboard focus must not transfer to a different field"
    );
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
    let current = displayed(&app).focused_action_id().unwrap().to_owned();
    assert_ne!(current, id);
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "notifications".into(),
        action_id: id,
    });
    assert_eq!(displayed(&app).focused_action_id(), Some(current.as_str()));
    let snapshot = displayed(&app);
    let id = snapshot.action_id(
        canopa_panel_protocol::InteractionTarget::Content,
        &snapshot.content.actions[0],
    );
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "other".into(),
        action_id: id.clone(),
    });
    assert_eq!(displayed(&app).focused_action_id(), Some(current.as_str()));
    let _ = app.update(Message::Surface(SurfaceEvent::EscapePressed(window)));
    let _ = app.update(Message::CanopaAccessibilityFocus {
        provider: "notifications".into(),
        action_id: id,
    });
    assert_eq!(app.foreground.active_panel_window(), None);
}
