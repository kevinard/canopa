use super::*;
use canopa_shell::external_panel::canopa::CanopaSession;
use iced::advanced::{Layout, layout, renderer::Headless, widget::Operation, widget::Tree};
use iced::{Rectangle, Size};

#[derive(Default)]
struct TextBounds(Vec<(String, Rectangle)>);

fn pointer_event(
    snapshot: &CanopaSnapshot,
    renderer: &iced::Renderer,
    tree: &mut Tree,
    viewport: Rectangle,
    cursor: iced::Point,
    event: iced::mouse::Event,
) -> Vec<Message> {
    let mut view = render(
        snapshot,
        "notifications",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        tree,
        renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut messages = Vec::new();
    view.as_widget_mut().update(
        tree,
        &iced::Event::Mouse(event),
        Layout::new(&node),
        iced::mouse::Cursor::Available(cursor),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut iced::advanced::Shell::new(&mut messages),
        &viewport,
    );
    messages
}

#[tokio::test]
async fn replacement_during_a_pointer_press_never_activates_the_new_action() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    for selected in [false, true] {
        let mut old = long_title_snapshot().await;
        if selected {
            old.move_focus(false);
        }
        let viewport = Rectangle::with_size(Size::new(360.0, 180.0));
        let mut tree = Tree::empty();
        let cursor = if selected {
            iced::Point::new(40.0, 80.0)
        } else {
            let mut view = render(
                &old,
                "notifications",
                false,
                crate::theme::ui(canopa_theme::UiKind::Panel),
            );
            tree.diff(view.as_widget());
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, viewport.size()),
            );
            let mut labels = TextBounds::default();
            view.as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
            labels
                .0
                .iter()
                .find(|(label, _)| label == "Refresh")
                .unwrap()
                .1
                .center()
        };
        assert!(
            pointer_event(
                &old,
                &renderer,
                &mut tree,
                viewport,
                cursor,
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)
            )
            .is_empty()
        );
        let mut next = long_title_snapshot().await;
        if selected {
            next.move_focus(false);
        }
        let released = pointer_event(
            &next,
            &renderer,
            &mut tree,
            viewport,
            cursor,
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
        );
        assert!(
            released.is_empty(),
            "an old press activated replacement content (selected={selected})"
        );
        assert!(
            pointer_event(
                &next,
                &renderer,
                &mut tree,
                viewport,
                cursor,
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)
            )
            .is_empty()
        );
        let fresh = pointer_event(
            &next,
            &renderer,
            &mut tree,
            viewport,
            cursor,
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
        );
        assert!(
            matches!(fresh.as_slice(), [Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { provider_name, .. })]
            if provider_name == "notifications"),
            "fresh press/release must remain actionable"
        );
    }
}

impl Operation for TextBounds {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn text(&mut self, _: Option<&iced::widget::Id>, bounds: Rectangle, text: &str) {
        self.0.push((text.to_owned(), bounds));
    }
}

#[tokio::test]
async fn scrolling_cards_leave_room_for_the_scrollbar() {
    struct CardBounds {
        id: iced::widget::Id,
        card: Option<Rectangle>,
        scroll: Option<(Rectangle, Rectangle)>,
    }
    impl Operation for CardBounds {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }

        fn container(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle) {
            if id == Some(&self.id) {
                self.card = Some(bounds);
            }
        }

        fn scrollable(
            &mut self,
            _: Option<&iced::widget::Id>,
            bounds: Rectangle,
            content: Rectangle,
            _: iced::Vector,
            _: &mut dyn iced::advanced::widget::operation::Scrollable,
        ) {
            self.scroll = Some((bounds, content));
        }
    }
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let snapshot = long_title_snapshot().await;
    for width in [220.0, 360.0, 480.0] {
        let mut view = render(
            &snapshot,
            "notifications",
            false,
            crate::theme::ui(canopa_theme::UiKind::Panel),
        );
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(width, 180.0)),
        );
        let mut bounds = CardBounds {
            id: super::super::canopa_accessibility_bounds::widget_id(&semantic_key(
                &snapshot,
                "notifications",
                &SemanticNode::Item("message".into()),
            )),
            card: None,
            scroll: None,
        };
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut bounds);
        let card = bounds.card.expect("notification card");
        let (viewport, content) = bounds.scroll.expect("reading viewport");
        assert!(content.height > viewport.height, "fixture must scroll");
        // Default Iced scrollbar is 10 px wide; keep another 6 px clear.
        assert!(
            card.x + card.width <= viewport.x + viewport.width - 16.0,
            "card overlaps the scrollbar at width {width}: {card:?}, viewport {viewport:?}"
        );
    }
}

