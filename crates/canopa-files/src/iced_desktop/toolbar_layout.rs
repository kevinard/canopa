//! The location widget always occupies child 0. Wide/narrow adaptation changes
//! geometry, never the parent owning its native focus, selection and caret.
use super::Message;
use iced::{
    Alignment, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse,
    widget::{row, scrollable},
};
use std::{cell::Cell, rc::Rc};

const GAP: f32 = 5.0;
const ROW_GAP: f32 = 4.0;
const MIN_LOCATION: f32 = 144.0;

pub(super) fn required(commands: f32, menu: f32, slots: usize) -> f32 {
    commands + MIN_LOCATION + menu + GAP * slots as f32
}

#[derive(Clone, Copy)]
struct Metrics {
    commands: f32,
    before: f32,
    count: usize,
    menu: f32,
}
struct Plan {
    compact: bool,
    location: f32,
    location_x: f32,
    menu: f32,
    commands: f32,
}
impl Metrics {
    fn plan(self, width: f32) -> Plan {
        let required = required(self.commands, self.menu, self.count);
        let compact = width < required;
        let menu = self.menu.min(width);
        let location = if compact {
            (width - menu - GAP).max(0.0)
        } else {
            width - menu - self.commands - GAP * self.count as f32
        };
        Plan {
            compact,
            location,
            location_x: if compact { 0.0 } else { self.before },
            menu,
            commands: if compact {
                width
            } else {
                (width - menu - GAP).max(0.0)
            },
        }
    }
}

/// The single None item is the configured location slot. Command widths include
/// their existing Canopa padding; the location is drawn over an elastic spacer.
pub(super) fn primary<'a>(
    location: Element<'a, Message>,
    menu: Element<'a, Message>,
    menu_width: f32,
    items: Vec<(f32, Option<Element<'a, Message>>)>,
) -> Element<'a, Message> {
    let slot = items
        .iter()
        .position(|(_, item)| item.is_none())
        .expect("validated location slot");
    let metrics = Metrics {
        commands: items
            .iter()
            .filter(|(_, item)| item.is_some())
            .map(|(width, _)| width)
            .sum(),
        before: items.iter().take(slot).map(|(width, _)| width + GAP).sum(),
        count: items.len(),
        menu: menu_width,
    };
    let spacer = Rc::new(Cell::new(MIN_LOCATION));
    let commands = items.into_iter().fold(
        row![].spacing(GAP).align_y(Alignment::Center),
        |row, (_, item)| row.push(item.unwrap_or_else(|| Element::new(Spacer(spacer.clone())))),
    );
    let commands = scrollable(commands)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill);
    Element::new(Toolbar {
        children: vec![location, menu, commands.into()],
        metrics,
        spacer,
    })
}

struct Spacer(Rc<Cell<f32>>);
impl Widget<Message, Theme, Renderer> for Spacer {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.0.get()), Length::Shrink)
    }
    fn layout(&mut self, _: &mut Tree, _: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.resolve(self.0.get(), Length::Shrink, Size::ZERO))
    }
    fn draw(
        &self,
        _: &Tree,
        _: &mut Renderer,
        _: &Theme,
        _: &renderer::Style,
        _: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
    }
}

struct Toolbar<'a> {
    children: Vec<Element<'a, Message>>,
    metrics: Metrics,
    spacer: Rc<Cell<f32>>,
}
impl Widget<Message, Theme, Renderer> for Toolbar<'_> {
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let width = limits.max().width;
        let plan = self.metrics.plan(width);
        self.spacer.set(if plan.compact {
            // The location moves to the first row. Its old slot keeps commands
            // on either side together without changing configured ordering.
            (width - self.metrics.commands - GAP * (self.metrics.count - 1) as f32).max(0.0)
        } else {
            plan.location
        });
        let widths = [plan.location, plan.menu, plan.commands];
        let mut nodes: Vec<_> = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(widths)
            .map(|((child, state), width)| {
                child.as_widget_mut().layout(
                    state,
                    renderer,
                    &layout::Limits::new(Size::ZERO, Size::new(width, limits.max().height))
                        .width(width)
                        .height(Length::Shrink),
                )
            })
            .collect();
        let top = nodes[0].size().height.max(nodes[1].size().height);
        let height = if plan.compact {
            top + ROW_GAP + nodes[2].size().height
        } else {
            top.max(nodes[2].size().height)
        };
        let first_row = if plan.compact { top } else { height };
        let points = [
            Point::new(plan.location_x, (first_row - nodes[0].size().height) / 2.0),
            Point::new(
                width - plan.menu,
                (first_row - nodes[1].size().height) / 2.0,
            ),
            Point::new(
                0.0,
                if plan.compact {
                    top + ROW_GAP
                } else {
                    (height - nodes[2].size().height) / 2.0
                },
            ),
        ];
        for (node, point) in nodes.iter_mut().zip(points) {
            node.move_to_mut(point);
        }
        layout::Node::with_children(
            limits.resolve(Length::Fill, Length::Shrink, Size::new(width, height)),
            nodes,
        )
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        for ((child, state), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child
                .as_widget_mut()
                .operate(state, layout, renderer, operation);
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
        let layouts: Vec<_> = layout.children().collect();
        let over_field = cursor.is_over(layouts[0].bounds()) || cursor.is_over(layouts[1].bounds());
        for (index, ((child, state), layout)) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layouts)
            .enumerate()
        {
            child.as_widget_mut().update(
                state,
                event,
                layout,
                if index == 2 && over_field {
                    cursor.levitate()
                } else {
                    cursor
                },
                renderer,
                clipboard,
                shell,
                viewport,
            );
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
        let layouts: Vec<_> = layout.children().collect();
        let over_field = cursor.is_over(layouts[0].bounds()) || cursor.is_over(layouts[1].bounds());
        // Commands own a blank spacer under the independently positioned input.
        for index in [2, 0, 1] {
            self.children[index].as_widget().draw(
                &tree.children[index],
                renderer,
                theme,
                style,
                layouts[index],
                if index == 2 && over_field {
                    cursor.levitate()
                } else {
                    cursor
                },
                viewport,
            );
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let layouts: Vec<_> = layout.children().collect();
        let index = if cursor.is_over(layouts[0].bounds()) {
            0
        } else if cursor.is_over(layouts[1].bounds()) {
            1
        } else {
            2
        };
        self.children[index].as_widget().mouse_interaction(
            &tree.children[index],
            layouts[index],
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
        let overlays: Vec<_> = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter_map(|((child, state), layout)| {
                child
                    .as_widget_mut()
                    .overlay(state, layout, renderer, viewport, translation)
            })
            .collect();
        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_location_slot_and_command_budget_remain_consistent_at_both_widths() {
        let metrics = Metrics {
            commands: 220.0,
            before: 86.0,
            count: 6,
            menu: 37.0,
        };
        let wide = metrics.plan(1000.0);
        assert!(!wide.compact);
        assert_eq!(wide.location_x, 86.0);
        assert_eq!(
            wide.location + metrics.commands + GAP * (metrics.count - 1) as f32,
            wide.commands
        );
        assert_eq!(wide.commands + GAP + wide.menu, 1000.0);
        let narrow = metrics.plan(340.0);
        assert!(narrow.compact);
        assert_eq!(narrow.location_x, 0.0);
        assert_eq!(narrow.location + GAP + narrow.menu, 340.0);
        assert_eq!(narrow.commands, 340.0);
    }
}
