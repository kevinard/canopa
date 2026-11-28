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

const TRAVEL: f32 = 16.0;
/// The entry's opacity from which the compositor blurs behind the panel.
const GLASS_OPACITY: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    pub started: Instant,
    pub duration: Duration,
    pub from: f32,
    pub closing: bool,
    pub direction: Vector,
    pub scale: f32,
    /// Where along the bar the panel's control sits (y for a side bar, x for
    /// a top or bottom one), in the panel's coordinates: it grows from there.
    pub anchor: Option<f32>,
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
            // Decelerate evenly: the slide out of the bar reads across the
            // whole entry and eases into place without a late jump.
            1.0 - (1.0 - t).powi(2)
        }
    }

    /// Elapsed share of the duration, without easing.
    pub fn fraction(self, now: Instant) -> f32 {
        (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32().max(f32::EPSILON))
        .clamp(0.0, 1.0)
    }

    /// Opacity of the panel: an entry fades in from faint over its first half,
    /// so it visibly appears rather than pops; an exit fades with its progress.
    pub fn opacity(self, now: Instant) -> f32 {
        if self.closing {
            self.progress(now)
        } else {
            let t = (self.fraction(now) / 0.5).clamp(0.0, 1.0);
            0.15 + 0.85 * (1.0 - (1.0 - t).powi(2))
        }
    }

    /// The panel grows from 95% out of the control that opened it, and shrinks
    /// a little back toward it as it leaves.
    pub fn zoom(self, now: Instant) -> f32 {
        if self.closing {
            0.97 + 0.03 * self.progress(now)
        } else {
            0.95 + 0.05 * self.progress(now)
        }
    }

    pub fn finished(self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.duration
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Presentation {
    /// Content is still on its way (a provider's first document): nothing is
    /// drawn and the entry has not begun.
    Waiting,
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
    /// The compositor blur behind the panel was dropped for its exit.
    glass_dropped: bool,
    /// The content's own glass shape, as it last reported it.
    glass: Option<Option<canopa_ui::glass::Shape>>,
    /// The shape last sent to the compositor, transformed like the frame.
    published: Option<Option<canopa_ui::glass::Shape>>,
    /// A pointer press began inside this panel during its entry.
    pressed_inside: bool,
    /// When the first frame of the transition was drawn: its clock starts
    /// there, so a panel whose first frame is slow to build (a calendar, Quick
    /// settings) still plays its whole transition instead of joining halfway.
    first_frame: Option<Instant>,
}

/// Where a panel grows from: the edge it comes from, at its control's place
/// along the bar when known.
fn growth_origin(animation: Animation, bounds: Rectangle) -> iced::Point {
    let origin = iced::Point::new(
        match animation.direction.x {
            x if x < 0.0 => bounds.x,
            x if x > 0.0 => bounds.x + bounds.width,
            _ => bounds.center_x(),
        },
        match animation.direction.y {
            y if y < 0.0 => bounds.y,
            y if y > 0.0 => bounds.y + bounds.height,
            _ => bounds.center_y(),
        },
    );
    match animation.anchor {
        Some(along) if animation.direction.y == 0.0 => {
            iced::Point::new(origin.x, along.clamp(bounds.y, bounds.y + bounds.height))
        }
        Some(along) => iced::Point::new(along.clamp(bounds.x, bounds.x + bounds.width), origin.y),
        None => origin,
    }
}

/// How the frame is drawn now: `None` while nothing is visible, otherwise the
/// origin, zoom and offset applied to it (identity once it has settled).
type Frame = Option<(iced::Point, f32, Vector)>;

impl Transition<'_> {
    fn frame(&self, state: &State, bounds: Rectangle) -> Frame {
        match self.presentation {
            Presentation::Waiting | Presentation::Preparing => None,
            Presentation::Still => Some((iced::Point::ORIGIN, 1.0, Vector::ZERO)),
            Presentation::Animated(animation) => {
                let now = state.now.unwrap_or(animation.started);
                if animation.closing {
                    None
                } else if state.notified || animation.finished(now) {
                    Some((iced::Point::ORIGIN, 1.0, Vector::ZERO))
                } else if animation.opacity(now) < GLASS_OPACITY {
                    // The compositor blurs at full strength at once: the glass
                    // joins once the panel's content is legible, so the entry
                    // never shows an empty slab of blur.
                    None
                } else {
                    Some((
                        growth_origin(animation, bounds),
                        animation.zoom(now),
                        self.offset(state, bounds),
                    ))
                }
            }
        }
    }

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

/// A glass shape as the frame is drawn: the compositor blurs exactly the
/// panel it can see, at every step of its entry.
fn place(frame: Frame, shape: Option<canopa_ui::glass::Shape>) -> Option<canopa_ui::glass::Shape> {
    let ((origin, zoom, offset), shape) = (frame?, shape?);
    let b = shape.bounds;
    Some(canopa_ui::glass::Shape {
        bounds: Rectangle {
            x: origin.x + (b.x - origin.x) * zoom + offset.x,
            y: origin.y + (b.y - origin.y) * zoom + offset.y,
            width: b.width * zoom,
            height: b.height * zoom,
        },
        radius: shape.radius.map(|r| r * zoom),
    })
}

/// The capture holds premultiplied colour; images take straight alpha. Without
/// this, everything translucent (the glass, its rim) draws darker while the
/// panel moves and brightens when the live panel replaces it.
fn unpremultiply(mut rgba: Vec<u8>) -> Vec<u8> {
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }
    rgba
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
            // The glass shape outlives the presentation that reported it.
            let (glass, published) = (state.glass.take(), state.published.take());
            *state = State {
                presentation: self.presentation,
                glass,
                published,
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
            let now = &match self.presentation {
                Presentation::Animated(animation) => {
                    let first = *state.first_frame.get_or_insert(*now);
                    animation.started + now.saturating_duration_since(first)
                }
                _ => *now,
            };
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
            // The fill fades out; the blur behind it must not outlast it.
            if !state.glass_dropped {
                state.glass_dropped = true;
                shell.publish(Message::Glass {
                    window: Some(self.window),
                    shape: None,
                });
            }
            return;
        }
        let offset = self.offset(state, layout.bounds());
        // The compositor blur follows the frame: the content's glass shape is
        // kept, and what is sent is that shape as drawn now (nothing before
        // the entry begins, the zoomed and offset shape while it plays).
        let frame = self.frame(state, layout.bounds());
        let mut messages = Vec::new();
        let mut inner = Shell::new(&mut messages);
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            &translated_event(event, offset),
            layout,
            cursor - offset,
            renderer,
            clipboard,
            &mut inner,
            viewport,
        );
        let reported = std::cell::Cell::new(None);
        shell.merge(inner, |message| match message {
            Message::Glass { window, shape } => {
                reported.set(Some(shape));
                Message::Glass {
                    window,
                    shape: place(frame, shape),
                }
            }
            message => message,
        });
        if let Some(shape) = reported.take() {
            state.glass = Some(shape);
            state.published = Some(place(frame, shape));
        } else if matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) && let Some(shape) = state.glass
        {
            let placed = place(frame, shape);
            if state.published != Some(placed) {
                state.published = Some(placed);
                shell.publish(Message::Glass {
                    window: None,
                    shape: placed,
                });
            }
        }
        // Finish after a pointer gesture in the panel, so its press and release
        // both target the visible control. The release of the click that opened
        // the panel is not one. Scrolling and typing reveal live content at once.
        let state = tree.state.downcast_mut::<State>();
        if let Event::Mouse(mouse::Event::ButtonPressed(_))
        | Event::Touch(iced::touch::Event::FingerPressed { .. }) = event
        {
            state.pressed_inside = cursor.is_over(layout.bounds());
        }
        let gesture_ended = match event {
            Event::Mouse(mouse::Event::ButtonReleased(_))
            | Event::Touch(
                iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. },
            ) => std::mem::take(&mut state.pressed_inside),
            Event::Mouse(mouse::Event::WheelScrolled { .. })
            | Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => true,
            _ => false,
        };
        if gesture_ended
            && matches!(self.presentation, Presentation::Animated(_))
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
        if matches!(
            self.presentation,
            Presentation::Preparing | Presentation::Waiting
        ) {
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
        // An entry shows live content: data that arrives while it plays (a
        // device name, the launcher's default action) appears at once. An
        // exit keeps the content it had when it began to leave.
        if !animation.closing {
            *image = None;
        }
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
                unpremultiply(rgba),
            ));
        }
        let offset = self.offset(state, bounds);
        // Grow from the edge the panel comes from, at its control.
        let zoom = animation.zoom(now);
        let origin = growth_origin(animation, bounds);
        // Snapped to whole device pixels, so text never shimmers as it settles.
        let snap = |value: f32| (value * animation.scale).round() / animation.scale;
        let scaled = Rectangle {
            x: snap(origin.x + (bounds.x - origin.x) * zoom + offset.x),
            y: snap(origin.y + (bounds.y - origin.y) * zoom + offset.y),
            width: snap(bounds.width * zoom),
            height: snap(bounds.height * zoom),
        };
        renderer.draw_image(
            iced::advanced::image::Image::new(image.as_ref().unwrap().clone())
                .opacity(animation.opacity(now)),
            scaled,
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
                    anchor: None,
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
            anchor: None,
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
    fn captures_return_to_straight_alpha() {
        // A 24% white rim over nothing, as captured, and opaque and clear pixels.
        let captured = vec![61, 61, 61, 61, 10, 20, 30, 255, 0, 0, 0, 0];
        assert_eq!(
            super::unpremultiply(captured),
            vec![255, 255, 255, 61, 10, 20, 30, 255, 0, 0, 0, 0]
        );
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
                anchor: None,
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
