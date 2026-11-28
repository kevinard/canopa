use super::canopa_semantics::{
    Body, Key as SemanticKey, Node as SemanticNode, body as selected_body, metadata_label,
};
// Host-owned layout for Canopa's declarative model. No notification-domain keys here.
use canopa_panel_protocol::{
    InteractionTarget, PanelAction, PanelIcon, PanelMetadata, RichBodySegment, StyledTextRun,
};
use canopa_shell::external_panel::canopa::CanopaSnapshot;
use canopa_ui::metrics;
use iced::widget::{column, container, rich_text, row, span};
use iced::{Element, Length, Theme};

use super::{Message, messages::ExternalPanelEvent};

mod action_widget;
mod reading;
mod slider_widget;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn render<'a>(
    snapshot: &'a CanopaSnapshot,
    provider: &'a str,
    drawer: bool,
    ui: canopa_theme::ResolvedStyle,
) -> Element<'a, Message> {
    render_with_back(snapshot, provider, drawer, ui, false, false, false, 0.0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_with_back<'a>(
    snapshot: &'a CanopaSnapshot,
    provider: &'a str,
    drawer: bool,
    ui: canopa_theme::ResolvedStyle,
    back: bool,
    adaptive: bool,
    at_end: bool,
    content_width: f32,
) -> Element<'a, Message> {
    let height = if adaptive {
        Length::Shrink
    } else {
        Length::Fill
    };
    let reading_id = iced::widget::Id::from(format!("canopa-reading/{provider}"));
    let card_ui = crate::theme::ui(canopa_theme::UiKind::Card);
    let card_overrides =
        crate::theme::appearance().customization.ui[canopa_theme::UiKind::Card as usize];
    let mut panel = column![].spacing(if ui.custom_gap { ui.gap.px() } else { 16.0 });
    let mut focused_slider = None;
    if let Some(error) = &snapshot.content.error {
        panel = panel.push(semantic(
            snapshot,
            provider,
            &SemanticNode::Error,
            crate::theme::text(error.clone()).color(crate::theme::palette().danger),
        ));
    }
    // Actions on the content form a footer below the list: everyday ones lead
    // and the destructive ones (Clear all) sit apart at its end. View and
    // settings actions join the heading, as icon buttons where they name a
    // symbol the shell draws.
    let (secondary, primary): (Vec<&PanelAction>, Vec<&PanelAction>) = snapshot
        .content
        .actions
        .iter()
        .partition(|action| action.secondary);
    let chips = |actions: Vec<&PanelAction>| {
        row(actions
            .into_iter()
            .map(|action| chip_button(snapshot, provider, InteractionTarget::Content, action)))
        .spacing(5)
        .wrap()
        .vertical_spacing(5)
    };
    let footer = (!primary.is_empty()).then(|| {
        let (destructive, everyday): (Vec<&PanelAction>, Vec<&PanelAction>) =
            primary.into_iter().partition(|action| action.destructive);
        // A bar of its own below a hairline, like the launcher's command bar.
        column![
            iced::widget::rule::horizontal(1).style(|theme| iced::widget::rule::Style {
                color: canopa_ui::colors(theme).text.scale_alpha(0.12),
                radius: 0.into(),
                fill_mode: iced::widget::rule::FillMode::Full,
                snap: true,
            }),
            row![
                chips(everyday),
                iced::widget::Space::new().width(Length::Fill),
                chips(destructive),
            ]
            .spacing(5)
            .padding([0, 2])
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10)
    });
    for section in &snapshot.content.sections {
        let identifies_each_item = !section.items.is_empty()
            && section.items.iter().all(|item| {
                item.attribution
                    .as_ref()
                    .is_some_and(|source| source.label == section.title)
            });
        let mut group = column![].spacing(10);
        if !section.title.is_empty() && (!identifies_each_item || !section.controls.is_empty()) {
            let label = crate::theme::strong_text(section.title.clone())
                .size(crate::theme::label_size())
                .style(canopa_ui::muted);
            group = group.push(if let Some(icon) = &section.icon {
                Element::from(
                    row![source_icon(snapshot, Some(icon), &section.title, 24), label]
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
                    request_activation_token: false,
                    destructive: false,
                    pressed: None,
                    secondary: false,
                    primary: false,
                    symbol: None,
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
            let title_size = (crate::theme::body_size() + crate::theme::title_size()) / 2.0;
            // An attributed item names its source and moment on one line:
            // "Messages · 5 minutes ago". Further details stay below the body.
            let inline_metadata = item.attribution.as_ref().and_then(|_| {
                item.metadata
                    .iter()
                    .enumerate()
                    .find(|(_, metadata)| !metadata.value.is_empty())
            });
            let detail_label = |index, metadata: &PanelMetadata| {
                semantic(
                    snapshot,
                    provider,
                    &SemanticNode::Metadata(item.id.clone(), index),
                    crate::theme::text(metadata_label(metadata))
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted),
                )
            };
            let inline_slider_actions = adaptive
                && item.slider.is_some()
                && item.attribution.is_none()
                && !item.actions.is_empty()
                && !ui.custom_padding
                && card_overrides.padding.is_none()
                && crate::theme::text_width(&item.title, title_size)
                    + item
                        .actions
                        .iter()
                        .map(|action| {
                            crate::theme::text_width(&action.label, crate::theme::body_size())
                                + 30.0
                        })
                        .sum::<f32>()
                    + item
                        .metadata
                        .iter()
                        .filter(|entry| entry.value.is_empty())
                        .map(|entry| {
                            crate::theme::text_width(&entry.label, crate::theme::label_size())
                                + 20.0
                        })
                        .sum::<f32>()
                    + 54.0
                    <= content_width;
            let actions = || {
                let target = InteractionTarget::Item {
                    id: item.id.clone(),
                };
                row(item
                    .actions
                    .iter()
                    .map(|action| action_button(snapshot, provider, target.clone(), action)))
                .spacing(metrics::GAP)
                .wrap()
            };
            let attributed = item.attribution.is_some();
            // On a message card, a destructive item action is the card's
            // close control. Other actions keep their visible labels.
            let (removals, card_actions): (Vec<&PanelAction>, Vec<&PanelAction>) = if attributed {
                item.actions.iter().partition(|action| action.destructive)
            } else {
                (Vec::new(), item.actions.iter().collect())
            };
            let statuses = row(item
                .metadata
                .iter()
                .enumerate()
                .filter(|(_, metadata)| metadata.value.is_empty())
                .map(|(index, metadata)| {
                    // On a message card a status such as "Unread" is a quiet
                    // accent dot; its label stays the accessible name.
                    let marker = if item.attribution.is_some() {
                        container(
                            container(iced::widget::Space::new().width(8).height(8)).style(|_| {
                                container::Style::default()
                                    .background(crate::theme::palette().accent)
                                    .border(iced::Border::default().rounded(4))
                            }),
                        )
                        .padding([0, 2])
                    } else {
                        container(
                            crate::theme::strong_text(metadata_label(metadata))
                                .size(crate::theme::label_size())
                                .style(canopa_ui::link),
                        )
                        .padding([2, 6])
                        .style(|_| {
                            container::Style::default()
                                .background(crate::theme::accent_surface())
                                .border(iced::Border::default().rounded(6))
                        })
                    };
                    marker
                        .id(super::canopa_accessibility_bounds::widget_id(
                            &semantic_key(
                                snapshot,
                                provider,
                                &SemanticNode::Metadata(item.id.clone(), index),
                            ),
                        ))
                        .into()
                }))
            .spacing(5);
            let mut leading_icon: Option<Element<'a, Message>> = None;
            let mut card = column![].spacing(
                card_overrides
                    .gap
                    .map(|p| p.px())
                    .unwrap_or(if inline_metadata.is_some() { 4.0 } else { 6.0 }),
            );
            let title = crate::theme::strong_text(item.title.clone())
                .size(title_size)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .width(Length::Fill);
            if let Some(source) = &item.attribution {
                let attribution = container(
                    crate::theme::strong_text(source.label.clone())
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .id(super::canopa_accessibility_bounds::widget_id(
                    &semantic_key(
                        snapshot,
                        provider,
                        &SemanticNode::Attribution(item.id.clone()),
                    ),
                ));
                // An unread dot leads the source line, before the app's name.
                let mut identity = row![statuses, attribution].spacing(6).width(Length::Fill);
                if let Some((index, metadata)) = inline_metadata {
                    identity = identity.push(
                        container(
                            crate::theme::text(format!("· {}", metadata.value))
                                .size(crate::theme::label_size())
                                .style(canopa_ui::muted)
                                .wrapping(iced::widget::text::Wrapping::None),
                        )
                        .id(super::canopa_accessibility_bounds::widget_id(
                            &semantic_key(
                                snapshot,
                                provider,
                                &SemanticNode::Metadata(item.id.clone(), index),
                            ),
                        )),
                    );
                }
                let identity = identity.align_y(iced::Center);
                // As on banners, the source icon leads the whole message when
                // the card is wide enough; narrow cards keep it on the source line.
                let source_row = if content_width >= 300.0 {
                    leading_icon = Some(source_icon(
                        snapshot,
                        source.icon.as_ref(),
                        &source.label,
                        32,
                    ));
                    row![identity]
                } else {
                    row![
                        source_icon(snapshot, source.icon.as_ref(), &source.label, 24),
                        identity
                    ]
                }
                .spacing(8)
                .align_y(iced::Alignment::Center);
                // The close control keeps the trailing corner to itself.
                let mut source_row = source_row;
                for removal in &removals {
                    let id = snapshot.action_id(
                        InteractionTarget::Item {
                            id: item.id.clone(),
                        },
                        removal,
                    );
                    source_row = source_row.push(
                        container(action_widget::dismiss(
                            id.clone(),
                            provider,
                            snapshot.focused_action_id() == Some(id.as_str()),
                        ))
                        .id(
                            super::canopa_accessibility_bounds::widget_id(&semantic_key(
                                snapshot,
                                provider,
                                &SemanticNode::Action(id),
                            )),
                        ),
                    );
                }
                card = card.push(source_row);
                card = card.push(title);
            } else {
                let mut title_row = row![title, statuses.wrap()]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);
                if inline_slider_actions {
                    title_row = title_row.push(actions());
                }
                card = card.push(title_row);
            }
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
            let mut detail = column![].spacing(3);
            let mut has_detail = false;
            for (index, metadata) in item.metadata.iter().enumerate() {
                if metadata.value.is_empty()
                    || inline_metadata.is_some_and(|(header_index, _)| index == header_index)
                {
                    continue;
                }
                has_detail = true;
                detail = detail.push(detail_label(index, metadata));
            }
            if has_detail {
                card = card.push(detail);
            }
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
            // A message card's actions share one place, its trailing bottom
            // edge, so the source line names only who and when.
            if !inline_slider_actions && !card_actions.is_empty() {
                card = card.push(if item.attribution.is_some() {
                    // Message actions are compact chips at the trailing edge.
                    let target = InteractionTarget::Item {
                        id: item.id.clone(),
                    };
                    // The application's own actions (one of them primary) share
                    // the card's width like the buttons of its banner.
                    if card_actions.iter().any(|action| action.primary) {
                        Element::from(
                            container(
                                row(card_actions.iter().map(|action| {
                                    let id = snapshot.action_id(target.clone(), action);
                                    container(action_widget::wide(
                                        id.clone(),
                                        &action.label,
                                        provider,
                                        snapshot.focused_action_id() == Some(id.as_str()),
                                        if action.primary {
                                            canopa_ui::Action::Primary
                                        } else {
                                            canopa_ui::Action::Normal
                                        },
                                    ))
                                    .id(super::canopa_accessibility_bounds::widget_id(
                                        &semantic_key(
                                            snapshot,
                                            provider,
                                            &SemanticNode::Action(id),
                                        ),
                                    ))
                                    .width(Length::FillPortion(1))
                                    .into()
                                }))
                                .spacing(8)
                                .width(Length::Fill),
                            )
                            .width(Length::Fill)
                            .padding(iced::Padding {
                                top: 4.0,
                                ..Default::default()
                            }),
                        )
                    } else {
                        Element::from(
                            container(
                                row(card_actions.iter().map(|action| {
                                    if action.request_activation_token && card_actions.len() == 1 {
                                        action_button_with_treatment(
                                            snapshot,
                                            provider,
                                            target.clone(),
                                            action,
                                            canopa_ui::Action::Normal,
                                        )
                                    } else {
                                        chip_button(snapshot, provider, target.clone(), action)
                                    }
                                }))
                                .spacing(6)
                                .width(Length::Fill)
                                .wrap()
                                .vertical_spacing(6)
                                .align_x(canopa_ui::direction::end()),
                            )
                            .width(Length::Fill)
                            .padding(iced::Padding {
                                top: 2.0,
                                ..Default::default()
                            }),
                        )
                    }
                } else {
                    actions().into()
                });
            }
            group = group.push(
                container(
                    container(match leading_icon {
                        Some(icon) => {
                            Element::from(row![container(icon).width(40), card].spacing(8))
                        }
                        None => card.into(),
                    })
                    .id(super::canopa_accessibility_bounds::widget_id(
                        &semantic_key(snapshot, provider, &SemanticNode::Item(item.id.clone())),
                    ))
                    .padding(canopa_ui::content_padding(canopa_theme::ResolvedStyle {
                        padding: card_overrides
                            .padding
                            .unwrap_or(canopa_theme::Insets::all(12)),
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
    let title = crate::surface::panel::panel_header_with_controls(
        snapshot.content.title.clone(),
        back,
        secondary
            .into_iter()
            .map(|action| header_control(snapshot, provider, action))
            .collect(),
    );
    let mut header = column![title].spacing(6);
    if let (Some(id), Some(label)) = (snapshot.focused_action_id(), snapshot.focused_label())
        && snapshot.slider(id).is_none()
    {
        header = header.push(action_widget::preview(id, label, provider));
    }
    let mut body = column![
        header,
        // The list takes the height the footer leaves.
        container(
            crate::theme::panel_scroll(panel.width(Length::Fill))
                .id(reading_id.clone())
                .spacing(6)
                .height(height)
        )
        .height(if adaptive {
            Length::Shrink
        } else {
            Length::Fill
        })
    ]
    .spacing(10);
    if let Some(footer) = footer {
        body = body.push(footer);
    }
    let frame = reading::wrap(
        container(body)
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
            .height(height)
            .style(move |_| {
                let mut frame = canopa_ui::frame(&crate::theme::appearance(), ui);
                if drawer && !ui.custom_radius {
                    frame.border.radius = 0.0.into();
                }
                crate::theme::glass(frame, crate::theme::Glass::Panel)
            })
            .into(),
        reading_id,
        focused_slider,
    );
    // The same glass as every other panel: blurred behind its exact shape.
    let radius = if drawer && !ui.custom_radius {
        [canopa_theme::Pixels::ZERO; 4]
    } else {
        ui.radius
    };
    let frame = container(crate::theme::glass_region(frame, radius))
        .padding(canopa_ui::padding(ui.margin))
        .width(Length::Fill)
        .height(height)
        .into();
    super::view::panel_surface(frame, adaptive, at_end)
}

fn source_icon<'a>(
    snapshot: &CanopaSnapshot,
    icon: Option<&PanelIcon>,
    label: &str,
    size: u16,
) -> Element<'a, Message> {
    icon.and_then(|icon| snapshot.icon(icon))
        .map(|handle| {
            iced::widget::image(handle.clone())
                .width(f32::from(size))
                .height(f32::from(size))
                .content_fit(iced::ContentFit::Contain)
                .into()
        })
        .unwrap_or_else(|| {
            container(
                crate::theme::strong_text(
                    label
                        .trim()
                        .chars()
                        .next()
                        .map(|c| c.to_ascii_uppercase().to_string())
                        .unwrap_or_else(|| "?".into()),
                )
                .size(if size > 24 {
                    crate::theme::body_size() + 2.0
                } else {
                    crate::theme::label_size()
                })
                .style(canopa_ui::link),
            )
            .center(f32::from(size))
            .style(move |_| {
                container::Style::default()
                    .background(crate::theme::accent_surface())
                    .border(iced::Border::default().rounded(f32::from(size) / 2.0))
            })
            .into()
        })
}

/// A compact, rounded action shared by panel toolbars and message cards.
/// A view or settings action in the heading: an icon button where the shell
/// draws its symbol, else a chip.
fn header_control<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    action: &PanelAction,
) -> Element<'a, Message> {
    let Some(glyph) = action
        .symbol
        .as_deref()
        .and_then(action_widget::symbol_glyph)
    else {
        return chip_button(snapshot, provider, InteractionTarget::Content, action);
    };
    let id = snapshot.action_id(InteractionTarget::Content, action);
    container(action_widget::symbol_button(
        id.clone(),
        glyph,
        &action.label,
        provider,
        snapshot.focused_action_id() == Some(id.as_str()),
        action.pressed,
    ))
    .id(super::canopa_accessibility_bounds::widget_id(
        &semantic_key(snapshot, provider, &SemanticNode::Action(id)),
    ))
    .into()
}

fn chip_button<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
) -> Element<'a, Message> {
    let id = snapshot.action_id(target, action);
    container(action_widget::chip(
        id.clone(),
        &action.label,
        provider,
        snapshot.focused_action_id() == Some(id.as_str()),
        action.destructive,
        action.pressed,
        action.primary,
    ))
    .id(super::canopa_accessibility_bounds::widget_id(
        &semantic_key(snapshot, provider, &SemanticNode::Action(id)),
    ))
    .into()
}

fn action_button<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
) -> Element<'a, Message> {
    action_button_with_treatment(
        snapshot,
        provider,
        target,
        action,
        canopa_ui::Action::Normal,
    )
}

fn action_button_with_treatment<'a>(
    snapshot: &CanopaSnapshot,
    provider: &str,
    target: InteractionTarget,
    action: &PanelAction,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    // A switch looks like one wherever it is: with its on/off state.
    if action.pressed.is_some() {
        return chip_button(snapshot, provider, target, action);
    }
    let id = snapshot.action_id(target, action);
    container(action_widget::render(
        id.clone(),
        &action.label,
        provider,
        snapshot.focused_action_id() == Some(id.as_str()),
        treatment,
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
