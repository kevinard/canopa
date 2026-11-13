//! The tab bar's tabs, which the pointer can drag.
//!
//! Where the window has native drag and drop, moving a pressed tab past a few
//! pixels asks for a native drag of it (see `drag_ui`'s tab drags): a picture
//! of the tab follows the pointer, also over other windows, and the window the
//! pointer is over shows a pin where the tab would land. The strip reports the
//! press and the start, records its tabs' places for those drops, and draws the
//! pin and the tab being dragged away; it moves nothing itself.
//!
//! Without native drag and drop, the strip carries the tab along the bar
//! itself, the other tabs making room, and releasing it moves the tab; a
//! release outside the window, which keeps the pointer while its button is
//! held, gives the tab a window of its own. Escape gives either drag up.
use super::{Message, drag_ui};
use iced::advanced::Renderer as _;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{
    Alignment, Color, Element, Event, Length, Padding, Point, Rectangle, Renderer, Size, Theme,
    Vector, keyboard, mouse, window,
};
use std::sync::{Arc, Mutex};

/// How far the pointer moves before a press on a tab becomes a drag.
const THRESHOLD: f32 = 6.0;
/// How far above and below the bar a dropped tab still counts as on it.
const BAR_MARGIN: f32 = 10.0;

/// Where a window's tabs were last drawn, for drops computed outside the view.
#[derive(Clone, Debug, Default)]
pub(super) struct TabGeometry {
    bar: Rectangle,
    /// Each tab's bounds, in visual order.
    slots: Vec<Rectangle>,
    /// The tab IDs in reading order.
    tabs: Vec<u64>,
    rtl: bool,
}

/// Where a tab dropped on a window goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Landing {
    /// The gap between drawn tabs, in visual order, that the pin marks.
    pub gap: usize,
    /// The reading-order position the tab takes.
    pub index: usize,
}

impl TabGeometry {
    /// The lower edge of the bar, where a tab's menu opens.
    pub fn bottom(&self) -> f32 {
        self.bar.y + self.bar.height
    }

    #[cfg(test)]
    pub(super) fn fixture(
        bar: Rectangle,
        slots: Vec<Rectangle>,
        tabs: Vec<u64>,
        rtl: bool,
    ) -> Self {
        Self {
            bar,
            slots,
            tabs,
            rtl,
        }
    }
    /// Where a tab dropped at `point` lands among a window's `count` tabs:
    /// on the bar, between the tabs it falls between; elsewhere in another
    /// window, after them. `dragged` is the tab when it comes from this
    /// window, whose own bar is the only place it moves to. `None`: it stays.
    pub(super) fn landing(
        &self,
        point: Point,
        dragged: Option<u64>,
        count: usize,
    ) -> Option<Landing> {
        let current = self.slots.len() == self.tabs.len() && self.tabs.len() == count;
        let bar = self.bar.expand(iced::Padding::from([BAR_MARGIN, 0.0]));
        let on_bar = current && !self.slots.is_empty() && bar.contains(point);
        let visual = |index: usize, count: usize| if self.rtl { count - index } else { index };
        let Some(dragged) = dragged else {
            if !on_bar {
                return Some(Landing {
                    gap: visual(count, count),
                    index: count,
                });
            }
            let gap = self.gap(point.x);
            return Some(Landing {
                gap,
                index: visual(gap, count),
            });
        };
        if !on_bar {
            return None;
        }
        let reading = self.tabs.iter().position(|tab| *tab == dragged)?;
        let slot = if self.rtl {
            count - 1 - reading
        } else {
            reading
        };
        let gap = self.gap(point.x);
        if gap == slot || gap == slot + 1 {
            return None;
        }
        let placed = gap - usize::from(slot < gap);
        Some(Landing {
            gap,
            index: if self.rtl { count - 1 - placed } else { placed },
        })
    }
    /// The gap at `x`: how many drawn tabs have their middle before it.
    fn gap(&self, x: f32) -> usize {
        self.slots.iter().filter(|slot| slot.center_x() < x).count()
    }
}

