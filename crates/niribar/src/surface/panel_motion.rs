//! Root-only panel transition. Rasterize once per transition, never once per frame.
//! The live widget tree stays in place, preserving focus and scrolling on entry.
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::advanced::{image::Renderer as _, renderer::Renderer as _};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

use crate::app::{Message, messages::PanelEvent};

const TRAVEL: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    pub started: Instant,
    pub duration: Duration,
    pub from: f32,
    pub closing: bool,
    pub direction: Vector,
    pub scale: f32,
}

impl Animation {
    pub fn progress(self, now: Instant) -> f32 {
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32().max(f32::EPSILON))
        .clamp(0.0, 1.0);
        if self.closing {
            // Ease both ends: the old accelerating fade still had 19% opacity
            // in its last tenth, then vanished at full speed. A smoothstep exit
            // retreats gently and is nearly transparent before removal.
            self.from * (1.0 - t * t * (3.0 - 2.0 * t))
        } else {
            1.0 - (1.0 - t).powi(3)
        }
    }

    pub fn finished(self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.duration
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Presentation {
    Preparing,
    Animated(Animation),
    #[default]
    Still,
}

pub fn wrap<'a>(
    content: Element<'a, Message>,
    window: iced::window::Id,
    presentation: Presentation,
) -> Element<'a, Message> {
    Element::new(Transition {
        content,
        window,
        presentation,
    })
}

struct Transition<'a> {
    content: Element<'a, Message>,
    window: iced::window::Id,
    presentation: Presentation,
}

#[derive(Default)]
struct State {
    presentation: Presentation,
    notified: bool,
    now: Option<Instant>,
    image: RefCell<Option<iced::widget::image::Handle>>,
}

impl Transition<'_> {
    fn closing(&self) -> bool {
        matches!(
            self.presentation,
            Presentation::Animated(Animation { closing: true, .. })
        )
    }

    fn offset(&self, state: &State, bounds: Rectangle) -> Vector {
        let Presentation::Animated(animation) = self.presentation else {
            return Vector::ZERO;
        };
        if state.notified || capture_size(bounds, animation.scale).is_none() {
            return Vector::ZERO;
        }
        animation.direction
            * (TRAVEL * (1.0 - animation.progress(state.now.unwrap_or(animation.started))))
    }
}

fn capture_size(bounds: Rectangle, scale: f32) -> Option<Size<u32>> {
    let size = Size::new(
        (bounds.width * scale).ceil() as u32,
        (bounds.height * scale).ceil() as u32,
    );
    (size.width > 0
        && size.height > 0
        && u64::from(size.width) * u64::from(size.height) <= 8_388_608)
        .then_some(size)
}

/// Input and cursor feedback use the same transform as the last displayed frame.
fn translated_event(event: &Event, offset: Vector) -> Event {
    use iced::touch::Event::*;
    match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Event::Mouse(mouse::Event::CursorMoved {
                position: *position - offset,
            })
        }
        Event::Touch(event) => Event::Touch(match *event {
            FingerPressed { id, position } => FingerPressed {
                id,
                position: position - offset,
            },
            FingerMoved { id, position } => FingerMoved {
                id,
                position: position - offset,
            },
            FingerLifted { id, position } => FingerLifted {
                id,
                position: position - offset,
            },
            FingerLost { id, position } => FingerLost {
                id,
                position: position - offset,
            },
        }),
        _ => event.clone(),
    }
}

