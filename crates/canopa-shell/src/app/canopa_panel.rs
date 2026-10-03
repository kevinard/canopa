use super::canopa_semantics::{
    Body, Key as SemanticKey, Node as SemanticNode, body as selected_body, metadata_label,
};
// Host-owned layout for Canopa's declarative model. No notification-domain keys here.
use canopa_panel_protocol::{
    InteractionTarget, PanelAction, PanelIcon, RichBodySegment, StyledTextRun,
};
use canopa_shell::external_panel::canopa::CanopaSnapshot;
use canopa_ui::metrics;
use iced::widget::{Space, column, container, rich_text, row, scrollable, span, tooltip};
use iced::{Element, Length, Theme};

use super::{Message, messages::ExternalPanelEvent};

mod action_widget;
mod reading;
mod slider_widget;

#[cfg(test)]
mod tests;

pub(super) fn render<'a>(
    snapshot: &'a CanopaSnapshot,
    provider: &'a str,
    drawer: bool,
    ui: canopa_theme::ResolvedStyle,
) -> Element<'a, Message> {
    let reading_id = iced::widget::Id::from(format!("canopa-reading/{provider}"));
    let card_ui = crate::theme::ui(canopa_theme::UiKind::Card);
    let card_overrides =
        crate::theme::appearance().customization.ui[canopa_theme::UiKind::Card as usize];
    let mut panel = column![].spacing(if ui.custom_gap { ui.gap.px() } else { 20.0 });
    let mut focused_slider = None;
    if let Some(error) = &snapshot.content.error {
        panel = panel.push(semantic(
            snapshot,
            provider,
            &SemanticNode::Error,
            crate::theme::text(error.clone()).color(crate::theme::palette().danger),
        ));
    }
    if !snapshot.content.actions.is_empty() {
        panel = panel.push(action_row(
            snapshot,
            provider,
            InteractionTarget::Content,
            &snapshot.content.actions,
        ));
    }
    for section in &snapshot.content.sections {
        let identifies_each_item = !section.items.is_empty()
            && section.items.iter().all(|item| {
                item.attribution
                    .as_ref()
                    .is_some_and(|source| source.label == section.title)
            });
        let mut group = column![].spacing(10);
        if !section.title.is_empty() && (!identifies_each_item || !section.controls.is_empty()) {
            let label = crate::theme::text(section.title.clone())
                .size(crate::theme::label_size())
                .style(canopa_ui::muted);
            group = group.push(if let Some(icon) = &section.icon {
                Element::from(
                    row![source_icon(snapshot, Some(icon), &section.title), label]
                        .spacing(10)
                        .align_y(iced::Alignment::Center),
                )
            } else {
                label.into()
            });
        }
        for control in &section.controls {
            let mut options = row![].spacing(metrics::GAP);
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
                column![
                    crate::theme::text(control.label.clone()).size(crate::theme::body_size()),
                    options.wrap()
                ]
                .spacing(metrics::GAP),
            ));
        }
        for item in &section.items {
            let mut heading = row![
                crate::theme::text(item.title.clone())
                    .size(crate::theme::title_size())
                    .font(iced::Font {
                        weight: iced::font::Weight::Semibold,
                        ..crate::theme::font()
                    })
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Length::Fill)
            ]
            .spacing(metrics::GAP)
            .align_y(iced::Alignment::Center);
            for (index, metadata) in item.metadata.iter().enumerate() {
                if metadata.value.is_empty() {
                    let marker = container(container(Space::new().width(7).height(7)).style(
                        |theme: &Theme| container::Style {
                            background: canopa_ui::link(theme).color.map(Into::into),
                            border: iced::Border {
                                radius: 4.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ))
                    .padding(4)
                    .id(super::canopa_accessibility_bounds::widget_id(
                        &semantic_key(
                            snapshot,
                            provider,
                            &SemanticNode::Metadata(item.id.clone(), index),
                        ),
                    ));
                    heading = heading.push(
                        tooltip(
                            marker,
                            crate::theme::text(metadata_label(metadata)),
                            tooltip::Position::Bottom,
                        )
                        .style(crate::theme::card),
                    );
                }
            }
            let mut card =
                column![heading].spacing(card_overrides.gap.map(|p| p.px()).unwrap_or(10.0));
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
                                    action_widget::link(
                                        id.clone(),
                                        &action.label,
                                        *style,
                                        provider,
                                        snapshot.focused_action_id() == Some(id.as_str()),
                                    ),
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
                                        crate::theme::text(
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
                        crate::theme::text(body.to_owned()).size(crate::theme::body_size()),
                    ));
                }
                Body::None => {}
            }
            for (index, metadata) in item.metadata.iter().enumerate() {
                if metadata.value.is_empty() {
                    continue;
                }
                card = card.push(semantic(
                    snapshot,
                    provider,
                    &SemanticNode::Metadata(item.id.clone(), index),
                    crate::theme::text(metadata_label(metadata))
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted),
                ));
            }
            let mut card = if let Some(source) = &item.attribution {
                card = card.push(semantic(
                    snapshot,
                    provider,
                    &SemanticNode::Attribution(item.id.clone()),
                    crate::theme::text(source.label.clone())
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted),
                ));
                let icon = source_icon(snapshot, source.icon.as_ref(), &source.label);
                column![
                    row![icon, card.width(Length::Fill)]
                        .spacing(10)
                        .align_y(iced::Alignment::Start)
                ]
                .spacing(metrics::GAP)
            } else {
                card
            };
            if let Some(slider) = &item.slider {
                let id = snapshot.slider_id(&item.id, slider);
                if snapshot.focused_action_id() == Some(id.as_str()) {
                    focused_slider = Some((
                        serde_json::to_string(&(provider, &item.id, &slider.key))
                            .expect("control identity is serializable"),
                        super::canopa_accessibility_bounds::widget_id(&semantic_key(
                            snapshot,
                            provider,
                            &SemanticNode::Action(id.clone()),
                        )),
                    ));
                }
                card = card.push(semantic(
                    snapshot,
                    provider,
                    &SemanticNode::Action(id.clone()),
                    slider_widget::render(
                        id.clone(),
                        provider,
                        slider,
                        snapshot.focused_action_id() == Some(id.as_str()),
                    ),
                ));
            }
            if !item.actions.is_empty() {
                card = card.push(action_row(
                    snapshot,
                    provider,
                    InteractionTarget::Item {
                        id: item.id.clone(),
                    },
                    &item.actions,
                ));
            }
            group = group.push(
                container(
                    container(card)
                        .id(super::canopa_accessibility_bounds::widget_id(
                            &semantic_key(snapshot, provider, &SemanticNode::Item(item.id.clone())),
                        ))
                        .padding(canopa_ui::content_padding(canopa_theme::ResolvedStyle {
                            padding: card_overrides
                                .padding
                                .unwrap_or(canopa_theme::Insets::all(14)),
                            ..card_ui
                        }))
                        .width(Length::Fill)
                        .style(crate::theme::card),
                )
                .padding(canopa_ui::padding(card_ui.margin)),
            );
        }
        panel = panel.push(semantic(
            snapshot,
            provider,
            &SemanticNode::Section(section.id.clone()),
            group,
        ));
    }
    let title = crate::surface::panel::panel_header(snapshot.content.title.clone());
    let mut header = column![title].spacing(6);
    if let (Some(id), Some(label)) = (snapshot.focused_action_id(), snapshot.focused_label())
        && snapshot.slider(id).is_none()
    {
        header = header.push(action_widget::preview(id, label, provider));
    }
    let frame = reading::wrap(
        container(
            column![
                header,
                scrollable(panel.width(Length::Fill))
                    .id(reading_id.clone())
                    .spacing(6)
                    .height(Length::Fill)
            ]
            .spacing(10),
        )
        .id(super::canopa_accessibility_bounds::widget_id(
            &semantic_key(snapshot, provider, &SemanticNode::Root),
        ))
        .padding(iced::Padding {
            top: if ui.custom_padding {
                ui.padding.top.px()
            } else {
                canopa_ui::panel::HEADER_PADDING[0] as f32
            } + if ui.custom_border {
                ui.border_width.px()
            } else {
                0.0
            },
            ..canopa_ui::content_padding(ui)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            let mut frame = canopa_ui::frame(&crate::theme::appearance(), ui);
            if drawer && !ui.custom_radius {
                frame.border.radius = 0.0.into();
            }
            frame
        })
        .into(),
        reading_id,
        focused_slider,
    );
    container(frame)
        .padding(canopa_ui::padding(ui.margin))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn source_icon<'a>(
    snapshot: &CanopaSnapshot,
    icon: Option<&PanelIcon>,
    label: &str,
) -> Element<'a, Message> {
    icon.and_then(|icon| snapshot.icon(icon))
        .map(|handle| {
            iced::widget::image(handle.clone())
                .width(34)
                .height(34)
                .content_fit(iced::ContentFit::Contain)
                .into()
        })
        .unwrap_or_else(|| {
            container(
                crate::theme::text(
                    label
                        .trim()
                        .chars()
                        .next()
                        .map(|c| c.to_ascii_uppercase().to_string())
                        .unwrap_or_else(|| "?".into()),
                )
                .size(crate::theme::title_size()),
            )
            .center(34)
            .style(crate::theme::card)
            .into()
        })
}

fn action_row<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    actions: &[PanelAction],
) -> Element<'a, Message> {
    row(actions
        .iter()
        .map(|action| action_button(snapshot, provider, target.clone(), action)))
    .spacing(metrics::GAP)
    .wrap()
    .into()
}

fn action_button<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
) -> Element<'a, Message> {
    let id = snapshot.action_id(target, action);
    container(action_widget::render(
        id.clone(),
        &action.label,
        provider,
        snapshot.focused_action_id() == Some(id.as_str()),
    ))
    .id(super::canopa_accessibility_bounds::widget_id(
        &semantic_key(snapshot, provider, &SemanticNode::Action(id)),
    ))
    .into()
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
                    ..crate::theme::font()
                })
                .underline(run.style.underline)
        })
        .collect();
    rich_text(spans)
        .size(crate::theme::body_size())
        .width(Length::Fill)
        .style(move |theme: &Theme| {
            if link {
                canopa_ui::link(theme)
            } else {
                iced::widget::text::Style::default()
            }
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
