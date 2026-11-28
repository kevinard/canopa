//! Glass: translucent surfaces whose background the compositor blurs.
//!
//! [`region`] wraps the visible shape of a translucent surface. It reports the
//! shape's laid-out bounds and corner radii whenever they change, so the host
//! can ask the compositor (ext-background-effect-v1) to blur exactly that shape
//! rather than the whole layer surface with its transparent margins.
//! [`rectangles`] approximates a rounded shape by row-coalesced rectangles for
//! a Wayland region.
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// A rounded rectangle in surface-local logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub bounds: Rectangle,
    /// Top-left, top-right, bottom-right, bottom-left.
    pub radius: [f32; 4],
}

/// Report `content`'s laid-out shape with these corner radii through
/// `on_change`, or `None` while it is not `visible` (for example while it
/// fades out, so the blur does not outlast the surface's fill).
pub fn region<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    radius: [f32; 4],
    visible: bool,
    on_change: impl Fn(Option<Shape>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    Element::new(Region {
        content: content.into(),
        radius,
        visible,
        children: false,
        on_change: Box::new(move |shapes: Vec<Shape>| on_change(shapes.first().copied())),
    })
}

/// Like [`region`], for a container whose direct children are each a glass
/// card (a stack of notifications): one shape per laid-out child.
pub fn children<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    radius: [f32; 4],
    visible: bool,
    on_change: impl Fn(Vec<Shape>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    Element::new(Region {
        content: content.into(),
        radius,
        visible,
        children: true,
        on_change: Box::new(on_change),
    })
}

struct Region<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    radius: [f32; 4],
    visible: bool,
    children: bool,
    on_change: Box<dyn Fn(Vec<Shape>) -> Message + 'a>,
}

#[derive(Default)]
struct State {
    /// `None` before the first report.
    reported: Option<Vec<Shape>>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Region<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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
        if let Event::Window(iced::window::Event::RedrawRequested(_)) = event {
            let shape = |bounds| Shape {
                bounds,
                radius: self.radius,
            };
            let shapes: Vec<Shape> = if !self.visible {
                Vec::new()
            } else if self.children {
                layout
                    .children()
                    .map(|child| shape(child.bounds()))
                    .filter(|shape| shape.bounds.width > 0.0 && shape.bounds.height > 0.0)
                    .collect()
            } else {
                vec![shape(layout.bounds())]
            };
            let state = tree.state.downcast_mut::<State>();
            if state.reported.as_ref() != Some(&shapes) {
                shell.publish((self.on_change)(shapes.clone()));
                state.reported = Some(shapes);
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
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// Row-coalesced rectangles (x, y, width, height) covering `shape`'s rounded
/// rectangle; corners are cut where their arcs leave a row's centre line.
pub fn rectangles(shape: Shape) -> Vec<(i32, i32, i32, i32)> {
    let b = shape.bounds;
    if !(b.width > 0.0 && b.height > 0.0) {
        return Vec::new();
    }
    let r = shape
        .radius
        .map(|r| r.max(0.0).min(b.width / 2.0).min(b.height / 2.0));
    let inset = |radius: f32, distance: f32| {
        if distance >= radius || radius == 0.0 {
            0.0
        } else {
            radius
                - (radius * radius - (radius - distance).powi(2))
                    .max(0.0)
                    .sqrt()
        }
    };
    let top = b.y.round() as i32;
    let rows = (b.y + b.height).round() as i32 - top;
    let mut rectangles: Vec<(i32, i32, i32, i32)> = Vec::new();
    for row in 0..rows {
        let y = row as f32 + 0.5;
        let left = inset(r[0], y).max(inset(r[3], b.height - y));
        let right = inset(r[1], y).max(inset(r[2], b.height - y));
        let x = (b.x + left).round() as i32;
        let width = ((b.x + b.width - right).round() as i32 - x).max(0);
        if width == 0 {
            continue;
        }
        let y = top + row;
        if let Some(last) = rectangles.last_mut()
            && last.0 == x
            && last.2 == width
            && last.1 + last.3 == y
        {
            last.3 += 1;
        } else {
            rectangles.push((x, y, width, 1));
        }
    }
    rectangles
}

/// Union of several shapes' rectangles.
pub fn union(shapes: impl IntoIterator<Item = Shape>) -> Vec<(i32, i32, i32, i32)> {
    shapes.into_iter().flat_map(rectangles).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_shape_is_one_rectangle_and_round_corners_are_cut() {
        let square = Shape {
            bounds: Rectangle::new(iced::Point::new(12.0, 8.0), Size::new(64.0, 100.0)),
            radius: [0.0; 4],
        };
        assert_eq!(rectangles(square), vec![(12, 8, 64, 100)]);
        let round = Shape {
            radius: [16.0; 4],
            ..square
        };
        let rows = rectangles(round);
        // The middle is full width, the first row is inset by the corner arcs.
        assert!(rows.iter().any(|r| r.0 == 12 && r.2 == 64));
        assert!(rows[0].0 > 12 && rows[0].2 < 64);
        let covered: i32 = rows.iter().map(|r| r.3).sum();
        assert_eq!(covered, 100);
    }

    #[test]
    fn empty_shapes_have_no_rectangles() {
        let empty = Shape {
            bounds: Rectangle::new(iced::Point::ORIGIN, Size::new(0.0, 10.0)),
            radius: [4.0; 4],
        };
        assert!(rectangles(empty).is_empty());
    }
}
