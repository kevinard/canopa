use super::*;
use crate::dialog_ui;
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
/// Common PWG paper keywords have familiar names. The full keyword is still
/// carried by the choice, and unrecognized variants keep their printer label.
fn paper_label(id: &str) -> String {
    match id {
        "iso_a3_297x420mm" => "A3".into(),
        "iso_a4_210x297mm" => "A4".into(),
        "iso_a5_148x210mm" => "A5".into(),
        "na_letter_8.5x11in" => fl!("portal-print-paper-letter"),
        "na_legal_8.5x14in" => fl!("portal-print-paper-legal"),
        _ => id.to_owned(),
    }
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
        dialog_ui::section(label.clone()),
        text_input(&label, value)
            .id(id)
            .style(dialog_ui::field)
            .padding([7, 10])
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
            .style(dialog_ui::surface)
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
        dialog_ui::heading(if request.context.title.is_empty() {
            fl!("portal-print-title")
        } else {
            request.context.title.clone()
        }),
        dialog_ui::detail(fl!(
            "portal-requested-by",
            requester = dialog.requester.as_str()
        )),
    ]
    .spacing(12)
    .width(Length::Fill);

    if let Some(error) = &dialog.error
        && dialog.target.is_none()
        && !dialog.loading
        && dialog.printers.is_empty()
    {
        let mut actions = vec![
            dialog_ui::action(
                text(fl!("portal-cancel")),
                Some(Message::Cancel),
                canopa_ui::Action::Quiet,
                false,
            )
            .into(),
            dialog_ui::action(
                text(fl!("portal-print-retry")),
                Some(Message::Refresh),
                canopa_ui::Action::Normal,
                false,
            )
            .into(),
        ];
        let formats = dialog.request.formats();
        if !formats.is_empty() {
            actions.push(
                dialog_ui::action(
                    text(if formats.as_slice() == [crate::print::Format::Pdf] {
                        fl!("portal-print-save-pdf")
                    } else {
                        fl!("portal-print-to-file")
                    }),
                    Some(Message::Destination(choice(
                        DestinationChoice::File,
                        fl!("portal-print-to-file"),
                    ))),
                    canopa_ui::Action::Primary,
                    false,
                )
                .into(),
            );
        }
        let guidance = column![
            text(fl!("portal-print-printers-unavailable")).size(18),
            text(error).size(14),
        ]
        .spacing(10);
        content = content.push(
            container(guidance.push(dialog_ui::footer(actions)))
                .padding(16)
                .width(Length::Fill)
                .style(dialog_ui::card),
        );
        return container(
            scrollable(container(content).padding([18, 24]).width(Length::Fill))
                .style(canopa_ui::scrollbar)
                .height(Length::Shrink),
        )
        .center(Length::Fill)
        .style(dialog_ui::surface)
        .into();
    }
    let printer = match &dialog.target {
        Some(DestinationChoice::Printer(destination)) => dialog
            .printers
            .iter()
            .find(|printer| &printer.destination == destination),
        _ => None,
    };
    let mut destination_heading = vec![
        dialog_ui::section(fl!("portal-print-destination"))
            .width(Length::Fill)
            .into(),
    ];
    if printer.is_some_and(|printer| printer.accepting && printer.state == PrinterState::Idle) {
        destination_heading.push(dialog_ui::detail(fl!("portal-print-idle")).into());
    }
    content = content
        .push(canopa_ui::direction::row(destination_heading).align_y(iced::Center))
        .push(
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
            .align_y(iced::Center),
        );
    if let Some(notice) = dialog.notice.clone() {
        content = content.push(text(notice).size(14).style(canopa_ui::muted));
    }
    if dialog.loading && dialog.capabilities.is_none() {
        content = content.push(text(fl!("portal-print-loading")).style(canopa_ui::muted));
    } else if dialog.printers.is_empty() && dialog.target.is_none() && dialog.error.is_none() {
        content = content.push(text(fl!("portal-print-no-printers")).style(canopa_ui::muted));
    }
    if let Some(printer) = printer {
        if !printer.location.is_empty() {
            content = content.push(text(&printer.location).style(canopa_ui::muted));
        }
        let state = match (printer.accepting, printer.state) {
            (false, _) => Some(fl!("portal-print-not-accepting")),
            (_, PrinterState::Stopped) => Some(fl!("portal-print-stopped")),
            (_, PrinterState::Processing) => Some(fl!("portal-print-busy")),
            _ => None,
        };
        if let Some(state) = state {
            content = content.push(dialog_ui::detail(state));
        }
    }
    if !dialog.loading || dialog.capabilities.is_some() {
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
        }
        if dialog.target.is_some() {
            content = content.push(
                canopa_ui::direction::row([
                    dialog_ui::tab(
                        fl!("portal-print-document-tab"),
                        dialog.section == Section::Document,
                        Some(Message::Section(Section::Document)),
                    ),
                    dialog_ui::tab(
                        if dialog.target == Some(DestinationChoice::File) {
                            fl!("portal-print-paper-tab")
                        } else {
                            fl!("portal-print-printer-tab")
                        },
                        dialog.section == Section::Paper,
                        Some(Message::Section(Section::Paper)),
                    ),
                ])
                .spacing(8),
            );
        }
        let mut options = column![].spacing(6).width(Length::Fill);
        if dialog.section == Section::Paper {
            if let Some(caps) = &dialog.capabilities {
                let mut media = vec![choice(None, fl!("portal-print-printer-default"))];
                media.extend(caps.media.iter().map(|media| {
                    choice(
                        Some(media.id.clone()),
                        fl!(
                            "portal-print-media",
                            name = paper_label(&media.id),
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
                    options = options.push(
                        checkbox(draft.custom)
                            .label(fl!("portal-print-custom-paper"))
                            .on_toggle(|value| Message::Edit(Edit::Custom(value)))
                            .style(canopa_ui::checkbox_style),
                    );
                }
                if draft.custom {
                    options = options
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
                        options = options.push(
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
                        options = options.push(
                            text(fl!("portal-print-custom-paper-hint"))
                                .size(13)
                                .style(canopa_ui::muted),
                        );
                    }
                    if let Some(error) = &dialog.error {
                        options = options.push(text(error).size(14));
                    }
                } else {
                    options = options.push(select(
                        fl!("portal-print-paper"),
                        media,
                        &draft.device.media,
                        Edit::Media,
                    ));
                }
                options = options.push(
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
                let mut controls = crate::print::worker::SettingField::ALL
                    .into_iter()
                    .filter_map(|field| {
                        let control = caps.controls.get(field);
                        let mut choices = vec![choice(None, fl!("portal-print-printer-default"))];
                        choices.extend(
                            control
                                .choices
                                .iter()
                                .map(|c| choice(Some(c.value.clone()), c.label.clone())),
                        );
                        (!control.choices.is_empty()).then(|| {
                            select(
                                field.label(),
                                choices,
                                draft.device.settings.get(field),
                                move |value| Edit::Setting(field, value),
                            )
                        })
                    });
                while let Some(first) = controls.next() {
                    let second = controls
                        .next()
                        .unwrap_or_else(|| iced::widget::space().width(Length::Fill).into());
                    options = options.push(canopa_ui::direction::row([first, second]).spacing(12));
                }
                if !draft.custom && dialog.checked.is_none() {
                    options = options.push(canopa_ui::action(
                        text(fl!("portal-print-check-settings")),
                        Some(Message::CheckCustomMedia),
                        canopa_ui::Action::Normal,
                        false,
                    ));
                }
                if !draft.custom
                    && let Some(error) = &dialog.error
                {
                    options = options.push(text(error).size(14));
                }
            }
            if dialog.target == Some(DestinationChoice::File) {
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
                options = options.push(
                    column![
                        text(fl!("portal-print-paper")).size(14),
                        pick_list(papers, paper, |choice| Message::Edit(Edit::Paper(choice)))
                            .placeholder(fl!("portal-print-custom-size"))
                            .width(Length::Fill)
                    ]
                    .spacing(4),
                );
                options = options
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
        } else if dialog.target.is_some() {
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
            options = options.push(
                canopa_ui::direction::row([
                    select(
                        fl!("portal-print-pages"),
                        pages,
                        &draft.layout.pages,
                        Edit::Pages,
                    ),
                    select(
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
                    ),
                ])
                .spacing(12),
            );
            if draft.layout.pages == Pages::Ranges {
                options = options.push(field(
                    fl!("portal-print-page-ranges-field"),
                    &draft.ranges,
                    "print-ranges",
                    Edit::Ranges,
                ));
            }
            options = options
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
                );
            if draft.layout.number_up > 1 {
                options = options.push(select(
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
            if let Some(caps) = &dialog.capabilities {
                options = options.push(
                    dialog_ui::detail(match caps.copies {
                        Some((minimum, maximum)) => fl!(
                            "portal-print-copies-range",
                            minimum = minimum,
                            maximum = maximum
                        ),
                        None => fl!("portal-print-copies-unknown"),
                    })
                    .size(13),
                );
            }
        }
        if dialog.target.is_some() {
            content = content.push(options);
        }
    }
    if let Some(error) = &dialog.error
        && dialog.capabilities.is_none()
    {
        content = content.push(text(error).size(14).style(text::danger));
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
        dialog_ui::detail(if dialog.loading && dialog.capabilities.is_some() {
            fl!("portal-print-checking-printer")
        } else if dialog.target == Some(DestinationChoice::File) && draft.path.is_none() {
            fl!("portal-print-awaiting-destination")
        } else {
            String::new()
        })
        .width(Length::Fill)
        .into(),
        dialog_ui::action(
            text(fl!("portal-cancel")),
            Some(Message::Cancel),
            canopa_ui::Action::Normal,
            false,
        )
        .into(),
        dialog_ui::action(
            text(label),
            dialog.grant().map(|_| Message::Approve(dialog.generation)),
            canopa_ui::Action::Primary,
            false,
        )
        .into(),
    ])
    .spacing(8)
    .align_y(iced::Center);
    dialog_ui::layout(content, actions)
}