#[tokio::test]
async fn short_actions_share_a_row_and_long_actions_wrap_within_the_panel() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let long_label = "Open the complete conversation in the application and show all replies";
    let content = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": {
            "title": "Notifications", "actions": [], "error": null,
            "sections": [{"id": "mail", "title": "Mail", "controls": [],
                "items": [{"id": "message", "title": "New message", "body": "Hello",
                    "attribution": {"label":"Mail", "icon":{"kind":"local_path", "path":"/missing-source.png"}},
                    "metadata": [{"key": "flag", "label": "Unread", "value": ""}], "actions": [
                        {"key": "reply", "label": "Reply"},
                        {"key": "dismiss", "label": "Dismiss"},
                        {"key": "open", "label": long_label}
                    ]
                }]
            }]
        }
    });
    let bytes = (serde_json::to_string(&content).unwrap() + "\n").into_bytes();
    let snapshot = CanopaSession::default()
        .read_content(&mut tokio::io::BufReader::new(bytes.as_slice()))
        .await
        .unwrap();
    for width in [220.0, 360.0] {
        let viewport = Rectangle::with_size(Size::new(width, 600.0));
        let mut view = render(
            &snapshot,
            "notifications",
            false,
            crate::theme::ui(canopa_theme::UiKind::Panel),
        );
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut labels = TextBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        let bounds = |label| labels.0.iter().find(|(text, _)| text == label).unwrap().1;
        assert_eq!(
            labels.0.iter().filter(|(label, _)| label == "Mail").count(),
            1,
            "source label must replace the duplicate group heading"
        );
        assert!(
            bounds("Mail").x >= bounds("M").x + 20.0,
            "the fallback icon belongs beside its source label"
        );
        assert_eq!(bounds("New message").x, bounds("Hello").x);
        assert!(bounds("Mail").y < bounds("New message").y);
        assert!(bounds("Unread").x > bounds("Mail").x);
        let key = semantic_key(
            &snapshot,
            "notifications",
            &SemanticNode::Metadata("message".into(), 0),
        );
        let mut measure = crate::app::canopa_accessibility_bounds::Measure::new(
            std::iter::once(key.clone()),
            Default::default(),
        );
        view.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut iced::advanced::widget::operation::black_box(&mut measure),
        );
        let marker = measure.bounds[&key];
        assert!(marker.width >= bounds("Unread").width && marker.height <= 24.0);
        assert!(marker.y < bounds("Hello").y && viewport.contains(marker.center()));
        let reply = bounds("Reply");
        let dismiss = bounds("Dismiss");
        let long = bounds(long_label);
        assert_eq!(reply.y, dismiss.y, "short actions should share one row");
        assert!(dismiss.x > reply.x);
        assert!(long.y > reply.y && long.height > reply.height);
        for area in [reply, dismiss, long] {
            assert!(viewport.contains(area.position()));
            assert!(viewport.contains(iced::Point::new(area.x + area.width, area.y + area.height)));
        }
    }
}

async fn long_title_snapshot() -> CanopaSnapshot {
    let content = serde_json::json!({
        "type": "replace_content", "protocol_version": 4,
        "content": {
            "title": "Notifications", "error": null,
            "actions": [{"key": "refresh", "label": "Refresh", "value": null}],
            "sections": [{"id": "mail", "title": "Mail", "controls": [],
                "items": [{"id": "message", "title": "Long contextual title ".repeat(50),
                    "attribution": {"label":"Mail"},
                    "body": "The notification body", "metadata": [],
                    "actions": [
                        {"key": "open", "label": "Open notification", "value": "revision-bound"},
                        {"key": "reply", "label": "Reply", "value": "revision-bound"}
                    ]
                }]
            }]
        }
    });
    let mut bytes = serde_json::to_vec(&content).unwrap();
    bytes.push(b'\n');
    CanopaSession::default()
        .read_content(&mut tokio::io::BufReader::new(bytes.as_slice()))
        .await
        .unwrap()
}

