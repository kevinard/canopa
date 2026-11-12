use super::*;
use core::renderer::Headless;
use core::widget::{Operation, Tree};
use futures_util::StreamExt;
use iced_layershell::actions::{LayerShellCustomAction, LayerShellCustomActionWithId};
use iced_layershell::reexport::core::{self, Layout, Rectangle, Size};

async fn software_renderer() -> iced::Renderer {
    iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer")
}

#[tokio::test]
async fn body_link_click_is_separate_from_the_card_default_action() {
    let renderer = software_renderer().await;
    let mut banner = banner_view(42, "Read the page");
    banner.default_action_key = Some("default".into());
    banner.body_content = crate::intake::interpreted_body_content(
        "<a href='https://example.org/page'><b>Read the page</b></a>",
    );
    banner.body_segments = banner_body_segments_from_body_content(&banner.body_content);
    for width in [360.0, 276.0] {
        let mut card = notification_banner_widget(&banner, &Default::default());
        let mut tree = Tree::new(card.as_widget());
        let node = card.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, Size::new(width, 1000.0)),
        );
        let mut measure = accessibility_bounds::Measure::new(
            std::iter::once((banner.target(), 1)),
            Default::default(),
        );
        card.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut core::widget::operation::black_box(&mut measure),
        );
        let point = measure.bounds[&(banner.target(), 1)].center();
        let mut messages = Vec::new();
        for event in [
            core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
            core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
        ] {
            card.as_widget_mut().update(
                &mut tree,
                &core::Event::Mouse(event),
                Layout::new(&node),
                core::mouse::Cursor::Available(point),
                &renderer,
                &mut core::clipboard::Null,
                &mut core::Shell::new(&mut messages),
                &Rectangle::with_size(node.size()),
            );
        }
        assert_eq!(messages.len(), 1, "one deliberate link interaction");
        assert!(
            matches!(&messages[0], BannerUiMessage::OpenLinkRequested(target, 0) if *target == banner.target()),
            "wrong link control: {messages:?}"
        );
    }
}

#[tokio::test]
async fn every_named_action_has_a_readable_row_and_invokes_its_opaque_key() {
    let renderer = software_renderer().await;
    let mut banner = banner_view(42, "Mail");
    banner.actions = ["Reply", "Archive", "Mark read", "Forward"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| BannerAction {
            key: format!("opaque/{index}/α"),
            label: label.into(),
            icon: None,
        })
        .collect();
    for width in [360.0, 276.0] {
        let mut card = notification_banner_widget(&banner, &Default::default());
        let mut tree = Tree::new(card.as_widget());
        let node = card.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, Size::new(width, 1000.0)),
        );
        let mut labels = TextBounds::default();
        card.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        let bounds: Vec<_> = banner
            .actions
            .iter()
            .map(|action| {
                labels
                    .0
                    .iter()
                    .find(|(label, _)| label == &action.label)
                    .unwrap()
                    .1
            })
            .collect();
        assert!(
            bounds[2].y >= bounds[0].y + bounds[0].height,
            "additional actions need another readable row at width {width}: {bounds:?}"
        );
        for (index, bounds) in bounds.into_iter().enumerate() {
            let mut messages = Vec::new();
            for event in [
                core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
                core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
            ] {
                card.as_widget_mut().update(
                    &mut tree,
                    &core::Event::Mouse(event),
                    Layout::new(&node),
                    core::mouse::Cursor::Available(bounds.center()),
                    &renderer,
                    &mut core::clipboard::Null,
                    &mut core::Shell::new(&mut messages),
                    &Rectangle::with_size(node.size()),
                );
            }
            assert!(
                matches!(messages.as_slice(),
                [BannerUiMessage::InvokeActionRequested(target, key)]
                if *target == banner.target() && key == &format!("opaque/{index}/α")),
                "wrong action at width {width}: {messages:?}"
            );
        }
    }
}

#[tokio::test]
async fn pointer_press_cannot_activate_replacement_content() {
    let renderer = software_renderer().await;
    let mut banner = banner_view(42, "Mail");
    banner.actions = vec![BannerAction {
        key: "old".into(),
        label: "Reply".into(),
        icon: None,
    }];
    let limits = core::layout::Limits::new(Size::ZERO, Size::new(360.0, 1000.0));
    let mut tree = Tree::empty();
    let mut messages = Vec::new();
    for (revision, event) in [
        (
            0,
            core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
        ),
        (
            1,
            core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
        ),
    ] {
        banner.revision = revision;
        banner.actions[0].key = format!("revision-{revision}");
        let mut card = notification_banner_widget(&banner, &Default::default());
        tree.diff(card.as_widget());
        let node = card.as_widget_mut().layout(&mut tree, &renderer, &limits);
        let mut labels = TextBounds::default();
        card.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        let point = labels
            .0
            .iter()
            .find(|(label, _)| label == "Reply")
            .unwrap()
            .1
            .center();
        card.as_widget_mut().update(
            &mut tree,
            &core::Event::Mouse(event),
            Layout::new(&node),
            core::mouse::Cursor::Available(point),
            &renderer,
            &mut core::clipboard::Null,
            &mut core::Shell::new(&mut messages),
            &Rectangle::with_size(node.size()),
        );
    }
    assert!(
        messages.is_empty(),
        "a press on an old revision activated the replacement: {messages:?}"
    );
}

async fn frame_actions(
    state: &mut BannerUiState,
    renderer: &iced::Renderer,
    viewport: Size,
    tree: &mut Tree,
) -> Vec<LayerShellCustomAction> {
    let mut actions = Vec::new();
    for message in frame_messages_in_tree(state, renderer, viewport, tree) {
        if let Some(mut stream) = iced_runtime::task::into_stream(banner_ui_update(state, message))
        {
            while let Some(action) = stream.next().await {
                let iced_runtime::Action::Output(message) = action else {
                    panic!("unexpected frame task");
                };
                let LayerShellCustomActionWithId(_, action) =
                    message.try_into().expect("layer-shell frame action");
                actions.push(action);
            }
        }
    }
    actions
}

#[tokio::test]
async fn measured_content_updates_pointer_input_instead_of_resizing_the_surface() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    queue_banner_ui_presentation(&mut state, 42, Arc::new(AtomicBool::new(false)));
    let mut tree = Tree::empty();
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 216.0), &mut tree).await;
    assert!(
        matches!(
            actions.as_slice(),
            [LayerShellCustomAction::SetInputRegion(_)]
        ),
        "measured content must not override compositor-assigned dimensions: {actions:?}"
    );
}

