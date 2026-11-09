use iced::widget::{Space, column, container, mouse_area, row};
use iced::{Element, Length};

use super::render::{BarRenderModel, PanelRenderModel, PopupRenderModel};
use super::{App, Message};
use crate::config::Edge;
use crate::surface::layout::zone_layout;

mod launcher;

impl App {
    pub(crate) fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        if let Some(perf) = &self.perf {
            perf.record_view(window);
        }
        if let Some(wallpaper) = self.wallpaper.view(window) {
            return wallpaper;
        }

        if self.foreground.is_dismissal_window(window) {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        }

        // Tray menus use compositor-managed popup grabs for outside dismissal.
        if let Some(model) = self.popup_render_model(window) {
            if let Some(session) = self
                .foreground
                .menu_session(window)
                .filter(|session| session.search_enabled())
            {
                return render_searchable_sni_popup_model(model, window, session);
            }
            let hovered_item = self.foreground.hovered_menu_item(window);
            let focused_item = self.foreground.focused_menu_item(window);
            return render_popup_model(
                model,
                window,
                hovered_item.as_deref(),
                focused_item.as_deref(),
            );
        }

        if let Some(popup) = self.surfaces.tooltip_popup(window) {
            return render_tooltip_popup(&popup.tree);
        }

        if let Some(model) = self.panel_render_model(window) {
            return crate::surface::panel_motion::wrap(
                canopa_ui::scroll_speed(render_panel_model(model)),
                window,
                self.foreground.panel_presentation(window),
            );
        }

        // Logical close precedes the asynchronous Wayland window removal. A
        // final redraw of an unregistered overlay must stay transparent; the
        // bar fallback would otherwise paint its entire former window.
        if self.surfaces.surface(window).is_none() {
            return Space::new().width(Length::Fill).height(Length::Fill).into();
        }

        let bar = render_bar_model(self.bar_render_model(window), window);
        if let Some(panel) = self.foreground.active_panel_window() {
            // Bars configured on the Overlay layer sit above the dismissal
            // surface. Consume their click before a module can activate.
            let dismiss = Message::Panel(super::messages::PanelEvent::Dismiss(panel));
            iced::widget::stack![
                bar,
                iced::widget::opaque(
                    mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                        .on_press(dismiss.clone())
                        .on_right_press(dismiss.clone())
                        .on_middle_press(dismiss)
                )
            ]
            .into()
        } else {
            bar
        }
    }

    pub(crate) fn style(&self, _theme: &iced::Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: crate::theme::palette().text,
        }
    }
}

fn render_panel_model<'a>(model: PanelRenderModel<'a>) -> Element<'a, Message> {
    let text_color = model.bar_style.resolve().color;
    if matches!(
        model.tree.content,
        crate::surface::panel::PanelContent::Menu(_)
            | crate::surface::panel::PanelContent::DmenuRequest
    ) {
        if model.confirmation && model.menu_entries.len() == 2 {
            return render_confirmation(model, text_color);
        }
        if model.launcher_session.is_some() {
            return launcher::view(model, text_color);
        }
        return render_panel_menu_model(model, text_color);
    }
    if let Some(content) = model.external_content {
        return render_external_panel_model(content, text_color, model);
    }

    let mut entries = model.entries;
    let is_bar_panel = matches!(
        model.tree.content,
        crate::surface::panel::PanelContent::BarPanel
    );
    let footer_entries = entries.split_off(model.footer_start);
    let mut padding = canopa_ui::padding(model.ui.padding);
    if is_bar_panel && !model.ui.custom_padding {
        padding.top = 4.0;
        padding.bottom = 4.0;
    } else if !model.drawer && !model.ui.custom_padding {
        padding.top = 8.0;
        padding.bottom = 4.0;
    }
    let gap = model.ui.gap.px();
    let mut body_padding = padding;
    if is_bar_panel && !model.ui.custom_padding {
        body_padding.bottom = 0.0;
    }
    let mut body = column![].spacing(gap).padding(body_padding);
    let mut entries = entries.into_iter().peekable();
    while let Some(entry) = entries.next() {
        if is_bar_panel
            && entries.peek().is_some_and(|next| {
                entry.paired_toggle && next.paired_toggle
                    || entry.kind == crate::modules::ModuleKind::Button
                        && next.kind == entry.kind
                        && next.zone == entry.zone
            })
        {
            let next = entries.next().expect("peeked panel entry");
            body = body.push(row![entry.element, next.element].spacing(gap));
        } else {
            body = body.push(entry.element);
        }
    }
    let compact = model.adaptive && model.drawer && is_bar_panel && model.max_height > 0.0;
    let height = if compact {
        Length::Shrink
    } else {
        Length::Fill
    };
    let scroll = crate::theme::panel_scroll(body).height(height);
    let body: Element<'_, Message> = if compact {
        let rows = footer_entries.len().div_ceil(2) as f32;
        let footer_height = if rows > 0.0 {
            1.0 + rows * (44.0 + gap.min(6.0)) + padding.top + padding.bottom
        } else {
            0.0
        };
        let header_height = crate::theme::panel_header_height() as f32
            + if model.ui.custom_padding {
                model.ui.padding.top.px() - 12.0
            } else {
                0.0
            };
        let frame_height = header_height
            + model.ui.margin.vertical()
            + model.ui.radius[2].px().max(model.ui.radius[3].px())
            + if model.ui.custom_border {
                2.0 * model.ui.border_width.px()
            } else {
                0.0
            };
        container(scroll)
            .max_height((model.max_height - frame_height - footer_height).max(0.0))
            .into()
    } else {
        scroll.into()
    };
    let mut content = column![body].height(height);
    if !footer_entries.is_empty() {
        let mut buttons = footer_entries.into_iter();
        let mut footer = column![crate::theme::divider()].spacing(gap.min(6.0));
        while let Some(first) = buttons.next() {
            let mut pair = row![first.element].spacing(gap);
            if let Some(second) = buttons.next() {
                pair = pair.push(second.element);
            } else {
                pair = pair.push(Space::new().width(Length::Fill));
            }
            footer = footer.push(pair);
        }
        content = content.push(footer.padding(padding));
    }

    let frame = panel_frame(
        content.into(),
        text_color,
        model.title,
        model.drawer,
        model.ui,
        false,
        height,
    );
    panel_surface(frame, compact, model.compact_at_end)
}

