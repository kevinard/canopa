use super::*;
use crate::print::{
    Orientation, Pages,
    worker::{PrinterState, Sides},
};
use draft::Edit;
use iced::{
    Length,
    widget::{checkbox, column, container, pick_list, row, scrollable, text, text_input},
};

/// A length in millimetres, with up to two decimals.
fn millimetres(value: f64) -> String {
    canopa_i18n::format::decimal(value, 2, false)
}
fn choice<T>(value: T, label: impl Into<String>) -> Choice<T> {
    Choice {
        value,
        label: label.into(),
    }
}
fn select<'a, T: Clone + PartialEq + 'a>(
    label: String,
    options: Vec<Choice<T>>,
    value: &T,
    edit: impl Fn(Choice<T>) -> Edit + 'static,
) -> Element<'a, Message> {
    let selected = options
        .iter()
        .find(|option| &option.value == value)
        .cloned();
    column![
        text(label).size(14),
        pick_list(options, selected, move |choice| Message::Edit(edit(choice))).width(Length::Fill)
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}
fn field<'a>(
    label: String,
    value: &'a str,
    id: &'static str,
    edit: impl Fn(String) -> Edit + 'static,
) -> Element<'a, Message> {
    column![
        text(label.clone()).size(14),
        text_input(&label, value)
            .id(id)
            .on_input(move |value| Message::Edit(edit(value)))
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

pub(super) fn view(dialog: &Dialog) -> Element<'_, Message> {
    match &dialog.screen {
        Screen::Picker(generation, picker) => {
            let generation = *generation;
            return picker
                .view()
                .map(move |message| Message::Picker(generation, message));
        }
        Screen::Preparing(_) => {
            return container(
                column![
                    text(fl!("portal-print-preparing-picker")),
                    canopa_ui::action(
                        text(canopa_ui::direction::back_label()),
                        Some(Message::Back),
                        canopa_ui::Action::Normal,
                        false
                    )
                ]
                .spacing(16),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into();
        }
        Screen::Main => {}
    }
    let request = &dialog.request.prepare;
    let draft = &dialog.draft;
    let mut choices = dialog
        .printers
        .iter()
        .map(|printer| {
            let instance = printer
                .destination
                .instance
                .as_ref()
                .map(|instance| format!(" / {instance}"))
                .unwrap_or_default();
            choice(
                DestinationChoice::Printer(printer.destination.clone()),
                format!("{}{instance}", printer.label),
            )
        })
        .collect::<Vec<_>>();
    if !dialog.request.formats().is_empty() {
        choices.push(choice(DestinationChoice::File, fl!("portal-print-to-file")));
    }
    let selected = choices
        .iter()
        .find(|choice| Some(&choice.value) == dialog.target.as_ref())
        .cloned();
    let destination = pick_list(choices, selected, Message::Destination)
        .placeholder(fl!("portal-print-choose-destination"))
        .width(Length::Fill);
    let mut content = column![
        text(fl!("portal-print-title")).size(24),
        text(if request.context.title.is_empty() {
            fl!("portal-print-document")
        } else {
            request.context.title.clone()
        })
        .size(18),
        text(if request.context.app_id.is_empty() {
            fl!("portal-requested-by-application")
        } else {
            request.context.app_id.clone()
        })
        .style(canopa_ui::muted),
        text(fl!("portal-print-destination")).size(14),
        canopa_ui::direction::row([
            destination.into(),
            canopa_ui::action(
                text(fl!("portal-print-refresh")),
                Some(Message::Refresh),
                canopa_ui::Action::Normal,
                false,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(iced::Center)
    ]
    .spacing(12)
    .width(Length::Fill);
    if let Some(notice) = dialog.notice.clone() {
        content = content.push(text(notice).size(14).style(canopa_ui::muted));
    }
    if dialog.loading {
        content = content.push(text(fl!("portal-print-loading")).style(canopa_ui::muted));
    } else if dialog.printers.is_empty() && dialog.target.is_none() && dialog.error.is_none() {
        content = content.push(text(fl!("portal-print-no-printers")).style(canopa_ui::muted));
    }
    if let Some(DestinationChoice::Printer(destination)) = &dialog.target
        && let Some(printer) = dialog
            .printers
            .iter()
            .find(|printer| &printer.destination == destination)
    {
        if !printer.location.is_empty() {
            content = content.push(text(&printer.location).style(canopa_ui::muted));
        }
        let state = match (printer.accepting, printer.state) {
            (false, _) => fl!("portal-print-not-accepting"),
            (_, PrinterState::Stopped) => fl!("portal-print-stopped"),
            (_, PrinterState::Processing) => fl!("portal-print-busy"),
            _ => fl!("portal-print-idle"),
        };
        content = content.push(text(state).size(14).style(canopa_ui::muted));
    }
    if !dialog.loading {
        if let Some(caps) = &dialog.capabilities {
            let mut media = vec![choice(None, fl!("portal-print-printer-default"))];
            media.extend(caps.media.iter().map(|media| {
                choice(
                    Some(media.id.clone()),
                    fl!(
                        "portal-print-media",
                        name = media.id.as_str(),
                        width = millimetres(media.width_mm),
                        height = millimetres(media.height_mm)
                    ),
                )
            }));
            let mut sides = vec![choice(None, fl!("portal-print-printer-default"))];
            sides.extend(caps.sides.iter().map(|value| {
                choice(
                    Some(*value),
                    match value {
                        Sides::OneSided => fl!("portal-print-one-sided"),
                        Sides::LongEdge => fl!("portal-print-two-sided-long"),
                        Sides::ShortEdge => fl!("portal-print-two-sided-short"),
                    },
                )
            }));
            let mut colors = vec![choice(None, fl!("portal-print-printer-default"))];
            colors.extend(caps.colors.iter().map(|value| {
                choice(
                    Some(*value),
                    if *value {
                        fl!("portal-print-color")
                    } else {
                        fl!("portal-print-monochrome")
                    },
                )
            }));
            if caps.custom_media {
                content = content.push(
                    checkbox(draft.custom)
                        .label(fl!("portal-print-custom-paper"))
                        .on_toggle(|value| Message::Edit(Edit::Custom(value)))
                        .style(canopa_ui::checkbox_style),
                );
            }
            if draft.custom {
                content = content
                    .push(
                        row![
                            field(
                                fl!("portal-print-width"),
                                &draft.custom_dimensions[0],
                                "print-custom-width",
                                |value| Edit::CustomDimension(0, value)
                            ),
                            field(
                                fl!("portal-print-height"),
                                &draft.custom_dimensions[1],
                                "print-custom-height",
                                |value| Edit::CustomDimension(1, value)
                            )
                        ]
                        .spacing(12),
                    )
                    .push(canopa_ui::action(
                        text(fl!("portal-print-check-paper")),
                        dialog.custom_size().map(|_| Message::CheckCustomMedia),
                        canopa_ui::Action::Normal,
                        false,
                    ));
                if let Some(media) = &draft.device.custom_media
                    && dialog.checked.is_some()
                {
                    content = content.push(
                        text(fl!(
                            "portal-print-paper-confirmed",
                            top = millimetres(media.top_mm),
                            bottom = millimetres(media.bottom_mm),
                            left = millimetres(media.left_mm),
                            right = millimetres(media.right_mm)
                        ))
                        .size(13)
                        .style(canopa_ui::muted),
                    );
                } else {
                    content = content.push(
                        text(fl!("portal-print-custom-paper-hint"))
                            .size(13)
                            .style(canopa_ui::muted),
                    );
                }
                if let Some(error) = &dialog.error {
                    content = content.push(text(error).size(14));
                }
            } else {
                content = content.push(select(
                    fl!("portal-print-paper"),
                    media,
                    &draft.device.media,
                    Edit::Media,
                ));
            }
            content = content.push(
                row![
                    select(
                        fl!("portal-print-sides"),
                        sides,
                        &draft.device.sides,
                        Edit::Sides
                    ),
                    select(
                        fl!("portal-print-color"),
                        colors,
                        &draft.device.color,
                        Edit::Color
                    )
                ]
                .spacing(12),
            );
            for field in crate::print::worker::SettingField::ALL {
                let control = caps.controls.get(field);
                if !control.choices.is_empty() {
                    let mut choices = vec![choice(None, fl!("portal-print-printer-default"))];
                    choices.extend(
                        control
                            .choices
                            .iter()
                            .map(|c| choice(Some(c.value.clone()), c.label.clone())),
                    );
                    content = content.push(select(
                        field.label(),
                        choices,
                        draft.device.settings.get(field),
                        move |value| Edit::Setting(field, value),
                    ));
                }
            }
            if !draft.custom && dialog.checked.is_none() {
                content = content.push(canopa_ui::action(
                    text(fl!("portal-print-check-settings")),
                    Some(Message::CheckCustomMedia),
                    canopa_ui::Action::Normal,
                    false,
                ));
            }
            if !draft.custom
                && let Some(error) = &dialog.error
            {
                content = content.push(text(error).size(14));
            }
            if let Some((minimum, maximum)) = caps.copies {
                content = content.push(
                    text(fl!(
                        "portal-print-copies-range",
                        minimum = minimum,
                        maximum = maximum
                    ))
                    .size(13)
                    .style(canopa_ui::muted),
                );
            } else {
                content = content.push(text(fl!("portal-print-copies-unknown")).size(13));
            }
        }
        if dialog.target == Some(DestinationChoice::File) {
            let formats = dialog
                .request
                .formats()
                .into_iter()
                .map(|format| choice(format, format.name().to_uppercase()))
                .collect();
            if let Some(format) = draft.format {
                content = content.push(select(
                    fl!("portal-print-file-format"),
                    formats,
                    &format,
                    Edit::Format,
                ));
            }
            content = content.push(
                canopa_ui::direction::row([
                    text(
                        draft
                            .path
                            .as_deref()
                            .map(path_label)
                            .unwrap_or_else(|| fl!("portal-print-choose-file-hint")),
                    )
                    .width(Length::Fill)
                    .into(),
                    canopa_ui::action(
                        text(fl!("portal-print-choose-file")),
                        Some(Message::ChooseFile),
                        canopa_ui::Action::Normal,
                        false,
                    )
                    .into(),
                ])
                .spacing(12)
                .align_y(iced::Center),
            );
            let papers = vec![
                choice((2100, 2970), "A4"),
                choice((2159, 2794), fl!("portal-print-paper-letter")),
                choice((2159, 3556), fl!("portal-print-paper-legal")),
                choice((1480, 2100), "A5"),
            ];
            let paper = draft.geometry[0]
                .parse::<f64>()
                .ok()
                .zip(draft.geometry[1].parse::<f64>().ok())
                .and_then(|(w, h)| {
                    papers
                        .iter()
                        .find(|paper| {
                            (f64::from(paper.value.0) / 10. - w).abs() < 0.001
                                && (f64::from(paper.value.1) / 10. - h).abs() < 0.001
                        })
                        .cloned()
                });
            content = content.push(
                column![
                    text(fl!("portal-print-paper")).size(14),
                    pick_list(papers, paper, |choice| Message::Edit(Edit::Paper(choice)))
                        .placeholder(fl!("portal-print-custom-size"))
                        .width(Length::Fill)
                ]
                .spacing(4),
            );
            content = content
                .push(
                    row![
                        field(
                            fl!("portal-print-width"),
                            &draft.geometry[0],
                            "print-width",
                            |v| Edit::Geometry(0, v)
                        ),
                        field(
                            fl!("portal-print-height"),
                            &draft.geometry[1],
                            "print-height",
                            |v| Edit::Geometry(1, v)
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        field(
                            fl!("portal-print-margin-top"),
                            &draft.geometry[2],
                            "print-top",
                            |v| Edit::Geometry(2, v)
                        ),
                        field(
                            fl!("portal-print-margin-bottom"),
                            &draft.geometry[3],
                            "print-bottom",
                            |v| Edit::Geometry(3, v)
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        field(
                            fl!("portal-print-margin-left"),
                            &draft.geometry[4],
                            "print-left",
                            |v| Edit::Geometry(4, v)
                        ),
                        field(
                            fl!("portal-print-margin-right"),
                            &draft.geometry[5],
                            "print-right",
                            |v| Edit::Geometry(5, v)
                        )
                    ]
                    .spacing(12),
                );
        }
        if dialog.target.is_some() {
            let mut pages = vec![
                choice(Pages::All, fl!("portal-print-all-pages")),
                choice(Pages::Ranges, fl!("portal-print-page-ranges")),
            ];
            if request.page_choices.current {
                pages.push(choice(Pages::Current, fl!("portal-print-current-page")));
            }
            if request.page_choices.selection {
                pages.push(choice(Pages::Selection, fl!("portal-print-selection")));
            }
            content = content.push(select(
                fl!("portal-print-pages"),
                pages,
                &draft.layout.pages,
                Edit::Pages,
            ));
            if draft.layout.pages == Pages::Ranges {
                content = content.push(field(
                    fl!("portal-print-page-ranges-field"),
                    &draft.ranges,
                    "print-ranges",
                    Edit::Ranges,
                ));
            }
            content = content
                .push(
                    row![
                        field(
                            fl!("portal-print-copies"),
                            &draft.copies,
                            "print-copies",
                            Edit::Copies
                        ),
                        field(
                            fl!("portal-print-scale"),
                            &draft.scale,
                            "print-scale",
                            Edit::Scale
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        checkbox(draft.layout.collate)
                            .label(fl!("portal-print-collate"))
                            .on_toggle(|value| Message::Edit(Edit::Collate(value)))
                            .style(canopa_ui::checkbox_style),
                        checkbox(draft.layout.reverse)
                            .label(fl!("portal-print-reverse"))
                            .on_toggle(|value| Message::Edit(Edit::Reverse(value)))
                            .style(canopa_ui::checkbox_style)
                    ]
                    .spacing(20),
                )
                .push(select(
                    fl!("portal-print-orientation"),
                    vec![
                        choice(Orientation::Portrait, fl!("portal-print-portrait")),
                        choice(Orientation::Landscape, fl!("portal-print-landscape")),
                        choice(
                            Orientation::ReversePortrait,
                            fl!("portal-print-reverse-portrait"),
                        ),
                        choice(
                            Orientation::ReverseLandscape,
                            fl!("portal-print-reverse-landscape"),
                        ),
                    ],
                    &draft.layout.orientation,
                    Edit::Orientation,
                ))
                .push(
                    row![
                        select(
                            fl!("portal-print-page-set"),
                            vec![
                                choice("all".to_owned(), fl!("portal-print-page-set-all")),
                                choice("even".into(), fl!("portal-print-even-pages")),
                                choice("odd".into(), fl!("portal-print-odd-pages"))
                            ],
                            &draft.layout.page_set,
                            Edit::PageSet
                        ),
                        select(
                            fl!("portal-print-pages-per-sheet"),
                            (1..=16)
                                .map(|value| choice(
                                    value,
                                    canopa_i18n::format::integer(value.into())
                                ))
                                .collect(),
                            &draft.layout.number_up,
                            Edit::NumberUp
                        )
                    ]
                    .spacing(12),
                )
                .push(select(
                    fl!("portal-print-arrangement"),
                    [
                        "lrtb", "lrbt", "rltb", "rlbt", "tblr", "tbrl", "btlr", "btrl",
                    ]
                    .into_iter()
                    .map(|value| {
                        choice(
                            value.to_owned(),
                            fl!("portal-print-arrangement-order", order = value),
                        )
                    })
                    .collect(),
                    &draft.layout.number_up_layout,
                    Edit::NumberLayout,
                ));
        }
    }
    if let Some(error) = &dialog.error
        && dialog.capabilities.is_none()
    {
        content = content.push(text(error).size(14));
    }
    if dialog.target.is_some()
        && !dialog.loading
        && draft
            .values(
                &dialog.request,
                dialog.target == Some(DestinationChoice::File),
            )
            .is_err()
    {
        content = content.push(text(fl!("portal-print-check-values")).size(14));
    }
    let label = if matches!(dialog.request.purpose, Purpose::Prepare) {
        request.accept_label.clone()
    } else {
        fl!("portal-print-accept")
    };
    let actions = canopa_ui::direction::row([
        canopa_ui::action(
            text(fl!("portal-cancel")),
            Some(Message::Cancel),
            canopa_ui::Action::Normal,
            false,
        )
        .into(),
        iced::widget::Space::new().width(Length::Fill).into(),
        canopa_ui::action(
            text(label),
            dialog.grant().map(|_| Message::Approve(dialog.generation)),
            canopa_ui::Action::Normal,
            true,
        )
        .into(),
    ])
    .spacing(10);
    let content = container(content)
        .width(Length::Fill)
        .padding(iced::Padding {
            right: 14.,
            ..Default::default()
        });
    container(column![scrollable(content).height(Length::Fill), actions].spacing(16))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(canopa_ui::surface)
        .into()
}