// The natural height of the fixture card that `queue_banner_ui_presentation`
// shows, measured rather than hard-coded so theme spacing changes do not stale
// the region assertions.
fn queued_card_height(renderer: &iced::Renderer, width: f32) -> i32 {
    let banner = banner_view(42, "Queued");
    let mut card = notification_banner_widget(&banner, &Default::default());
    let mut tree = Tree::new(card.as_widget());
    let node = card.as_widget_mut().layout(
        &mut tree,
        renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(width, 1000.0)),
    );
    node.size().height.ceil() as i32
}

// An external wire peer records actual wl_region requests. No compositor or
// private geometry helper is mocked; object setup uses ordinary client proxies.
fn region_requests(callback: &iced_layershell::actions::ActionCallback) -> Vec<(u16, Vec<i32>)> {
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use wayland_client::{
        Connection, Proxy,
        protocol::{wl_compositor, wl_region, wl_registry},
    };
    struct RegionClient;
    wayland_client::delegate_noop!(RegionClient: ignore wl_registry::WlRegistry);
    wayland_client::delegate_noop!(RegionClient: ignore wl_compositor::WlCompositor);
    wayland_client::delegate_noop!(RegionClient: ignore wl_region::WlRegion);
    let (client, mut peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    let connection = Connection::from_socket(client).unwrap();
    let queue = connection.new_event_queue::<RegionClient>();
    let handle = queue.handle();
    let registry = connection.display().get_registry(&handle, ());
    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(1, 4, &handle, ());
    let region = compositor.create_region(&handle, ());
    (callback.0)(&region);
    connection.flush().unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        match peer.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("read region requests: {error}"),
        }
    }
    let mut requests = Vec::new();
    let mut remaining = bytes.as_slice();
    while !remaining.is_empty() {
        let object = u32::from_ne_bytes(remaining[..4].try_into().unwrap());
        let header = u32::from_ne_bytes(remaining[4..8].try_into().unwrap());
        let length = (header >> 16) as usize;
        assert!(length >= 8 && length <= remaining.len());
        if object == region.id().protocol_id() {
            requests.push((
                header as u16,
                remaining[8..length]
                    .chunks_exact(4)
                    .map(|word| i32::from_ne_bytes(word.try_into().unwrap()))
                    .collect(),
            ));
        }
        remaining = &remaining[length..];
    }
    requests
}

#[tokio::test]
async fn bottom_start_banners_take_input_only_in_their_corner() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut tree = Tree::empty();
    let _task = banner_ui_update(
        &mut state,
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation {
                position: crate::BannerPosition::BottomStart,
                ..BannerPresentation::visible_banners(vec![banner_view(42, "Queued")])
            },
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    );
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 600.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    let height = queued_card_height(&renderer, 360.0);
    assert_eq!(
        region_requests(region),
        vec![
            (2, vec![0, 0, i32::MAX, i32::MAX]),
            (1, vec![0, 600 - height, 360, height])
        ]
    );
}

#[tokio::test]
async fn queued_input_regions_use_the_latest_viewport_and_remain_empty_after_clear() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut tree = Tree::empty();
    queue_banner_ui_presentation(&mut state, 42, Arc::new(AtomicBool::new(false)));
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 216.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(old)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    let reset = (2, vec![0, 0, i32::MAX, i32::MAX]);
    assert_eq!(
        region_requests(old),
        vec![
            reset.clone(),
            (1, vec![616, 0, 360, queued_card_height(&renderer, 360.0)])
        ]
    );
    let resized = frame_actions(&mut state, &renderer, Size::new(276.0, 216.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(current)] = resized.as_slice() else {
        panic!("{resized:?}");
    };
    let narrow = vec![
        reset.clone(),
        (1, vec![0, 0, 276, queued_card_height(&renderer, 276.0)]),
    ];
    assert_eq!(region_requests(current), narrow);
    assert_eq!(
        region_requests(old),
        narrow,
        "a delayed command must not restore the old region"
    );
    let _ = banner_ui_update(
        &mut state,
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation::default(),
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    );
    let cleared = frame_actions(&mut state, &renderer, Size::new(276.0, 216.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(empty)] = cleared.as_slice() else {
        panic!("{cleared:?}");
    };
    for callback in [old, current, empty] {
        assert_eq!(
            region_requests(callback),
            vec![reset.clone()],
            "cleared banners must remain click-through"
        );
    }
}

#[tokio::test]
async fn compositor_viewport_keeps_cards_width_capped_and_top_right() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    state.presentation = top_end(BannerPresentation::visible_banners(vec![banner_view(
        42, "Fits",
    )]));
    for (viewport, expected_x, expected_width) in [
        (Size::new(976.0, 216.0), 616.0, 360.0),
        (Size::new(276.0, 416.0), 0.0, 276.0),
    ] {
        let frames = frame_messages(&state, &renderer, viewport);
        let measured = frames
            .iter()
            .find_map(|message| match message {
                BannerUiMessage::LayoutFrame { size, .. } => Some(*size),
                _ => None,
            })
            .expect("measured content");
        assert_eq!(
            measured.width, expected_width,
            "cards must fit narrow outputs without stretching on wide ones"
        );
        let mut view = banner_ui_view(&state, state.surface.window().unwrap());
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, viewport),
        );
        let mut scroll = ScrollBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
        assert_eq!(
            scroll.0[0].0,
            Rectangle {
                x: expected_x,
                y: 0.0,
                width: expected_width,
                height: viewport.height
            }
        );
    }
}

