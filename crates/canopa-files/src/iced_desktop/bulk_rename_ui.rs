//! Rename of several items: a rule, its fields and a live preview. The draft
//! keeps the selection it was opened for; the worker observes those items
//! again when the batch is accepted.
use super::file_operations::{Event, Origin, Submission};
use super::name_dialog::{Dialog, control};
use super::*;
use crate::{
    bulk_rename::{Item, Plan, Problem, Rule},
    operation_jobs::Request,
    operations::OperationRequest,
};
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Length,
    widget::{column, container, row, scrollable, text, text_input},
};
use std::{collections::BTreeSet, ffi::OsString};

/// Rows drawn in the preview; the rest are counted below it.
const PREVIEW_ROWS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Replace,
    Add,
    Number,
}

impl Mode {
    const ALL: [Self; 3] = [Self::Replace, Self::Add, Self::Number];

    fn label(self) -> String {
        match self {
            Self::Replace => fl!("files-rename-batch-mode-replace"),
            Self::Add => fl!("files-rename-batch-mode-add"),
            Self::Number => fl!("files-rename-batch-mode-number"),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Replace => "batch-mode-replace",
            Self::Add => "batch-mode-add",
            Self::Number => "batch-mode-number",
        }
    }

    /// The fields this mode shows, in order.
    fn fields(self) -> [Field; 2] {
        match self {
            Self::Replace => [Field::Find, Field::With],
            Self::Add => [Field::Prefix, Field::Suffix],
            Self::Number => [Field::Base, Field::Start],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Find,
    With,
    Prefix,
    Suffix,
    Base,
    Start,
}

impl Field {
    fn key(self) -> &'static str {
        match self {
            Self::Find => "batch-find",
            Self::With => "batch-with",
            Self::Prefix => "batch-prefix",
            Self::Suffix => "batch-suffix",
            Self::Base => "batch-base",
            Self::Start => "batch-start",
        }
    }

    fn label(self) -> String {
        match self {
            Self::Find => fl!("files-rename-batch-find"),
            Self::With => fl!("files-rename-batch-replace-with"),
            Self::Prefix => fl!("files-rename-batch-prefix"),
            Self::Suffix => fl!("files-rename-batch-suffix"),
            Self::Base => fl!("files-rename-batch-base"),
            Self::Start => fl!("files-rename-batch-start"),
        }
    }
}

pub(super) fn field_id(window: window::Id, field: Field) -> iced::widget::Id {
    control(window, field.key())
}

pub(super) struct Batch {
    mode: Mode,
    find: String,
    with: String,
    prefix: String,
    suffix: String,
    base: String,
    start: String,
    items: Vec<Item>,
    existing: BTreeSet<OsString>,
    plan: Plan,
}

impl Batch {
    fn new(items: Vec<Item>, existing: BTreeSet<OsString>) -> Self {
        let mut batch = Self {
            mode: Mode::Replace,
            find: String::new(),
            with: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            base: String::new(),
            start: "1".into(),
            items,
            existing,
            plan: Plan::default(),
        };
        batch.update();
        batch
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn first_field(&self) -> Field {
        self.mode.fields()[0]
    }

    fn value(&self, field: Field) -> &str {
        match field {
            Field::Find => &self.find,
            Field::With => &self.with,
            Field::Prefix => &self.prefix,
            Field::Suffix => &self.suffix,
            Field::Base => &self.base,
            Field::Start => &self.start,
        }
    }

    fn value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Find => &mut self.find,
            Field::With => &mut self.with,
            Field::Prefix => &mut self.prefix,
            Field::Suffix => &mut self.suffix,
            Field::Base => &mut self.base,
            Field::Start => &mut self.start,
        }
    }

    /// The rule the fields describe; `None` while the start is not a number.
    fn rule(&self) -> Option<Rule> {
        Some(match self.mode {
            Mode::Replace => Rule::Replace {
                find: self.find.clone(),
                with: self.with.clone(),
            },
            Mode::Add => Rule::Add {
                prefix: self.prefix.clone(),
                suffix: self.suffix.clone(),
            },
            Mode::Number => Rule::Number {
                name: self.base.clone(),
                start: self.start.trim().parse().ok()?,
            },
        })
    }