fn render_confirmation<'a>(
    model: PanelRenderModel<'a>,
    text_color: Option<iced::Color>,
) -> Element<'a, Message> {
    let palette = crate::theme::palette();
    let message = row![
        container(crate::theme::icon(
            include_bytes!("../../assets/alert.svg"),
            22.0,
            palette.danger,
        ))
        .center(38)
        .style(move |_| container::Style::default()
            .background(palette.danger.scale_alpha(0.12))
            .border(iced::Border::default().rounded(10))),
        crate::theme::text(model.menu_prompt.unwrap_or_default()).width(Length::Fill),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);
    let actions = row(model.menu_entries.iter().enumerate().map(|(index, entry)| {
        let mut label = crate::theme::strong_text(entry.label.clone());
        if index > 0 {
            label = label.color(palette.danger);
        }
        crate::theme::action(
            container(label).center(Length::Fill),
            Some(Message::Panel(super::messages::PanelEvent::ActivateIndex(
                index,
            ))),
            canopa_ui::Action::Normal,
            model.focused_index == Some(index),
        )
        .width(Length::FillPortion(1))
        .height(44)
        .into()
    }))
    .spacing(8);
    let content = column![message, crate::theme::divider(), actions]
        .spacing(12)
        .padding(iced::Padding {
            left: model.ui.padding.left.px(),
            right: model.ui.padding.right.px(),
            ..Default::default()
        });
    let frame = panel_frame(
        content.into(),
        text_color,
        model.title,
        model.drawer,
        model.ui,
        false,
        Length::Shrink,
    );
    panel_surface(frame, true, model.drawer)
}

fn panel_frame<'a>(
    content: Element<'a, Message>,
    text_color: Option<iced::Color>,
    title: String,
    drawer: bool,
    ui: canopa_theme::ResolvedStyle,
    back: bool,
    height: Length,
) -> Element<'a, Message> {
    let bottom_radius = if drawer && !ui.custom_radius {
        0.0
    } else {
        ui.radius[2].px().max(ui.radius[3].px())
    };
    // Scrollbars and footer content must stay above the rounded lower corners.
    let content = container(content)
        .padding(iced::Padding {
            bottom: bottom_radius,
            ..Default::default()
        })
        .height(height);
    let content = column![
        container(crate::surface::panel::panel_header_with_back(title, back)).padding(
            iced::Padding {
                top: if ui.custom_padding {
                    ui.padding.top.px()
                } else {
                    12.0
                },
                bottom: 12.0,
                left: ui.padding.left.px(),
                right: ui.padding.right.px()
            }
        ),
        content,
    ]
    .height(height);
    styled_panel_frame(content, text_color, drawer, ui, height)
}

fn styled_panel_frame<'a>(
    content: impl Into<Element<'a, Message>>,
    text_color: Option<iced::Color>,
    drawer: bool,
    ui: canopa_theme::ResolvedStyle,
    height: Length,
) -> Element<'a, Message> {
    let frame = container(content)
        .padding(if ui.custom_border {
            ui.border_width.px()
        } else {
            0.0
        })
        .width(Length::Fill)
        .height(height)
        .style(move |_| iced::widget::container::Style {
            text_color: text_color.or(Some(canopa_ui::resolve_color(
                &crate::theme::appearance(),
                ui.foreground,
            ))),
            ..{
                let mut style = canopa_ui::frame(&crate::theme::appearance(), ui);
                if drawer && !ui.custom_radius {
                    style.border.radius = 0.0.into();
                }
                style
            }
        });
    container(frame)
        .padding(canopa_ui::padding(ui.margin))
        .width(Length::Fill)
        .height(height)
        .into()
}

pub(super) fn panel_surface<'a>(
    frame: Element<'a, Message>,
    compact: bool,
    at_end: bool,
) -> Element<'a, Message> {
    // Keep the layer and its keyboard identity stable as the panel fits its
    // content. Clicking the transparent remainder dismisses the panel.
    let close = Message::Panel(super::messages::PanelEvent::Close);
    let outside = |visible| {
        mouse_area(Space::new().width(Length::Fill).height(if visible {
            Length::Fill
        } else {
            Length::Fixed(0.0)
        }))
        .on_press(close.clone())
        .on_right_press(close.clone())
        .on_middle_press(close.clone())
    };
    column![
        outside(compact && at_end),
        frame,
        outside(compact && !at_end)
    ]
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