#[tokio::test]
async fn fully_visible_card_is_acknowledged_while_a_lower_card_is_clipped() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    state.presentation = top_end(BannerPresentation::visible_banners(vec![
        banner_view(1, "Upper"),
        banner_view(2, "Lower"),
    ]));
    let targets: Vec<_> = state
        .presentation
        .banners
        .iter()
        .map(BannerView::target)
        .collect();
    let displayed = Arc::new(Mutex::new(Vec::new()));
    let observed = displayed.clone();
    state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
        generation: 0,
        acknowledgement: BannerPresentationAcknowledgement::for_banners(
            targets.clone(),
            move |target| {
                observed.lock().unwrap().push(target);
            },
        ),
    });
    let mut tree = Tree::empty();
    for message in frame_messages_in_tree(&state, &renderer, Size::new(360.0, 120.0), &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        vec![targets[0]],
        "the upper card must not wait for the clipped lower card"
    );
    assert!(state.pending_acknowledgement.is_some());
    for message in frame_messages_in_tree(&state, &renderer, Size::new(360.0, 320.0), &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        targets,
        "each card reports first display once"
    );
    assert!(state.pending_acknowledgement.is_none());
}

#[derive(Default, Debug)]
struct TextBounds(Vec<(String, Rectangle)>);

#[tokio::test]
async fn queued_card_frames_cannot_acknowledge_replacement_content() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let displayed = Arc::new(Mutex::new(Vec::new()));
    let mut tree = Tree::empty();
    let viewport = Size::new(360.0, 240.0);
    let mut frames = Vec::new();
    for revision in [1, 2] {
        let mut banner = banner_view(42, "Replaced");
        banner.revision = revision;
        let observed = displayed.clone();
        let acknowledgement =
            BannerPresentationAcknowledgement::for_banners([banner.target()], move |target| {
                observed.lock().unwrap().push(target)
            });
        let _ = banner_ui_update(
            &mut state,
            BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
                presentation: top_end(BannerPresentation::visible_banners(vec![banner])),
                acknowledgement,
            }),
        );
        frames.push(frame_messages_in_tree(
            &state, &renderer, viewport, &mut tree,
        ));
    }
    for message in frames.remove(0) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        vec![BannerTarget {
            notification_id: InternalNotificationId::new(42).unwrap(),
            revision: 1
        }]
    );
    assert!(
        state.pending_acknowledgement.is_some(),
        "the new frame remains pending"
    );
    for message in frames.remove(0) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        vec![
            BannerTarget {
                notification_id: InternalNotificationId::new(42).unwrap(),
                revision: 1
            },
            BannerTarget {
                notification_id: InternalNotificationId::new(42).unwrap(),
                revision: 2
            },
        ]
    );
    assert!(state.pending_acknowledgement.is_none());
}

#[tokio::test]
async fn display_observer_preserves_action_clicks_across_view_rebuilds() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut banner = banner_view(42, "Action card");
    banner.actions = vec![BannerAction {
        key: "reply".into(),
        label: "Reply".into(),
        icon: None,
    }];
    let target = banner.target();
    state.presentation = top_end(BannerPresentation::visible_banners(vec![banner]));
    state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
        generation: 0,
        acknowledgement: BannerPresentationAcknowledgement::for_banners([target], |_| {}),
    });
    let viewport = Size::new(360.0, 240.0);
    let mut tree = Tree::empty();
    let position = {
        let mut view = banner_ui_view(&state, state.surface.window().unwrap());
        tree.diff(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, viewport),
        );
        let mut labels = TextBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        labels
            .0
            .iter()
            .find(|(text, _)| text == "Reply")
            .unwrap()
            .1
            .center()
    };
    assert!(
        event_messages_in_tree(
            &state,
            &renderer,
            viewport,
            &mut tree,
            core::Event::Mouse(core::mouse::Event::ButtonPressed(core::mouse::Button::Left)),
            core::mouse::Cursor::Available(position)
        )
        .is_empty()
    );
    // Consume the first display between press and release. Its removal must not
    // discard the button's pressed state on the next view rebuild.
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(state.pending_acknowledgement.is_none());
    let messages = event_messages_in_tree(
        &state,
        &renderer,
        viewport,
        &mut tree,
        core::Event::Mouse(core::mouse::Event::ButtonReleased(
            core::mouse::Button::Left,
        )),
        core::mouse::Cursor::Available(position),
    );
    assert!(
        matches!(messages.as_slice(), [BannerUiMessage::InvokeActionRequested(actual, key)]
        if *actual == target && key == "reply"),
        "wrong action hit result: {messages:?}"
    );
}

impl Operation for TextBounds {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn text(&mut self, _: Option<&core::widget::Id>, bounds: Rectangle, text: &str) {
        self.0.push((text.to_owned(), bounds));
    }
}

#[derive(Default)]
struct ScrollBounds(Vec<(Rectangle, Rectangle, core::Vector)>);

fn scroll_bounds_in_tree(
    state: &BannerUiState,
    renderer: &iced::Renderer,
    viewport: Size,
    tree: &mut Tree,
) -> ScrollBounds {
    let mut view = banner_ui_view(state, state.surface.window().unwrap());
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        tree,
        renderer,
        &core::layout::Limits::new(Size::ZERO, viewport),
    );
    let mut scroll = ScrollBounds::default();
    view.as_widget_mut()
        .operate(tree, Layout::new(&node), renderer, &mut scroll);
    scroll
}

#[tokio::test]
async fn ordinary_stack_wheel_steps_acknowledge_an_oversized_middle_card() {
    let renderer = software_renderer().await;
    for height in [150.0, 216.0, 426.0] {
        let mut state = banner_ui_test_state();
        let banners: Vec<_> = (1..=3)
            .map(|id| {
                let mut banner = banner_view(id, "Body");
                banner.summary = "Summary line\n".repeat(30);
                banner
            })
            .collect();
        let targets: Vec<_> = banners.iter().map(BannerView::target).collect();
        state.presentation = top_end(BannerPresentation::visible_banners(banners));
        let displayed = Arc::new(Mutex::new(Vec::new()));
        let observed = displayed.clone();
        state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
            generation: 0,
            acknowledgement: BannerPresentationAcknowledgement::for_banners(
                targets.clone(),
                move |target| {
                    observed.lock().unwrap().push(target);
                },
            ),
        });
        let viewport = Size::new(360.0, height);
        let mut tree = Tree::empty();
        for step in 0..=20 {
            if step > 0 {
                let _ = event_messages_in_tree(
                    &state,
                    &renderer,
                    viewport,
                    &mut tree,
                    core::Event::Mouse(core::mouse::Event::WheelScrolled {
                        delta: core::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
                    }),
                    core::mouse::Cursor::Available(core::Point::new(355.0, height / 2.0)),
                );
            }
            for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
                let _ = banner_ui_update(&mut state, message);
            }
        }
        assert_eq!(
            *displayed.lock().unwrap(),
            targets,
            "ordinary wheel traversal must fully reveal the middle card at height {height}"
        );
    }
}