    fn update(&mut self) {
        self.plan = match self.rule() {
            Some(rule) => crate::bulk_rename::plan(&rule, &self.items, &self.existing),
            None => Plan::default(),
        };
    }

    #[cfg(test)]
    pub fn preview(&self) -> Vec<String> {
        self.plan
            .rows
            .iter()
            .map(|row| {
                let mut line = row.name.to_string_lossy().into_owned();
                if let Some(problem) = row.problem {
                    line.push_str(&format!(" [{}]", problem_label(problem)));
                }
                line
            })
            .collect()
    }
}

fn problem_label(problem: Problem) -> String {
    match problem {
        Problem::Invalid => fl!("files-rename-batch-invalid"),
        Problem::Duplicate => fl!("files-rename-batch-duplicate"),
        Problem::Exists => fl!("files-rename-batch-exists"),
        Problem::Swap => fl!("files-rename-batch-swap"),
    }
}

impl Desktop {
    /// Rename the selected items of one folder together.
    pub(super) fn open_batch_dialog(&mut self, origin: Arc<Origin>) -> Task<Message> {
        let Some(folder) = super::name_dialog::batch_folder(&origin.selected).map(Path::to_owned)
        else {
            return Task::none();
        };
        // Number counts in the order the items are shown.
        let mut items: Vec<Item> = Vec::new();
        if let Some(browser) = self
            .active(origin.window)
            .and_then(|tab| tab.browser.as_ref())
        {
            for entry in browser.visible_entries() {
                if origin.selected.contains(&entry.path) {
                    items.push(Item {
                        path: entry.path.clone(),
                        directory: entry.is_directory(),
                    });
                }
            }
        }
        for path in &origin.selected {
            if !items.iter().any(|item| &item.path == path) {
                items.push(Item {
                    path: path.clone(),
                    directory: false,
                });
            }
        }
        let existing = self.listed_names(origin.window, &folder);
        let batch = Box::new(Batch::new(items, existing));
        self.open_dialog(origin, String::new(), (560.0, 540.0), |dialog| {
            dialog.batch = Some(batch)
        })
    }

    /// The open batch draft `id`, while it accepts changes.
    fn batch_draft(&mut self, id: window::Id) -> Option<&mut Dialog> {
        self.file_operations
            .name
            .as_mut()
            .filter(|dialog| dialog.window == id && !dialog.busy && dialog.batch.is_some())
    }

    pub(super) fn batch_mode(&mut self, id: window::Id, mode: Mode) -> Task<Message> {
        let Some(dialog) = self.batch_draft(id) else {
            return Task::none();
        };
        let Some(batch) = dialog.batch.as_deref_mut() else {
            return Task::none();
        };
        if batch.mode == mode {
            return Task::none();
        }
        batch.mode = mode;
        batch.update();
        dialog.error = None;
        iced::widget::operation::focus(field_id(id, mode.fields()[0]))
    }

    pub(super) fn batch_field(&mut self, id: window::Id, field: Field, value: String) {
        if let Some(dialog) = self.batch_draft(id)
            && let Some(batch) = dialog.batch.as_deref_mut()
        {
            *batch.value_mut(field) = value;
            batch.update();
            dialog.error = None;
        }
    }