impl Widget<Message, Theme, Renderer> for Transition<'_> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State {
            presentation: self.presentation,
            ..State::default()
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }
    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.presentation != self.presentation {
            *state = State {
                presentation: self.presentation,
                ..State::default()
            };
        }
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if !self.closing() {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        }
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        if let Event::Window(iced::window::Event::RedrawRequested(now)) = event {
            state.now = Some(*now);
            match self.presentation {
                Presentation::Preparing if !state.notified => {
                    state.notified = true;
                    shell.publish(Message::Panel(PanelEvent::MotionReady(self.window)));
                }
                Presentation::Animated(animation) => {
                    if animation.finished(*now) {
                        if !state.notified {
                            state.notified = true;
                            shell.publish(Message::Panel(PanelEvent::MotionFinished {
                                window: self.window,
                                closing: animation.closing,
                            }));
                        }
                    } else {
                        shell.request_redraw();
                    }
                }
                _ => {}
            }
        }
        if self.closing() {
            return;
        }
        let offset = self.offset(state, layout.bounds());
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            &translated_event(event, offset),
            layout,
            cursor - offset,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        // Finish after a pointer gesture, so its press and release both target
        // the visible control. Scrolling and typing reveal live content at once.
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonReleased(_) | mouse::Event::WheelScrolled { .. })
                | Event::Keyboard(iced::keyboard::Event::KeyPressed { .. })
                | Event::Touch(
                    iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. }
                )
        ) && matches!(self.presentation, Presentation::Animated(_))
            && !state.notified
        {
            state.notified = true;
            shell.publish(Message::Panel(PanelEvent::MotionFinished {
                window: self.window,
                closing: false,
            }));
        }
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        if self.presentation == Presentation::Preparing {
            return;
        }
        let draw_content = |renderer: &mut Renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            )
        };
        let Presentation::Animated(animation) = self.presentation else {
            draw_content(renderer);
            return;
        };
        let now = state.now.unwrap_or(animation.started);
        if state.notified || animation.finished(now) {
            if !animation.closing {
                draw_content(renderer);
            }
            return;
        }
        let bounds = layout.bounds();
        // Bound transient allocations on unusual output configurations.
        let Some(physical) = capture_size(bounds, animation.scale) else {
            draw_content(renderer);
            return;
        };
        let mut image = state.image.borrow_mut();
        if image.is_none() {
            draw_content(renderer);
            let rgba = renderer::Headless::screenshot(
                renderer,
                physical,
                animation.scale,
                iced::Color::TRANSPARENT,
            );
            // This widget must be the window root: the capture consumed the full scene.
            renderer.reset(*viewport);
            *image = Some(iced::widget::image::Handle::from_rgba(
                physical.width,
                physical.height,
                rgba,
            ));
        }
        let progress = animation.progress(now);
        let offset = self.offset(state, bounds);
        renderer.draw_image(
            iced::advanced::image::Image::new(image.as_ref().unwrap().clone()).opacity(progress),
            Rectangle {
                x: bounds.x + offset.x,
                y: bounds.y + offset.y,
                ..bounds
            },
            *viewport,
        );
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.closing() {
            mouse::Interaction::None
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor - self.offset(tree.state.downcast_ref::<State>(), layout.bounds()),
                viewport,
                renderer,
            )
        }
    }
    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        if self.closing() {
            None
        } else {
            self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::renderer::Headless as _;

    #[tokio::test]
    async fn mouse_and_touch_activate_the_visible_control_on_each_edge() {
        let renderer = Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
            .await
            .expect("software renderer");
        for direction in [
            Vector::new(-1.0, 0.0),
            Vector::new(1.0, 0.0),
            Vector::new(0.0, -1.0),
            Vector::new(0.0, 1.0),
        ] {
            for touch in [false, true] {
                let start = Instant::now();
                let animation = Animation {
                    started: start,
                    duration: Duration::from_secs(1),
                    from: 0.0,
                    closing: false,
                    direction,
                    scale: 1.5,
                };
                let mut view = wrap(
                    iced::widget::container(
                        iced::widget::button("Hit")
                            .width(30)
                            .height(30)
                            .on_press(Message::Panel(PanelEvent::Close)),
                    )
                    .padding(32)
                    .width(100)
                    .height(100)
                    .into(),
                    iced::window::Id::unique(),
                    Presentation::Animated(animation),
                );
                let mut tree = Tree::new(view.as_widget());
                let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
                let node = view.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(Size::ZERO, viewport.size()),
                );
                let now = start + Duration::from_millis(20);
                let offset = direction * (TRAVEL * (1.0 - animation.progress(now)));
                // A point inside the displayed button but outside its resting bounds.
                let position = iced::Point::new(
                    if direction.x > 0.0 { 61.0 } else { 33.0 },
                    if direction.y > 0.0 { 61.0 } else { 33.0 },
                ) + offset;
                assert!(
                    !Rectangle::new(iced::Point::new(32.0, 32.0), Size::new(30.0, 30.0))
                        .contains(position)
                );
                let mut events = vec![Event::Window(iced::window::Event::RedrawRequested(now))];
                if touch {
                    events.extend([
                        Event::Touch(iced::touch::Event::FingerPressed {
                            id: iced::touch::Finger(1),
                            position,
                        }),
                        Event::Touch(iced::touch::Event::FingerLifted {
                            id: iced::touch::Finger(1),
                            position,
                        }),
                    ]);
                } else {
                    events.extend([
                        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                    ]);
                }
                let mut messages = Vec::new();
                for event in events {
                    view.as_widget_mut().update(
                        &mut tree,
                        &event,
                        Layout::new(&node),
                        mouse::Cursor::Available(position),
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &viewport,
                    );
                }
                assert_eq!(
                    messages
                        .iter()
                        .filter(|message| matches!(message, Message::Panel(PanelEvent::Close)))
                        .count(),
                    1,
                    "direction={direction:?}, touch={touch}"
                );
            }
        }
    }

    #[test]
    fn entry_and_interrupted_exit_are_bounded_and_finish_at_the_deadline() {
        let start = Instant::now();
        let entry = Animation {
            started: start,
            duration: Duration::from_millis(180),
            from: 0.0,
            closing: false,
            direction: Vector::new(-1.0, 0.0),
            scale: 1.0,
        };
        let halfway = start + Duration::from_millis(90);
        let exit = Animation {
            started: halfway,
            duration: Duration::from_millis(120),
            from: entry.progress(halfway),
            closing: true,
            ..entry
        };
        assert_eq!(entry.progress(start), 0.0);
        assert_eq!(entry.progress(start + Duration::from_secs(1)), 1.0);
        assert_eq!(exit.progress(halfway), entry.progress(halfway));
        assert!(exit.progress(halfway + Duration::from_millis(60)) < exit.from);
        assert_eq!(exit.progress(halfway + exit.duration), 0.0);
        assert!(exit.finished(halfway + exit.duration));
    }

    #[test]
    fn closing_tapers_to_transparent_before_the_window_is_removed() {
        let start = Instant::now();
        for from in [0.25, 0.75, 1.0] {
            let exit = Animation {
                started: start,
                duration: Duration::from_millis(160),
                from,
                closing: true,
                direction: Vector::new(-1.0, 0.0),
                scale: 1.0,
            };
            let samples: Vec<_> = (0..=10)
                .map(|frame| exit.progress(start + exit.duration.mul_f32(frame as f32 / 10.0)))
                .collect();
            assert_eq!(samples[0], from);
            assert_eq!(samples[10], 0.0);
            assert!(samples.windows(2).all(|pair| pair[0] >= pair[1]));
            assert!(samples[9] < from * 0.04, "no visible cutoff at removal");
            assert!(
                samples[9] < samples[5] - samples[6],
                "fade slows at the end"
            );
            assert!(samples[0] - samples[1] < samples[4] - samples[5]);
        }
    }
}