#[tokio::test]
async fn oversized_card_and_stack_have_independently_reachable_scrollbars() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut oversized = banner_view(1, "Body");
    oversized.summary = "A summary line\n".repeat(12);
    oversized.actions = vec![BannerAction {
        key: "opaque/action".into(),
        label: "Last action".into(),
        icon: None,
    }];
    state.presentation = top_end(BannerPresentation::visible_banners(vec![
        oversized,
        banner_view(2, "Lower"),
    ]));
    let targets: Vec<_> = state
        .presentation
        .banners
        .iter()
        .map(BannerView::target)
        .collect();
    let displayed = Arc::new(Mutex::new(Vec::new()));
    let observed = displayed.clone();
    state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
        generation: 0,
        acknowledgement: BannerPresentationAcknowledgement::for_banners(
            targets.clone(),
            move |target| {
                observed.lock().unwrap().push(target);
            },
        ),
    });
    let viewport = Size::new(360.0, 150.0);
    let mut tree = Tree::empty();
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(*displayed.lock().unwrap(), vec![targets[0]]);
    // The card rail sits immediately left of a distinct outer-stack gutter.
    for event in [
        core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
        core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
    ] {
        assert!(
            event_messages_in_tree(
                &state,
                &renderer,
                viewport,
                &mut tree,
                core::Event::Mouse(event),
                core::mouse::Cursor::Available(core::Point::new(343.0, 87.0))
            )
            .is_empty()
        );
    }
    let scrolled = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    assert_eq!(
        scrolled.0[0].2.y, 0.0,
        "card scrolling must not move the stack"
    );
    assert!(
        scrolled.0[1].2.y > 100.0,
        "card scrollbar must be reachable: {:?}",
        scrolled.0
    );
    // The separate outer rail still reaches the lower card without scrolling
    // through all the first card's contents or waiting for wheel transactions.
    for event in [
        core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
        core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
    ] {
        assert!(
            event_messages_in_tree(
                &state,
                &renderer,
                viewport,
                &mut tree,
                core::Event::Mouse(event),
                core::mouse::Cursor::Available(core::Point::new(355.0, 147.0))
            )
            .is_empty()
        );
    }
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        targets,
        "only full bounded card viewports count as first display"
    );
    let after = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    assert!(after.0[0].2.y > 0.0);
    assert_eq!(
        after.0[1].2.y, scrolled.0[1].2.y,
        "stack navigation preserves internal scroll position"
    );
}

impl Operation for ScrollBounds {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
    fn scrollable(
        &mut self,
        _: Option<&core::widget::Id>,
        bounds: Rectangle,
        content: Rectangle,
        translation: core::Vector,
        _: &mut dyn core::widget::operation::Scrollable,
    ) {
        self.0.push((bounds, content, translation));
    }
}

#[tokio::test]
async fn oversized_card_wheel_scrolling_preserves_body_navigation_and_action_clicks() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut banner = banner_view(42, "Multiple images");
    banner.summary = "Summary line\n".repeat(10);
    banner.body_segments = (0..6)
        .map(|_| BannerBodySegment::Image {
            image: test_banner_visual(),
        })
        .collect();
    banner.actions = vec![BannerAction {
        key: "opaque key/α".into(),
        label: "Review changes\nand open report\nin the editor".into(),
        icon: Some(test_banner_visual()),
    }];
    let target = banner.target();
    state.presentation = top_end(BannerPresentation::visible_banners(vec![banner]));
    state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
        generation: 0,
        acknowledgement: BannerPresentationAcknowledgement::for_banners([target], |_| {}),
    });
    let viewport = Size::new(276.0, 150.0);
    let mut tree = Tree::empty();
    let wheel = |y| {
        core::Event::Mouse(core::mouse::Event::WheelScrolled {
            delta: core::mouse::ScrollDelta::Pixels { x: 0.0, y },
        })
    };
    // Scroll the card from its left padding to reveal the body's own viewport.
    assert!(
        event_messages_in_tree(
            &state,
            &renderer,
            viewport,
            &mut tree,
            wheel(-230.0),
            core::mouse::Cursor::Available(core::Point::new(5.0, 75.0))
        )
        .is_empty()
    );
    let before = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    let (body, content, _) = before.0[2];
    assert!(content.height > body.height * 2.0);
    let body_cursor = core::Point::new(body.center_x(), body.y - before.0[1].2.y + 20.0);
    assert!(Rectangle::with_size(viewport).contains(body_cursor));
    // A fresh pointer gesture ends Iced's wheel transaction, without sleeps or
    // poking private scroll state. Wheel input can now target the body preview.
    let _ = event_messages_in_tree(
        &state,
        &renderer,
        viewport,
        &mut tree,
        core::Event::Mouse(core::mouse::Event::CursorLeft),
        core::mouse::Cursor::Unavailable,
    );
    assert!(
        event_messages_in_tree(
            &state,
            &renderer,
            viewport,
            &mut tree,
            wheel(-1000.0),
            core::mouse::Cursor::Available(body_cursor)
        )
        .is_empty()
    );
    let body_scrolled = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    assert!(
        body_scrolled.0[2].2.y > 0.0,
        "remaining body images are reachable"
    );
    assert_eq!(
        body_scrolled.0[1].2, before.0[1].2,
        "body scrolling must not move the card"
    );
    let _ = event_messages_in_tree(
        &state,
        &renderer,
        viewport,
        &mut tree,
        core::Event::Mouse(core::mouse::Event::CursorLeft),
        core::mouse::Cursor::Unavailable,
    );
    let _ = event_messages_in_tree(
        &state,
        &renderer,
        viewport,
        &mut tree,
        wheel(-1000.0),
        core::mouse::Cursor::Available(core::Point::new(5.0, 75.0)),
    );
    let scrolled = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    let point = {
        let mut view = banner_ui_view(&state, state.surface.window().unwrap());
        tree.diff(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, viewport),
        );
        let mut labels = TextBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        let action = labels
            .0
            .iter()
            .find(|(text, _)| text.contains("in the editor"))
            .unwrap()
            .1;
        let point = core::Point::new(
            action.center_x(),
            action.y + action.height - scrolled.0[1].2.y - 5.0,
        );
        let mut geometry = crate::presentation::renderer::accessibility_bounds::Measure::new(
            (0..5).map(|slot| (target, slot)),
            Default::default(),
        );
        view.as_widget_mut().operate(
            &mut tree,
            Layout::new(&node),
            &renderer,
            &mut core::widget::operation::black_box(&mut geometry),
        );
        assert!(
            geometry.bounds[&(target, 4)].contains(point),
            "accessible bounds must follow the actual scrolled action hit region"
        );
        assert!(
            geometry.bounds[&(target, 0)].is_within(&Rectangle::with_size(viewport)),
            "accessible card clips its oversized descendants"
        );
        point
    };
    assert!(
        Rectangle::with_size(viewport).contains(point),
        "last action line is visible: {point:?}"
    );
    assert!(
        event_messages_in_tree(
            &state,
            &renderer,
            viewport,
            &mut tree,
            core::Event::Mouse(core::mouse::Event::ButtonPressed(core::mouse::Button::Left)),
            core::mouse::Cursor::Available(point)
        )
        .is_empty()
    );
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(state.pending_acknowledgement.is_none());
    let messages = event_messages_in_tree(
        &state,
        &renderer,
        viewport,
        &mut tree,
        core::Event::Mouse(core::mouse::Event::ButtonReleased(
            core::mouse::Button::Left,
        )),
        core::mouse::Cursor::Available(point),
    );
    assert!(
        matches!(messages.as_slice(), [BannerUiMessage::InvokeActionRequested(actual, key)]
        if *actual == target && key == "opaque key/α"),
        "last-line action hit: {messages:?}"
    );
    let after = scroll_bounds_in_tree(&state, &renderer, viewport, &mut tree);
    assert_eq!(
        after.0[1].2, scrolled.0[1].2,
        "acknowledgement preserves card scroll"
    );
    assert_eq!(
        after.0[2].2, body_scrolled.0[2].2,
        "acknowledgement preserves body scroll"
    );
}

