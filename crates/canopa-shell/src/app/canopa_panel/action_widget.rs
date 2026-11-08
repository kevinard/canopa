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

pub(super) fn preview<'a>(id: &str, label: &str, provider: &str) -> Element<'a, Message> {
    action(
        id.to_owned(),
        provider,
        // Preserve all sender-controlled context without consuming the panel.
        container(crate::theme::panel_scroll(
            crate::theme::text(format!("Selected: {label}")).width(Length::Fill),
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
