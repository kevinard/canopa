//! Focus traversal happens inside the keyboard event, before queued text input.
use super::{Message, Page};
use iced::{
    Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Id, Operation, Tree, operation, tree},
    },
    keyboard,
};

pub fn group<'a>(
    content: impl Into<Element<'a, Message>>,
    dismiss: Option<Message>,
    context: String,
    default_focus: String,
    blocked: bool,
) -> Element<'a, Message> {
    Element::new(Frame {
        content: content.into(),
        id: None,
        activate: None,
        left: None,
        right: None,
        group: true,
        dismiss,
        context,
        default_focus: Some(Id::from(default_focus)),
        blocked,
        preserve_disabled: false,
    })
}
pub fn control<'a>(
    id: impl Into<String>,
    activate: Option<Message>,
    left: Option<Message>,
    right: Option<Message>,
    preserve_disabled: bool,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(Frame {
        content: content.into(),
        id: Some(Id::from(id.into())),
        activate,
        left,
        right,
        group: false,
        dismiss: None,
        context: String::new(),
        default_focus: None,
        blocked: false,
        preserve_disabled,
    })
}
#[derive(Default)]
struct State {
    focused: bool,
    id: Option<Id>,
    context: String,
    clear_focus: bool,
}
impl operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
    }
    fn unfocus(&mut self) {
        self.focused = false;
    }
}
struct Frame<'a> {
    content: Element<'a, Message>,
    id: Option<Id>,
    activate: Option<Message>,
    left: Option<Message>,
    right: Option<Message>,
    group: bool,
    dismiss: Option<Message>,
    context: String,
    default_focus: Option<Id>,
    blocked: bool,
    preserve_disabled: bool,
}
impl Frame<'_> {
    fn enabled(&self) -> bool {
        self.activate.is_some() || self.left.is_some() || self.right.is_some()
    }
}
#[derive(Default)]
struct Scan {
    order: Vec<Id>,
    focused: Option<Id>,
    bounds: Vec<(Id, Rectangle)>,
    scroll: Option<(Id, Rectangle, Vector)>,
}
impl Operation for Scan {
    fn traverse(&mut self, apply: &mut dyn FnMut(&mut dyn Operation)) {
        apply(self);
    }
    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        if let Some(id) = id {
            self.order.push(id.clone());
            self.bounds.push((id.clone(), bounds));
            if state.is_focused() {
                self.focused = Some(id.clone());
            }
        }
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn operation::Scrollable,
    ) {
        if let Some(id) = id {
            self.scroll = Some((id.clone(), bounds, translation));
        }
    }
}
impl Widget<Message, Theme, Renderer> for Frame<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            focused: false,
            id: self.id.clone(),
            context: self.context.clone(),
            clear_focus: false,
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if self.group && state.context != self.context {
            state.context = self.context.clone();
            state.clear_focus = true;
        }
        if state.id != self.id || (!self.enabled() && !self.preserve_disabled) {
            state.id = self.id.clone();
            state.focused = false;
        }
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
        if self.id.is_some() && self.enabled() {
            operation.focusable(
                self.id.as_ref(),
                layout.bounds(),
                tree.state.downcast_mut::<State>(),
            );
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
        if self.group {
            if self.blocked
                && matches!(
                    event,
                    Event::Keyboard(_) | Event::Mouse(_) | Event::Touch(_)
                )
            {
                shell.capture_event();
                return;
            }
            if tree.state.downcast_ref::<State>().clear_focus {
                tree.state.downcast_mut::<State>().clear_focus = false;
                let mut scan = Scan::default();
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    layout,
                    renderer,
                    &mut scan,
                );
                if scan.focused.is_some()
                    && let Some(target) = &self.default_focus
                {
                    self.content.as_widget_mut().operate(
                        &mut tree.children[0],
                        layout,
                        renderer,
                        &mut operation::focusable::focus::<()>(target.clone()),
                    );
                }
            }
            if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
                let page = match key.as_ref() {
                    keyboard::Key::Character("1") => Some(Page::Displays),
                    keyboard::Key::Character("2") => Some(Page::Input),
                    keyboard::Key::Character("3") => Some(Page::Personal),
                    keyboard::Key::Character("4") => Some(Page::Power),
                    keyboard::Key::Character("5") => Some(Page::Launcher),
                    _ => None,
                };
                let shortcut = if modifiers.control()
                    && self.dismiss.is_none()
                    && let Some(page) = page
                {
                    Some(Message::Page(page))
                } else if modifiers.control()
                    && self.dismiss.is_some()
                    && *key == keyboard::Key::Named(keyboard::key::Named::Enter)
                {
                    Some(Message::Decision(true))
                } else if modifiers.control() && key.as_ref() == keyboard::Key::Character("s") {
                    Some(Message::Save)
                } else if *key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.dismiss.clone()
                } else {
                    None
                };
                if let Some(message) = shortcut {
                    if let Message::Page(page) = &message {
                        self.content.as_widget_mut().operate(
                            &mut tree.children[0],
                            layout,
                            renderer,
                            &mut operation::focusable::focus::<()>(Id::from(format!(
                                "settings-button-{}",
                                page.label()
                            ))),
                        );
                    }
                    shell.publish(message);
                    shell.capture_event();
                    shell.invalidate_widgets();
                    return;
                }
                if *key == keyboard::Key::Named(keyboard::key::Named::Tab)
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo()
                {
                    let mut scan = Scan::default();
                    self.content.as_widget_mut().operate(
                        &mut tree.children[0],
                        layout,
                        renderer,
                        &mut scan,
                    );
                    if !scan.order.is_empty() {
                        let current = scan
                            .focused
                            .as_ref()
                            .and_then(|id| scan.order.iter().position(|v| v == id));
                        let index = if modifiers.shift() {
                            current.map_or(scan.order.len() - 1, |i| {
                                (i + scan.order.len() - 1) % scan.order.len()
                            })
                        } else {
                            current.map_or(0, |i| (i + 1) % scan.order.len())
                        };
                        let target = scan.order[index].clone();
                        self.content.as_widget_mut().operate(
                            &mut tree.children[0],
                            layout,
                            renderer,
                            &mut operation::focusable::focus::<()>(target.clone()),
                        );
                        if let (Some((scroll, view, offset)), Some((_, bounds))) = (
                            scan.scroll,
                            scan.bounds.iter().find(|(id, _)| *id == target),
                        ) {
                            let y = if bounds.y - offset.y < view.y {
                                Some((bounds.y - view.y).max(0.))
                            } else if bounds.y + bounds.height - offset.y > view.y + view.height {
                                Some((bounds.y + bounds.height - view.y - view.height).max(0.))
                            } else {
                                None
                            };
                            if let Some(y) = y {
                                self.content.as_widget_mut().operate(
                                    &mut tree.children[0],
                                    layout,
                                    renderer,
                                    &mut operation::scrollable::scroll_to::<()>(
                                        scroll,
                                        iced::widget::scrollable::AbsoluteOffset {
                                            x: None,
                                            y: Some(y),
                                        },
                                    ),
                                );
                            }
                        }
                    }
                    shell.capture_event();
                    shell.invalidate_widgets();
                    shell.request_redraw();
                    return;
                }
            }
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            ) {
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    layout,
                    renderer,
                    &mut operation::focusable::unfocus::<()>(),
                );
            }
        } else if self.enabled() {
            let state = tree.state.downcast_mut::<State>();
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            ) {
                state.focused = cursor.is_over(layout.bounds());
            }
            if state.focused
                && let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
                && modifiers.is_empty()
            {
                let message = match key {
                    keyboard::Key::Named(
                        keyboard::key::Named::Enter | keyboard::key::Named::Space,
                    ) => self.activate.clone(),
                    keyboard::Key::Character(value) if value == " " => self.activate.clone(),
                    keyboard::Key::Named(
                        keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                    ) => self.left.clone(),
                    keyboard::Key::Named(
                        keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                    ) => self.right.clone(),
                    _ => None,
                };
                if let Some(message) = message {
                    shell.publish(message);
                    shell.capture_event();
                    shell.invalidate_widgets();
                    shell.request_redraw();
                    return;
                }
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
        if !shell.is_empty() {
            shell.invalidate_widgets();
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if self.enabled() && tree.state.downcast_ref::<State>().focused {
            use iced::advanced::Renderer as _;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: iced::Border {
                        color: canopa_ui::colors(theme).focus,
                        width: 2.,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                },
                Color::TRANSPARENT,
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
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
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
