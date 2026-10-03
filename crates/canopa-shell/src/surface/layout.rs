use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{
    Clipboard, Layout, Renderer as _, Shell, Widget, layout, mouse, overlay, renderer,
};
use iced::widget::{column, row};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// Edge padding applied on all sides of the bar content area (px).
#[cfg(test)]
pub const BAR_PADDING: f32 = 4.0;

/// Gap between modules within a zone (px).
#[cfg(test)]
pub const MODULE_GAP: f32 = 4.0;

/// Render three zones in a column (vertical bar) or row (horizontal bar).
///
/// **Vertical** (`horizontal = false`, left/right bars):
/// - Start zone stacks downward; center is pinned to the bar surface midpoint; end stacks upward.
///
/// **Horizontal** (`horizontal = true`, top/bottom bars):
/// - Start zone aligns to the left; center is pinned to the bar surface midpoint; end aligns to the right.
///
/// `BAR_PADDING` is applied as padding on all four edges of the outer container.
/// `MODULE_GAP` separates modules within each zone.
/// Visible zone bounds are clipped so zones do not overlap on short outputs.
pub fn zone_layout<'a, Message: Clone + 'a>(
    start: Vec<Element<'a, Message>>,
    center: Vec<Element<'a, Message>>,
    end: Vec<Element<'a, Message>>,
    horizontal: bool,
    padding: iced::Padding,
    gap: f32,
) -> Element<'a, Message> {
    let zone = |items: Vec<Element<'a, Message>>| -> Element<'a, Message> {
        if horizontal {
            row(items)
                .spacing(gap)
                .height(Length::Fill)
                .clip(true)
                .into()
        } else {
            column(items)
                .spacing(gap)
                .width(Length::Fill)
                .clip(true)
                .into()
        }
    };

    CenteredZoneLayout::new(
        [zone(start), zone(center), zone(end)],
        horizontal,
        padding,
        gap,
    )
    .into()
}

struct CenteredZoneLayout<'a, Message> {
    zones: [Element<'a, Message>; 3],
    horizontal: bool,
    padding: iced::Padding,
    gap: f32,
}

impl<'a, Message> CenteredZoneLayout<'a, Message> {
    fn new(
        zones: [Element<'a, Message>; 3],
        horizontal: bool,
        padding: iced::Padding,
        gap: f32,
    ) -> Self {
        Self {
            zones,
            horizontal,
            padding,
            gap,
        }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for CenteredZoneLayout<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.zones.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.zones);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
        let content = content_size(size, self.padding);

        let child_limits = layout::Limits::new(Size::ZERO, content);
        let mut nodes: Vec<layout::Node> = self
            .zones
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(zone, tree)| zone.as_widget_mut().layout(tree, renderer, &child_limits))
            .collect();

        let main_lengths = nodes
            .iter()
            .map(|node| main_length(node.size(), self.horizontal))
            .collect::<Vec<_>>();
        let slots = axis_slots(
            main_length(content, self.horizontal),
            main_lengths[0],
            main_lengths[1],
            main_lengths[2],
            self.gap,
            center_shift(self.padding, self.horizontal),
        );

        for (node, slot) in nodes.iter_mut().zip(slots) {
            position_node(node, slot.origin, content, self.horizontal, self.padding);
        }

