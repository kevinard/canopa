use super::*;
use crate::app::foreground::{ForegroundEffect, ForegroundIntent, PanelMenuScrollContext};
use iced::advanced::{Layout, layout, renderer::Headless, widget::Operation, widget::Tree};
use iced::{Event, Point, Rectangle, Size, mouse};

#[derive(Default)]
struct ScrollGeometry {
    viewport: Rectangle,
    content: Rectangle,
    offset: f32,
}

impl Operation for ScrollGeometry {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: iced::Vector,
        _: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        self.viewport = bounds;
        self.content = content_bounds;
        self.offset = translation.y;
    }
}

struct MenuWidgets {
    renderer: iced::Renderer,
    tree: Tree,
    window: iced::window::Id,
    cursor: Point,
    size: Size,
    now: iced::time::Instant,
    redraw_request: iced::window::RedrawRequest,
}

impl MenuWidgets {
    async fn new(app: &App) -> Self {
        Self {
            renderer: iced::Renderer::new(
                iced::Font::DEFAULT,
                iced::Pixels(16.0),
                Some("tiny-skia"),
            )
            .await
            .expect("software renderer"),
            tree: Tree::empty(),
            window: app.foreground.active_panel_window().expect("panel window"),
            cursor: Point::new(40.0, 200.0),
            size: Size::new(360.0, 400.0),
            now: iced::time::Instant::now(),
            redraw_request: iced::window::RedrawRequest::Wait,
        }
    }

    fn event(&mut self, app: &mut App, event: Event, scroll_to: Option<f32>) -> ScrollGeometry {
        if let Event::Window(iced::window::Event::RedrawRequested(now)) = &event {
            self.now = *now;
        }
        let (messages, geometry, redraw_request) = {
            let mut view = app.view(self.window);
            self.tree.diff(view.as_widget());
            let viewport = Rectangle::with_size(self.size);
            let node = view.as_widget_mut().layout(
                &mut self.tree,
                &self.renderer,
                &layout::Limits::new(Size::ZERO, viewport.size()),
            );
            if let Some(y) = scroll_to {
                let mut operation = iced::advanced::widget::operation::scrollable::scroll_to::<()>(
                    panel_menu_scroll_id(self.window),
                    iced::widget::scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(y),
                    },
                );
                view.as_widget_mut().operate(
                    &mut self.tree,
                    Layout::new(&node),
                    &self.renderer,
                    &mut operation,
                );
            }
            let mut messages = Vec::new();
            let mut shell = iced::advanced::Shell::new(&mut messages);
            view.as_widget_mut().update(
                &mut self.tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(self.cursor),
                &self.renderer,
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &viewport,
            );
            let redraw_request = shell.redraw_request();
            let mut geometry = ScrollGeometry::default();
            view.as_widget_mut().operate(
                &mut self.tree,
                Layout::new(&node),
                &self.renderer,
                &mut geometry,
            );
            (messages, geometry, redraw_request)
        };
        self.redraw_request = redraw_request;
        for message in messages {
            let _ = app.update(message);
        }
        geometry
    }

    fn redraw(&mut self, app: &mut App, scroll_to: Option<f32>) -> ScrollGeometry {
        self.now += Duration::from_secs_f64(1.0 / 120.0);
        self.event(
            app,
            Event::Window(iced::window::Event::RedrawRequested(self.now)),
            scroll_to,
        )
    }

    fn settle(&mut self, app: &mut App) -> ScrollGeometry {
        self.redraw(app, None);
        self.now += Duration::from_millis(250);
        self.redraw(app, None);
        self.redraw(app, None)
    }

    fn point_at(&mut self, app: &mut App, point: Point) {
        self.cursor = point;
        self.event(
            app,
            Event::Mouse(mouse::Event::CursorMoved { position: point }),
            None,
        );
    }
}

fn installed_apps_drawer(long_labels: bool) -> App {
    let mut app = app_with_installed_apps_menu(
        (0..100)
            .map(|index| {
                let suffix = if long_labels && index % 7 == 0 {
                    " with a very long application name that would otherwise wrap across several lines in the drawer"
                } else { "" };
                installed_app(&format!("App {index:03}{suffix}"), &format!("app-{index}.desktop"))
            })
            .collect(),
    );
    app.config.bars[0].panel.presentation = crate::config::PanelPresentation::Drawer;
    open_apps_menu_panel(&mut app);
    app
}

