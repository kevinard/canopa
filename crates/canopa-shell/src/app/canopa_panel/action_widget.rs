//! Action widgets whose scroll and press state belong to an opaque interaction ID.
use super::*;

pub(super) fn render<'a>(
    id: String,
    label: &str,
    provider: &str,
    focused: bool,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    let label = if matches!(treatment, canopa_ui::Action::Quiet) {
        // A card's secondary action stays in the background until wanted.
        crate::theme::text(label.to_owned())
            .size(crate::theme::label_size())
            .style(canopa_ui::muted)
    } else {
        crate::theme::text(label.to_owned())
    };
    action(id, provider, label.into(), false, focused, treatment)
}

/// A button sharing its row equally with the card's other actions, like the
/// buttons of a notification banner.
pub(super) fn wide<'a>(
    id: String,
    label: &str,
    provider: &str,
    focused: bool,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    use iced::widget::button;
    let primary = matches!(treatment, canopa_ui::Action::Primary);
    let content = button(
        iced::widget::container(
            crate::theme::strong_text(label.to_owned())
                .size(crate::theme::label_size() + 1.0)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .center_x(Length::Fill),
    )
    .on_press(Message::ExternalPanel(
        ExternalPanelEvent::ButtonActivated {
            provider_name: provider.to_owned(),
            element_id: id.clone(),
        },
    ))
    .padding([8, 10])
    .width(Length::Fill)
    .style(move |theme, status| {
        // The same buttons as on the banner: the main action filled, the
        // others tonal with a hairline edge.
        let colors = canopa_ui::colors(theme);
        let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let mut style = if primary {
            canopa_ui::action_style(theme, status, canopa_ui::Action::Primary, false)
        } else {
            button::Style {
                background: Some(
                    colors
                        .text
                        .scale_alpha(if engaged { 0.20 } else { 0.13 })
                        .into(),
                ),
                text_color: colors.text,
                ..Default::default()
            }
        };
        style.border = if focused {
            iced::Border {
                color: colors.focus,
                width: 2.0,
                radius: 12.0.into(),
            }
        } else {
            iced::Border {
                color: colors.text.scale_alpha(if primary { 0.0 } else { 0.18 }),
                width: if primary { 0.0 } else { 1.0 },
                radius: 12.0.into(),
            }
        };
        style
    });
    crate::surface::identity::identify(id, content)
}

/// A pill-shaped toolbar action: label-sized text on a soft wash.
pub(super) fn chip<'a>(
    id: String,
    label: &str,
    provider: &str,
    focused: bool,
    destructive: bool,
    toggle: Option<bool>,
    primary: bool,
) -> Element<'a, Message> {
    use iced::widget::button;
    let pressed = toggle == Some(true);
    let label = crate::theme::strong_text(label.to_owned())
        .size(crate::theme::label_size())
        .wrapping(iced::widget::text::Wrapping::Word);
    // A view or setting switch shows its state with a small switch.
    let content: Element<'a, Message> = match toggle {
        Some(on) => iced::widget::row![mini_switch(on), label]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .into(),
        None => label.into(),
    };
    // The main action is the easiest target on its card.
    let content = button(content)
        .padding(if primary { [8, 16] } else { [7, 12] })
        .on_press(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        ))
        .style(move |theme, status| {
            let colors = canopa_ui::colors(theme);
            let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
            // A destructive chip names its consequence in the danger tone at
            // rest, on the same neutral wash as the others, and takes a danger
            // wash under the pointer.
            let (wash, text) = match (destructive, engaged) {
                (true, true) => (colors.danger, colors.danger),
                (true, false) => (colors.text, colors.danger),
                _ => (colors.text, colors.text),
            };
            // The item's main action is filled with the accent.
            if primary && !destructive {
                let fill = colors.accent;
                return button::Style {
                    background: Some(
                        if engaged {
                            fill.scale_alpha(0.86)
                        } else {
                            fill
                        }
                        .into(),
                    ),
                    text_color: colors.on_accent,
                    border: iced::Border {
                        color: colors.focus,
                        width: if focused { 2.0 } else { 0.0 },
                        radius: 14.0.into(),
                    },
                    ..Default::default()
                };
            }
            button::Style {
                background: Some(
                    if pressed && !engaged {
                        crate::theme::accent_surface()
                    } else {
                        wash.scale_alpha(if engaged { 0.18 } else { 0.11 })
                    }
                    .into(),
                ),
                text_color: text,
                border: iced::Border {
                    color: colors.focus,
                    width: if focused { 2.0 } else { 0.0 },
                    radius: 14.0.into(),
                },
                ..Default::default()
            }
        });
    crate::surface::identity::identify(id, content)
}

fn mini_switch<'a>(on: bool) -> Element<'a, Message> {
    let knob = container(iced::widget::Space::new().width(8).height(8)).style(move |theme| {
        let colors = canopa_ui::colors(theme);
        container::Style::default()
            .background(if on { colors.on_accent } else { colors.text })
            .border(iced::Border::default().rounded(4))
    });
    container(knob)
        .width(22)
        .height(12)
        .padding(2)
        .align_x(if on {
            canopa_ui::direction::end()
        } else {
            canopa_ui::direction::start()
        })
        .align_y(iced::Alignment::Center)
        .style(move |theme| {
            let colors = canopa_ui::colors(theme);
            container::Style::default()
                .background(if on {
                    colors.accent
                } else {
                    colors.muted.scale_alpha(0.45)
                })
                .border(iced::Border::default().rounded(6))
        })
        .into()
}

