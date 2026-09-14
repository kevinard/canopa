use super::*;
use iced_runtime::{Action, window};

async fn ready_actions(task: iced::Task<BannerUiMessage>) -> Vec<Action<BannerUiMessage>> {
    let mut actions = Vec::new();
    if let Some(mut stream) = iced_runtime::task::into_stream(task) {
        // Iced's output tasks yield once before emitting. Drain immediate work,
        // without waiting for a surface's ten-second first-frame deadline.
        while let Ok(Some(action)) =
            tokio::time::timeout(Duration::from_millis(20), stream.next()).await
        {
            actions.push(action);
        }
    }
    actions
}

#[tokio::test]
async fn first_frame_cancels_the_surface_deadline_task() {
    let mut state = banner_ui_test_state();
    let probe = state.surface.window().unwrap();
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(probe));
    queue_banner_ui_presentation(&mut state, 1, Arc::new(AtomicBool::new(false)));
    let task = banner_ui_update(&mut state, BannerUiMessage::WindowClosed(probe));
    let mut stream = iced_runtime::task::into_stream(task).unwrap();
    let Action::Output(BannerUiMessage::OpenSurface(window)) = stream.next().await.unwrap() else {
        panic!("expected new surface");
    };
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(window));
    assert!(
        tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("a rendered stack must not retain its deadline task")
            .is_none()
    );
}

#[tokio::test]
async fn first_redraw_retires_the_empty_startup_probe() {
    let mut state = banner_ui_test_state();
    let probe = state.surface.window().unwrap();
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowFrame(probe),
    ))
    .await;
    assert!(
        matches!(
            actions.as_slice(),
            [Action::Window(window::Action::Close(_))]
        ),
        "startup readiness must not leave a surface pinned to the startup output"
    );
    assert_eq!(*state.renderer_state.borrow(), BannerRendererState::Ready);
}

#[tokio::test]
async fn stacks_open_serially_keep_their_window_and_ignore_retired_window_events() {
    let mut state = banner_ui_test_state();
    let probe = state.surface.window().unwrap();
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(probe));
    queue_banner_ui_presentation(&mut state, 1, Arc::new(AtomicBool::new(false)));
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowClosed(probe),
    ))
    .await;
    let [Action::Output(BannerUiMessage::OpenSurface(current))] = actions.as_slice() else {
        panic!("new content must wait for the probe to close: {actions:?}");
    };
    let current = *current;
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(current));
    for message in [
        BannerUiMessage::WindowClosed(probe),
        BannerUiMessage::WindowFrame(probe),
        BannerUiMessage::WindowTimedOut(probe),
        BannerUiMessage::WindowTimedOut(current),
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation::visible_banners(vec![banner_view(1, "Replacement")]),
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    ] {
        assert!(
            ready_actions(banner_ui_update(&mut state, message))
                .await
                .is_empty()
        );
    }
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::Runtime(BannerRuntimeInput::Replace {
            presentation: BannerPresentation::default(),
            acknowledgement: BannerPresentationAcknowledgement::new(|| {}),
        }),
    ))
    .await;
    assert!(
        matches!(actions.as_slice(), [Action::Window(window::Action::Close(id))] if *id == current)
    );
    queue_banner_ui_presentation(&mut state, 2, Arc::new(AtomicBool::new(false)));
    assert!(
        ready_actions(banner_ui_update(
            &mut state,
            BannerUiMessage::WindowClosed(probe)
        ))
        .await
        .is_empty()
    );
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowClosed(current),
    ))
    .await;
    let [Action::Output(BannerUiMessage::OpenSurface(next))] = actions.as_slice() else {
        panic!("expected the next stack: {actions:?}");
    };
    assert_ne!(*next, current);
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowTimedOut(*next),
    ))
    .await;
    assert!(matches!(actions.as_slice(), [Action::Exit]));
    assert_eq!(*state.renderer_state.borrow(), BannerRendererState::Stopped);
}

#[tokio::test]
async fn lost_output_reopens_a_visible_stack_but_bounds_unmapped_retries() {
    let mut state = banner_ui_test_state();
    queue_banner_ui_presentation(&mut state, 1, Arc::new(AtomicBool::new(false)));
    let first = state.surface.window().unwrap();
    let _ = banner_ui_update(&mut state, BannerUiMessage::WindowFrame(first));
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowClosed(first),
    ))
    .await;
    let [Action::Output(BannerUiMessage::OpenSurface(fallback))] = actions.as_slice() else {
        panic!("a surviving compositor output must be offered the current stack: {actions:?}");
    };
    assert_ne!(*fallback, first);
    assert_eq!(*state.renderer_state.borrow(), BannerRendererState::Ready);
    let actions = ready_actions(banner_ui_update(
        &mut state,
        BannerUiMessage::WindowClosed(*fallback),
    ))
    .await;
    assert!(matches!(actions.as_slice(), [Action::Exit]));
    assert_eq!(*state.renderer_state.borrow(), BannerRendererState::Stopped);
}
