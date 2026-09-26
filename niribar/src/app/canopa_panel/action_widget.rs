//! Action widgets whose scroll and press state belong to an opaque interaction ID.
use super::*;

pub(super) fn render<'a>(id: String, label: &str, provider: &str) -> Element<'a, Message> {
    action(id, provider, text(label.to_owned()).into(), false)
}

pub(super) fn preview<'a>(id: &str, label: &str, provider: &str) -> Element<'a, Message> {
    action(
        id.to_owned(),
        provider,
        // Preserve all sender-controlled context without consuming the panel.
        container(scrollable(
            text(format!("Selected: {label}")).width(Length::Fill),
        ))
        .width(Length::Fill)
        .max_height(54)
        .into(),
        false,
    )
}

pub(super) fn link<'a>(
    id: String,
    label: &str,
    style: canopa_panel_protocol::StyledTextStyle,
    provider: &str,
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
    action(id, provider, content, true)
}

fn action<'a>(
    id: String,
    provider: &str,
    content: Element<'a, Message>,
    link: bool,
) -> Element<'a, Message> {
    let content = button(content)
        .style(move |theme, status| {
            if link {
                iced::widget::button::text(theme, status)
            } else {
                iced::widget::button::primary(theme, status)
            }
        })
        .width(Length::Fill)
        .on_press(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        ));
    crate::surface::identity::identify(id, content)
}
