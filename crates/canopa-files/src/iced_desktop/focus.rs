//! Form traversal is a widget operation inside the keyboard event. Native text
//! focus changes before the next event, including batched Tab/typing input.
use super::Message;
use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Id, Operation, Tree, operation, tree},
    },
    keyboard,
};

#[derive(Default)]
struct Snapshot {
    focused: Option<Id>,
    bounds: Vec<(Id, Rectangle)>,
    scrolls: Vec<(Id, Rectangle, Vector)>,
}
impl Operation for Snapshot {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if let Some(id) = id {
            self.bounds.push((id.clone(), bounds));
        }
    }
    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        state: &mut dyn operation::Focusable,
    ) {
        if let Some(id) = id {
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
            self.scrolls.push((id.clone(), bounds, translation));
        }
    }
}
impl Snapshot {
    fn offset(
        &self,
        target: &Id,
        scroll: &Id,
    ) -> Option<iced::widget::scrollable::AbsoluteOffset<Option<f32>>> {
        let (_, bounds) = self.bounds.iter().find(|(id, _)| id == target)?;
        let (_, viewport, offset) = self.scrolls.iter().find(|(id, _, _)| id == scroll)?;
        let axis = |start: f32, end: f32, view_start: f32, view_end: f32, offset: f32| {
            if start - offset < view_start {
                Some((start - view_start).max(0.0))
            } else if end - offset > view_end {
                Some((end - view_end).max(0.0))
            } else {
                None
            }
        };
        Some(iced::widget::scrollable::AbsoluteOffset {
            x: axis(
                bounds.x,
                bounds.x + bounds.width,
                viewport.x,
                viewport.x + viewport.width,
                offset.x,
            ),
            y: axis(
                bounds.y,
                bounds.y + bounds.height,
                viewport.y,
                viewport.y + viewport.height,
                offset.y,
            ),
        })
    }
}

fn advance(
    order: &[Id],
    reverse: bool,
    mut apply: impl FnMut(&mut dyn Operation),
) -> Option<Snapshot> {
    if order.is_empty() {
        return None;
    }
    let mut snapshot = Snapshot::default();
    apply(&mut snapshot);
    let current = snapshot
        .focused
        .as_ref()
        .and_then(|id| order.iter().position(|item| item == id));
    let next = if reverse {
        current.map_or(order.len() - 1, |i| (i + order.len() - 1) % order.len())
    } else {
        current.map_or(0, |i| (i + 1) % order.len())
    };
    apply(&mut operation::focusable::focus::<()>(order[next].clone()));
    snapshot.focused = Some(order[next].clone());
    Some(snapshot)
}

fn ensure_default(order: &[Id], default: Option<&Id>, mut apply: impl FnMut(&mut dyn Operation)) {
    let Some(default) = default else {
        return;
    };
    let mut snapshot = Snapshot::default();
    apply(&mut snapshot);
    if snapshot
        .focused
        .as_ref()
        .is_none_or(|id| !order.contains(id))
    {
        apply(&mut operation::focusable::focus::<()>(default.clone()));
    }
}

pub(super) fn group<'a>(
    content: impl Into<Element<'a, Message>>,
    order: Vec<Id>,
    reveal: Vec<(Id, Id)>,
    default: Option<Id>,
) -> Element<'a, Message> {
    group_with_shortcuts(content, order, reveal, default, vec![])
}

/// Form-owned accelerators run before native fields, synchronously clearing
/// their focus. An application-level key listener runs too late to stop text.
pub(super) fn group_with_shortcuts<'a>(
    content: impl Into<Element<'a, Message>>,
    order: Vec<Id>,
    reveal: Vec<(Id, Id)>,
    default: Option<Id>,
    shortcuts: Vec<(crate::interface::KeyChord, Message)>,
) -> Element<'a, Message> {
    Element::new(Frame {
        mode: Mode::Group {
            order,
            reveal,
            default,
            shortcuts,
            preserve_focus: false,
        },
        content: content.into(),
    })
}

