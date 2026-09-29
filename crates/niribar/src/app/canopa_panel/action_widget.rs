//! Action widgets whose scroll and press state belong to an opaque interaction ID.
use super::*;

pub(super) fn render<'a>(
    id: String,
    label: &str,
    provider: &str,
    focused: bool,
) -> Element<'a, Message> {
    action(
        id,
        provider,
        text(label.to_owned()).size(metrics::BODY).into(),
        false,
        focused,
    )
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
        true,
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
    action(id, provider, content, true, focused)
}

fn action<'a>(
    id: String,
    provider: &str,
    content: Element<'a, Message>,
    link: bool,
    focused: bool,
) -> Element<'a, Message> {
    let content = canopa_ui::action(
        content,
        Some(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        )),
        if link {
            canopa_ui::Action::Quiet
        } else {
            canopa_ui::Action::Normal
        },
        focused,
    )
    .width(if link { Length::Fill } else { Length::Shrink });
    crate::surface::identity::identify(id, content)
}
