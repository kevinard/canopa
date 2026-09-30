//! Panel touchpad gain and frame-paced movement through existing scroll widgets.
mod motion;
mod operation;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

const TOUCHPAD_SCROLL_MULTIPLIER: f32 = 3.0;

pub fn scroll_speed<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(ScrollSpeed {
        content: content.into(),
    })
}

struct ScrollSpeed<'a, Message> {
    content: Element<'a, Message>,
}

impl<Message> Widget<Message, Theme, Renderer> for ScrollSpeed<'_, Message> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<motion::Motion>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(motion::Motion::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
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
        let mut scrolled = false;
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            &mut self::operation::Observe {
                inner: operation,
                scrolled: &mut scrolled,
            },
        );
        // Keyboard reveal must take over immediately. Accessibility/layout
        // queries pass through without cancelling touchpad motion.
        if scrolled {
            tree.state.downcast_mut::<motion::Motion>().clear();
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
        let motion = tree.state.downcast_mut::<motion::Motion>();
        let content = self.content.as_widget_mut();
        let child = &mut tree.children[0];
        let scaled;
        let event = if let Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x, y },
        }) = event
        {
            let delta = Vector::new(*x, *y) * TOUCHPAD_SCROLL_MULTIPLIER;
            // Empty axis frames (including gesture end) carry no new distance.
            // Finish presenting the movement already received.
            if delta == Vector::ZERO {
                return;
            }
            let mut hit = operation::HitTest::new(cursor, delta, *viewport);
            content.operate(child, layout, renderer, &mut hit);
            if hit.can_scroll {
                motion.push(delta);
                shell.capture_event();
                shell.request_redraw();
                return;
            }
            motion.clear();
            scaled = Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels {
                    x: delta.x,
                    y: delta.y,
                },
            });
            &scaled
        } else {
            event
        };

        if matches!(
            event,
            Event::Keyboard(iced::keyboard::Event::KeyPressed { .. })
                | Event::Mouse(
                    mouse::Event::ButtonPressed(_)
                        | mouse::Event::CursorMoved { .. }
                        | mouse::Event::CursorLeft
                )
                | Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { .. }
                })
                | Event::Touch(_)
        ) {
            motion.clear();
        }

        let mut messages = Vec::new();
        let mut scroll_shell = Shell::new(&mut messages);
        if let Event::Window(iced::window::Event::RedrawRequested(now)) = event
            && motion.is_active()
        {
            let mut hit = operation::HitTest::new(cursor, motion.pending(), *viewport);
            content.operate(child, layout, renderer, &mut hit);
            if !hit.can_scroll {
                // Do not accumulate motion against a scroll limit.
                motion.clear();
            }
            let delta = motion.advance(*now);
            if delta != Vector::ZERO {
                content.update(
                    child,
                    &Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Pixels {
                            x: delta.x,
                            y: delta.y,
                        },
                    }),
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    &mut scroll_shell,
                    viewport,
                );
            }
            if motion.is_active() {
                shell.request_redraw();
            }
        }
        // A synthetic wheel event's capture status must not suppress the actual
        // redraw event (caret/scrollbar state also needs that event).
        content.update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
        shell.merge(scroll_shell, std::convert::identity);
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