/// Handle local search controls before an editor inserts their modified key,
/// preserving the native text focus, selection and caret.
pub(super) fn editing_shortcuts<'a>(
    content: impl Into<Element<'a, Message>>,
    shortcuts: Vec<(crate::interface::KeyChord, Message)>,
) -> Element<'a, Message> {
    Element::new(Frame {
        mode: Mode::Group {
            order: Vec::new(),
            reveal: Vec::new(),
            default: None,
            shortcuts,
            preserve_focus: true,
        },
        content: content.into(),
    })
}

/// Rebuild after browser transitions before queued input uses the old widgets.
/// Browser text fields keep their native focus and traversal behavior.
pub(super) fn transitions<'a>(
    content: impl Into<Element<'a, Message>>,
    shortcuts: Vec<(crate::interface::KeyChord, Message)>,
    input: Option<Id>,
) -> Element<'a, Message> {
    Element::new(Frame {
        mode: Mode::Transitions { shortcuts, input },
        content: content.into(),
    })
}

/// A preview controller owns keyboard focus. Iced sliders otherwise capture
/// Up/Down merely on hover and wheel changes do not emit their release message.
pub(super) fn pointer_slider<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(Frame {
        mode: Mode::PointerSlider,
        content: content.into(),
    })
}

pub(super) fn control<'a>(
    id: Id,
    message: Option<Message>,
    appearance: &Appearance,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    control_with_keyboard(id, message.clone(), message, appearance, content)
}

/// A focusable push button, named for assistive technologies by its drawn
/// text; its default action sends `message` as a click does.
pub(super) fn button<'a>(
    id: Id,
    message: Option<Message>,
    appearance: &Appearance,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    button_with_keyboard(id, message.clone(), message, appearance, content)
}

pub(super) fn button_with_keyboard<'a>(
    id: Id,
    message: Option<Message>,
    keyboard_message: Option<Message>,
    appearance: &Appearance,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    super::accessibility_ui::button(
        super::accessibility_ui::focus_key(&id),
        message.clone(),
        control_with_keyboard(id, message, keyboard_message, appearance, content),
    )
}

/// A focusable check box named by its drawn label.
pub(super) fn check_box<'a>(
    id: Id,
    checked: bool,
    message: Option<Message>,
    appearance: &Appearance,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    super::accessibility_ui::check_box(
        super::accessibility_ui::focus_key(&id),
        checked,
        message.clone(),
        control(id, message, appearance, content),
    )
}

pub(super) fn control_with_keyboard<'a>(
    id: Id,
    message: Option<Message>,
    keyboard_message: Option<Message>,
    appearance: &Appearance,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let style = appearance.style(UiKind::Button);
    let mut border = canopa_ui::frame(appearance, style).border;
    border.width = if style.custom_border {
        border.width.max(1.0)
    } else {
        2.0
    };
    border.color = canopa_ui::color(appearance.role(ColorRole::Focus));
    Element::new(Frame {
        mode: Mode::Control {
            id,
            message,
            keyboard_message,
            border,
        },
        content: content.into(),
    })
}

pub(super) fn activation_key(key: &keyboard::Key) -> bool {
    matches!(
        key,
        keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space)
    ) || key.as_ref() == keyboard::Key::Character(" ")
}

