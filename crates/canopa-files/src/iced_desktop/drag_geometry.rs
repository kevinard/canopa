//! Rendered drag regions and press ordering; no filesystem policy.
use super::{
    Message,
    drag_ui::{Event as DragEvent, Region, Scene},
};
use iced::advanced::Renderer as _;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay, renderer,
    widget::{Id, Operation, Tree, operation, tree},
};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};
use std::any::Any;

// Scrollable draws with a renderer translation while keeping its children's
// Layout in content coordinates. Its Operation supplies the exact translation
// and viewport, including nested scrollables and fractional offsets.
struct Collect {
    scene: Scene,
    translation: Vector,
    clip: Option<Rectangle>,
    scroll: Option<(Vector, Rectangle)>,
}
impl Operation for Collect {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        let previous = (self.translation, self.clip);
        if let Some((translation, bounds)) = self.scroll.take() {
            self.clip = self
                .clip
                .and_then(|clip| clip.intersection(&(bounds - self.translation)));
            self.translation += translation;
        }
        operate(self);
        (self.translation, self.clip) = previous;
    }

    fn scrollable(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        self.scroll = Some((translation, bounds));
    }

    fn custom(&mut self, _id: Option<&Id>, bounds: Rectangle, state: &mut dyn Any) {
        if let Some(region) = state.downcast_ref::<Region>()
            && let Some(clip) = self.clip
            && let Some(bounds) = (bounds - self.translation).intersection(&clip)
        {
            self.scene.record(region.clone(), bounds);
        }
    }
}

pub(super) fn root<'a>(
    window: iced::window::Id,
    scene: Scene,
    feedback: Option<Rectangle>,
    grabbing: bool,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(RegionWidget {
        content: content.into(),
        scene,
        marker: None,
        root: Some(window),
        feedback,
        grabbing,
    })
}
pub(super) fn mark<'a>(
    scene: Scene,
    region: Region,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(RegionWidget {
        content: content.into(),
        scene,
        marker: Some(region),
        root: None,
        feedback: None,
        grabbing: false,
    })
}
struct RegionWidget<'a> {
    content: Element<'a, Message>,
    scene: Scene,
    marker: Option<Region>,
    root: Option<iced::window::Id>,
    feedback: Option<Rectangle>,
    /// Items are being dragged from this window: the pointer grabs them.
    grabbing: bool,
}
#[derive(Default)]
struct State {
    pressed: Option<Point>,
    started: bool,
}

impl RegionWidget<'_> {
    fn collect(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        viewport: &Rectangle,
    ) {
        self.scene.begin();
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            &mut Collect {
                scene: self.scene.clone(),
                translation: Vector::ZERO,
                clip: layout.bounds().intersection(viewport),
                scroll: None,
            },
        );
        self.scene.commit();
    }
}

impl Widget<Message, Theme, Renderer> for RegionWidget<'_> {
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
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        if self.root.is_some() {
            // A rebuilt view can be drawn without receiving RedrawRequested.
            // Refresh its new regions/context here as well as after scrolling.
            self.collect(tree, Layout::new(&node), renderer, &node.bounds());
        }
        node
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(marker) = &mut self.marker {
            operation.custom(None, layout.bounds(), marker);
        }
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
        if let Some(window) = self.root {
            let state = tree.state.downcast_mut::<State>();
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    let source = cursor.position().and_then(|point| self.scene.source(point));
                    state.pressed = source.as_ref().and(cursor.position());
                    state.started = false;
                    // Publish before children can collapse an existing selection.
                    // Native Origins are consumed once by the global event listener,
                    // which also sees presses captured by an overlay.
                    shell.publish(Message::Drag(DragEvent::Press(window, source)));
                    // Menus a button opens start where it was pressed.
                    if let Some(position) = cursor.position() {
                        shell.publish(Message::Pointer(window, position));
                    }
                }
                Event::Mouse(mouse::Event::CursorMoved { position })
                    if !state.started
                        && state
                            .pressed
                            .is_some_and(|start| start.distance(*position) >= 6.0) =>
                {
                    state.started = true;
                    shell.publish(Message::Drag(DragEvent::Start(window)));
                }
                // Before rows and the folder publish the menu this press opens.
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    if let Some(position) = cursor.position() {
                        shell.publish(Message::Pointer(window, position));
                    }
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    state.pressed = None;
                    shell.publish(Message::Drag(DragEvent::Release(window)));
                }
                _ => {}
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
        if self.root.is_some()
            && matches!(
                event,
                Event::Window(iced::window::Event::RedrawRequested(_))
            )
        {
            self.collect(tree, layout, renderer, viewport);
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
        if self.grabbing {
            return mouse::Interaction::Grabbing;
        }
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
        if self.root.is_some()
            && let Some(bounds) = self.feedback
        {
            renderer.fill_quad(
                iced::advanced::renderer::Quad {
                    bounds,
                    border: iced::Border {
                        color: theme.palette().primary,
                        width: 2.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                iced::Color::TRANSPARENT,
            );
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