#[tokio::test]
async fn scrolling_multiple_body_images_preserves_action_hit_regions() {
    let mut banner = banner_view(42, "Multiple images");
    banner.body_segments = (0..6)
        .map(|_| BannerBodySegment::Image {
            image: test_banner_visual(),
        })
        .collect();
    banner.actions = vec![BannerAction {
        key: "one".into(),
        label: "First action".into(),
        icon: None,
    }];
    let renderer = software_renderer().await;
    let mut card = notification_banner_widget(&banner, &Default::default());
    let mut tree = Tree::new(card.as_widget());
    let node = card.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(360.0, 1000.0)),
    );
    let mut scroll = ScrollBounds::default();
    card.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
    assert_eq!(scroll.0.len(), 1, "body owns its own scrollable preview");
    let (viewport, content, _) = scroll.0[0];
    assert!(viewport.height <= 160.0 && content.height > viewport.height * 2.0);
    let mut messages = Vec::new();
    card.as_widget_mut().update(
        &mut tree,
        &core::Event::Mouse(core::mouse::Event::WheelScrolled {
            delta: core::mouse::ScrollDelta::Lines { x: 0.0, y: -20.0 },
        }),
        Layout::new(&node),
        core::mouse::Cursor::Available(viewport.center()),
        &renderer,
        &mut core::clipboard::Null,
        &mut core::Shell::new(&mut messages),
        &Rectangle::with_size(node.size()),
    );
    let mut scrolled = ScrollBounds::default();
    card.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut scrolled);
    assert!(
        scrolled.0[0].2.y > 0.0,
        "wheel input must reveal remaining images"
    );
    let mut labels = TextBounds::default();
    card.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
    let action_bounds = labels
        .0
        .iter()
        .find(|(text, _)| text == "First action")
        .unwrap()
        .1;
    assert!(action_bounds.y >= viewport.y + viewport.height);
    for event in [
        core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
        core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
    ] {
        card.as_widget_mut().update(
            &mut tree,
            &core::Event::Mouse(event),
            Layout::new(&node),
            core::mouse::Cursor::Available(action_bounds.center()),
            &renderer,
            &mut core::clipboard::Null,
            &mut core::Shell::new(&mut messages),
            &Rectangle::with_size(node.size()),
        );
    }
    assert!(
        matches!(messages.as_slice(), [BannerUiMessage::InvokeActionRequested(target, key)]
        if *target == banner.target() && key == "one"),
        "wrong action hit result: {messages:?}"
    );
}

#[tokio::test]
async fn multiline_summary_keeps_every_line_above_the_body_and_activates_the_card() {
    let mut banner = banner_view(42, "Body follows the complete summary");
    banner.summary = "Download complete\nReview destination\nOpen document".into();
    let renderer = software_renderer().await;
    for width in [360.0, 276.0] {
        let mut card = notification_banner_widget(&banner, &Default::default());
        let mut tree = Tree::new(card.as_widget());
        let node = card.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, Size::new(width, 1000.0)),
        );
        let mut labels = TextBounds::default();
        card.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
        let summary = labels
            .0
            .iter()
            .find(|(text, _)| text == &banner.summary)
            .unwrap()
            .1;
        assert!(
            summary.height >= 45.0,
            "three summary lines are clipped: {summary:?}"
        );
        let mut body = ScrollBounds::default();
        card.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut body);
        assert!(body.0[0].0.y >= summary.y + summary.height);
        assert!(summary.x >= 0.0 && summary.x + summary.width <= width);
        // The final summary line belongs to the default activation hit region.
        let point = core::Point::new(summary.center_x(), summary.y + summary.height - 5.0);
        let mut messages = Vec::new();
        for event in [
            core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
            core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
        ] {
            card.as_widget_mut().update(
                &mut tree,
                &core::Event::Mouse(event),
                Layout::new(&node),
                core::mouse::Cursor::Available(point),
                &renderer,
                &mut core::clipboard::Null,
                &mut core::Shell::new(&mut messages),
                &Rectangle::with_size(node.size()),
            );
        }
        assert!(
            matches!(messages.as_slice(), [BannerUiMessage::ActivateRequested(target)] if *target == banner.target())
        );
    }
}