    pub(super) fn submit_batch_dialog(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.batch_draft(id) else {
            return Task::none();
        };
        let Some(batch) = dialog.batch.as_deref() else {
            return Task::none();
        };
        if batch.rule().is_none() {
            dialog.error = Some(fl!("files-rename-batch-start-invalid"));
            return Task::none();
        }
        let Some(renames) = batch.plan.renames() else {
            dialog.error = Some(if batch.plan.problems() > 0 {
                fl!(
                    "files-rename-batch-problems",
                    problems = batch.plan.problems()
                )
            } else {
                fl!("files-rename-batch-nothing")
            });
            return Task::none();
        };
        let submission = Submission {
            origin: Some(dialog.origin.clone()),
            owner: dialog.origin.window,
            dialog: Some(id),
            affected: renames
                .iter()
                .flat_map(|step| [step.source.clone(), step.destination.clone()])
                .collect(),
            reveal: None,
            refresh_all: false,
        };
        dialog.busy = true;
        dialog.error = None;
        self.submit_file_operation(
            Request::Mutation(OperationRequest::RenameBatch { renames }),
            submission,
        )
    }

    #[cfg(test)]
    pub(super) fn batch_preview(&self) -> Option<Vec<String>> {
        Some(
            self.file_operations
                .name
                .as_ref()?
                .batch
                .as_ref()?
                .preview(),
        )
    }
}

