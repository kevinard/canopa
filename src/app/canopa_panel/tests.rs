use super::*;
use iced::advanced::{Layout, layout, renderer::Headless, widget::Operation, widget::Tree};
use iced::{Rectangle, Size};
use niribar::external_panel::canopa::CanopaSession;

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
    let mut view = render(snapshot, "notifications");
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
            iced::Point::new(40.0, 55.0)
        } else {
            let mut view = render(&old, "notifications");
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

async fn long_title_snapshot() -> CanopaSnapshot {
    let content = serde_json::json!({
        "type": "replace_content", "protocol_version": 3,
        "content": {
            "title": "Notifications", "error": null,
            "actions": [{"key": "refresh", "label": "Refresh", "value": null}],
            "sections": [{"id": "mail", "title": "Mail", "controls": [],
                "items": [{"id": "message", "title": "Long contextual title ".repeat(50),
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
    let cursor = iced::Point::new(40.0, 55.0);
    let mut tree = Tree::empty();
    {
        let mut view = render(&snapshot, "notifications");
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
    let mut view = render(&snapshot, "notifications");
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
    let mut view = render(&snapshot, "notifications");
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