pub(super) struct TabStrip<'a> {
    /// In visual order: the tabs, and the new-tab button after them (before
    /// them right to left).
    children: Vec<Element<'a, Message>>,
    /// The tab IDs in reading order.
    tabs: Vec<u64>,
    rtl: bool,
    spacing: f32,
    window: window::Id,
    window_size: Size,
    /// Whether presses become native drags.
    native: bool,
    geometry: Arc<Mutex<TabGeometry>>,
    /// The gap a tab dragged over this window would land in.
    pin: Option<usize>,
    /// The tab being dragged away from this window.
    lifted: Option<u64>,
    colors: Colors,
}

/// The pin's colour, and the wash over a tab being dragged away.
#[derive(Clone, Copy)]
pub(super) struct Colors {
    pub pin: Color,
    pub cover: Color,
}

/// `tabs` (in reading order) and the `trailing` new-tab button as one
/// draggable strip of window `window`, which is `window_size` large.
#[allow(clippy::too_many_arguments)]
pub(super) fn tab_strip<'a>(
    tabs: Vec<(u64, Element<'a, Message>)>,
    trailing: Element<'a, Message>,
    spacing: f32,
    window: window::Id,
    window_size: Size,
    native: bool,
    geometry: Arc<Mutex<TabGeometry>>,
    pin: Option<usize>,
    lifted: Option<u64>,
    colors: Colors,
) -> Element<'a, Message> {
    let rtl = canopa_ui::direction::is_rtl();
    let (ids, mut children): (Vec<_>, Vec<_>) = tabs.into_iter().unzip();
    children.push(trailing);
    if rtl {
        children.reverse();
    }
    Element::new(TabStrip {
        children,
        tabs: ids,
        rtl,
        spacing,
        window,
        window_size,
        native,
        geometry,
        pin,
        lifted,
        colors,
    })
}

#[derive(Default)]
struct State {
    press: Option<Press>,
}

#[derive(Clone, Copy)]
struct Press {
    /// The pressed tab, as a visual slot among the tabs.
    slot: usize,
    origin: Point,
    pointer: Point,
    dragging: bool,
}

impl TabStrip<'_> {
    /// The visual index of the first tab among the children.
    fn first(&self) -> usize {
        usize::from(self.rtl)
    }
    fn slots(&self, layout: Layout<'_>) -> Vec<Rectangle> {
        layout
            .children()
            .skip(self.first())
            .take(self.tabs.len())
            .map(|child| child.bounds())
            .collect()
    }
    fn outside(&self, point: Point) -> bool {
        point.x < 0.0
            || point.y < 0.0
            || point.x > self.window_size.width
            || point.y > self.window_size.height
    }
    /// The reading-order index of visual slot `slot`.
    fn logical(&self, slot: usize) -> usize {
        if self.rtl {
            self.tabs.len() - 1 - slot
        } else {
            slot
        }
    }
    /// Whether the strip itself carries a pressed tab: only without native drags.
    fn carried(&self, press: Option<Press>) -> Option<Press> {
        press.filter(|press| press.dragging && !self.native)
    }
}

impl TabStrip<'_> {
    /// Ghost a tab being dragged away, leaving its outline.
    fn cover(&self, renderer: &mut Renderer, bounds: Rectangle) {
        renderer.with_layer(bounds.expand(1.0), |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border {
                        color: self.colors.pin,
                        width: 1.5,
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                },
                self.colors.cover,
            );
        });
    }
    /// A pin in gap `gap` between the drawn `slots`: where a dropped tab lands.
    fn draw_pin(
        &self,
        renderer: &mut Renderer,
        slots: &[Rectangle],
        gap: usize,
        viewport: Rectangle,
    ) {
        const WIDTH: f32 = 3.0;
        let x = match gap {
            0 => slots[0].x - self.spacing / 2.0,
            gap => {
                slots[(gap - 1).min(slots.len() - 1)].x
                    + slots[(gap - 1).min(slots.len() - 1)].width
                    + self.spacing / 2.0
            }
        };
        let first = slots[0];
        let bounds = Rectangle::new(
            Point::new(x - WIDTH / 2.0, first.y - 2.0),
            Size::new(WIDTH, first.height + 4.0),
        );
        renderer.with_layer(viewport, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border::default().rounded(WIDTH / 2.0),
                    ..Default::default()
                },
                self.colors.pin,
            );
        });
    }
}

