//! Keyboard reading of Center content without changing or invoking the selected action.
use super::*;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Event, Rectangle, Renderer, Size, Theme, mouse};

pub(super) fn wrap<'a>(
    content: Element<'a, Message>,
    id: iced::widget::Id,
    focused_slider: Option<iced::widget::Id>,
) -> Element<'a, Message> {
    Element::new(ReadingPanel {
        content,
        id,
        focused_slider,
    })
}

struct ReadingPanel<'a> {
    content: Element<'a, Message>,
    id: iced::widget::Id,
    focused_slider: Option<iced::widget::Id>,
}

#[derive(Default)]
struct FocusedSlider(Option<iced::widget::Id>);

impl Widget<Message, Theme, Renderer> for ReadingPanel<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<FocusedSlider>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(FocusedSlider::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
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
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        if matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) && tree.state.downcast_ref::<FocusedSlider>().0 != self.focused_slider
        {
            tree.state.downcast_mut::<FocusedSlider>().0 = self.focused_slider.clone();
            if let Some(target) = &self.focused_slider {
                let mut find = SliderPosition {
                    target: target.clone(),
                    scroll: self.id.clone(),
                    bounds: None,
                    viewport: None,
                };
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    layout,
                    renderer,
                    &mut find,
                );
                if let (Some(target), Some((viewport, translation))) = (find.bounds, find.viewport)
                {
                    let target = target - translation;
                    let delta = if target.y < viewport.y {
                        target.y - viewport.y
                    } else {
                        (target.y + target.height - viewport.y - viewport.height).max(0.0)
                    };
                    if delta != 0.0 {
                        use iced::advanced::widget::operation::scrollable::{
                            AbsoluteOffset, scroll_by,
                        };
                        self.content.as_widget_mut().operate(
                            &mut tree.children[0],
                            layout,
                            renderer,
                            &mut scroll_by(self.id.clone(), AbsoluteOffset { x: 0.0, y: delta }),
                        );
                        shell.request_redraw();
                    }
                }
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if !shell.is_event_captured()
            && let Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(key),
                modifiers,
                ..
            }) = event
            && modifiers.is_empty()
            && matches!(
                key,
                iced::keyboard::key::Named::PageUp
                    | iced::keyboard::key::Named::PageDown
                    | iced::keyboard::key::Named::Home
                    | iced::keyboard::key::Named::End
            )
        {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                &mut ScrollReading {
                    id: self.id.clone(),
                    key: *key,
                },
            );
            shell.capture_event();
            shell.request_redraw();
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
}

struct SliderPosition {
    target: iced::widget::Id,
    scroll: iced::widget::Id,
    bounds: Option<Rectangle>,
    viewport: Option<(Rectangle, iced::Vector)>,
}
impl Operation for SliderPosition {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle) {
        if id == Some(&self.target) {
            self.bounds = Some(bounds);
        }
    }
    fn scrollable(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: iced::Vector,
        _: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        if id == Some(&self.scroll) {
            self.viewport = Some((bounds, translation));
        }
    }
}

struct ScrollReading {
    id: iced::widget::Id,
    key: iced::keyboard::key::Named,
}

impl Operation for ScrollReading {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
    fn scrollable(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        content: Rectangle,
        _: iced::Vector,
        state: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        if id != Some(&self.id) {
            return;
        }
        use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, RelativeOffset};
        use iced::keyboard::key::Named;
        match self.key {
            Named::PageUp | Named::PageDown => state.scroll_by(
                AbsoluteOffset {
                    x: 0.0,
                    y: bounds.height
                        * if self.key == Named::PageDown {
                            0.8
                        } else {
                            -0.8
                        },
                },
                bounds,
                content,
            ),
            Named::Home | Named::End => state.snap_to(RelativeOffset {
                x: None,
                y: Some(if self.key == Named::Home { 0.0 } else { 1.0 }),
            }),
            _ => {}
        }
    }
}
