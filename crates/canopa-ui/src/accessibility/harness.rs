//! Lay out a view outside the application loop, to collect and operate on it.
use super::collect::{Collect, Snapshot};
use iced::advanced::widget::{Operation, Tree, operation};
use iced::advanced::{Layout, layout};
use iced::{Element, Size};

/// A laid-out view outside the application loop, to collect and operate on.
pub struct Harness<'a, M, Theme, Renderer> {
    element: Element<'a, M, Theme, Renderer>,
    tree: Tree,
    layout: layout::Node,
    renderer: &'a Renderer,
}

impl<'a, M, Theme, Renderer> Harness<'a, M, Theme, Renderer>
where
    M: Clone + Send + 'static,
    Renderer: iced::advanced::Renderer,
{
    pub fn new(
        element: impl Into<Element<'a, M, Theme, Renderer>>,
        renderer: &'a Renderer,
        size: Size,
    ) -> Self {
        let mut element = element.into();
        let mut tree = Tree::new(element.as_widget());
        let layout = element.as_widget_mut().layout(
            &mut tree,
            renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
        Self {
            element,
            tree,
            layout,
            renderer,
        }
    }

    /// Replace the view, keeping widget state (focus, scrolling) as Iced does.
    pub fn rebuild(&mut self, element: impl Into<Element<'a, M, Theme, Renderer>>, size: Size) {
        self.element = element.into();
        self.tree.diff(self.element.as_widget());
        self.layout = self.element.as_widget_mut().layout(
            &mut self.tree,
            self.renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
    }

    pub fn operate(&mut self, operation: &mut dyn Operation) {
        self.element.as_widget_mut().operate(
            &mut self.tree,
            Layout::new(&self.layout),
            self.renderer,
            operation,
        );
    }

    /// Move keyboard focus as [`super::Event::Focus`] does in an application.
    pub fn focus(&mut self, id: iced::widget::Id) {
        let mut focus = operation::focusable::focus::<()>(id);
        self.operate(&mut focus);
    }

    pub fn collect(&mut self) -> Snapshot<M> {
        let mut collect = Collect::<M>::new();
        self.operate(&mut operation::black_box(&mut collect));
        collect.snapshot()
    }
}
