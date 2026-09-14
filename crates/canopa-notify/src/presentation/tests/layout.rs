use super::*;
use core::renderer::Headless;
use core::widget::{Operation, Tree};
use iced_layershell::reexport::core::{self, Layout, Rectangle, Size};

async fn software_renderer() -> iced::Renderer {
    iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .expect("software renderer")
}

#[derive(Default, Debug)]
struct TextBounds(Vec<(String, Rectangle)>);

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
    let mut card = notification_banner_widget(&banner);
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
    let mut card = notification_banner_widget(&banner);
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
    let mut body = notification_body_widget(&banner);
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
    let mut view = banner_ui_view(state, iced::window::Id::unique());
    tree.diff(view.as_widget());
    let node = view.as_widget_mut().layout(
        tree,
        renderer,
        &core::layout::Limits::new(Size::ZERO, viewport),
    );
    let mut messages = Vec::new();
    view.as_widget_mut().update(
        tree,
        &core::Event::Window(core::window::Event::RedrawRequested(
            core::time::Instant::now(),
        )),
        Layout::new(&node),
        core::mouse::Cursor::Unavailable,
        renderer,
        &mut core::clipboard::Null,
        &mut core::Shell::new(&mut messages),
        &Rectangle::with_size(viewport),
    );
    messages
}

#[tokio::test]
async fn measured_replacements_keep_frame_ownership_without_redundant_resizes() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut tree = Tree::empty();
    let first = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, first.clone());
    for message in frame_messages_in_tree(&state, &renderer, Size::new(360.0, 1.0), &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    let viewport = Size::new(360.0, state.surface_size.1 as f32);
    let old_frames = frame_messages_in_tree(&state, &renderer, viewport, &mut tree);
    let replacement = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, replacement.clone());
    for message in old_frames {
        assert_eq!(
            banner_ui_update(&mut state, message).units(),
            0,
            "old geometry must not resize the new generation"
        );
    }
    assert!(first.load(Ordering::SeqCst));
    assert!(!replacement.load(Ordering::SeqCst));
    for message in frame_messages_in_tree(&state, &renderer, viewport, &mut tree) {
        assert_eq!(
            banner_ui_update(&mut state, message).units(),
            0,
            "same-size replacement must not resize the surface"
        );
    }
    assert!(
        replacement.load(Ordering::SeqCst),
        "same geometry still needs the replacement's own frame"
    );
    assert!(
        frame_messages_in_tree(&state, &renderer, viewport, &mut tree).is_empty(),
        "stable frames must not cause a redraw loop"
    );
}

#[tokio::test]
async fn delayed_resize_is_repaired_from_the_actual_viewport() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let mut tree = Tree::empty();
    queue_banner_ui_presentation(&mut state, 42, Arc::new(AtomicBool::new(false)));
    for message in frame_messages_in_tree(&state, &renderer, Size::new(360.0, 1.0), &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    let old_viewport = Size::new(360.0, state.surface_size.1 as f32);
    queue_banner_ui_presentation(&mut state, 42, Arc::new(AtomicBool::new(false)));
    state.presentation.banners[0].body = "Long body\n".repeat(20);
    state.presentation.banners[0].body_segments.clear();
    for message in frame_messages_in_tree(&state, &renderer, old_viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    let current_viewport = Size::new(360.0, state.surface_size.1 as f32);
    assert!(current_viewport.height > old_viewport.height);
    for message in frame_messages_in_tree(&state, &renderer, current_viewport, &mut tree) {
        let _ = banner_ui_update(&mut state, message);
    }
    // Layer-shell task outputs are asynchronous: the old resize can arrive last.
    let repairs: usize = frame_messages_in_tree(&state, &renderer, old_viewport, &mut tree)
        .into_iter()
        .map(|message| banner_ui_update(&mut state, message).units())
        .sum();
    assert_eq!(
        repairs, 1,
        "cached desired size must not hide a stale actual viewport"
    );
    for message in frame_messages_in_tree(&state, &renderer, current_viewport, &mut tree) {
        assert_eq!(banner_ui_update(&mut state, message).units(), 0);
    }
    assert!(frame_messages_in_tree(&state, &renderer, current_viewport, &mut tree).is_empty());
}

#[tokio::test]
async fn three_card_stack_with_overflow_shrinks_after_clear_and_defers_clipped_display() {
    let renderer = software_renderer().await;
    let mut state = banner_ui_test_state();
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 1, acknowledged.clone());
    state
        .presentation
        .banners
        .extend([banner_view(2, "Second"), banner_view(3, "Third")]);
    state.presentation.overflow_count = 4;
    for message in frame_messages(&state, &renderer, Size::new(360.0, 1.0)) {
        let _ = banner_ui_update(&mut state, message);
    }
    let height = state.surface_size.1;
    assert!(
        height > 300,
        "measurement must include every card and overflow"
    );
    for message in frame_messages(&state, &renderer, Size::new(360.0, height as f32 / 2.0)) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(
        !acknowledged.load(Ordering::SeqCst),
        "clipped stack must not start undisplayed timeouts"
    );
    for message in frame_messages(&state, &renderer, Size::new(360.0, height as f32)) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(acknowledged.load(Ordering::SeqCst));
    let _ = banner_ui_update(
        &mut state,
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation::default(),
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    );
    for message in frame_messages(&state, &renderer, Size::new(360.0, height as f32)) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert_eq!(state.surface_size, (360, 1));
}

#[tokio::test]
async fn surface_fits_measured_rich_cards_before_acknowledging_display() {
    let mut state = banner_ui_test_state();
    let acknowledged = Arc::new(AtomicBool::new(false));
    queue_banner_ui_presentation(&mut state, 42, acknowledged.clone());
    let banner = &mut state.presentation.banners[0];
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
        label: "First action".into(),
        icon: None,
    }];
    let renderer = software_renderer().await;

    for message in frame_messages(&state, &renderer, Size::new(360.0, 1.0)) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(
        state.surface_size.1 > 160,
        "rich card needs more room than the old fixed surface: {:?}",
        state.surface_size
    );
    assert!(
        !acknowledged.load(Ordering::SeqCst),
        "a measurement frame is not a displayed banner"
    );
    let viewport = Size::new(state.surface_size.0 as f32, state.surface_size.1 as f32);
    for message in frame_messages(&state, &renderer, viewport) {
        let _ = banner_ui_update(&mut state, message);
    }
    assert!(
        acknowledged.load(Ordering::SeqCst),
        "the fitting frame must acknowledge its presentation"
    );
}