fn render_external_panel_model<'a>(
    content: &'a crate::surface::panel::ExternalPanelContent,
    text_color: Option<iced::Color>,
    model: PanelRenderModel<'a>,
) -> Element<'a, Message> {
    let PanelRenderModel {
        title,
        drawer,
        ui,
        return_to_bar: back,
        adaptive,
        compact_at_end: at_end,
        content_width,
        ..
    } = model;
    if let Some(snapshot) = content.document.canopa() {
        return super::canopa_panel::render_with_back(
            snapshot,
            &content.provider_name,
            drawer,
            ui,
            back,
            adaptive,
            at_end,
            content_width,
        );
    }
    let compact = adaptive
        && matches!(
            content.elements(),
            [crate::surface::panel::ExternalPanelElement::Status { .. }]
                | [
                    crate::surface::panel::ExternalPanelElement::Status { .. },
                    crate::surface::panel::ExternalPanelElement::Button { .. },
                ]
        );
    let mut padding = canopa_ui::padding(ui.padding);
    if compact && !ui.custom_padding {
        padding.top = 0.0;
        padding.bottom = 0.0;
    }
    let height = if compact {
        Length::Shrink
    } else {
        Length::Fill
    };
    let mut body = column![].spacing(ui.gap.px()).padding(padding);
    for element in content.elements() {
        let rendered: Element<'_, Message> = match element {
            crate::surface::panel::ExternalPanelElement::Text { text: value } => {
                crate::theme::text(value.clone()).into()
            }
            crate::surface::panel::ExternalPanelElement::Button { id, label } => {
                crate::theme::button(crate::theme::text(label.clone()))
                    .on_press(Message::ExternalPanel(
                        super::messages::ExternalPanelEvent::ButtonActivated {
                            provider_name: content.provider_name.clone(),
                            element_id: id.clone(),
                        },
                    ))
                    .into()
            }
            crate::surface::panel::ExternalPanelElement::SelectableList {
                id,
                entries,
                allow_custom,
            } => render_external_selectable_list(
                content.provider_name.as_str(),
                id.as_str(),
                entries,
                *allow_custom,
                content.list_search(id),
                content.input_error(id),
            ),
            crate::surface::panel::ExternalPanelElement::TextInput {
                id,
                label,
                value,
                secret,
                error,
            } => render_external_text_input(
                content.provider_name.as_str(),
                id.as_str(),
                label.as_str(),
                value.as_str(),
                *secret,
                content.input_error(id).or(error.as_deref()),
            ),
            crate::surface::panel::ExternalPanelElement::Status {
                title,
                message,
                severity,
            } => {
                let is_error = *severity == crate::surface::panel::ExternalPanelSeverity::Error;
                let palette = crate::theme::palette();
                container(
                    row![
                        container(if is_error {
                            crate::theme::icon(
                                include_bytes!("../../assets/alert.svg"),
                                20.0,
                                palette.danger,
                            )
                        } else {
                            crate::theme::strong_text("i").color(palette.accent).into()
                        })
                        .center(32)
                        .style(move |_| container::Style::default()
                            .background(if is_error {
                                palette.danger.scale_alpha(0.12)
                            } else {
                                palette.background
                            })
                            .border(iced::Border::default().rounded(9))),
                        column![
                            crate::theme::strong_text(title.clone()),
                            crate::theme::text(message.clone()).style(canopa_ui::muted),
                        ]
                        .width(Length::Fill)
                        .spacing(5),
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Start),
                )
                .padding(14)
                .width(Length::Fill)
                .style(crate::theme::card)
                .into()
            }
        };
        // Preserve edits and focus on the same field while replacing the
        // retained state of controls whose provider-owned identity changed.
        let rendered = match element {
            crate::surface::panel::ExternalPanelElement::Button { id, label } => {
                crate::surface::identity::identify(
                    (content.provider_name.clone(), id.clone(), label.clone()),
                    rendered,
                )
            }
            crate::surface::panel::ExternalPanelElement::TextInput { id, secret, .. } => {
                crate::surface::identity::identify(
                    (content.provider_name.clone(), id.clone(), *secret),
                    rendered,
                )
            }
            crate::surface::panel::ExternalPanelElement::SelectableList { id, .. } => {
                crate::surface::identity::identify(
                    (content.provider_name.clone(), id.clone()),
                    rendered,
                )
            }
            _ => rendered,
        };
        body = body.push(rendered);
    }

    let frame = panel_frame(
        crate::theme::panel_scroll(body).height(height).into(),
        text_color,
        title,
        drawer,
        ui,
        back,
        height,
    );
    panel_surface(frame, compact, at_end)
}

fn render_external_text_input<'a>(
    provider_name: &str,
    element_id: &str,
    label: &str,
    value: &str,
    secret: bool,
    error: Option<&str>,
) -> Element<'a, Message> {
    let provider_for_change = provider_name.to_string();
    let element_for_change = element_id.to_string();
    let provider_for_submit = provider_name.to_string();
    let element_for_submit = element_id.to_string();
    let submit_value = value.to_string();
    let mut input = crate::theme::input(label, value)
        .on_input(move |value| {
            Message::ExternalPanel(super::messages::ExternalPanelEvent::TextInputChanged {
                provider_name: provider_for_change.clone(),
                element_id: element_for_change.clone(),
                value,
            })
        })
        .on_submit(Message::ExternalPanel(
            super::messages::ExternalPanelEvent::TextInputSubmitted {
                provider_name: provider_for_submit,
                element_id: element_for_submit,
                value: submit_value,
            },
        ))
        .width(Length::Fill);
    input = input.secure(secret);

    let mut block = column![input].spacing(4);
    if let Some(error) = error {
        block = block.push(crate::theme::text(error.to_string()).size(crate::theme::label_size()));
    }
    container(block).width(Length::Fill).into()
}