/// How far the dragged tab is drawn from its place: the pointer's travel,
/// kept within the strip's first and last tabs.
fn travel(slots: &[Rectangle], dragged: usize, dx: f32) -> f32 {
    let (Some(first), Some(last)) = (slots.first(), slots.last()) else {
        return 0.0;
    };
    dx.clamp(first.x - slots[dragged].x, last.x - slots[dragged].x)
}

/// The visual slot the dragged tab would land in: it passes a tab once its
/// leading edge crosses that tab's middle, half a tab's travel.
fn landing(slots: &[Rectangle], dragged: usize, dx: f32) -> usize {
    let left = slots[dragged].x + travel(slots, dragged, dx);
    let right = left + slots[dragged].width;
    slots
        .iter()
        .enumerate()
        .filter(|&(slot, bounds)| {
            if slot < dragged {
                bounds.center_x() <= left
            } else {
                slot > dragged && bounds.center_x() < right
            }
        })
        .count()
}

/// Where the tab in visual slot `slot` is drawn while the tab in `dragged`
/// would land in `target`: the tabs between them move over by one.
fn shifted(slot: usize, dragged: usize, target: usize) -> usize {
    if slot < dragged && slot >= target {
        slot + 1
    } else if slot > dragged && slot <= target {
        slot - 1
    } else {
        slot
    }
}