/// A message card's round close control, the same one the banners use.
pub(super) fn dismiss<'a>(id: String, provider: &str, focused: bool) -> Element<'a, Message> {
    use iced::widget::button;
    let content = button(
        container(crate::theme::icon(
            crate::theme::CLOSE_ICON,
            14.0,
            crate::theme::palette().text,
        ))
        .center(28),
    )
    .padding(0)
    .on_press(Message::ExternalPanel(
        ExternalPanelEvent::ButtonActivated {
            provider_name: provider.to_owned(),
            element_id: id.clone(),
        },
    ))
    .style(move |theme, status| {
        let colors = canopa_ui::colors(theme);
        let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(
                colors
                    .text
                    .scale_alpha(if engaged { 0.18 } else { 0.10 })
                    .into(),
            ),
            border: iced::Border {
                color: colors.focus,
                width: if focused { 2.0 } else { 0.0 },
                radius: 14.0.into(),
            },
            ..Default::default()
        }
    });
    crate::surface::identity::identify(id, content)
}

pub(super) fn preview<'a>(id: &str, label: &str, provider: &str) -> Element<'a, Message> {
    action(
        id.to_owned(),
        provider,
        // Preserve all sender-controlled context without consuming the panel.
        container(crate::theme::panel_scroll(
            crate::theme::text(fl!("shell-provider-selected", label = label.to_string()))
                .width(Length::Fill),
        ))
        .width(Length::Fill)
        .max_height(54)
        .into(),
        false,
        true,
        canopa_ui::Action::Normal,
    )
}

pub(super) fn link<'a>(
    id: String,
    label: &str,
    style: canopa_panel_protocol::StyledTextStyle,
    provider: &str,
    focused: bool,
) -> Element<'a, Message> {
    let content = styled_body(
        vec![StyledTextRun {
            text: label.into(),
            style: canopa_panel_protocol::StyledTextStyle {
                underline: true,
                ..style
            },
        }],
        true,
    );
    action(
        id,
        provider,
        content,
        true,
        focused,
        canopa_ui::Action::Quiet,
    )
}

fn action<'a>(
    id: String,
    provider: &str,
    content: Element<'a, Message>,
    link: bool,
    focused: bool,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    let content = crate::theme::action(
        content,
        Some(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        )),
        treatment,
        focused,
    )
    .width(if link { Length::Fill } else { Length::Shrink });
    crate::surface::identity::identify(id, content)
}

/// The shell's glyph for a provider action's symbolic icon name, where it has one.
pub(super) fn symbol_glyph(name: &str) -> Option<&'static [u8]> {
    match name.strip_suffix("-symbolic").unwrap_or(name) {
        "preferences-system" | "emblem-system" => {
            Some(include_bytes!("../../../assets/settings.svg"))
        }
        "view-group" => Some(include_bytes!("../../../assets/group.svg")),
        "view-refresh" => Some(include_bytes!("../../../assets/refresh.svg")),
        _ => None,
    }
}

/// A view or settings action drawn as a round icon button beside the panel's
/// close button, its label the tooltip. A switch (`toggle`) keeps its label
/// beside the icon, so what it does and whether it is on read at a glance; on,
/// it takes the accent.
pub(super) fn symbol_button<'a>(
    id: String,
    glyph: &'static [u8],
    label: &str,
    provider: &str,
    focused: bool,
    toggle: Option<bool>,
) -> Element<'a, Message> {
    use iced::widget::button;
    let colors = crate::theme::palette();
    let pressed = toggle == Some(true);
    let icon = crate::theme::icon(
        glyph,
        16.0,
        if pressed { colors.accent } else { colors.text },
    );
    let inner: Element<'a, Message> = if toggle.is_some() {
        iced::widget::row![
            icon,
            crate::theme::strong_text(label.to_owned())
                .size(crate::theme::label_size())
                .color(if pressed { colors.accent } else { colors.text })
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        icon.into()
    };
    let content = button(container(inner).center_y(32).padding(if toggle.is_some() {
        [0, 12]
    } else {
        [0, 8]
    }))
    .padding(0)
    .height(32)
    .on_press(Message::ExternalPanel(
        ExternalPanelEvent::ButtonActivated {
            provider_name: provider.to_owned(),
            element_id: id.clone(),
        },
    ))
    .style(move |theme, status| {
        let colors = canopa_ui::colors(theme);
        let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let wash = if pressed {
            colors.accent.scale_alpha(if engaged { 0.30 } else { 0.20 })
        } else {
            colors.text.scale_alpha(if engaged { 0.16 } else { 0.07 })
        };
        button::Style {
            background: Some(wash.into()),
            text_color: colors.text,
            border: iced::Border {
                color: colors.focus,
                width: if focused { 2.0 } else { 0.0 },
                radius: 16.0.into(),
            },
            ..Default::default()
        }
    });
    let content = iced::widget::tooltip(
        content,
        crate::theme::text(label.to_owned()).size(crate::theme::label_size()),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(crate::theme::card)
    .padding(6)
    .gap(4);
    crate::surface::identity::identify(id, content)
}