#[tokio::test]
async fn drawer_provider_header_keeps_one_title_and_closes_through_foreground() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let snapshot = long_title_snapshot().await;
    let viewport = Rectangle::with_size(Size::new(360.0, 800.0));
    let mut view = render(
        &snapshot,
        "notifications",
        true,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    let mut tree = Tree::new(view.as_widget());
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut labels = TextBounds::default();
    view.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
    assert_eq!(
        labels
            .0
            .iter()
            .filter(|(label, _)| label == "Notifications")
            .count(),
        1
    );
    let close = labels
        .0
        .iter()
        .find(|(label, _)| label == "Close")
        .unwrap()
        .1;
    assert!(viewport.contains(close.center()));
    let mut messages = Vec::new();
    for event in [
        iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
        iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
    ] {
        view.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(event),
            Layout::new(&node),
            iced::mouse::Cursor::Available(close.center()),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut iced::advanced::Shell::new(&mut messages),
            &viewport,
        );
    }
    assert!(matches!(
        messages.as_slice(),
        [Message::Panel(super::super::messages::PanelEvent::Close)]
    ));
}

#[derive(Default)]
struct ScrollBounds(Vec<(Rectangle, iced::Vector)>);

impl Operation for ScrollBounds {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: iced::Vector,
        _: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        self.0.push((bounds, translation));
    }
}

#[tokio::test]
async fn selected_preview_scrolls_without_activation_and_resets_for_next_action() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let mut snapshot = long_title_snapshot().await;
    snapshot.move_focus(false);
    let viewport = Rectangle::with_size(Size::new(360.0, 180.0));
    let cursor = iced::Point::new(40.0, 80.0);
    let mut tree = Tree::empty();
    {
        let mut view = render(
            &snapshot,
            "notifications",
            false,
            crate::theme::ui(canopa_theme::UiKind::Panel),
        );
        tree.diff(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut messages = Vec::new();
        view.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -3.0 },
            }),
            Layout::new(&node),
            iced::mouse::Cursor::Available(cursor),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut iced::advanced::Shell::new(&mut messages),
            &viewport,
        );
        assert!(
            messages.is_empty(),
            "reading the context must not activate it"
        );
        let mut scroll = ScrollBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
        assert!(
            scroll
                .0
                .iter()
                .any(|(bounds, offset)| bounds.contains(cursor) && offset.y > 0.0),
            "the full contextual label must remain scrollable"
        );
    }
    snapshot.move_focus(false);
    let mut view = render(
        &snapshot,
        "notifications",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut scroll = ScrollBounds::default();
    view.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
    assert!(
        scroll
            .0
            .iter()
            .any(|(bounds, offset)| bounds.contains(cursor) && offset.y == 0.0),
        "a newly selected action must reveal the start of its label"
    );
    let mut messages = Vec::new();
    for event in [
        iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
        iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
    ] {
        view.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(event),
            Layout::new(&node),
            iced::mouse::Cursor::Available(cursor),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut iced::advanced::Shell::new(&mut messages),
            &viewport,
        );
    }
    assert!(
        matches!(messages.as_slice(), [Message::ExternalPanel(
        ExternalPanelEvent::ButtonActivated { provider_name, element_id }
    )] if provider_name == "notifications" && Some(element_id.as_str()) == snapshot.focused_action_id()),
        "clicking the preview must invoke only the newly selected action"
    );
}

#[tokio::test]
async fn long_keyboard_selection_leaves_panel_content_visible() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer");
    let mut snapshot = long_title_snapshot().await;
    snapshot.move_focus(false);
    let mut view = render(
        &snapshot,
        "notifications",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    let mut tree = Tree::new(view.as_widget());
    let viewport = Rectangle::with_size(Size::new(360.0, 180.0));
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut labels = TextBounds::default();
    view.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
    let (_, refresh) = labels
        .0
        .iter()
        .find(|(label, _)| label == "Refresh")
        .unwrap();
    assert!(
        refresh.height >= 16.0,
        "content action collapsed: {refresh:?}"
    );
    assert!(
        viewport.contains(refresh.position())
            && viewport.contains(iced::Point::new(
                refresh.x + refresh.width,
                refresh.y + refresh.height,
            )),
        "content action is clipped by the selected preview: {refresh:?}"
    );
}

