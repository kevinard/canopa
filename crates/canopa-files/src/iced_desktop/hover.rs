//! A pointer-hover highlight drawn behind its content. Hovering only repaints:
//! it publishes no messages, so moving across a long listing rebuilds nothing.
use super::Message;
use iced::advanced::Renderer as _;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

pub(super) struct Hover<'a> {
    content: Element<'a, Message>,
    color: Color,
    radius: f32,
    enabled: bool,
    /// A wash drawn over the content, fading it: a row being dragged away.
    cover: Option<Color>,
}

/// `content` with a `color` fill behind it while the pointer is over it.
pub(super) fn hover<'a>(
    content: impl Into<Element<'a, Message>>,
    color: Color,
    radius: f32,
    enabled: bool,
) -> Element<'a, Message> {
    Element::new(Hover {
        content: content.into(),
        color,
        radius,
        enabled,
        cover: None,
    })
}

/// `content` faded under a `cover` wash, such as rows that are being dragged.
pub(super) fn faded<'a>(
    content: impl Into<Element<'a, Message>>,
    cover: Color,
    radius: f32,
) -> Element<'a, Message> {
    Element::new(Hover {
        content: content.into(),
        color: Color::TRANSPARENT,
        radius,
        enabled: false,
        cover: Some(cover),
    })
}

#[derive(Default)]
struct State {
    hovered: bool,
}

impl Widget<Message, Theme, Renderer> for Hover<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
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
        if let Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        | Event::Window(iced::window::Event::Unfocused) = event
        {
            let state = tree.state.downcast_mut::<State>();
            let hovered = self.enabled && cursor.is_over(layout.bounds());
            if state.hovered != hovered {
                state.hovered = hovered;
                shell.request_redraw();
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
        let bounds = layout.bounds();
        if self.enabled && cursor.is_over(bounds) && bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border::default().rounded(self.radius),
                    ..Default::default()
                },
                self.color,
            );
        }
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if let Some(cover) = self.cover {
            renderer.with_layer(*viewport, |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: iced::Border::default().rounded(self.radius),
                        ..Default::default()
                    },
                    cover,
                );
            });
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
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
