use super::canopa_semantics::{
    Body, Key as SemanticKey, Node as SemanticNode, body as selected_body,
};
// Host-owned layout for Canopa's declarative model. No notification-domain keys here.
use canopa_panel_protocol::{InteractionTarget, PanelAction, RichBodySegment, StyledTextRun};
use iced::widget::{button, column, container, rich_text, scrollable, span, text};
use iced::{Element, Length, Theme};
use niribar::external_panel::canopa::CanopaSnapshot;

use super::{Message, messages::ExternalPanelEvent};

mod action_widget;
mod reading;

#[cfg(test)]
mod tests;

pub(super) fn render<'a>(snapshot: &'a CanopaSnapshot, provider: &'a str) -> Element<'a, Message> {
    let reading_id = iced::widget::Id::from(format!("canopa-reading/{provider}"));
    let mut panel = column![].spacing(10);
    if let Some(error) = &snapshot.content.error {
        panel = panel.push(semantic(
            snapshot,
            provider,
            &SemanticNode::Error,
            text(error.clone()).color(crate::theme::CRITICAL),
        ));
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
            let mut options = column![text(control.label.clone())].spacing(8);
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
                options = options.push(action_button(
                    snapshot,
                    provider,
                    InteractionTarget::Section {
                        id: section.id.clone(),
                    },
                    &action,
                ));
            }
            group = group.push(semantic(
                snapshot,
                provider,
                &SemanticNode::Control(section.id.clone(), control.key.clone()),
                options,
            ));
        }
        for item in &section.items {
            let mut card = column![text(item.title.clone()).size(16)].spacing(6);
            match selected_body(item) {
                Body::Rich(body) => {
                    let mut runs = Vec::new();
                    let mut run_start = 0;
                    for (index, segment) in body.segments.iter().enumerate() {
                        match segment {
                            RichBodySegment::Text { text, style } => {
                                if runs.is_empty() {
                                    run_start = index;
                                }
                                runs.push(StyledTextRun {
                                    text: text.clone(),
                                    style: *style,
                                });
                            }
                            RichBodySegment::Link { action, style, .. } => {
                                if !runs.is_empty() {
                                    card = card.push(semantic(
                                        snapshot,
                                        provider,
                                        &SemanticNode::Body(item.id.clone(), Some(run_start)),
                                        styled(std::mem::take(&mut runs)),
                                    ));
                                }
                                let id = snapshot.action_id(
                                    InteractionTarget::Item {
                                        id: item.id.clone(),
                                    },
                                    action,
                                );
                                card = card.push(semantic(
                                    snapshot,
                                    provider,
                                    &SemanticNode::Action(id.clone()),
                                    action_widget::link(id, &action.label, *style, provider),
                                ));
                            }
                            RichBodySegment::Image { image } => {
                                if !runs.is_empty() {
                                    card = card.push(semantic(
                                        snapshot,
                                        provider,
                                        &SemanticNode::Body(item.id.clone(), Some(run_start)),
                                        styled(std::mem::take(&mut runs)),
                                    ));
                                }
                                let canopa_panel_protocol::RichBodyImageSource::LocalPath { path } =
                                    &image.source;
                                if let Some(handle) = snapshot.image(path) {
                                    card = card.push(semantic(
                                        snapshot,
                                        provider,
                                        &SemanticNode::Body(item.id.clone(), Some(index)),
                                        iced::widget::image(handle.clone())
                                            .width(Length::Fill)
                                            .height(160)
                                            .content_fit(iced::ContentFit::Contain),
                                    ));
                                } else {
                                    card = card.push(semantic(
                                        snapshot,
                                        provider,
                                        &SemanticNode::Body(item.id.clone(), Some(index)),
                                        text(
                                            image
                                                .alt
                                                .clone()
                                                .unwrap_or_else(|| "[image unavailable]".into()),
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                    if !runs.is_empty() {
                        card = card.push(semantic(
                            snapshot,
                            provider,
                            &SemanticNode::Body(item.id.clone(), Some(run_start)),
                            styled(runs),
                        ));
                    }
                }
                Body::Styled(body) => {
                    card = card.push(semantic(
                        snapshot,
                        provider,
                        &SemanticNode::Body(item.id.clone(), None),
                        styled(body.runs.clone()),
                    ));
                }
                Body::Plain(body) => {
                    card = card.push(semantic(
                        snapshot,
                        provider,
                        &SemanticNode::Body(item.id.clone(), None),
                        text(body.to_owned()),
                    ));
                }
                Body::None => {}
            }
            for (index, metadata) in item.metadata.iter().enumerate() {
                card = card.push(semantic(
                    snapshot,
                    provider,
                    &SemanticNode::Metadata(item.id.clone(), index),
                    text(format!("{}: {}", metadata.label, metadata.value)).size(12),
                ));
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
                    .id(super::canopa_accessibility_bounds::widget_id(
                        &semantic_key(snapshot, provider, &SemanticNode::Item(item.id.clone())),
                    ))
                    .padding(10)
                    .width(Length::Fill)
                    .style(iced::widget::container::rounded_box),
            );
        }
        panel = panel.push(semantic(
            snapshot,
            provider,
            &SemanticNode::Section(section.id.clone()),
            group,
        ));
    }
    let mut header = column![text(snapshot.content.title.clone()).size(20)].spacing(6);
    if let (Some(id), Some(label)) = (snapshot.focused_action_id(), snapshot.focused_label()) {
        header = header.push(action_widget::preview(id, label, provider));
    } else {
        header = header.push(
            text("Tab / arrows: select · Enter: activate · PgUp/PgDn: read · Esc: close").size(12),
        );
    }
    reading::wrap(
        container(
            column![
                header,
                scrollable(panel.width(Length::Fill))
                    .id(reading_id.clone())
                    .height(Length::Fill)
            ]
            .spacing(10),
        )
        .id(super::canopa_accessibility_bounds::widget_id(
            &semantic_key(snapshot, provider, &SemanticNode::Root),
        ))
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::BASE)),
            text_color: Some(crate::theme::TEXT),
            ..Default::default()
        })
        .into(),
        reading_id,
    )
}

fn action_button<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
) -> Element<'a, Message> {
    let id = snapshot.action_id(target, action);
    semantic(
        snapshot,
        provider,
        &SemanticNode::Action(id.clone()),
        action_widget::render(id, &action.label, provider),
    )
}

fn styled<'a>(runs: Vec<StyledTextRun>) -> Element<'a, Message> {
    styled_body(runs, false)
}

fn styled_body<'a>(runs: Vec<StyledTextRun>, link: bool) -> Element<'a, Message> {
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
    rich_text(spans)
        .size(14)
        .width(Length::Fill)
        .style(move |theme: &Theme| iced::widget::text::Style {
            color: link.then(|| {
                if theme.extended_palette().is_dark {
                    iced::Color::from_rgb8(117, 183, 255)
                } else {
                    iced::Color::from_rgb8(0, 85, 170)
                }
            }),
        })
        .into()
}

pub(super) fn semantic_key(
    snapshot: &CanopaSnapshot,
    provider: &str,
    node: &SemanticNode,
) -> SemanticKey {
    SemanticKey {
        provider: provider.to_owned(),
        generation: snapshot.generation(),
        node: node.clone(),
    }
}

fn semantic<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    key: &SemanticNode,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .id(super::canopa_accessibility_bounds::widget_id(
            &semantic_key(snapshot, provider, key),
        ))
        .width(Length::Fill)
        .into()
}
