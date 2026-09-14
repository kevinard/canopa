//! Host-owned layout for Canopa's declarative model. No notification-domain keys here.
use canopa_panel_protocol::{InteractionTarget, PanelAction, RichBodySegment, StyledTextRun};
use iced::widget::{button, column, container, rich_text, scrollable, span, text};
use iced::{Element, Length};
use niribar::external_panel::canopa::CanopaSnapshot;

use super::{Message, messages::ExternalPanelEvent};

mod action_widget;

#[cfg(test)]
mod tests;

pub(super) fn render<'a>(snapshot: &'a CanopaSnapshot, provider: &'a str) -> Element<'a, Message> {
    let mut panel = column![].spacing(10);
    if let Some(error) = &snapshot.content.error {
        panel = panel.push(text(error.clone()).color(crate::theme::CRITICAL));
    }
    for action in &snapshot.content.actions {
        panel = panel.push(action_button(
            snapshot,
            provider,
            InteractionTarget::Content,
            action,
        ));
    }
    for section in &snapshot.content.sections {
        let mut group = column![text(section.title.clone()).size(17)].spacing(8);
        for control in &section.controls {
            group = group.push(text(control.label.clone()));
            for option in &control.options {
                let action = PanelAction {
                    key: control.key.clone(),
                    label: format!(
                        "{}{}",
                        if option.value == control.value {
                            "✓ "
                        } else {
                            ""
                        },
                        option.label
                    ),
                    value: Some(option.value.clone()),
                };
                group = group.push(action_button(
                    snapshot,
                    provider,
                    InteractionTarget::Section {
                        id: section.id.clone(),
                    },
                    &action,
                ));
            }
        }
        for item in &section.items {
            let mut card = column![text(item.title.clone()).size(16)].spacing(6);
            if let Some(body) = &item.rich_body {
                let mut runs = Vec::new();
                for segment in &body.segments {
                    match segment {
                        RichBodySegment::Text { text, style } => runs.push(StyledTextRun {
                            text: text.clone(),
                            style: *style,
                        }),
                        RichBodySegment::Image { image } => {
                            if !runs.is_empty() {
                                card = card.push(styled(std::mem::take(&mut runs)));
                            }
                            let canopa_panel_protocol::RichBodyImageSource::LocalPath { path } =
                                &image.source;
                            if let Some(handle) = snapshot.image(path) {
                                card = card.push(
                                    iced::widget::image(handle.clone())
                                        .width(Length::Fill)
                                        .height(160)
                                        .content_fit(iced::ContentFit::Contain),
                                );
                            } else {
                                card = card.push(text(
                                    image
                                        .alt
                                        .clone()
                                        .unwrap_or_else(|| "[image unavailable]".into()),
                                ));
                            }
                        }
                    }
                }
                if !runs.is_empty() {
                    card = card.push(styled(runs));
                }
            } else if let Some(body) = &item.styled_body {
                card = card.push(styled(body.runs.clone()));
            } else if let Some(body) = &item.body {
                card = card.push(text(body.clone()));
            }
            for metadata in &item.metadata {
                card = card.push(text(format!("{}: {}", metadata.label, metadata.value)).size(12));
            }
            for action in &item.actions {
                card = card.push(action_button(
                    snapshot,
                    provider,
                    InteractionTarget::Item {
                        id: item.id.clone(),
                    },
                    action,
                ));
            }
            group = group.push(
                container(card)
                    .padding(10)
                    .width(Length::Fill)
                    .style(iced::widget::container::rounded_box),
            );
        }
        panel = panel.push(group);
    }
    let mut header = column![text(snapshot.content.title.clone()).size(20)].spacing(6);
    if let (Some(id), Some(label)) = (snapshot.focused_action_id(), snapshot.focused_label()) {
        header = header.push(action_widget::preview(id, label, provider));
    } else {
        header = header.push(text("Tab / arrows: select · Enter: activate · Esc: close").size(12));
    }
    container(
        column![
            header,
            scrollable(panel.width(Length::Fill)).height(Length::Fill)
        ]
        .spacing(10),
    )
    .padding(10)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(crate::theme::BASE)),
        text_color: Some(crate::theme::TEXT),
        ..Default::default()
    })
    .into()
}

fn action_button<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
) -> Element<'a, Message> {
    action_widget::render(snapshot.action_id(target, action), &action.label, provider)
}

fn styled<'a>(runs: Vec<StyledTextRun>) -> Element<'a, Message> {
    let spans: Vec<iced::widget::text::Span<'static, (), iced::Font>> = runs
        .into_iter()
        .map(|run| {
            span(run.text)
                .font(iced::Font {
                    weight: if run.style.bold {
                        iced::font::Weight::Bold
                    } else {
                        iced::font::Weight::Normal
                    },
                    style: if run.style.italic {
                        iced::font::Style::Italic
                    } else {
                        iced::font::Style::Normal
                    },
                    ..iced::Font::DEFAULT
                })
                .underline(run.style.underline)
        })
        .collect();
    rich_text(spans).size(14).width(Length::Fill).into()
}
