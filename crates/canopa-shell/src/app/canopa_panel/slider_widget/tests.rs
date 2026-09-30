use super::*;
use iced::advanced::renderer::Headless;

fn spec() -> PanelSlider {
    PanelSlider {
        key: "level".into(),
        label: "Level".into(),
        min: 0,
        max: 100,
        step: 1,
        value: 40,
        unit: "%".into(),
    }
}
fn id(generation: u64) -> String {
    serde_json::to_string(&(
        generation,
        canopa_panel_protocol::InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: "device".into(),
            },
            action_key: "level".into(),
            value: None,
        },
    ))
    .unwrap()
}
fn event(
    tree: &mut Tree,
    renderer: &Renderer,
    generation: u64,
    input: Event,
    point: iced::Point,
) -> Vec<Message> {
    let mut view = render(id(generation), "levels", &spec(), true);
    tree.diff(view.as_widget());
    let viewport = Rectangle::with_size(Size::new(320.0, 90.0));
    let layout = view.as_widget_mut().layout(
        tree,
        renderer,
        &layout::Limits::new(Size::ZERO, viewport.size()),
    );
    let mut messages = Vec::new();
    view.as_widget_mut().update(
        tree,
        &input,
        Layout::new(&layout),
        mouse::Cursor::Available(point),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut Shell::new(&mut messages),
        &viewport,
    );
    messages
}
fn values(messages: &[Message]) -> Vec<u16> {
    messages
        .iter()
        .filter_map(|message| {
            let Message::ExternalPanel(ExternalPanelEvent::ButtonActivated { element_id, .. }) =
                message
            else {
                return None;
            };
            let (_, canopa_panel_protocol::InteractionEvent::Invoke { value, .. }): (
                u64,
                canopa_panel_protocol::InteractionEvent,
            ) = serde_json::from_str(element_id).unwrap();
            Some(value.unwrap().parse().unwrap())
        })
        .collect()
}
#[tokio::test]
async fn slider_drag_commits_the_final_value_once_and_replacement_cancels_it() {
    let renderer = Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let mut tree = Tree::empty();
    let start = iced::Point::new(100.0, 40.0);
    let end = iced::Point::new(400.0, 40.0);
    let down = event(
        &mut tree,
        &renderer,
        1,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    assert!(values(&down).is_empty());
    assert!(matches!(
        down.as_slice(),
        [Message::CanopaAccessibilityFocus { .. }]
    ));
    assert!(
        values(&event(
            &mut tree,
            &renderer,
            1,
            Event::Mouse(mouse::Event::CursorMoved { position: end }),
            end
        ))
        .is_empty()
    );
    let released = event(
        &mut tree,
        &renderer,
        1,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        end,
    );
    assert_eq!(values(&released), [100]);
    assert!(
        values(&event(
            &mut tree,
            &renderer,
            1,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            end
        ))
        .is_empty()
    );
    event(
        &mut tree,
        &renderer,
        1,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    assert!(
        values(&event(
            &mut tree,
            &renderer,
            2,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            end
        ))
        .is_empty()
    );
}
#[tokio::test]
async fn slider_keyboard_repeats_preview_then_commit_and_home_end_reach_bounds() {
    use iced::keyboard::{self, Key, key::Named};
    let renderer = Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let mut tree = Tree::empty();
    let key = |named, pressed| {
        let key = Key::Named(named);
        if pressed {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: key.clone(),
                modified_key: key,
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::ArrowRight),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: None,
                repeat: false,
            })
        } else {
            Event::Keyboard(keyboard::Event::KeyReleased {
                key: key.clone(),
                modified_key: key,
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::ArrowRight),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
            })
        }
    };
    // The pointer is outside the control: keyboard focus, not hover, owns these keys.
    let point = iced::Point::new(900.0, 900.0);
    for _ in 0..2 {
        assert!(
            values(&event(
                &mut tree,
                &renderer,
                1,
                key(Named::ArrowRight, true),
                point
            ))
            .is_empty()
        );
    }
    assert_eq!(
        values(&event(
            &mut tree,
            &renderer,
            1,
            key(Named::ArrowRight, false),
            point
        )),
        [42]
    );
    for (named, value) in [(Named::Home, 0), (Named::End, 100)] {
        event(&mut tree, &renderer, 1, key(named, true), point);
        assert_eq!(
            values(&event(&mut tree, &renderer, 1, key(named, false), point)),
            [value]
        );
    }
}