fn render_external_selectable_list<'a>(
    provider_name: &str,
    list_id: &str,
    entries: &'a [crate::surface::panel::ExternalSelectableEntry],
    allow_custom: bool,
    search_query: &str,
    error: Option<&str>,
) -> Element<'a, Message> {
    let provider_for_search = provider_name.to_string();
    let list_for_search = list_id.to_string();
    let search = crate::theme::input("Search", search_query)
        .on_input(move |query| {
            Message::ExternalPanel(super::messages::ExternalPanelEvent::ListSearchChanged {
                provider_name: provider_for_search.clone(),
                list_id: list_for_search.clone(),
                query,
            })
        })
        .width(Length::Fill);

    let mut rows = column![].spacing(6);
    for item in external_selectable_list_items_with_custom(entries, search_query, allow_custom) {
        match item {
            ExternalSelectableListItem::Entry(entry) => {
                let provider = provider_name.to_string();
                let list_id = list_id.to_string();
                let entry_id = entry.id.clone();
                let mut row_content = column![crate::theme::text(entry.label.clone())].spacing(2);
                if let Some(description) = &entry.description {
                    row_content = row_content.push(
                        crate::theme::text(description.clone()).size(crate::theme::label_size()),
                    );
                }
                if let Some(status) = &entry.status {
                    row_content = row_content
                        .push(crate::theme::text(status.clone()).size(crate::theme::label_size()));
                }
                let border_color = if entry.active {
                    crate::theme::palette().accent
                } else {
                    crate::theme::palette().border
                };
                let row = external_selectable_row(row_content, border_color);
                if entry.disabled {
                    rows = rows.push(row);
                } else {
                    rows = rows.push(mouse_area(row).on_press(Message::ExternalPanel(
                        super::messages::ExternalPanelEvent::ListEntryActivated {
                            provider_name: provider,
                            list_id,
                            entry_id,
                        },
                    )));
                }
            }
            ExternalSelectableListItem::CustomValue(value) => {
                let provider = provider_name.to_string();
                let list_id = list_id.to_string();
                let submitted_value = value.clone();
                let row_content = column![
                    crate::theme::text(value),
                    crate::theme::text("Use custom value").size(crate::theme::label_size())
                ]
                .spacing(2);
                let row = external_selectable_row(row_content, crate::theme::palette().accent);
                rows = rows.push(mouse_area(row).on_press(Message::ExternalPanel(
                    super::messages::ExternalPanelEvent::ListCustomValueSubmitted {
                        provider_name: provider,
                        list_id,
                        value: submitted_value,
                    },
                )));
            }
        }
    }

    let mut block = column![search].spacing(8);
    if let Some(error) = error {
        block = block.push(crate::theme::text(error.to_owned()).size(crate::theme::label_size()));
    }
    container(block.push(rows)).width(Length::Fill).into()
}

fn external_selectable_row<'a>(
    row_content: iced::widget::Column<'a, Message>,
    border_color: iced::Color,
) -> iced::widget::Container<'a, Message> {
    container(row_content)
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::palette().raised)),
            border: iced::Border {
                color: border_color,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExternalSelectableListItem<'a> {
    Entry(&'a crate::surface::panel::ExternalSelectableEntry),
    CustomValue(String),
}

#[cfg(test)]
fn external_selectable_list_items<'a>(
    entries: &'a [crate::surface::panel::ExternalSelectableEntry],
    query: &str,
) -> Vec<ExternalSelectableListItem<'a>> {
    external_selectable_list_items_with_custom(entries, query, false)
}

fn external_selectable_list_items_with_custom<'a>(
    entries: &'a [crate::surface::panel::ExternalSelectableEntry],
    query: &str,
    allow_custom: bool,
) -> Vec<ExternalSelectableListItem<'a>> {
    if super::panel_search::is_blank_query(query) {
        return entries
            .iter()
            .map(ExternalSelectableListItem::Entry)
            .collect();
    }

    let matches = entries
        .iter()
        .enumerate()
        .filter_map(|(source_index, entry)| {
            let fields = [
                super::panel_search::SearchText {
                    field: super::panel_search::SearchField::Label,
                    text: entry.label.as_str(),
                },
                super::panel_search::SearchText {
                    field: super::panel_search::SearchField::Secondary,
                    text: entry.description.as_deref().unwrap_or(""),
                },
                super::panel_search::SearchText {
                    field: super::panel_search::SearchField::Secondary,
                    text: entry.status.as_deref().unwrap_or(""),
                },
            ];
            super::panel_search::best_match(&fields, query).map(|score| {
                super::panel_search::SearchMatch {
                    item: entry,
                    score,
                    source_index,
                }
            })
        })
        .collect();
    let mut items: Vec<_> = super::panel_search::rank_matches(matches)
        .into_iter()
        .map(ExternalSelectableListItem::Entry)
        .collect();
    let custom_value = query.trim();
    let exact_match = entries.iter().any(|entry| entry.label == custom_value);
    if allow_custom && !custom_value.is_empty() && !exact_match {
        items.push(ExternalSelectableListItem::CustomValue(
            custom_value.to_string(),
        ));
    }
    items
}