#[tokio::test]
async fn keyboard_reading_scrolls_content_without_changing_or_invoking_selection() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let mut snapshot = long_title_snapshot().await;
    snapshot.move_focus(false);
    let selected = snapshot.focused_action_id().unwrap().to_owned();
    let viewport = Rectangle::with_size(Size::new(360.0, 180.0));
    let mut view = render(
        &snapshot,
        "notifications",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    let mut tree = Tree::new(view.as_widget());
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut offsets = Vec::new();
    use iced::keyboard::{self, key::Named};
    for key in [
        Named::PageDown,
        Named::PageDown,
        Named::End,
        Named::PageUp,
        Named::Home,
    ] {
        let mut messages = Vec::new();
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key),
            modified_key: keyboard::Key::Named(key),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::PageDown),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: None,
            repeat: true,
        });
        let mut shell = iced::advanced::Shell::new(&mut messages);
        view.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            iced::mouse::Cursor::Unavailable,
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut shell,
            &viewport,
        );
        assert!(shell.is_event_captured());
        assert!(messages.is_empty(), "reading must not activate an action");
        let mut scroll = ScrollBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
        offsets.push(scroll.0.last().unwrap().1.y);
    }
    assert!(offsets[0] > 0.0 && offsets[1] > offsets[0] && offsets[2] > offsets[1]);
    assert!(offsets[3] < offsets[2] && offsets[3] > 0.0);
    assert_eq!(offsets[4], 0.0);
    assert_eq!(snapshot.focused_action_id(), Some(selected.as_str()));
}

async fn link_snapshot(session: &CanopaSession, value: &str) -> CanopaSnapshot {
    let content = serde_json::json!({
        "type":"replace_content", "protocol_version":canopa_panel_protocol::PROTOCOL_VERSION,
        "content":{"title":"Notifications", "actions":[], "error":null,
            "sections":[{"id":"mail", "title":"Mail", "controls":[], "items":[{
                "id":"message", "title":"Message", "metadata":[], "actions":[],
                "rich_body":{"segments":[
                    {"kind":"text", "text":"Before the link", "style":{"bold":false,"italic":false,"underline":false}},
                    {"kind":"link", "uri":"https://example.org/page", "action":{"key":"opaque-link", "label":"Read the page", "value":value}, "style":{"bold":true,"italic":false,"underline":false}},
                    {"kind":"text", "text":"After the link", "style":{"bold":false,"italic":false,"underline":false}}
                ]}
            }]}]
        }
    });
    let mut bytes = serde_json::to_vec(&content).unwrap();
    bytes.push(b'\n');
    session
        .read_content(&mut tokio::io::BufReader::new(bytes.as_slice()))
        .await
        .unwrap()
}

#[tokio::test]
async fn canopa_body_links_support_selection_clicks_and_replacement_guards() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let session = CanopaSession::default();
    let mut old = link_snapshot(&session, "old-revision").await;
    old.move_focus(true);
    let id = old.action_id(
        InteractionTarget::Item {
            id: "message".into(),
        },
        &PanelAction {
            request_activation_token: false,
            key: "opaque-link".into(),
            label: "Read the page".into(),
            value: Some("old-revision".into()),
        },
    );
    assert_eq!(old.focused_action_id(), Some(id.as_str()));
    let viewport = Rectangle::with_size(Size::new(360.0, 600.0));
    let mut tree = Tree::empty();
    let point = {
        let mut view = render(
            &old,
            "notifications",
            false,
            crate::theme::ui(canopa_theme::UiKind::Panel),
        );
        tree.diff(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let key = semantic_key(&old, "notifications", &SemanticNode::Action(id.clone()));
        let mut measure = crate::app::canopa_accessibility_bounds::Measure::new(
            std::iter::once(key.clone()),
            Default::default(),
        );
        view.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut iced::advanced::widget::operation::black_box(&mut measure),
        );
        let bounds = measure.bounds[&key];
        assert!(bounds.height >= 16.0 && viewport.contains(bounds.center()));
        bounds.center()
    };
    assert!(
        pointer_event(
            &old,
            &renderer,
            &mut tree,
            viewport,
            point,
            iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)
        )
        .is_empty()
    );
    let events = pointer_event(
        &old,
        &renderer,
        &mut tree,
        viewport,
        point,
        iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
    );
    assert!(
        matches!(events.as_slice(), [Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { element_id, .. })] if element_id == &id)
    );
    let mut written = Vec::new();
    assert!(session.write_interaction(&mut written, &id).await.unwrap());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&written).unwrap(),
        serde_json::json!({"type":"invoke", "target":{"target":"item","id":"message"}, "action_key":"opaque-link", "value":"old-revision"})
    );
    assert!(
        pointer_event(
            &old,
            &renderer,
            &mut tree,
            viewport,
            point,
            iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)
        )
        .is_empty()
    );
    let mut next = link_snapshot(&session, "new-revision").await;
    next.move_focus(true);
    assert!(
        pointer_event(
            &next,
            &renderer,
            &mut tree,
            viewport,
            point,
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)
        )
        .is_empty()
    );
    assert!(!session.write_interaction(&mut written, &id).await.unwrap());
}

