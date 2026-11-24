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
        crate::theme::strong_text(label.to_owned())
            .size(crate::theme::label_size())
            .style(canopa_ui::link)
    } else {
        crate::theme::text(label.to_owned())
    };
    action(id, provider, label.into(), false, focused, treatment)
}

/// A pill-shaped toolbar action: label-sized text on a soft wash.
pub(super) fn chip<'a>(
    id: String,
    label: &str,
    provider: &str,
    focused: bool,
    destructive: bool,
    toggle: Option<bool>,
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
    let content = button(content)
        .padding([7, 8])
        .on_press(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        ))
        .style(move |theme, status| {
            let colors = canopa_ui::colors(theme);
            let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
            // Destructive actions use the danger tone.
            let (wash, text) = if destructive {
                (colors.danger, colors.danger)
            } else {
                (colors.text, colors.text)
            };
            button::Style {
                background: Some(
                    if pressed && !engaged {
                        crate::theme::accent_surface()
                    } else {
                        wash.scale_alpha(if engaged { 0.18 } else { 0.08 })
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
