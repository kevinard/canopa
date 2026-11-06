//! Locate a scroll viewport and distinguish state changes from read-only queries.
use iced::advanced::widget::{Id, Operation, operation};
use iced::{Rectangle, Vector, mouse};

pub(super) struct HitTest {
    pub(super) can_scroll: bool,
    cursor: mouse::Cursor,
    delta: Vector,
    offset: Vector,
    clip: Rectangle,
    child_scroll: Option<(Vector, Rectangle)>,
}

impl HitTest {
    pub(super) fn new(cursor: mouse::Cursor, delta: Vector, viewport: Rectangle) -> Self {
        Self {
            can_scroll: false,
            cursor,
            delta,
            offset: Vector::ZERO,
            clip: viewport,
            child_scroll: None,
        }
    }
}

impl Operation for HitTest {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        let previous = (self.offset, self.clip);
        if let Some((translation, bounds)) = self.child_scroll.take() {
            self.clip = self
                .clip
                .intersection(&(bounds - self.offset))
                .unwrap_or_default();
            self.offset += translation;
        }
        operate(self);
        (self.offset, self.clip) = previous;
    }

    fn scrollable(
        &mut self,
        _: Option<&Id>,
        bounds: Rectangle,
        content: Rectangle,
        translation: Vector,
        _: &mut dyn operation::Scrollable,
    ) {
        if self.cursor.is_over(bounds - self.offset) && self.cursor.is_over(self.clip) {
            let accepts = |delta: f32, offset: f32, extent: f32| {
                (delta < 0.0 && offset < extent) || (delta > 0.0 && offset > 0.0)
            };
            self.can_scroll |= accepts(self.delta.x, translation.x, content.width - bounds.width)
                || accepts(self.delta.y, translation.y, content.height - bounds.height);
        }
        self.child_scroll = Some((translation, bounds));
    }
}

pub(super) struct Observe<'a> {
    pub(super) inner: &'a mut dyn Operation,
    pub(super) scrolled: &'a mut bool,
}

impl Operation for Observe<'_> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        let scrolled = &mut *self.scrolled;
        self.inner
            .traverse(&mut |inner| operate(&mut Observe { inner, scrolled }));
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        self.inner.container(id, bounds);
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content: Rectangle,
        translation: Vector,
        state: &mut dyn operation::Scrollable,
    ) {
        self.inner.scrollable(
            id,
            bounds,
            content,
            translation,
            &mut ObserveScroll {
                inner: state,
                scrolled: self.scrolled,
            },
        );
    }
    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        self.inner.focusable(id, bounds, state);
    }
    fn text_input(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::TextInput,
    ) {
        self.inner.text_input(id, bounds, state);
    }
    fn text(&mut self, id: Option<&Id>, bounds: Rectangle, text: &str) {
        self.inner.text(id, bounds, text);
    }
    fn custom(&mut self, id: Option<&Id>, bounds: Rectangle, state: &mut dyn std::any::Any) {
        self.inner.custom(id, bounds, state);
    }
    fn finish(&self) -> operation::Outcome<()> {
        self.inner.finish()
    }
}

struct ObserveScroll<'a> {
    inner: &'a mut dyn operation::Scrollable,
    scrolled: &'a mut bool,
}

impl operation::Scrollable for ObserveScroll<'_> {
    fn snap_to(&mut self, offset: operation::scrollable::RelativeOffset<Option<f32>>) {
        *self.scrolled = true;
        self.inner.snap_to(offset);
    }
    fn scroll_to(&mut self, offset: operation::scrollable::AbsoluteOffset<Option<f32>>) {
        *self.scrolled = true;
        self.inner.scroll_to(offset);
    }
    fn scroll_by(
        &mut self,
        offset: operation::scrollable::AbsoluteOffset,
        bounds: Rectangle,
        content: Rectangle,
    ) {
        *self.scrolled = true;
        self.inner.scroll_by(offset, bounds, content);
    }
}