fn render_panel_menu_model<'a>(
    model: PanelRenderModel<'a>,
    text_color: Option<iced::Color>,
) -> Element<'a, Message> {
    let placeholder = model.menu_prompt.as_deref().unwrap_or("Search");
    let search = crate::theme::input(placeholder, &model.search_query)
        .id(model.search_input_id.clone())
        .on_input(|query| Message::Panel(super::messages::PanelEvent::SearchChanged(query)))
        .width(Length::Fill);
    let back_slot: Element<'_, Message> = if model.can_go_back {
        crate::theme::button(crate::theme::text("‹ Back").size(crate::theme::label_size()))
            .on_press(Message::Panel(super::messages::PanelEvent::Back))
            .height(38)
            .into()
    } else {
        Space::new().width(0).height(38).into()
    };
    let header_spacing = if model.can_go_back { 8 } else { 0 };
    let header = row![search, back_slot]
        .align_y(iced::Alignment::Center)
        .spacing(header_spacing);

    let header = container(header).padding(iced::Padding {
        left: model.ui.padding.left.px(),
        right: model.ui.padding.right.px(),
        ..Default::default()
    });
    let content = column![header, render_menu_scroll(&model, Length::Fill)];
    let frame = panel_frame(
        content.into(),
        text_color,
        model.title,
        model.drawer,
        model.ui,
        false,
        Length::Fill,
    );
    panel_surface(frame, false, false)
}

fn render_menu_scroll<'a>(model: &PanelRenderModel<'a>, height: Length) -> Element<'a, Message> {
    let metrics = model.menu_metrics;
    let menu_row_gap = metrics.gap;
    let is_launcher = model.launcher_session.is_some();
    let mut list = column![].spacing(menu_row_gap).padding(iced::Padding {
        top: if is_launcher {
            0.0
        } else {
            model.ui.padding.top.px() / 2.0
        },
        bottom: if is_launcher {
            0.0
        } else {
            model.ui.padding.bottom.px() / 2.0
        },
        left: if is_launcher {
            0.0
        } else {
            model.ui.padding.left.px()
        },
        right: if is_launcher {
            launcher::scrollbar_gutter(model, height)
        } else {
            model.ui.padding.right.px()
        },
    });
    let entry_count = model.menu_entries.len();
    let range = super::panel_sizing::menu_render_range(
        entry_count,
        model.menu_scroll_y,
        model.menu_viewport_height,
        metrics.extent,
    );
    let (start, end) = (range.start, range.end);
    let top_spacer = start as f32 * metrics.extent;
    let bottom_spacer = entry_count.saturating_sub(end) as f32 * metrics.extent;

    if top_spacer > 0.0 {
        // The column supplies the gap between a spacer and its adjacent row.
        list = list.push(
            Space::new()
                .width(Length::Fill)
                .height(top_spacer - menu_row_gap),
        );
    }

    for index in start..end {
        list = list.push(render_menu_row(model, index));
    }

    if bottom_spacer > 0.0 {
        list = list.push(
            Space::new()
                .width(Length::Fill)
                .height(bottom_spacer - menu_row_gap),
        );
    }

    let mut scroll = crate::theme::panel_scroll(list);
    if is_launcher {
        use iced::widget::scrollable::{Direction, Scrollbar};
        let scrollbar = if launcher::scrollbar_gutter(model, height) >= 8.0 {
            Scrollbar::new().width(8).scroller_width(5).margin(1)
        } else {
            Scrollbar::hidden()
        };
        scroll = scroll
            .direction(Direction::Vertical(scrollbar))
            .style(|theme, status| {
                use iced::widget::scrollable;
                let mut style = scrollable::default(theme, status);
                let palette = canopa_ui::colors(theme);
                style.vertical_rail.background = Some(palette.muted.scale_alpha(0.10).into());
                style.vertical_rail.border.radius = 3.0.into();
                style.vertical_rail.scroller.background = palette.muted.scale_alpha(0.64).into();
                style
            });
    }
    scroll
        .id(model.menu_scroll_id.clone())
        .on_scroll(|viewport| {
            Message::Panel(super::messages::PanelEvent::MenuScrolled {
                y: viewport.absolute_offset().y,
                viewport_height: viewport.bounds().height,
            })
        })
        .height(height)
        .into()
}

fn render_menu_row<'a>(model: &PanelRenderModel<'a>, index: usize) -> Element<'a, Message> {
    let metrics = model.menu_metrics;
    let menu_row_gap = metrics.gap;
    let entry = &model.menu_entries[index];
    let focused = model.focused_index == Some(index);
    if let super::panel_menu_session::PanelMenuEntryTarget::Launcher(target) = &entry.target {
        return launcher::result_row(model, entry, target, focused);
    }

    let mut row_content = row![]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center);
    if model.show_icons {
        let icon = entry.icon.as_deref().and_then(|name| model.icons.get(name));
        row_content = row_content.push(panel_menu_icon_element(icon));
    }
    row_content = row_content.push(
        container(
            crate::theme::text(entry.label.clone())
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .clip(true),
    );
    if entry.is_submenu {
        row_content = row_content.push(crate::theme::text("›"));
    }
    let keyboard_focus = model.keyboard_focus;
    let row = iced::widget::button(
        container(row_content)
            .height(Length::Fill)
            .align_y(iced::Alignment::Center),
    )
    .on_press(Message::Panel(super::messages::PanelEvent::ActivateIndex(
        index,
    )))
    .padding(canopa_ui::content_padding(metrics.row))
    .width(Length::Fill)
    // Rendering, virtual spacers and keyboard reveal must use one pitch,
    // even for long labels or a different font's line metrics.
    .height(metrics.extent - menu_row_gap - metrics.row.margin.vertical())
    .clip(true)
    .style(move |_, status| {
        let frame = crate::theme::highlighted_frame(
            metrics.row,
            focused,
            status == iced::widget::button::Status::Hovered,
            false,
            focused && keyboard_focus,
        );
        iced::widget::button::Style {
            background: frame.background,
            text_color: frame.text_color.unwrap(),
            border: frame.border,
            ..Default::default()
        }
    });
    let row = container(row).padding(canopa_ui::padding(metrics.row.margin));
    crate::surface::pointer_focus::pointer_focus(
        row,
        Message::Panel(super::messages::PanelEvent::FocusIndex(index)),
    )
}