#[tokio::test]
async fn focused_slider_is_revealed_in_a_short_panel() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let content = serde_json::from_value(serde_json::json!({
        "title":"Levels", "actions":[], "error":null, "sections":[{
            "id":"s", "title":"Devices", "controls":[], "items":(0..12).map(|index| serde_json::json!({
                "id":index.to_string(), "title":format!("Device {index}"), "body":null,
                "metadata":[], "actions":[], "slider":{"key":"volume", "label":"Volume", "min":0,"max":100,"value":40,"step":1,"unit":"%"}
            })).collect::<Vec<_>>()
        }]
    })).unwrap();
    let mut snapshot = CanopaSession::default().prepare(content).await.unwrap();
    snapshot.move_focus(false);
    let id = snapshot.focused_action_id().unwrap().to_owned();
    let key = semantic_key(&snapshot, "levels", &SemanticNode::Action(id));
    let mut view = render(
        &snapshot,
        "levels",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    let mut widgets = Tree::new(view.as_widget());
    let viewport = Rectangle::with_size(Size::new(360.0, 300.0));
    let layout = view.as_widget_mut().layout(
        &mut widgets,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let measure = |view: &mut Element<'_, Message>, widgets: &mut Tree| {
        let mut measure = super::super::canopa_accessibility_bounds::Measure::new(
            std::iter::once(key.clone()),
            Default::default(),
        );
        view.as_widget_mut().operate(
            widgets,
            Layout::new(&layout),
            &renderer,
            &mut iced::advanced::widget::operation::black_box(&mut measure),
        );
        measure.bounds[&key]
    };
    assert!(measure(&mut view, &mut widgets).y > viewport.height);
    view.as_widget_mut().update(
        &mut widgets,
        &iced::Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        )),
        Layout::new(&layout),
        iced::mouse::Cursor::Unavailable,
        &renderer,
        &mut iced::advanced::clipboard::Null,
        &mut iced::advanced::Shell::new(&mut Vec::new()),
        &viewport,
    );
    let bounds = measure(&mut view, &mut widgets);
    assert!(
        bounds.y >= 0.0 && bounds.y + bounds.height <= viewport.height,
        "focused slider was not visible: {bounds:?}"
    );
    // Reading elsewhere after focusing a slider must survive a live replacement.
    use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, scroll_by};
    view.as_widget_mut().operate(
        &mut widgets,
        Layout::new(&layout),
        &renderer,
        &mut scroll_by(
            iced::widget::Id::from("canopa-reading/levels"),
            AbsoluteOffset {
                x: 0.0,
                y: -100_000.0,
            },
        ),
    );
    let mut replacement = CanopaSession::default()
        .prepare(snapshot.content.clone())
        .await
        .unwrap();
    replacement.preserve_focus_from(&snapshot);
    assert_ne!(
        snapshot.focused_action_id(),
        replacement.focused_action_id()
    );
    let mut view = render(
        &replacement,
        "levels",
        false,
        crate::theme::ui(canopa_theme::UiKind::Panel),
    );
    widgets.diff(view.as_widget());
    let layout = view.as_widget_mut().layout(
        &mut widgets,
        &renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    view.as_widget_mut().update(
        &mut widgets,
        &iced::Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        )),
        Layout::new(&layout),
        iced::mouse::Cursor::Unavailable,
        &renderer,
        &mut iced::advanced::clipboard::Null,
        &mut iced::advanced::Shell::new(&mut Vec::new()),
        &viewport,
    );
    let mut scroll = ScrollBounds::default();
    view.as_widget_mut()
        .operate(&mut widgets, Layout::new(&layout), &renderer, &mut scroll);
    assert_eq!(
        scroll.0.last().unwrap().1.y,
        0.0,
        "replacement must not scroll back to preserved focus"
    );
}