impl Widget<Message, Theme, Renderer> for TabStrip<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
        // A tab closed or opened during a press moves the others under it.
        let state = tree.state.downcast_mut::<State>();
        if state
            .press
            .is_some_and(|press| press.slot >= self.tabs.len())
        {
            state.press = None;
        }
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
        layout::flex::resolve(
            layout::flex::Axis::Horizontal,
            renderer,
            limits,
            Length::Fill,
            Length::Shrink,
            Padding::ZERO,
            self.spacing,
            Alignment::Center,
            &mut self.children,
            &mut tree.children,
        )
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
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
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let slots = self.slots(layout);
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // The press still reaches the tab, which shows it.
                state.press = cursor.position().and_then(|point| {
                    let slot = slots.iter().position(|bounds| bounds.contains(point))?;
                    Some(Press {
                        slot,
                        origin: point,
                        pointer: point,
                        dragging: false,
                    })
                });
                if let Some(press) = state.press
                    && self.native
                {
                    shell.publish(Message::Drag(drag_ui::Event::TabPress(
                        self.window,
                        self.tabs[self.logical(press.slot)],
                        slots[press.slot],
                        press.origin - slots[press.slot].position(),
                    )));
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(press) = state.press.as_mut() {
                    press.pointer = *position;
                    if !press.dragging && press.origin.distance(*position) >= THRESHOLD {
                        press.dragging = true;
                        if self.native {
                            shell.publish(Message::Drag(drag_ui::Event::TabStart(
                                self.window,
                                self.tabs[self.logical(press.slot)],
                            )));
                        }
                    }
                    if press.dragging {
                        shell.request_redraw();
                        shell.capture_event();
                        return;
                    }
                }
            }
            // A native drag took the pointer: the window it is over takes the drop.
            Event::Mouse(mouse::Event::CursorLeft) => {
                let dragging = state.press.take().is_some_and(|press| press.dragging);
                if dragging {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(press) = state.press.take()
                    && press.dragging
                {
                    let tab = self.tabs[self.logical(press.slot)];
                    if self.outside(press.pointer) {
                        shell.publish(Message::DetachTab(self.window, tab));
                    } else {
                        let target = landing(&slots, press.slot, press.pointer.x - press.origin.x);
                        if target != press.slot {
                            shell.publish(Message::MoveTab(self.window, tab, self.logical(target)));
                        }
                    }
                    shell.request_redraw();
                    shell.capture_event();
                    return;
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if state.press.is_some_and(|press| press.dragging) => {
                state.press = None;
                shell.request_redraw();
                shell.capture_event();
                return;
            }
            Event::Window(window::Event::Unfocused) => {
                let dragging = state.press.take().is_some_and(|press| press.dragging);
                if dragging {
                    shell.request_redraw();
                }
            }
            _ => {}
        }
        for ((child, tree), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if tree
            .state
            .downcast_ref::<State>()
            .press
            .is_some_and(|press| press.dragging)
        {
            return mouse::Interaction::Grabbing;
        }
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
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
        let slots = self.slots(layout);
        if let Ok(mut geometry) = self.geometry.lock() {
            *geometry = TabGeometry {
                bar: layout.bounds(),
                slots: slots.clone(),
                tabs: self.tabs.clone(),
                rtl: self.rtl,
            };
        }
        let press = self.carried(tree.state.downcast_ref::<State>().press);
        let children = self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children());
        let Some(press) = press else {
            let lifted = self
                .lifted
                .and_then(|tab| self.tabs.iter().position(|candidate| *candidate == tab))
                .map(|reading| self.logical(reading));
            for (index, ((child, tree), layout)) in children.enumerate() {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
                if lifted.is_some_and(|slot| index == slot + self.first()) {
                    self.cover(renderer, layout.bounds());
                }
            }
            if let Some(gap) = self.pin.filter(|_| !slots.is_empty()) {
                self.draw_pin(renderer, &slots, gap, *viewport);
            }
            return;
        };
        // Hover follows the dragged tab, not the pointer's place on the others.
        let cursor = mouse::Cursor::Unavailable;
        let detaching = self.outside(press.pointer);
        let dx = press.pointer.x - press.origin.x;
        let target = if detaching {
            press.slot
        } else {
            landing(&slots, press.slot, dx)
        };
        let first = self.first();
        let mut dragged = None;
        for (index, ((child, tree), layout)) in children.enumerate() {
            let slot = index
                .checked_sub(first)
                .filter(|slot| *slot < self.tabs.len());
            let offset = match slot {
                Some(slot) if slot == press.slot => {
                    dragged = Some((child, tree, layout));
                    continue;
                }
                Some(slot) => slots[shifted(slot, press.slot, target)].x - slots[slot].x,
                None => 0.0,
            };
            renderer.with_translation(Vector::new(offset, 0.0), |renderer| {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
            });
        }
        let Some((child, tree, layout)) = dragged else {
            return;
        };
        let offset = if detaching {
            0.0
        } else {
            travel(&slots, press.slot, dx)
        };
        renderer.with_layer(*viewport, |renderer| {
            renderer.with_translation(Vector::new(offset, 0.0), |renderer| {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
                if detaching {
                    self.cover(renderer, layout.bounds());
                }
            });
        });
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots() -> Vec<Rectangle> {
        (0..4)
            .map(|index| {
                Rectangle::new(
                    Point::new(index as f32 * 104.0, 0.0),
                    Size::new(100.0, 28.0),
                )
            })
            .collect()
    }

    #[test]
    fn a_dragged_tab_lands_past_the_tabs_whose_middle_it_passed() {
        let slots = slots();
        assert_eq!(landing(&slots, 0, 10.0), 0);
        assert_eq!(landing(&slots, 0, 60.0), 1);
        assert_eq!(landing(&slots, 0, 1000.0), 3);
        assert_eq!(landing(&slots, 3, -60.0), 2);
        assert_eq!(landing(&slots, 3, -1000.0), 0);
        assert_eq!(landing(&slots, 1, -60.0), 0);
    }

    #[test]
    fn the_tabs_between_make_room_and_the_dragged_one_stays_on_the_bar() {
        // Tab 0 dragged to slot 2: tabs 1 and 2 move back one place.
        assert_eq!(shifted(1, 0, 2), 0);
        assert_eq!(shifted(2, 0, 2), 1);
        assert_eq!(shifted(3, 0, 2), 3);
        // Tab 3 dragged to slot 1: tabs 1 and 2 move on one place.
        assert_eq!(shifted(0, 3, 1), 0);
        assert_eq!(shifted(1, 3, 1), 2);
        assert_eq!(shifted(2, 3, 1), 3);
        let slots = slots();
        assert_eq!(travel(&slots, 1, -500.0), -104.0);
        assert_eq!(travel(&slots, 1, 500.0), 208.0);
    }
}
