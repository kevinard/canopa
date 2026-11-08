//! A numeric gesture belongs to one captured provider generation. Preview stays
//! local; release sends the final value once, even when events share a frame.
use super::*;
use canopa_panel_protocol::PanelSlider;
use canopa_shell::external_panel::canopa::slider_value_action;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, renderer,
    widget::{Operation, Tree, tree},
};
use iced::widget::column;
use iced::{Event, Rectangle, Renderer, Size, mouse};

pub(super) fn render<'a>(
    id: String,
    provider: &str,
    spec: &PanelSlider,
    focused: bool,
) -> Element<'a, Message> {
    crate::surface::identity::identify(
        id.clone(),
        Element::new(NumericSlider {
            id,
            provider: provider.into(),
            spec: spec.clone(),
            focused,
        }),
    )
}

struct NumericSlider {
    id: String,
    provider: String,
    spec: PanelSlider,
    focused: bool,
}
struct State {
    value: u16,
    pointer: bool,
    keyboard: bool,
    submitted: Option<u16>,
}
#[derive(Clone)]
enum Input {
    Preview(u16),
    Release,
}

impl NumericSlider {
    fn content(&self, value: u16) -> Element<'static, Input> {
        let focused = self.focused;
        container(
            column![
                row![
                    crate::theme::text(self.spec.label.clone())
                        .size(crate::theme::label_size())
                        .width(Length::Fill),
                    crate::theme::text(format!("{value}{}", self.spec.unit))
                        .size(crate::theme::body_size())
                ],
                iced::widget::slider(self.spec.min..=self.spec.max, value, Input::Preview)
                    .step(self.spec.step)
                    .height(24)
                    .style(|theme, status| {
                        let mut style = iced::widget::slider::default(theme, status);
                        style.rail.backgrounds.1 = canopa_ui::colors(theme).border.into();
                        style.rail.width = 5.0;
                        style
                    })
                    .on_release(Input::Release),
            ]
            .spacing(4),
        )
        .padding(4)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            border: iced::Border {
                color: if focused {
                    canopa_ui::colors(theme).accent
                } else {
                    iced::Color::TRANSPARENT
                },
                width: 1.0,
                radius: 4.into(),
            },
            ..Default::default()
        })
        .into()
    }
    fn commit(&self, state: &mut State, shell: &mut Shell<'_, Message>) {
        if state.value != state.submitted.unwrap_or(self.spec.value)
            && let Some(element_id) = slider_value_action(&self.id, state.value)
        {
            shell.publish(Message::ExternalPanel(
                ExternalPanelEvent::ButtonActivated {
                    provider_name: self.provider.clone(),
                    element_id,
                },
            ));
            state.submitted = Some(state.value);
        }
    }
}

impl Widget<Message, Theme, Renderer> for NumericSlider {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State {
            value: self.spec.value,
            pointer: false,
            keyboard: false,
            submitted: None,
        })
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content(self.spec.value).as_widget())]
    }
    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.keyboard && !self.focused {
            state.keyboard = false;
        }
        if !state.pointer && !state.keyboard && state.submitted.is_none() {
            state.value = self.spec.value;
        }
        let value = state.value;
        tree.diff_children(&[self.content(value)]);
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
        let value = tree.state.downcast_ref::<State>().value;
        self.content(value)
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
        let value = tree.state.downcast_ref::<State>().value;
        self.content(value).as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
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
        use iced::keyboard::{Event as KeyEvent, Key, key::Named};
        let state = tree.state.downcast_mut::<State>();
        if let Event::Keyboard(KeyEvent::KeyPressed {
            key: Key::Named(key),
            modifiers,
            repeat,
            ..
        }) = event
        {
            if self.focused && modifiers.is_empty() && !state.pointer && (!repeat || state.keyboard)
            {
                let step = self.spec.step;
                let next = match key {
                    Named::ArrowLeft | Named::ArrowDown => {
                        Some(state.value.saturating_sub(step).max(self.spec.min))
                    }
                    Named::ArrowRight | Named::ArrowUp => {
                        Some(state.value.saturating_add(step).min(self.spec.max))
                    }
                    Named::Home => Some(self.spec.min),
                    Named::End => Some(self.spec.max),
                    _ => None,
                };
                if let Some(value) = next {
                    state.value = value;
                    state.keyboard = true;
                    shell.capture_event();
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
            // The stock slider uses pointer hover for keyboard input. This host
            // instead routes keys exclusively to the explicitly focused control.
            return;
        }
        if let Event::Keyboard(KeyEvent::KeyReleased {
            key: Key::Named(key),
            ..
        }) = event
            && matches!(
                key,
                Named::ArrowLeft
                    | Named::ArrowRight
                    | Named::ArrowUp
                    | Named::ArrowDown
                    | Named::Home
                    | Named::End
            )
            && state.keyboard
        {
            if self.focused {
                self.commit(state, shell);
            }
            state.keyboard = false;
            shell.capture_event();
            return;
        }
        let mut messages = Vec::new();
        let mut local = Shell::new(&mut messages);
        self.content(state.value).as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut local,
            viewport,
        );
        if local.is_event_captured() {
            shell.capture_event();
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerPressed { .. })
            ) {
                state.pointer = true;
                shell.publish(Message::CanopaAccessibilityFocus {
                    provider: self.provider.clone(),
                    action_id: self.id.clone(),
                });
            }
        }
        shell.request_redraw_at(local.redraw_request());
        for message in messages {
            match message {
                Input::Preview(value) => {
                    state.value = value;
                    if matches!(event, Event::Mouse(mouse::Event::WheelScrolled { .. })) {
                        self.commit(state, shell);
                    }
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
                Input::Release if state.pointer => {
                    if !matches!(event, Event::Touch(iced::touch::Event::FingerLost { .. })) {
                        self.commit(state, shell);
                    }
                    state.pointer = false;
                    shell.capture_event();
                    shell.request_redraw();
                }
                Input::Release => {}
            }
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
        let value = tree.state.downcast_ref::<State>().value;
        self.content(value).as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
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
        let value = tree.state.downcast_ref::<State>().value;
        self.content(value).as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
}

#[cfg(test)]
mod tests;