fn panel_menu_icon_element<'a>(
    handle: Option<&iced::widget::image::Handle>,
) -> Element<'a, Message> {
    const ICON_SIZE: u32 = 18;
    match handle {
        Some(handle) => iced::widget::image(handle.clone())
            .width(ICON_SIZE)
            .height(ICON_SIZE)
            .into(),
        None => Space::new().width(ICON_SIZE).height(ICON_SIZE).into(),
    }
}

fn render_tooltip_popup<'a>(
    tree: &'a crate::surface::tooltip_popup::TooltipTree,
) -> Element<'a, Message> {
    crate::surface::tooltip_popup::view_tooltip::<Message>(tree)
}

fn render_popup_model<'a>(
    model: PopupRenderModel<'a>,
    window: iced::window::Id,
    hovered_item: Option<&str>,
    focused_item: Option<&str>,
) -> Element<'a, Message> {
    use crate::surface::menu_popup::view_menu_popup;

    view_menu_popup(
        model.tree,
        model.service,
        model.menu_path,
        window,
        hovered_item,
        focused_item,
    )
}

fn render_searchable_sni_popup_model<'a>(
    model: PopupRenderModel<'a>,
    window: iced::window::Id,
    session: &'a super::selectable_menu_session::SelectableMenuSession,
) -> Element<'a, Message> {
    let ui = crate::theme::ui(canopa_theme::UiKind::Menu);
    let (row_ui, row_height) = crate::surface::menu_popup::popup_row_metrics();
    let entries_snapshot = session.entries_snapshot();
    let focused_index = session.focused_index();
    let hovered_index = session.hovered_index();
    let search = crate::theme::input("Search", session.search_query())
        .on_input(move |query| {
            Message::Menu(super::messages::MenuEvent::SearchChanged { window, query })
        })
        .width(Length::Fill);

    let mut entries = column![].spacing(ui.gap.px());
    for (index, entry) in entries_snapshot.iter().enumerate() {
        let row = container(
            crate::theme::text(entry.label.clone()).size(crate::theme::body_size() * 13.0 / 14.0),
        )
        .padding(canopa_ui::content_padding(row_ui))
        .height(row_height)
        .width(Length::Fill)
        .style(move |_| {
            crate::theme::highlighted_frame(
                row_ui,
                focused_index == Some(index),
                hovered_index == Some(index),
                false,
                focused_index == Some(index),
            )
        });
        let row = container(row).padding(canopa_ui::padding(row_ui.margin));
        let super::selectable_menu_session::SelectableMenuEntryTarget::SniMenuItem(item_id) =
            &entry.target
        else {
            continue;
        };
        let menu_item_id = item_id.parse().unwrap_or(0);
        let service = model.service.to_string();
        let menu_path = model.menu_path.clone();
        let hover_id = item_id.clone();
        entries = entries.push(
            mouse_area(row)
                .on_enter(Message::Menu(super::messages::MenuEvent::ItemHoverEnter {
                    window,
                    item_id: hover_id,
                }))
                .on_press(Message::Menu(super::messages::MenuEvent::ItemSelected {
                    service,
                    menu_item_id,
                    menu_path,
                })),
        );
    }

    let menu = mouse_area(
        container(column![search, entries].spacing(4))
            .style(move |_| canopa_ui::frame(&crate::theme::appearance(), ui))
            .padding(canopa_ui::content_padding(ui)),
    )
    .on_exit(Message::Menu(super::messages::MenuEvent::ItemHoverExit(
        window,
    )));

    container(menu)
        .padding(canopa_ui::padding(ui.margin))
        .into()
}

