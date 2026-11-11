//! Read semantics, drawn text, keyboard focus and on-screen bounds from the
//! laid-out widget tree in one traversal.
use super::Node;
use super::widget::Report;
use iced::advanced::widget::{Id, Operation, operation};
use iced::{Rectangle, Vector};

pub(crate) enum Child {
    Node(usize),
    Text(String, Rectangle),
}

pub(crate) struct Collected<M> {
    pub(crate) node: Node<M>,
    pub(crate) bounds: Rectangle,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<Child>,
    /// The focus target: explicit, or the first focusable widget inside a control.
    pub(crate) focus: Option<Id>,
    pub(crate) focused: bool,
}

impl<M: Clone> Clone for Collected<M> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            bounds: self.bounds,
            parent: self.parent,
            children: self
                .children
                .iter()
                .map(|child| match child {
                    Child::Node(index) => Child::Node(*index),
                    Child::Text(text, bounds) => Child::Text(text.clone(), *bounds),
                })
                .collect(),
            focus: self.focus.clone(),
            focused: self.focused,
        }
    }
}

/// Everything the windows displayed at the time of one collection.
pub struct Snapshot<M> {
    pub(crate) nodes: Vec<Collected<M>>,
}

impl<M: Clone> Clone for Snapshot<M> {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
        }
    }
}

impl<M> std::fmt::Debug for Snapshot<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot({} nodes)", self.nodes.len())
    }
}

impl<M> Snapshot<M> {
    /// The keys of the top-level nodes, one per window.
    pub fn roots(&self) -> impl Iterator<Item = &str> {
        self.nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .filter_map(|node| node.node.key.as_deref())
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// The collecting operation. Run it through [`super::Bridge`], or directly
/// with [`operation::black_box`] in tests.
pub struct Collect<M> {
    nodes: Vec<Collected<M>>,
    stack: Vec<usize>,
    pending: Option<usize>,
    offset: Vector,
    pending_scroll: Option<Vector>,
}

impl<M> Default for Collect<M> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            stack: Vec::new(),
            pending: None,
            offset: Vector::ZERO,
            pending_scroll: None,
        }
    }
}

impl<M: Clone + Send + 'static> Collect<M> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn snapshot(&self) -> Snapshot<M> {
        Snapshot {
            nodes: self.nodes.clone(),
        }
    }
}

impl<M: Clone + Send + 'static> Operation<Snapshot<M>> for Collect<M> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Snapshot<M>>)) {
        let node = self.pending.take();
        let offset = self.offset;
        if let Some(translation) = self.pending_scroll.take() {
            // Scrolled content keeps its unscrolled layout; subtract the offset.
            self.offset += translation;
        }
        if let Some(node) = node {
            self.stack.push(node);
        }
        operate(self);
        if node.is_some() {
            self.stack.pop();
        }
        self.offset = offset;
    }

    fn container(&mut self, _: Option<&Id>, _: Rectangle) {
        self.pending = None;
    }

    fn scrollable(
        &mut self,
        _: Option<&Id>,
        _: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn operation::Scrollable,
    ) {
        self.pending = None;
        self.pending_scroll = Some(translation);
    }

    fn focusable(&mut self, id: Option<&Id>, _: Rectangle, state: &mut dyn operation::Focusable) {
        self.pending = None;
        let Some(&top) = self.stack.last() else {
            return;
        };
        let node = &mut self.nodes[top];
        if node.focus.is_none() && node.node.absorbs_text() {
            node.focus = id.cloned();
        }
        if state.is_focused() {
            node.focused = true;
        }
    }

    fn text_input(&mut self, _: Option<&Id>, _: Rectangle, _: &mut dyn operation::TextInput) {
        self.pending = None;
    }

    fn text(&mut self, _: Option<&Id>, bounds: Rectangle, text: &str) {
        self.pending = None;
        if let Some(&top) = self.stack.last()
            && !text.trim().is_empty()
        {
            self.nodes[top]
                .children
                .push(Child::Text(text.to_owned(), bounds - self.offset));
        }
    }

    fn custom(&mut self, _: Option<&Id>, bounds: Rectangle, state: &mut dyn std::any::Any) {
        self.pending = None;
        let Some(Report(node)) = state.downcast_mut::<Report<M>>() else {
            return;
        };
        let index = self.nodes.len();
        let parent = self.stack.last().copied();
        self.nodes.push(Collected {
            focus: node.focus.clone(),
            node: node.clone(),
            bounds: bounds - self.offset,
            parent,
            children: Vec::new(),
            focused: node.focused,
        });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(Child::Node(index));
        }
        self.pending = Some(index);
    }

    fn finish(&self) -> operation::Outcome<Snapshot<M>> {
        // Always report, so a bridge never waits on a collection that found nothing.
        operation::Outcome::Some(self.snapshot())
    }
}