        layout::Node::with_children(size, nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.zones
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((zone, tree), layout)| {
                    zone.as_widget_mut()
                        .operate(tree, layout, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let visible = visible_rects(layout, self.horizontal, self.padding, self.gap);

        for (((zone, tree), layout), visible) in self
            .zones
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .zip(visible)
        {
            let cursor = if cursor.is_over(visible) {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };

            zone.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let visible = visible_rects(layout, self.horizontal, self.padding, self.gap);

        self.zones
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .zip(visible)
            .filter(|(_, visible)| cursor.is_over(*visible))
            .map(|(((zone, tree), layout), _)| {
                zone.as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(viewport) = layout.bounds().intersection(viewport) else {
            return;
        };
        let visible = visible_rects(layout, self.horizontal, self.padding, self.gap);

        for (((zone, tree), layout), visible) in self
            .zones
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .zip(visible)
        {
            let Some(viewport) = visible.intersection(&viewport) else {
                continue;
            };
            if viewport.width <= 0.0 || viewport.height <= 0.0 {
                continue;
            }

            let cursor = if cursor.is_over(visible) {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };

            renderer.with_layer(viewport, |renderer| {
                zone.as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, &viewport);
            });
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        overlay::from_children(
            &mut self.zones,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message> From<CenteredZoneLayout<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(layout: CenteredZoneLayout<'a, Message>) -> Self {
        Element::new(layout)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AxisSlot {
    origin: f32,
    visible_origin: f32,
    visible_length: f32,
}

fn content_size(size: Size, padding: iced::Padding) -> Size {
    Size::new(
        (size.width - padding.left - padding.right).max(0.0),
        (size.height - padding.top - padding.bottom).max(0.0),
    )
}

fn center_shift(p: iced::Padding, horizontal: bool) -> f32 {
    if horizontal {
        (p.right - p.left) / 2.0
    } else {
        (p.bottom - p.top) / 2.0
    }
}

fn main_length(size: Size, horizontal: bool) -> f32 {
    if horizontal { size.width } else { size.height }
}

fn cross_length(size: Size, horizontal: bool) -> f32 {
    if horizontal { size.height } else { size.width }
}

fn axis_slots(
    axis_length: f32,
    start_length: f32,
    center_length: f32,
    end_length: f32,
    gap: f32,
    center_shift: f32,
) -> [AxisSlot; 3] {
    let center_visible = center_length > 0.0;

    if center_visible {
        let center_origin = (axis_length - center_length) / 2.0 + center_shift;
        let center_end = center_origin + center_length;
        let start_limit = (center_origin - gap).max(0.0);
        let end_limit = (axis_length - center_end - gap).max(0.0);
        let start_visible = start_length.min(start_limit);
        let end_visible = end_length.min(end_limit);

        return [
            AxisSlot {
                origin: 0.0,
                visible_origin: 0.0,
                visible_length: start_visible,
            },
            AxisSlot {
                origin: center_origin,
                visible_origin: center_origin.max(0.0),
                visible_length: center_end.min(axis_length) - center_origin.max(0.0),
            },
            AxisSlot {
                origin: axis_length - end_length,
                visible_origin: axis_length - end_visible,
                visible_length: end_visible,
            },
        ];
    }

    let gap = if start_length > 0.0 && end_length > 0.0 {
        gap.min(axis_length)
    } else {
        0.0
    };
    let available = (axis_length - gap).max(0.0);
    let (start_visible, end_visible) = fit_edge_zones(start_length, end_length, available);

    [
        AxisSlot {
            origin: 0.0,
            visible_origin: 0.0,
            visible_length: start_visible,
        },
        AxisSlot {
            origin: axis_length / 2.0,
            visible_origin: axis_length / 2.0,
            visible_length: 0.0,
        },
        AxisSlot {
            origin: axis_length - end_length,
            visible_origin: axis_length - end_visible,
            visible_length: end_visible,
        },
    ]
}

fn fit_edge_zones(start_length: f32, end_length: f32, available: f32) -> (f32, f32) {
    if start_length + end_length <= available {
        return (start_length, end_length);
    }

    let mut start_reduction = (start_length + end_length - available) / 2.0;
    let mut end_reduction = start_reduction;

    if start_reduction > start_length {
        end_reduction += start_reduction - start_length;
        start_reduction = start_length;
    }
    if end_reduction > end_length {
        start_reduction += end_reduction - end_length;
        end_reduction = end_length;
    }

    (
        (start_length - start_reduction).max(0.0),
        (end_length - end_reduction).max(0.0),
    )
}

fn position_node(
    node: &mut layout::Node,
    origin: f32,
    content: Size,
    horizontal: bool,
    padding: iced::Padding,
) {
    let size = node.size();
    let cross_origin =
        (cross_length(content, horizontal) - cross_length(size, horizontal)).max(0.0) / 2.0;

    let point = if horizontal {
        Point::new(padding.left + origin, padding.top + cross_origin)
    } else {
        Point::new(padding.left + cross_origin, padding.top + origin)
    };

    node.move_to_mut(point);
}

fn visible_rects(
    layout: Layout<'_>,
    horizontal: bool,
    padding: iced::Padding,
    gap: f32,
) -> [Rectangle; 3] {
    let bounds = layout.bounds();
    let content = content_size(bounds.size(), padding);
    let axis_length = main_length(content, horizontal);
    let child_sizes = layout
        .children()
        .map(|child| child.bounds().size())
        .collect::<Vec<_>>();
    let slots = axis_slots(
        axis_length,
        main_length(child_sizes[0], horizontal),
        main_length(child_sizes[1], horizontal),
        main_length(child_sizes[2], horizontal),
        gap,
        center_shift(padding, horizontal),
    );

    slots.map(|slot| {
        if horizontal {
            Rectangle {
                x: bounds.x + padding.left + slot.visible_origin,
                y: bounds.y + padding.top,
                width: slot.visible_length.max(0.0),
                height: content.height,
            }
        } else {
            Rectangle {
                x: bounds.x + padding.left,
                y: bounds.y + padding.top + slot.visible_origin,
                width: content.width,
                height: slot.visible_length.max(0.0),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_padding_is_four() {
        assert_eq!(BAR_PADDING, 4.0);
    }

    #[test]
    fn module_gap_is_four() {
        assert_eq!(MODULE_GAP, 4.0);
    }

    #[test]
    fn center_slot_is_pinned_to_axis_midpoint() {
        let slots = axis_slots(100.0, 10.0, 20.0, 30.0, 4.0, 0.0);

        assert_eq!(slots[1].origin, 40.0);
        assert_eq!(slots[1].visible_origin, 40.0);
        assert_eq!(slots[1].visible_length, 20.0);
    }

    #[test]
    fn start_and_end_are_constrained_by_visible_center() {
        let slots = axis_slots(100.0, 60.0, 20.0, 60.0, 4.0, 0.0);

        assert_eq!(slots[0].origin, 0.0);
        assert_eq!(slots[0].visible_length, 36.0);
        assert_eq!(slots[1].origin, 40.0);
        assert_eq!(slots[1].visible_length, 20.0);
        assert_eq!(slots[2].origin, 40.0);
        assert_eq!(slots[2].visible_origin, 64.0);
        assert_eq!(slots[2].visible_length, 36.0);
    }

    #[test]
    fn absent_center_does_not_reserve_the_midpoint() {
        let slots = axis_slots(100.0, 70.0, 0.0, 20.0, 4.0, 0.0);

        assert_eq!(slots[0].visible_length, 70.0);
        assert_eq!(slots[2].visible_origin, 80.0);
        assert_eq!(slots[2].visible_length, 20.0);
    }

    #[test]
    fn absent_center_clips_start_and_end_at_collision_boundary() {
        let slots = axis_slots(100.0, 80.0, 0.0, 80.0, 4.0, 0.0);

        assert_eq!(slots[0].visible_length, 48.0);
        assert_eq!(slots[2].visible_origin, 52.0);
        assert_eq!(slots[2].visible_length, 48.0);
    }

    #[test]
    fn oversized_center_is_clipped_symmetrically_by_axis_bounds() {
        let slots = axis_slots(100.0, 20.0, 120.0, 20.0, 4.0, 0.0);

        assert_eq!(slots[0].visible_length, 0.0);
        assert_eq!(slots[1].origin, -10.0);
        assert_eq!(slots[1].visible_origin, 0.0);
        assert_eq!(slots[1].visible_length, 100.0);
        assert_eq!(slots[2].visible_length, 0.0);
    }
}
