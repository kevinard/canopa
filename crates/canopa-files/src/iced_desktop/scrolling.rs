//! Keyboard scrolling for read-only facts, preserving the focused action.
use super::*;
use iced::{
    advanced::widget::{Operation, operation},
    widget::{Id, scrollable},
};

struct PageScroll {
    target: iced::widget::Id,
    direction: f32,
}

impl Operation<Message> for PageScroll {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: iced::Rectangle,
        content_bounds: iced::Rectangle,
        _: iced::Vector,
        state: &mut dyn operation::Scrollable,
    ) {
        if id == Some(&self.target) {
            state.scroll_by(
                scrollable::AbsoluteOffset {
                    x: 0.0,
                    y: self.direction * (bounds.height - 32.0).max(32.0),
                },
                bounds,
                content_bounds,
            );
        }
    }
}

pub(super) fn key(target: Id, key: keyboard::Key, modifiers: keyboard::Modifiers) -> Task<Message> {
    if modifiers.is_empty() {
        use keyboard::key::Named;
        return match key {
            keyboard::Key::Named(Named::Home) => iced::widget::operation::scroll_to(
                target,
                scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(0.0),
                },
            ),
            keyboard::Key::Named(Named::End) => iced::widget::operation::snap_to_end(target),
            keyboard::Key::Named(Named::ArrowUp | Named::ArrowDown) => {
                iced::widget::operation::scroll_by(
                    target,
                    scrollable::AbsoluteOffset {
                        x: 0.0,
                        y: if key == keyboard::Key::Named(Named::ArrowUp) {
                            -32.0
                        } else {
                            32.0
                        },
                    },
                )
            }
            keyboard::Key::Named(Named::PageUp | Named::PageDown) => {
                iced::advanced::widget::operate(PageScroll {
                    target,
                    direction: if key == keyboard::Key::Named(Named::PageUp) {
                        -1.0
                    } else {
                        1.0
                    },
                })
            }
            _ => Task::none(),
        };
    }
    Task::none()
}