pub(super) fn view<'a>(
    desktop: &'a Desktop,
    dialog: &'a Dialog,
    batch: &'a Batch,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let muted = Some(chrome::ink(appearance, ColorRole::Muted));
    let busy = dialog.busy;

    let modes = Mode::ALL.iter().map(|mode| {
        let selected = *mode == batch.mode;
        let message = (!busy).then_some(Message::FileOperation(Event::BatchMode(id, *mode)));
        super::focus::radio(
            control(id, mode.key()),
            selected,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(mode.label()).font(font).size(small + 1.0),
                message,
                if selected {
                    canopa_ui::Action::Primary
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            ),
        )
    });

    let field = |field: Field| -> Element<'a, Message> {
        let value = batch.value(field);
        let mut input = text_input("", value)
            .id(field_id(id, field))
            .font(font)
            .size(size)
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| super::view::input_style(appearance, theme, status));
        if !busy {
            input = input
                .on_input(move |value| Message::FileOperation(Event::BatchField(id, field, value)))
                .on_submit(Message::FileOperation(Event::NameSubmit(id)));
        }
        let mut node = super::accessibility_ui::Node::new(super::accessibility_ui::Role::TextInput)
            .key(format!("files-{}", field.key()))
            .label(field.label())
            .value(value.to_owned())
            .read_only(busy);
        if !busy {
            node = node
                .on_text(move |value| Message::FileOperation(Event::BatchField(id, field, value)));
        }
        column![
            chrome::section_title(appearance, field.label()).size(small + 1.0),
            super::accessibility_ui::semantic(node, input),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    };
    let [first, second] = batch.mode.fields();
    let fields = canopa_ui::direction::row([field(first), field(second)]).spacing(10);

    let status = match (&dialog.error, busy) {
        (Some(error), _) => error.clone(),
        (None, true) => fl!("files-accepting-operation"),
        (None, false) if batch.plan.problems() > 0 => {
            fl!(
                "files-rename-batch-problems",
                problems = batch.plan.problems()
            )
        }
        (None, false) if batch.plan.changes() > 0 => {
            fl!("files-rename-batch-summary", changes = batch.plan.changes())
        }
        (None, false) => fl!("files-rename-batch-nothing"),
    };
    let alarming = dialog.error.is_some() || batch.plan.problems() > 0;
    let feedback = if dialog.error.is_some() {
        super::accessibility_ui::Node::alert(status.clone()).key("files-name-error")
    } else {
        super::accessibility_ui::Node::status(status.clone()).key("files-name-busy")
    };
    let status = super::accessibility_ui::semantic(
        feedback,
        text(status)
            .font(font)
            .size(small)
            .color(chrome::ink(
                appearance,
                if alarming {
                    ColorRole::Danger
                } else {
                    ColorRole::Muted
                },
            ))
            .wrapping(text::Wrapping::None),
    );

    let mut rows = column![].spacing(6);
    for planned in batch.plan.rows.iter().take(PREVIEW_ROWS) {
        let original = planned
            .source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let name = planned.name.to_string_lossy().into_owned();
        let ink = match (planned.problem, planned.changed()) {
            (Some(_), _) => Some(chrome::ink(appearance, ColorRole::Danger)),
            (None, true) => None,
            (None, false) => muted,
        };
        let mut new_name = column![chrome::single_line(name, font, small + 1.0, ink, true)];
        if let Some(problem) = planned.problem {
            new_name = new_name.push(chrome::single_line(
                problem_label(problem),
                font,
                small,
                Some(chrome::ink(appearance, ColorRole::Danger)),
                true,
            ));
        }
        rows = rows.push(
            canopa_ui::direction::row([
                container(chrome::single_line(
                    original,
                    font,
                    small + 1.0,
                    muted,
                    true,
                ))
                .width(Length::FillPortion(1))
                .into(),
                desktop.icons.chrome(chrome::Icon::Forward, 12.0, 0.6),
                container(new_name.spacing(2))
                    .width(Length::FillPortion(1))
                    .into(),
            ])
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
    }
    if let Some(more) = batch
        .plan
        .rows
        .len()
        .checked_sub(PREVIEW_ROWS)
        .filter(|more| *more > 0)
    {
        rows = rows.push(
            text(fl!("files-rename-batch-more", count = more))
                .font(font)
                .size(small)
                .style(canopa_ui::muted),
        );
    }
    let preview = container(
        scrollable(container(rows).padding([8, 10]))
            .id(control(id, "batch-preview"))
            .style(canopa_ui::scrollbar)
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .style(move |_| canopa_ui::frame(appearance, appearance.style(UiKind::Card)));

    let action = |label: String, name, event| {
        let message = (!busy).then_some(Message::FileOperation(event));
        super::focus::button(
            control(id, name),
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(size),
                message,
                if name == "submit" {
                    canopa_ui::Action::Primary
                } else {
                    canopa_ui::Action::Normal
                },
                false,
                appearance,
            )
            .padding(chrome::DIALOG_BUTTON_PADDING),
        )
    };
    let mut title_font = font;
    title_font.weight = iced::font::Weight::Semibold;
    let location = super::display_location(
        batch
            .items
            .first()
            .and_then(|item| item.path.parent())
            .unwrap_or(&dialog.origin.directory),
    );
    let body = column![
        chrome::heading_parts(
            desktop,
            chrome::Icon::Edit,
            chrome::Tone::Accent,
            text(dialog.title())
                .font(title_font)
                .size(appearance.typography().title_size.px())
                .into(),
            Some(chrome::single_line(
                location,
                font,
                small + 1.0,
                muted,
                true
            )),
            None,
        ),
        canopa_ui::direction::row(modes).spacing(6),
        fields,
        text(fl!("files-rename-batch-hint"))
            .font(font)
            .size(small)
            .style(canopa_ui::muted),
        row![
            chrome::section_title(appearance, fl!("files-rename-batch-preview")).size(small + 1.0),
            iced::widget::space().width(Length::Fill),
            status,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        preview,
        canopa_ui::direction::row([
            iced::widget::space().width(Length::Fill).into(),
            action(fl!("files-cancel"), "cancel", Event::NameCancel(id)),
            action(fl!("files-command-rename"), "submit", Event::NameSubmit(id)),
        ])
        .spacing(8),
    ]
    .spacing(12);
    let surface = appearance.clone();
    let body = container(body)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(&surface, true));
    let order = if busy {
        Vec::new()
    } else {
        let mut order: Vec<_> = Mode::ALL
            .iter()
            .map(|mode| control(id, mode.key()))
            .collect();
        order.extend(batch.mode.fields().map(|field| field_id(id, field)));
        order.extend([control(id, "cancel"), control(id, "submit")]);
        order
    };
    super::accessibility_ui::dialog(
        "files-name-dialog",
        dialog.title(),
        fl!("files-rename-batch-hint"),
        false,
        super::focus::group(
            body,
            order,
            Vec::new(),
            (!busy).then(|| field_id(id, batch.first_field())),
        ),
    )
}