#[tokio::test]
async fn installed_apps_cross_cached_rows_without_icon_io_on_the_ui_thread() {
    let mut apps: Vec<_> = (0..100)
        .map(|i| installed_app(&format!("Icon app {i:03}"), &format!("icon-{i}.desktop")))
        .collect();
    for (i, app) in apps.iter_mut().enumerate() {
        app.icon = Some(format!("scroll-boundary-fixture-{i}"));
    }
    let mut app = app_with_installed_apps_menu(apps);
    app.config.menus[0].show_icons = true;
    app.config.bars[0].panel.presentation = crate::config::PanelPresentation::Drawer;
    open_apps_menu_panel(&mut app);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.redraw(&mut app, None);
    crate::icon_theme::take_lookup_count();
    let mut previous = 0.0;
    for _ in 0..180 {
        widgets.event(
            &mut app,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -5.0 },
            }),
            None,
        );
        let geometry = widgets.redraw(&mut app, None);
        assert!(geometry.offset > previous);
        previous = geometry.offset;
    }
    assert!(previous > 40.0 * MENU_ROW_EXTENT);
    assert_eq!(
        crate::icon_theme::take_lookup_count(),
        0,
        "newly rendered icons must not do filesystem lookup on the scrolling thread"
    );
    let before = widgets.settle(&mut app);
    let focused = app
        .foreground
        .active_panel_menu_render_state()
        .focused_index;
    // A delayed batch replaces empty slots with prepared pixels at the current
    // position, without changing row pitch, focus, or pending touchpad travel.
    let loaded = (0..100)
        .map(|i| {
            (
                format!("scroll-boundary-fixture-{i}"),
                Some(iced::widget::image::Handle::from_rgba(
                    2,
                    2,
                    vec![255u8; 16],
                )),
            )
        })
        .collect();
    let _ = app.update(Message::Panel(
        crate::app::messages::PanelEvent::IconsLoaded(loaded),
    ));
    let after = widgets.redraw(&mut app, None);
    assert_eq!(after.offset, before.offset);
    assert_eq!(after.content.height, before.content.height);
    assert_eq!(
        app.foreground
            .active_panel_menu_render_state()
            .focused_index,
        focused
    );
    assert_eq!(crate::icon_theme::take_lookup_count(), 0);
    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Close));
    let _ = app.update(Message::Panel(
        crate::app::messages::PanelEvent::IconsLoaded(vec![]),
    ));
    assert!(app.foreground.active_panel_window().is_none());
}

#[tokio::test]
async fn installed_apps_touchpad_motion_continues_between_input_packets() {
    for refresh_rate in [60, 120, 144] {
        check_touchpad_motion(refresh_rate).await;
    }
}

async fn check_touchpad_motion(refresh_rate: u32) {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.redraw(&mut app, None);
    let start = widgets.now;
    let mut previous = 0.0;
    let mut steps = Vec::new();
    // A steady gesture delivered in packets at 40 Hz.
    let mut next_packet = Duration::ZERO;
    for frame in 0..refresh_rate / 2 {
        let elapsed = Duration::from_secs_f64(frame as f64 / refresh_rate as f64);
        if elapsed >= next_packet {
            next_packet += Duration::from_millis(25);
            widgets.event(
                &mut app,
                Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -3.0 },
                }),
                None,
            );
        }
        let geometry = widgets.event(
            &mut app,
            Event::Window(iced::window::Event::RedrawRequested(
                start + Duration::from_secs_f64((frame + 1) as f64 / refresh_rate as f64),
            )),
            None,
        );
        if frame > refresh_rate / 10 {
            steps.push(geometry.offset - previous);
            assert_eq!(
                widgets.redraw_request,
                iced::window::RedrawRequest::NextFrame
            );
        }
        previous = geometry.offset;
    }
    assert!(
        steps
            .iter()
            .all(|step| *step > 0.0 && *step <= (720.0 / refresh_rate as f32).ceil()),
        "uneven movement at {refresh_rate} Hz: {steps:?}"
    );
    assert_eq!(widgets.settle(&mut app).offset, 180.0);
    assert_ne!(
        widgets.redraw_request,
        iced::window::RedrawRequest::NextFrame
    );
}

#[tokio::test]
async fn installed_apps_touchpad_cannot_override_a_programmatic_scroll() {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.redraw(&mut app, None);
    widgets.event(
        &mut app,
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -40.0 },
        }),
        None,
    );
    let first = widgets.redraw(&mut app, None).offset;
    assert!(first > 0.0 && first < 120.0);
    // Even a subpixel operation must cancel the remaining touchpad movement.
    widgets.redraw(&mut app, Some(first + 0.25));
    assert_eq!(widgets.settle(&mut app).offset, first);
    assert_ne!(
        widgets.redraw_request,
        iced::window::RedrawRequest::NextFrame
    );
}

#[tokio::test]
async fn installed_apps_touchpad_reverses_without_a_backlog_at_the_scroll_limit() {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    let geometry = widgets.redraw(&mut app, None);
    let limit = geometry.content.height - geometry.viewport.height;
    widgets.redraw(&mut app, Some(limit));
    for delta in [-40.0, -40.0, -40.0, 1.0] {
        widgets.event(
            &mut app,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: delta },
            }),
            None,
        );
    }
    assert!(widgets.redraw(&mut app, None).offset < limit.round());
    assert_eq!(widgets.settle(&mut app).offset, (limit - 3.0).round());
}