fn render_bar_model<'a>(
    model: BarRenderModel<'a>,
    window: iced::window::Id,
) -> Element<'a, Message> {
    let frame = model.geometry.frame;
    let ui = model.ui;
    let legacy = !ui.custom_border && !ui.custom_radius;
    let mut padding = canopa_ui::padding(ui.padding);
    if !legacy {
        padding.top += ui.border_width.px();
        padding.bottom += ui.border_width.px();
        padding.left += ui.border_width.px();
        padding.right += ui.border_width.px();
    }
    let show_workspace_label = !model.is_horizontal
        && !model.center.is_empty()
        && model
            .center
            .iter()
            .all(|entry| entry.kind == crate::modules::ModuleKind::WorkspaceItem);
    let mut center: Vec<_> = model
        .center
        .into_iter()
        .map(|entry| entry.element)
        .collect();
    let label_size = if show_workspace_label {
        let available = frame.width - padding.left - padding.right;
        [10.0, 9.0]
            .into_iter()
            .find(|size| available >= crate::theme::text_width("Workspace", *size))
    } else {
        None
    };
    if let Some(label_size) = label_size {
        center.insert(
            0,
            container(
                crate::theme::strong_text("Workspace")
                    .size(label_size)
                    .style(canopa_ui::muted),
            )
            .center_x(Length::Fill)
            .into(),
        );
    }
    let content: Element<'_, Message> = container(zone_layout(
        model.start.into_iter().map(|e| e.element).collect(),
        center,
        model.end.into_iter().map(|e| e.element).collect(),
        model.is_horizontal,
        padding,
        ui.gap.px(),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
    let content = if legacy {
        let strip = container(Space::new()).style(|_| container::Style {
            background: Some(crate::theme::palette().raised.into()),
            ..Default::default()
        });
        if model.is_horizontal {
            let strip = strip.width(Length::Fill).height(1);
            let body = container(content).height(Length::Fill);
            match model.edge {
                Edge::Top => column![body, strip],
                _ => column![strip, body],
            }
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            let strip = strip.width(1).height(Length::Fill);
            let body = container(content).width(Length::Fill);
            match model.edge {
                Edge::Left => row![body, strip],
                _ => row![strip, body],
            }
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
    } else {
        content
    };
    let bar = mouse_area(
        container(content)
            .width(frame.width)
            .height(frame.height)
            .clip(true)
            .style(move |_| canopa_ui::frame(&crate::theme::appearance(), ui)),
    )
    .on_press(Message::Surface(super::messages::SurfaceEvent::BarClick(
        window,
    )))
    .on_right_press(Message::Surface(
        super::messages::SurfaceEvent::BarRightClick(window),
    ))
    .on_enter(Message::Surface(
        super::messages::SurfaceEvent::BarHoverEnter(window),
    ))
    .on_exit(Message::Surface(
        super::messages::SurfaceEvent::BarHoverExit(window),
    ));
    container(bar)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: frame.y,
            left: frame.x,
            right: 0.0,
            bottom: 0.0,
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{Config, MenuConfig, MenuSourceConfig, MenuSourceKind};
    use crate::surface::menu_popup::{MenuItem, MenuTree};

    #[tokio::test]
    async fn dismissal_window_stays_transparent_until_physically_removed() {
        use crate::app::{
            Message,
            messages::{PanelEvent, SurfaceEvent},
        };
        use iced::advanced::{
            Layout, layout,
            renderer::{Headless, Renderer as _},
            widget::Tree,
        };
        use iced::{Color, Rectangle, Size, mouse};

        let mut renderer =
            iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
                .await
                .expect("software renderer");
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(Message::Panel(PanelEvent::Open {
            origin_window: origin,
            origin: crate::surface::panel::PanelOrigin::Bar,
            content: crate::surface::panel::PanelContent::BarPanel,
            anchor: None,
        }));
        let backdrop = app.foreground.role_snapshot().roles_for_test_dismissal();
        let mut tree = Tree::empty();
        let viewport = Rectangle::with_size(Size::new(1280.0, 800.0));
        for (window, closing, transparent) in [
            (origin, false, false),
            (backdrop, false, true),
            (backdrop, true, true),
        ] {
            if closing {
                // Physical removal is an asynchronous task. Iced can redraw the
                // same window after App has already processed the close event.
                let _ = app.update(Message::Surface(SurfaceEvent::PointerPressed(backdrop)));
            }
            let mut view = app.view(window);
            tree.diff(view.as_widget());
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, viewport.size()),
            );
            renderer.reset(viewport);
            view.as_widget().draw(
                &tree,
                &mut renderer,
                &iced::Theme::Dark,
                &iced::advanced::renderer::Style {
                    text_color: Color::WHITE,
                },
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &viewport,
            );
            let rgba = renderer.screenshot(Size::new(1280, 800), 1.0, Color::TRANSPARENT);
            let painted = rgba.chunks_exact(4).filter(|pixel| pixel[3] != 0).count();
            if transparent {
                assert_eq!(
                    painted, 0,
                    "closing={closing}: dismissal window painted {painted} screen pixels"
                );
            } else {
                assert!(painted > 0, "registered bar must still render");
            }
        }
    }

    #[test]
    fn default_surface_view_renders_without_panic() {
        let (app, _) = App::boot(Config::default(), vec![]);
        let _ = app.view(iced::window::Id::unique());
    }

    #[test]
    fn popup_surface_view_renders_without_panic() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let popup = iced::window::Id::unique();
        app.register_menu_popup(
            popup,
            super::super::foreground::MenuPopupMeta {
                service: "org.test".into(),
                menu_path: zbus::zvariant::OwnedObjectPath::try_from("/menu").unwrap(),
                tree: MenuTree {
                    items: vec![MenuItem {
                        id: "1".into(),
                        label: "Open".into(),
                        enabled: true,
                        visible: true,
                        icon_name: None,
                        children: vec![],
                        is_separator: false,
                    }],
                    anchor: (0, 0),
                    screen_size: (0, 0),
                },
                origin: super::super::surfaces::BarSurfaceIdentity::all_screens(),
            },
        );

        let _ = app.view(popup);
    }

    #[test]
    fn panel_surface_view_renders_without_panic() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let origin = iced::window::Id::unique();
        let panel = iced::window::Id::unique();
        {
            let surface = app.surface_mut(origin);
            surface.bar_name = "main".into();
            surface.output_name = "DP-1".into();
            surface.screen_size = (1920, 1080);
        }
        let request = app.panel_open_request(
            origin,
            crate::surface::panel::PanelOrigin::Bar,
            crate::surface::panel::PanelContent::BarPanel,
            None,
        );
        let effects = app.prepare_panel_for_test(request);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            crate::app::foreground::ForegroundEffect::OpenPanelWindow { .. }
        )));
        app.register_panel_popup(panel, origin);

        let _ = app.view(panel);
    }

    #[test]
    fn external_selectable_list_search_filters_and_ranks_entries() {
        let entries = vec![
            crate::surface::panel::ExternalSelectableEntry {
                id: "coffee".into(),
                label: "Coffee Shop".into(),
                description: Some("Guest Wi-Fi".into()),
                disabled: false,
                active: false,
                status: None,
            },
            crate::surface::panel::ExternalSelectableEntry {
                id: "printer".into(),
                label: "Office Printer".into(),
                description: Some("Known network".into()),
                disabled: false,
                active: false,
                status: None,
            },
        ];

        let visible = super::external_selectable_list_items(&entries, "guest")
            .into_iter()
            .filter_map(|item| match item {
                super::ExternalSelectableListItem::Entry(entry) => Some(entry.id.as_str()),
                super::ExternalSelectableListItem::CustomValue(_) => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(visible, vec!["coffee"]);
    }

    #[test]
    fn external_selectable_list_custom_value_appears_only_when_allowed() {
        let entries = vec![crate::surface::panel::ExternalSelectableEntry {
            id: "home".into(),
            label: "Home Wi-Fi".into(),
            description: None,
            disabled: false,
            active: false,
            status: None,
        }];

        let disallowed =
            super::external_selectable_list_items_with_custom(&entries, "Hidden", false);
        assert_eq!(
            disallowed
                .iter()
                .map(|item| match item {
                    super::ExternalSelectableListItem::Entry(entry) => entry.id.as_str(),
                    super::ExternalSelectableListItem::CustomValue(value) => value.as_str(),
                })
                .collect::<Vec<_>>(),
            Vec::<&str>::new()
        );

        let allowed = super::external_selectable_list_items_with_custom(&entries, "Hidden", true);
        assert_eq!(
            allowed,
            vec![super::ExternalSelectableListItem::CustomValue(
                "Hidden".into()
            )]
        );
    }

    #[test]
    fn external_selectable_list_custom_value_uses_case_sensitive_exact_match() {
        let entries = vec![crate::surface::panel::ExternalSelectableEntry {
            id: "home".into(),
            label: "Home Wi-Fi".into(),
            description: None,
            disabled: false,
            active: false,
            status: None,
        }];

        let exact = super::external_selectable_list_items_with_custom(&entries, "Home Wi-Fi", true);
        assert!(
            exact
                .iter()
                .all(|item| { !matches!(item, super::ExternalSelectableListItem::CustomValue(_)) })
        );

        let different_case =
            super::external_selectable_list_items_with_custom(&entries, "home wi-fi", true);
        assert!(different_case.iter().any(|item| {
            matches!(
                item,
                super::ExternalSelectableListItem::CustomValue(value) if value == "home wi-fi"
            )
        }));
    }

    #[test]
    fn external_selectable_list_custom_value_trims_and_rejects_blank() {
        let entries = Vec::new();

        assert!(
            super::external_selectable_list_items_with_custom(&entries, "   ", true).is_empty()
        );
        assert_eq!(
            super::external_selectable_list_items_with_custom(&entries, "  Hidden  ", true),
            vec![super::ExternalSelectableListItem::CustomValue(
                "Hidden".into()
            )]
        );
    }

    #[test]
    fn installed_apps_hover_view_path_does_not_rebuild_every_entry_widget() {
        let mut config = Config {
            menus: vec![MenuConfig {
                name: "apps".into(),
                show_icons: false,
                prompt: None,
                source: Some(MenuSourceConfig {
                    kind: MenuSourceKind::InstalledApps,
                }),
                items: vec![],
            }],
            ..Default::default()
        };
        config.bars[0].actions.bindings.clear();

        let (mut app, _) = App::boot(config, vec![]);
        app.foreground.set_installed_apps_for_test(
            (0..5_000)
                .map(|index| crate::desktop_apps::DesktopApplication {
                    id: format!("app-{index}.desktop"),
                    name: format!("App {index:04}"),
                    generic_name: None,
                    comment: None,
                    keywords: vec![],
                    icon: None,
                    desktop_file: "fixture.desktop".into(),
                    dbus_activatable: false,
                })
                .collect(),
        );
        let origin = iced::window::Id::unique();
        app.surface_mut(origin);
        let _ = app.update(crate::app::Message::Panel(
            crate::app::messages::PanelEvent::Open {
                origin_window: origin,
                origin: crate::surface::panel::PanelOrigin::Bar,
                content: crate::surface::panel::PanelContent::Menu("apps".into()),
                anchor: None,
            },
        ));
        let panel = app.foreground.active_panel_window().expect("panel");
        let _ = app.view(panel);

        let started = std::time::Instant::now();
        for index in 0..100 {
            let _ = app.update(crate::app::Message::Panel(
                crate::app::messages::PanelEvent::FocusIndex(index),
            ));
            let _ = app.view(panel);
        }

        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "hover view path rebuilt too much installed-app widget state: {:?}",
            started.elapsed()
        );
    }
}
