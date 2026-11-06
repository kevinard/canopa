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

fn choice<T>(value: T, label: impl Into<String>) -> Choice<T> {
    Choice {
        value,
        label: label.into(),
    }
}
fn select<'a, T: Clone + PartialEq + 'a>(
    label: &'a str,
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
    label: &'a str,
    value: &'a str,
    id: &'static str,
    edit: impl Fn(String) -> Edit + 'static,
) -> Element<'a, Message> {
    column![
        text(label).size(14),
        text_input(label, value)
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
                    text("Preparing destination picker…"),
                    canopa_ui::action(
                        text("Back"),
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
        choices.push(choice(DestinationChoice::File, "Print to file"));
    }
    let selected = choices
        .iter()
        .find(|choice| Some(&choice.value) == dialog.target.as_ref())
        .cloned();
    let destination = pick_list(choices, selected, Message::Destination)
        .placeholder("Choose a destination")
        .width(Length::Fill);
    let mut content = column![
        text("Print").size(24),
        text(if request.context.title.is_empty() {
            "Document"
        } else {
            &request.context.title
        })
        .size(18),
        text(if request.context.app_id.is_empty() {
            "Requested by an application"
        } else {
            &request.context.app_id
        })
        .style(canopa_ui::muted),
        text("Destination").size(14),
        row![
            destination,
            canopa_ui::action(
                text("Refresh"),
                Some(Message::Refresh),
                canopa_ui::Action::Normal,
                false
            )
        ]
        .spacing(8)
        .align_y(iced::Center)
    ]
    .spacing(12)
    .width(Length::Fill);
    if let Some(notice) = dialog.notice {
        content = content.push(text(notice).size(14).style(canopa_ui::muted));
    }
    if dialog.loading {
        content = content.push(text("Loading printer information…").style(canopa_ui::muted));
    } else if dialog.printers.is_empty() && dialog.target.is_none() && dialog.error.is_none() {
        content = content.push(
            text("No printers are available. Configure a printer, then refresh.")
                .style(canopa_ui::muted),
        );
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
            (false, _) => "This printer is not accepting jobs.",
            (_, PrinterState::Stopped) => {
                "This printer is stopped. Submitted jobs may wait in its queue."
            }
            (_, PrinterState::Processing) => "This printer is busy. Submitted jobs will be queued.",
            _ => "Print jobs are sent to this printer’s queue.",
        };
        content = content.push(text(state).size(14).style(canopa_ui::muted));
    }
    if !dialog.loading {
        if let Some(caps) = &dialog.capabilities {
            let mut media = vec![choice(None, "Printer default")];
            media.extend(caps.media.iter().map(|media| {
                choice(
                    Some(media.id.clone()),
                    format!("{} — {} × {} mm", media.id, media.width_mm, media.height_mm),
                )
            }));
            let mut sides = vec![choice(None, "Printer default")];
            sides.extend(caps.sides.iter().map(|value| {
                choice(
                    Some(*value),
                    match value {
                        Sides::OneSided => "One-sided",
                        Sides::LongEdge => "Two-sided, long edge",
                        Sides::ShortEdge => "Two-sided, short edge",
                    },
                )
            }));
            let mut colors = vec![choice(None, "Printer default")];
            colors.extend(
                caps.colors
                    .iter()
                    .map(|value| choice(Some(*value), if *value { "Color" } else { "Monochrome" })),
            );
            if caps.custom_media {
                content = content.push(
                    checkbox(draft.custom)
                        .label("Custom paper size")
                        .on_toggle(|value| Message::Edit(Edit::Custom(value)))
                        .style(canopa_ui::checkbox_style),
                );
            }
            if draft.custom {
                content = content
                    .push(
                        row![
                            field(
                                "Width (mm)",
                                &draft.custom_dimensions[0],
                                "print-custom-width",
                                |value| Edit::CustomDimension(0, value)
                            ),
                            field(
                                "Height (mm)",
                                &draft.custom_dimensions[1],
                                "print-custom-height",
                                |value| Edit::CustomDimension(1, value)
                            )
                        ]
                        .spacing(12),
                    )
                    .push(canopa_ui::action(
                        text("Check paper size"),
                        dialog.custom_size().map(|_| Message::CheckCustomMedia),
                        canopa_ui::Action::Normal,
                        false,
                    ));
                if let Some(media) = &draft.device.custom_media
                    && dialog.checked.is_some()
                {
                    content = content.push(
                        text(format!(
                            "Paper confirmed. Margins (mm): top {}, bottom {}, left {}, right {}.",
                            media.top_mm, media.bottom_mm, media.left_mm, media.right_mm
                        ))
                        .size(13)
                        .style(canopa_ui::muted),
                    );
                } else {
                    content = content.push(text("Enter millimetres with up to two decimal places, then check this size with the printer.").size(13).style(canopa_ui::muted));
                }
                if let Some(error) = &dialog.error {
                    content = content.push(text(error).size(14));
                }
            } else {
                content = content.push(select("Paper", media, &draft.device.media, Edit::Media));
            }
            content = content.push(
                row![
                    select("Sides", sides, &draft.device.sides, Edit::Sides),
                    select("Color", colors, &draft.device.color, Edit::Color)
                ]
                .spacing(12),
            );
            for field in crate::print::worker::SettingField::ALL {
                let control = caps.controls.get(field);
                if !control.choices.is_empty() {
                    let mut choices = vec![choice(None, "Printer default")];
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
                    text("Check printer settings"),
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
                    text(format!("Printer supports {minimum}–{maximum} copies."))
                        .size(13)
                        .style(canopa_ui::muted),
                );
            } else {
                content = content.push(
                    text("The printer did not report a supported copy count. Refresh to retry.")
                        .size(13),
                );
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
                content = content.push(select("File format", formats, &format, Edit::Format));
            }
            content = content.push(
                row![
                    text(draft.path.as_deref().map(path_label).unwrap_or_else(|| {
                        "Choose where to save the printed document.".into()
                    }))
                    .width(Length::Fill),
                    canopa_ui::action(
                        text("Choose file…"),
                        Some(Message::ChooseFile),
                        canopa_ui::Action::Normal,
                        false
                    )
                ]
                .spacing(12)
                .align_y(iced::Center),
            );
            let papers = vec![
                choice((2100, 2970), "A4"),
                choice((2159, 2794), "Letter"),
                choice((2159, 3556), "Legal"),
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
                    text("Paper").size(14),
                    pick_list(papers, paper, |choice| Message::Edit(Edit::Paper(choice)))
                        .placeholder("Custom size")
                        .width(Length::Fill)
                ]
                .spacing(4),
            );
            content = content
                .push(
                    row![
                        field("Width (mm)", &draft.geometry[0], "print-width", |v| {
                            Edit::Geometry(0, v)
                        }),
                        field("Height (mm)", &draft.geometry[1], "print-height", |v| {
                            Edit::Geometry(1, v)
                        })
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        field("Top margin (mm)", &draft.geometry[2], "print-top", |v| {
                            Edit::Geometry(2, v)
                        }),
                        field(
                            "Bottom margin (mm)",
                            &draft.geometry[3],
                            "print-bottom",
                            |v| Edit::Geometry(3, v)
                        )
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        field("Left margin (mm)", &draft.geometry[4], "print-left", |v| {
                            Edit::Geometry(4, v)
                        }),
                        field(
                            "Right margin (mm)",
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
                choice(Pages::All, "All pages"),
                choice(Pages::Ranges, "Page ranges"),
            ];
            if request.page_choices.current {
                pages.push(choice(Pages::Current, "Current page"));
            }
            if request.page_choices.selection {
                pages.push(choice(Pages::Selection, "Selection"));
            }
            content = content.push(select("Pages", pages, &draft.layout.pages, Edit::Pages));
            if draft.layout.pages == Pages::Ranges {
                content = content.push(field(
                    "Page ranges (for example 1-3, 5)",
                    &draft.ranges,
                    "print-ranges",
                    Edit::Ranges,
                ));
            }
            content = content
                .push(
                    row![
                        field("Copies", &draft.copies, "print-copies", Edit::Copies),
                        field("Scale (%)", &draft.scale, "print-scale", Edit::Scale)
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        checkbox(draft.layout.collate)
                            .label("Collate copies")
                            .on_toggle(|value| Message::Edit(Edit::Collate(value)))
                            .style(canopa_ui::checkbox_style),
                        checkbox(draft.layout.reverse)
                            .label("Reverse order")
                            .on_toggle(|value| Message::Edit(Edit::Reverse(value)))
                            .style(canopa_ui::checkbox_style)
                    ]
                    .spacing(20),
                )
                .push(select(
                    "Orientation",
                    vec![
                        choice(Orientation::Portrait, "Portrait"),
                        choice(Orientation::Landscape, "Landscape"),
                        choice(Orientation::ReversePortrait, "Reverse portrait"),
                        choice(Orientation::ReverseLandscape, "Reverse landscape"),
                    ],
                    &draft.layout.orientation,
                    Edit::Orientation,
                ))
                .push(
                    row![
                        select(
                            "Page set",
                            vec![
                                choice("all".to_owned(), "All"),
                                choice("even".into(), "Even pages"),
                                choice("odd".into(), "Odd pages")
                            ],
                            &draft.layout.page_set,
                            Edit::PageSet
                        ),
                        select(
                            "Pages per sheet",
                            (1..=16)
                                .map(|value| choice(value, value.to_string()))
                                .collect(),
                            &draft.layout.number_up,
                            Edit::NumberUp
                        )
                    ]
                    .spacing(12),
                )
                .push(select(
                    "Page arrangement",
                    [
                        ("lrtb", "Left to right, top to bottom"),
                        ("lrbt", "Left to right, bottom to top"),
                        ("rltb", "Right to left, top to bottom"),
                        ("rlbt", "Right to left, bottom to top"),
                        ("tblr", "Top to bottom, left to right"),
                        ("tbrl", "Top to bottom, right to left"),
                        ("btlr", "Bottom to top, left to right"),
                        ("btrl", "Bottom to top, right to left"),
                    ]
                    .into_iter()
                    .map(|(value, label)| choice(value.to_owned(), label))
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
        content = content
            .push(text("Check the copies, scale, page ranges and paper dimensions.").size(14));
    }
    let label = if matches!(dialog.request.purpose, Purpose::Prepare) {
        &request.accept_label
    } else {
        "Print"
    };
    let actions = row![
        canopa_ui::action(
            text("Cancel"),
            Some(Message::Cancel),
            canopa_ui::Action::Normal,
            false
        ),
        iced::widget::Space::new().width(Length::Fill),
        canopa_ui::action(
            text(label),
            dialog.grant().map(|_| Message::Approve(dialog.generation)),
            canopa_ui::Action::Normal,
            true
        )
    ]
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