#[tokio::test]
async fn long_named_action_labels_wrap_and_their_last_lines_remain_clickable() {
    let renderer = software_renderer().await;
    for label in [
        "Review changes\nand open report\nin the editor",
        "Review the downloaded document and open it in the selected application",
        "ReviewTheDownloadedDocumentAndOpenItInTheSelectedApplicationWithoutSpaces",
    ] {
        for width in [360.0, 276.0] {
            let mut banner = banner_view(42, "Body above actions");
            banner.progress = Some(42);
            banner.actions = vec![
                BannerAction {
                    key: "opaque key/α".into(),
                    label: label.into(),
                    icon: None,
                },
                BannerAction {
                    key: "other".into(),
                    label: label.into(),
                    icon: Some(test_banner_visual()),
                },
            ];
            let mut card = notification_banner_widget(&banner, &Default::default());
            let mut tree = Tree::new(card.as_widget());
            let node = card.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &core::layout::Limits::new(Size::ZERO, Size::new(width, 1000.0)),
            );
            let mut labels = TextBounds::default();
            card.as_widget_mut()
                .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
            let bounds: Vec<_> = labels
                .0
                .iter()
                .filter(|(text, _)| text == label)
                .map(|(_, bounds)| *bounds)
                .collect();
            assert_eq!(bounds.len(), 2);
            assert!(
                bounds[0].x + bounds[0].width <= bounds[1].x,
                "action labels overlap: {bounds:?}"
            );
            for (action, bounds) in banner.actions.iter().zip(bounds) {
                assert!(
                    bounds.height >= 36.0,
                    "long action is clipped at width {width}: {bounds:?}"
                );
                assert!(bounds.x >= 0.0 && bounds.x + bounds.width <= width);
                assert!(
                    bounds.y + bounds.height + 13.0 <= node.size().height,
                    "progress and padding must remain below actions"
                );
                let point = core::Point::new(bounds.center_x(), bounds.y + bounds.height - 4.0);
                let mut messages = Vec::new();
                for event in [
                    core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
                    core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
                ] {
                    card.as_widget_mut().update(
                        &mut tree,
                        &core::Event::Mouse(event),
                        Layout::new(&node),
                        core::mouse::Cursor::Available(point),
                        &renderer,
                        &mut core::clipboard::Null,
                        &mut core::Shell::new(&mut messages),
                        &Rectangle::with_size(node.size()),
                    );
                }
                assert!(
                    matches!(messages.as_slice(), [BannerUiMessage::InvokeActionRequested(target, key)] if *target == banner.target() && key == &action.key),
                    "wrong last-line action: {messages:?}"
                );
            }
        }
    }
}

#[tokio::test]
async fn rich_banner_keeps_named_action_labels_inside_the_card() {
    let mut banner = banner_view(42, "Before bold / After italic");
    banner.body_segments = vec![
        BannerBodySegment::plain_text("Before bold"),
        BannerBodySegment::Image {
            image: test_banner_visual(),
        },
        BannerBodySegment::plain_text("After italic"),
    ];
    banner.progress = Some(42);
    banner.actions = vec![
        BannerAction {
            key: "one".into(),
            label: "First action".into(),
            icon: None,
        },
        BannerAction {
            key: "two".into(),
            label: "Second action".into(),
            icon: None,
        },
    ];
    let renderer = software_renderer().await;
    let mut card = notification_banner_widget(&banner, &Default::default());
    let mut tree = Tree::new(card.as_widget());
    let node = card.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(360.0, 1000.0)),
    );
    let mut labels = TextBounds::default();
    card.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
    for label in ["First action", "Second action"] {
        let (_, bounds) = labels
            .0
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("missing label {label}: {labels:?}"));
        assert!(
            bounds.height >= 12.0
                && bounds.y >= 0.0
                && bounds.y + bounds.height <= node.size().height,
            "{label} is clipped: {bounds:?}, card size {:?}",
            node.size()
        );
    }
}

#[tokio::test]
async fn rich_body_makes_room_for_text_on_both_sides_of_an_image() {
    let mut banner = banner_view(42, "Before / After");
    banner.body_segments = vec![
        BannerBodySegment::plain_text("Before"),
        BannerBodySegment::Image {
            image: test_banner_visual(),
        },
        BannerBodySegment::plain_text("After"),
    ];
    let renderer = software_renderer().await;
    let mut body = notification_body_widget(&banner, &Default::default());
    let mut tree = Tree::new(body.as_widget());
    let node = body.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(258.0, 1000.0)),
    );
    // Two 13px text lines, a 48px image and their separation cannot fit in 70px.
    assert!(
        node.size().height >= 82.0,
        "body cannot fit text/image/text: {:?}",
        node.size()
    );
    assert!(
        node.size().height <= 160.0,
        "body preview must remain bounded"
    );
}

fn frame_messages(
    state: &BannerUiState,
    renderer: &iced::Renderer,
    viewport: Size,
) -> Vec<BannerUiMessage> {
    frame_messages_in_tree(state, renderer, viewport, &mut Tree::empty())
}

fn frame_messages_in_tree(
    state: &BannerUiState,
    renderer: &iced::Renderer,
    viewport: Size,
    tree: &mut Tree,
) -> Vec<BannerUiMessage> {
    event_messages_in_tree(
        state,
        renderer,
        viewport,
        tree,
        core::Event::Window(core::window::Event::RedrawRequested(
            core::time::Instant::now(),
        )),
        core::mouse::Cursor::Unavailable,
    )
}

fn event_messages_in_tree(
    state: &BannerUiState,
    renderer: &iced::Renderer,
    viewport: Size,
    tree: &mut Tree,
    event: core::Event,
    cursor: core::mouse::Cursor,
) -> Vec<BannerUiMessage> {
    event_messages_for_window(
        state,
        state.surface.window().unwrap(),
        renderer,
        viewport,
        tree,
        event,
        cursor,
    )
}

fn event_messages_for_window(
    state: &BannerUiState,
    window: iced::window::Id,
    renderer: &iced::Renderer,
    viewport: Size,
    tree: &mut Tree,
    event: core::Event,
    cursor: core::mouse::Cursor,
) -> Vec<BannerUiMessage> {
    let mut view = banner_ui_view(state, window);
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        tree,
        renderer,
        &core::layout::Limits::new(Size::ZERO, viewport),
    );
    let mut messages = Vec::new();
    view.as_widget_mut().update(
        tree,
        &event,
        Layout::new(&node),
        cursor,
        renderer,
        &mut core::clipboard::Null,
        &mut core::Shell::new(&mut messages),
        &Rectangle::with_size(viewport),
    );
    messages
}