enum Mode {
    PointerSlider,
    Transitions {
        shortcuts: Vec<(crate::interface::KeyChord, Message)>,
        input: Option<Id>,
    },
    Group {
        order: Vec<Id>,
        reveal: Vec<(Id, Id)>,
        default: Option<Id>,
        shortcuts: Vec<(crate::interface::KeyChord, Message)>,
        preserve_focus: bool,
    },
    Control {
        id: Id,
        message: Option<Message>,
        keyboard_message: Option<Message>,
        border: Border,
    },
}
struct Frame<'a> {
    mode: Mode,
    content: Element<'a, Message>,
}
struct State {
    identity: Option<Id>,
    focused: bool,
    prepared_input: Option<Id>,
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
impl Frame<'_> {
    fn activate_shortcut(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        let shortcuts = match &self.mode {
            Mode::Group { shortcuts, .. } | Mode::Transitions { shortcuts, .. } => shortcuts,
            Mode::Control { .. } | Mode::PointerSlider => return false,
        };
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return false;
        };
        let Some(chord) = super::key_chord(key, *modifiers) else {
            return false;
        };
        let Some(message) = shortcuts
            .iter()
            .find(|(key, _)| *key == chord)
            .map(|(_, message)| message.clone())
        else {
            return false;
        };
        if !matches!(
            self.mode,
            Mode::Group {
                preserve_focus: true,
                ..
            }
        ) {
            tree.state.downcast_mut::<State>().prepared_input = None;
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                &mut operation::focusable::unfocus::<()>(),
            );
        }
        shell.publish(message);
        shell.invalidate_widgets();
        shell.capture_event();
        shell.request_redraw();
        true
    }

    fn identity(&self) -> Option<Id> {
        match &self.mode {
            Mode::Control { id, .. } => Some(id.clone()),
            _ => None,
        }
    }
}
impl Widget<Message, Theme, Renderer> for Frame<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            identity: self.identity(),
            focused: false,
            prepared_input: None,
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        if tree.state.downcast_ref::<State>().identity != self.identity() {
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
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        if let Mode::Transitions { input, .. } = &self.mode {
            let state = tree.state.downcast_mut::<State>();
            if state.prepared_input != *input {
                state.prepared_input = input.clone();
                if let Some(input) = input {
                    // Prepare a newly exposed field before deferred keys resume.
                    // An asynchronous focus/select task can lose the first character.
                    let content = self.content.as_widget_mut();
                    content.operate(
                        &mut tree.children[0],
                        Layout::new(&node),
                        renderer,
                        &mut operation::focusable::focus::<()>(input.clone()),
                    );
                    content.operate(
                        &mut tree.children[0],
                        Layout::new(&node),
                        renderer,
                        &mut operation::text_input::select_all::<()>(input.clone()),
                    );
                }
            }
        }
        if let Mode::Group { order, default, .. } = &self.mode {
            ensure_default(order, default.as_ref(), |operation| {
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    Layout::new(&node),
                    renderer,
                    operation,
                );
            });
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
        if let Mode::Control { id, .. } = &self.mode {
            operation.focusable(
                Some(id),
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
        if matches!(self.mode, Mode::Group { .. })
            && self.activate_shortcut(tree, event, layout, renderer, shell)
        {
            return;
        }
        match &self.mode {
            Mode::PointerSlider => {
                if matches!(
                    event,
                    Event::Keyboard(_) | Event::Mouse(mouse::Event::WheelScrolled { .. })
                ) {
                    return;
                }
            }
            Mode::Transitions { .. } => {}
            Mode::Group {
                order,
                reveal,
                default,
                ..
            } => {
                ensure_default(order, default.as_ref(), |operation| {
                    self.content.as_widget_mut().operate(
                        &mut tree.children[0],
                        layout,
                        renderer,
                        operation,
                    )
                });
                if let Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Tab),
                    modifiers,
                    ..
                }) = event
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.logo()
                    && !order.is_empty()
                {
                    let content = self.content.as_widget_mut();
                    if let Some(snapshot) = advance(order, modifiers.shift(), |operation| {
                        content.operate(&mut tree.children[0], layout, renderer, operation)
                    }) && let Some(target) = &snapshot.focused
                        && let Some((_, scroll)) = reveal.iter().find(|(id, _)| id == target)
                        && let Some(offset) = snapshot.offset(target, scroll)
                    {
                        content.operate(
                            &mut tree.children[0],
                            layout,
                            renderer,
                            &mut operation::scrollable::scroll_to::<()>(scroll.clone(), offset),
                        );
                    }
                    shell.capture_event();
                    shell.invalidate_layout();
                    shell.request_redraw();
                    return;
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
            }
            Mode::Control {
                message,
                keyboard_message,
                ..
            } => {
                let state = tree.state.downcast_mut::<State>();
                if matches!(
                    event,
                    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                ) {
                    state.focused = message.is_some() && cursor.is_over(layout.bounds());
                }
                if state.focused
                    && let Some(message) = keyboard_message
                    && let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) =
                        event
                    && modifiers.is_empty()
                    && activation_key(key)
                {
                    shell.publish(message.clone());
                    shell.invalidate_widgets();
                    shell.capture_event();
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
        if matches!(self.mode, Mode::Transitions { .. }) && !shell.is_event_captured() {
            // Native text editing keeps precedence over rebound browser commands.
            self.activate_shortcut(tree, event, layout, renderer, shell);
        }
        if matches!(self.mode, Mode::Group { .. } | Mode::Transitions { .. }) && !shell.is_empty() {
            // Widget messages can change the controls. Apply them and rebuild
            // before subsequent queued input traverses the interface.
            shell.invalidate_widgets();
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
        if let Mode::Control {
            message: Some(_),
            border,
            ..
        } = &self.mode
            && tree.state.downcast_ref::<State>().focused
        {
            use iced::advanced::Renderer as _;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: *border,
                    ..Default::default()
                },
                Color::TRANSPARENT,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_moves_focus_to_keep_before_the_first_event_and_preserves_discard_focus() {
        let ids: Vec<_> = ["name", "keep", "discard"].map(Id::from).into();
        let mut states = [
            State {
                identity: None,
                focused: true,
                prepared_input: None,
            },
            State {
                identity: None,
                focused: false,
                prepared_input: None,
            },
            State {
                identity: None,
                focused: false,
                prepared_input: None,
            },
        ];
        ensure_default(&ids[1..], Some(&ids[1]), |operation| {
            for (id, state) in ids.iter().zip(&mut states) {
                operation.focusable(Some(id), Rectangle::default(), state);
            }
        });
        assert!(!states[0].focused);
        assert!(states[1].focused);
        advance(&ids[1..], false, |operation| {
            for (id, state) in ids.iter().zip(&mut states) {
                operation.focusable(Some(id), Rectangle::default(), state);
            }
        });
        ensure_default(&ids[1..], Some(&ids[1]), |operation| {
            for (id, state) in ids.iter().zip(&mut states) {
                operation.focusable(Some(id), Rectangle::default(), state);
            }
        });
        assert!(states[2].focused);
    }

    #[test]
    fn rapid_tabs_change_native_focus_before_the_next_keyboard_event() {
        let ids: Vec<_> = ["chooser", "add", "delete", "name", "command"]
            .map(Id::from)
            .into();
        let mut states: Vec<_> = ids
            .iter()
            .map(|id| State {
                identity: Some(id.clone()),
                focused: false,
                prepared_input: None,
            })
            .collect();
        for _ in 0..4 {
            advance(&ids, false, |operation| {
                for (id, state) in ids.iter().zip(&mut states) {
                    operation.focusable(Some(id), Rectangle::default(), state);
                }
            });
        }
        // Ctrl+A/typing now sees Name focused synchronously, without an app
        // message, future, queued intent or focus-mutation acknowledgement.
        assert!(states[3].focused);
        assert_eq!(states.iter().filter(|state| state.focused).count(), 1);
        for _ in 0..2 {
            advance(&ids, true, |operation| {
                for (id, state) in ids.iter().zip(&mut states) {
                    operation.focusable(Some(id), Rectangle::default(), state);
                }
            });
        }
        assert!(states[1].focused);
    }

    #[test]
    fn traversal_wraps_and_clears_focus_outside_the_enabled_order() {
        let ids: Vec<_> = ["one", "two", "disabled"].map(Id::from).into();
        let mut states = [
            State {
                identity: None,
                focused: false,
                prepared_input: None,
            },
            State {
                identity: None,
                focused: false,
                prepared_input: None,
            },
            State {
                identity: None,
                focused: true,
                prepared_input: None,
            },
        ];
        advance(&ids[..2], true, |operation| {
            for (id, state) in ids.iter().zip(&mut states) {
                operation.focusable(Some(id), Rectangle::default(), state);
            }
        });
        assert!(states[1].focused);
        assert!(!states[2].focused);
        advance(&ids[..2], false, |operation| {
            for (id, state) in ids.iter().zip(&mut states) {
                operation.focusable(Some(id), Rectangle::default(), state);
            }
        });
        assert!(states[0].focused);
    }
}
