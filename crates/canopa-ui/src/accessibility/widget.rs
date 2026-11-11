//! A transparent wrapper reporting a [`Node`] to the collecting operation.
//!
//! It shares the wrapped widget's tree state, so adding or removing semantics
//! never resets focus, scrolling or typed drafts.
use super::Node;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Describe `content` with `node`.
///
/// For a node with nothing to draw (a live region announcing a count), wrap a
/// plain `space()`: `Row`/`Column::push` drops a child with a fixed zero width
/// or height, and the node with it.
pub fn semantic<'a, M, Theme, Renderer>(
    node: Node<M>,
    content: impl Into<Element<'a, M, Theme, Renderer>>,
) -> Element<'a, M, Theme, Renderer>
where
    M: Clone + Send + 'static,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    Element::new(Semantic {
        node: Report(node),
        content: content.into(),
    })
}

/// What the wrapper hands to [`Operation::custom`].
pub(crate) struct Report<M>(pub(crate) Node<M>);

struct Semantic<'a, M, Theme, Renderer> {
    node: Report<M>,
    content: Element<'a, M, Theme, Renderer>,
}

impl<M, Theme, Renderer> Widget<M, Theme, Renderer> for Semantic<'_, M, Theme, Renderer>
where
    M: Clone + Send + 'static,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }
    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }
    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }
    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.custom(None, layout.bounds(), &mut self.node);
        let content = &mut self.content;
        operation.traverse(&mut |operation| {
            content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        });
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
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
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