#[tokio::test]
async fn retiring_window_does_not_display_or_acknowledge_the_next_stack() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let old = state.surface.window().unwrap();
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(old));
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, acknowledged.clone());
    let messages = event_messages_for_window(
        &state,
        old,
        &renderer,
        Size::new(776.0, 516.0),
        &mut Tree::empty(),
        core::Event::Window(core::window::Event::RedrawRequested(
            core::time::Instant::now(),
        )),
        core::mouse::Cursor::Unavailable,
    );
    assert!(
        messages.is_empty(),
        "a retiring surface must render no new content: {messages:?}"
    );
    assert!(!acknowledged.load(Ordering::SeqCst));
}

#[tokio::test]
async fn late_layout_from_lost_output_cannot_restore_its_input_region() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, acknowledged.clone());
    let old = state.surface.window().unwrap();
    let delayed = frame_messages(&state, &renderer, Size::new(776.0, 516.0));
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(old));
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowClosed(old));
    let actions = frame_actions(
        &mut state,
        &renderer,
        Size::new(276.0, 216.0),
        &mut Tree::empty(),
    )
    .await;
    let [LayerShellCustomAction::SetInputRegion(current)] = actions.as_slice() else {
        panic!("fallback must install its measured region: {actions:?}");
    };
    for message in delayed {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(acknowledged.load(Ordering::SeqCst));
    assert_eq!(
        region_requests(current),
        vec![
            (2, vec![0, 0, i32::MAX, i32::MAX]),
            (1, vec![0, 0, 276, queued_card_height(&renderer, 276.0)]),
        ],
        "late geometry cannot change the fallback output's input region"
    );
}

#[tokio::test]
async fn scrolling_reveals_a_pending_card_without_restarting_already_displayed_cards() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    state.presentation = top_end(BannerPresentation::visible_banners(vec![
        banner_view(1, "Upper"),
        banner_view(2, "Lower"),
    ]));
    let targets: Vec<_> = state
        .presentation
        .banners
        .iter()
        .map(BannerView::target)
        .collect();
    let displayed = Arc::new(Mutex::new(Vec::new()));
    let observed = displayed.clone();
    state.pending_acknowledgement = Some(PendingBannerAcknowledgement {
        generation: 0,
        acknowledgement: BannerPresentationAcknowledgement::for_banners(
            targets.clone(),
            move |target| {
                observed.lock().unwrap().push(target);
            },
        ),
    });
    let viewport = Size::new(360.0, 120.0);
    let mut tree = Tree::empty();
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    for wheel in [-20.0, 20.0, -20.0] {
        for message in event_messages_in_tree(
            &state,
            &renderer,
            viewport,
            &mut tree,
            core::Event::Mouse(core::mouse::Event::WheelScrolled {
                delta: core::mouse::ScrollDelta::Lines { x: 0.0, y: wheel },
            }),
            core::mouse::Cursor::Available(core::Point::new(355.0, 110.0)),
        ) {
            let _ = banner_ui_update(&mut state, message);
        }
        for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
            let _ = banner_ui_update(&mut state, message);
        }
        let mut view = banner_ui_view(&state, state.surface.window().unwrap());
        tree.diff(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &core::layout::Limits::new(Size::ZERO, viewport),
        );
        let mut scroll = ScrollBounds::default();
        view.as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
        assert_eq!(
            scroll.0[0].2.y > 0.0,
            wheel < 0.0,
            "outer scroll geometry: {:?}",
            scroll.0
        );
    }
    assert_eq!(
        *displayed.lock().unwrap(),
        targets,
        "scrolling must reveal lower content without acknowledging either card twice"
    );
    assert!(state.pending_acknowledgement.is_none());
    // Clearing the pending acknowledgement must not rebuild the scrollable tree.
    let mut view = banner_ui_view(&state, state.surface.window().unwrap());
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, viewport),
    );
    let mut scroll = ScrollBounds::default();
    view.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
    assert!(
        scroll.0[0].2.y > 0.0,
        "finishing acknowledgement must preserve the user's scroll position"
    );
}

#[tokio::test]
async fn measured_replacements_keep_frame_ownership_without_redundant_input_updates() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut tree = Tree::empty();
    let first = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, first.clone());
    let viewport = Size::new(976.0, 216.0);
    let old_frames = frame_messages_in_tree(&state, &renderer, viewport, &mut tree);
    let replacement = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, replacement.clone());
    for message in old_frames {
        assert_eq!(
            banner_ui_update(&mut state, message).units(),
            0,
            "old geometry must not change the new generation's input region"
        );
    }
    assert!(first.load(Ordering::SeqCst));
    assert!(!replacement.load(Ordering::SeqCst));
    assert!(matches!(
        frame_actions(&mut state, &renderer, viewport, &mut tree)
            .await
            .as_slice(),
        [LayerShellCustomAction::SetInputRegion(_)]
    ));
    assert!(
        replacement.load(Ordering::SeqCst),
        "same geometry still needs the replacement's own frame"
    );
    let next = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, next.clone());
    assert!(
        frame_actions(&mut state, &renderer, viewport, &mut tree)
            .await
            .is_empty(),
        "same-size replacement must not update the input region"
    );
    assert!(next.load(Ordering::SeqCst));
    assert!(
        frame_messages_in_tree(&state, &renderer, viewport, &mut tree).is_empty(),
        "stable frames must not cause a redraw loop"
    );
}

#[tokio::test]
async fn three_card_stack_with_overflow_clamps_input_and_clears_to_click_through() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 1, acknowledged.clone());
    state
        .presentation
        .banners
        .extend([banner_view(2, "Second"), banner_view(3, "Third")]);
    state.presentation.overflow_count = 4;
    let mut tree = Tree::empty();
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 216.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(
        region_requests(region),
        vec![
            (2, vec![0, 0, i32::MAX, i32::MAX]),
            (1, vec![616, 0, 360, 216])
        ]
    );
    assert!(
        !acknowledged.load(Ordering::SeqCst),
        "clipped stack must not start undisplayed timeouts"
    );
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 600.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    let requests = region_requests(region);
    assert!(
        requests[1].1[3] > 300 && requests[1].1[3] < 600,
        "input follows content, including overflow, not empty surface space"
    );
    assert!(acknowledged.load(Ordering::SeqCst));
    let _ = banner_ui_update(
        &mut state,
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation::default(),
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    );
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 600.0), &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(
        region_requests(region),
        vec![(2, vec![0, 0, i32::MAX, i32::MAX])]
    );
}

