//! Keep row interaction state attached to its original file path as a list changes.
//! Rebuild on identity changes and use Tree::diff for widget tag reconciliation.
use super::{Message, view::RowKey};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, mouse};

pub(super) fn keyed_row<'a>(
    key: RowKey,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(KeyedRow {
        key,
        content: content.into(),
    })
}

struct KeyedRow<'a> {
    key: RowKey,
    content: Element<'a, Message>,
}
struct State {
    key: RowKey,
}

impl Widget<Message, Theme, Renderer> for KeyedRow<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            key: self.key.clone(),
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        if tree.state.downcast_ref::<State>().key != self.key {
            *tree = Tree::new(self as &dyn Widget<Message, Theme, Renderer>);
        } else {
            tree.diff_children(std::slice::from_ref(&self.content));
        }
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