#[tokio::test]
async fn installed_apps_touchpad_does_not_queue_scrolls_over_the_header() {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.redraw(&mut app, None);
    widgets.point_at(&mut app, Point::new(40.0, 20.0));
    widgets.event(
        &mut app,
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -40.0 },
        }),
        None,
    );
    widgets.point_at(&mut app, Point::new(40.0, 200.0));
    assert_eq!(widgets.settle(&mut app).offset, 0.0);
}

#[tokio::test]
async fn installed_apps_keyboard_passes_viewport_edge_with_stationary_pointer() {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.event(
        &mut app,
        Event::Mouse(mouse::Event::CursorMoved {
            position: widgets.cursor,
        }),
        None,
    );
    widgets.redraw(&mut app, None);
    let start = panel_focus(&app).expect("initial focus");
    let movements = (start + 1..100)
        .map(|index| (1, index))
        .chain((0..99).rev().map(|index| (-1, index)))
        .chain([(-1, 99), (1, 0)]);
    for (delta, expected) in movements {
        let effects = app.foreground.handle(ForegroundIntent::PanelFocusMove {
            delta,
            scroll: PanelMenuScrollContext {
                row_extent: app.active_menu_metrics().extent,
                default_viewport_height: DEFAULT_MENU_VIEWPORT_HEIGHT,
            },
        });
        let target = effects.iter().find_map(|effect| match effect {
            ForegroundEffect::ScrollPanelMenuToFocused { y, .. } => Some(*y),
            _ => None,
        });
        widgets.redraw(&mut app, target);
        widgets.redraw(&mut app, None);
        assert_eq!(
            panel_focus(&app),
            Some(expected),
            "stationary pointer stole keyboard focus at row {expected}"
        );
    }
}

#[tokio::test]
async fn installed_apps_touchpad_preserves_fractions_with_more_travel() {
    let mut app = installed_apps_drawer(false);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.redraw(&mut app, None);
    for _ in 0..120 {
        widgets.event(
            &mut app,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -0.25 },
            }),
            None,
        );
    }
    widgets.event(
        &mut app,
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: 0.0, y: 0.0 },
        }),
        None,
    );
    assert_eq!(widgets.settle(&mut app).offset, 90.0);
    widgets.event(
        &mut app,
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
        }),
        None,
    );
    assert_eq!(widgets.redraw(&mut app, None).offset, 150.0);
    assert_eq!(widgets.redraw(&mut app, Some(221.0)).offset, 221.0);
}

#[tokio::test]
async fn installed_apps_scroll_keeps_content_height_stable() {
    for long_labels in [false, true] {
        check_wheel_scrolling(long_labels).await;
    }
}

async fn check_wheel_scrolling(long_labels: bool) {
    let mut app = installed_apps_drawer(long_labels);
    let mut widgets = MenuWidgets::new(&app).await;
    let initial = widgets.redraw(&mut app, None);
    let mut expected_offset = 0.0_f32;
    for (step, delta) in std::iter::repeat_n(-40.0, 120)
        .chain(std::iter::repeat_n(40.0, 120))
        .enumerate()
    {
        widgets.event(
            &mut app,
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: delta },
            }),
            None,
        );
        let geometry = widgets.settle(&mut app);
        assert!(
            (geometry.content.height - initial.content.height).abs() < 0.01,
            "content height changed at scroll step {step}: {} -> {}",
            initial.content.height,
            geometry.content.height
        );
        expected_offset = (expected_offset - delta * 3.0)
            .clamp(0.0, initial.content.height - initial.viewport.height);
        // Iced rounds the rendered translation, including a fractional limit.
        assert!(
            (geometry.offset - expected_offset.round()).abs() < 0.01,
            "wheel offset jumped at step {step}: expected {expected_offset}, got {}; viewport {} -> {}",
            geometry.offset,
            initial.viewport.height,
            geometry.viewport.height
        );
        // Hit the first complete row through actual pointer input. This catches
        // spacer seams and accumulated row-height drift, not just total height.
        let row_extent = app.active_menu_metrics().extent;
        let index = (geometry.offset / row_extent).ceil() as usize;
        widgets.point_at(
            &mut app,
            Point::new(
                40.0,
                geometry.viewport.y + 8.0 + index as f32 * row_extent - geometry.offset + 22.0,
            ),
        );
        assert_eq!(
            panel_focus(&app),
            Some(index),
            "wrong row under pointer at scroll step {step}"
        );
    }
}

#[tokio::test]
async fn installed_apps_tall_drawer_renders_all_visible_rows() {
    let mut app = installed_apps_drawer(true);
    let mut widgets = MenuWidgets::new(&app).await;
    widgets.size.height = 4000.0;
    widgets.redraw(&mut app, None);
    let geometry = widgets.redraw(&mut app, None);
    let row_extent = app.active_menu_metrics().extent;
    widgets.point_at(
        &mut app,
        Point::new(40.0, geometry.viewport.y + 8.0 + 75.0 * row_extent + 22.0),
    );
    assert_eq!(
        panel_focus(&app),
        Some(75),
        "a tall drawer must not contain unrendered visible rows"
    );
}