#[tokio::test]
async fn oversized_card_is_bounded_and_acknowledged_without_losing_natural_content() {
    let mut state = banner_ui_test_state();
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, acknowledged.clone());
    let banner = &mut state.presentation.banners[0];
    banner.summary = "Download complete\nReview destination\nOpen document".into();
    banner.body_segments = vec![
        BannerBodySegment::plain_text("Before"),
        BannerBodySegment::Image {
            image: test_banner_visual(),
        },
        BannerBodySegment::plain_text("After"),
    ];
    banner.progress = Some(42);
    banner.actions = vec![BannerAction {
        key: "one".into(),
        label: "Review changes\nand open report\nin the editor".into(),
        icon: None,
    }];
    let renderer = software_renderer().await;

    let mut tree = Tree::empty();
    let actions = frame_actions(&mut state, &renderer, Size::new(976.0, 150.0), &mut tree).await;
    assert!(matches!(
        actions.as_slice(),
        [LayerShellCustomAction::SetInputRegion(_)]
    ));
    assert!(
        acknowledged.load(Ordering::SeqCst),
        "a fully visible bounded compact card must acknowledge display"
    );
    let mut view = banner_ui_view(&state, state.surface.window().unwrap());
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(976.0, 150.0)),
    );
    let mut scroll = ScrollBounds::default();
    view.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut scroll);
    let natural_height = scroll
        .0
        .iter()
        .find(|(viewport, content, _)| viewport.height == 150.0 && content.height > 200.0)
        .map(|(_, content, _)| content.height.ceil())
        .expect("the complete natural card must remain internally scrollable");
    drop(view);
    let _ = event_messages_in_tree(
        &state,
        &renderer,
        Size::new(976.0, 150.0),
        &mut tree,
        core::Event::Mouse(core::mouse::Event::WheelScrolled {
            delta: core::mouse::ScrollDelta::Lines { x: 0.0, y: -20.0 },
        }),
        core::mouse::Cursor::Available(core::Point::new(621.0, 75.0)),
    );
    assert!(
        scroll_bounds_in_tree(&state, &renderer, Size::new(976.0, 150.0), &mut tree).0[1]
            .2
            .y
            > 0.0
    );
    let grown = Size::new(976.0, natural_height);
    let actions = frame_actions(&mut state, &renderer, grown, &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    let requests = region_requests(region);
    assert_eq!(
        requests[1].1[3], natural_height as i32,
        "rich card retains its natural height"
    );
    assert!(
        acknowledged.load(Ordering::SeqCst),
        "the fitting frame must acknowledge its presentation"
    );
    assert_eq!(
        scroll_bounds_in_tree(&state, &renderer, grown, &mut tree).0[1]
            .2
            .y,
        0.0,
        "growing the viewport must remove obsolete internal translation"
    );

    let replacement = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, replacement.clone());
    let actions = frame_actions(&mut state, &renderer, grown, &mut tree).await;
    let [LayerShellCustomAction::SetInputRegion(region)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(
        region_requests(region),
        vec![
            (2, vec![0, 0, i32::MAX, i32::MAX]),
            (1, vec![616, 0, 360, queued_card_height(&renderer, 360.0)])
        ]
    );
    assert!(
        replacement.load(Ordering::SeqCst),
        "replacing wrapped content must shrink and acknowledge the new frame"
    );
}

#[tokio::test]
async fn custom_banner_font_and_spacing_reflow_without_losing_action_targets() {
    let renderer = software_renderer().await;
    let mut banner = banner_view(
        48,
        "A larger body that wraps onto several lines in a narrow notification",
    );
    banner.actions = vec![BannerAction {
        key: "opaque-action".into(),
        label: "Open message".into(),
        icon: None,
    }];
    let mut appearance = canopa_theme::Appearance::default();
    let custom = std::sync::Arc::make_mut(&mut appearance.customization);
    custom.typography.family = vec!["monospace".into()];
    custom.typography.body_size = canopa_theme::Pixels::whole(22);
    custom.typography.title_size = canopa_theme::Pixels::whole(26);
    custom.ui[canopa_theme::UiKind::Notification as usize].padding =
        Some(canopa_theme::Insets::all(18));
    custom.ui[canopa_theme::UiKind::Notification as usize].margin =
        Some(canopa_theme::Insets::all(6));
    custom.ui[canopa_theme::UiKind::Notification as usize].border_width =
        Some(canopa_theme::Pixels::whole(3));
    let mut card = notification_banner_widget(&banner, &appearance);
    let mut tree = Tree::new(card.as_widget());
    let node = card.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &core::layout::Limits::new(Size::ZERO, Size::new(360.0, 1500.0)),
    );
    let mut labels = TextBounds::default();
    card.as_widget_mut()
        .operate(&mut tree, Layout::new(&node), &renderer, &mut labels);
    let action = labels
        .0
        .iter()
        .find(|(label, _)| label == "Open message")
        .unwrap()
        .1;
    assert!(action.height >= 22.0);
    assert!(action.x >= 27.0 && action.y + action.height <= node.size().height);
    let mut messages = Vec::new();
    for event in [
        core::mouse::Event::ButtonPressed(core::mouse::Button::Left),
        core::mouse::Event::ButtonReleased(core::mouse::Button::Left),
    ] {
        card.as_widget_mut().update(
            &mut tree,
            &core::Event::Mouse(event),
            Layout::new(&node),
            core::mouse::Cursor::Available(action.center()),
            &renderer,
            &mut core::clipboard::Null,
            &mut core::Shell::new(&mut messages),
            &Rectangle::with_size(node.size()),
        );
    }
    assert_eq!(messages.len(), 1);
    assert!(format!("{:?}", messages[0]).contains("opaque-action"));
}

/// These geometry checks were written for the top-end corner.
fn top_end(mut presentation: BannerPresentation) -> BannerPresentation {
    presentation.position = crate::BannerPosition::TopEnd;
    presentation
}
