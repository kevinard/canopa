//! Application-owned custom-action drafts. Source snapshots remain immutable
//! until a serialized configuration worker acknowledges Save or Reload.
use super::*;
use crate::actions::{ActionConditions, CustomAction};
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{checkbox, column, container, row, scrollable, text, text_input},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Name,
    Icon,
    Command,
    Patterns,
    Minimum,
    Maximum,
    Shortcut,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Check {
    Files,
    Folders,
    Toolbar,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub window: window::Id,
    pub entry: u64,
    pub selection: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    Save,
    Reload,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Confirmation {
    Close,
    Reload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Select(u64),
    Add,
    Delete,
    Earlier,
    Later,
    Field(Field),
    Check(Check),
    Record,
    Reload,
    Save,
    Keep,
    Discard,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Select(window::Id, u64),
    Add(window::Id),
    Remove(Target),
    Move(Target, bool),
    Edit(Target, Field, String),
    Toggle(Target, Check, bool),
    Save(window::Id),
    Reload(window::Id),
    Record(window::Id),
    RecordKeyboard(window::Id),
    Confirm(window::Id, bool),
    Finished(u64, Option<Arc<configuration_ui::Reply>>),
}

#[derive(Clone)]
struct Draft {
    serial: u64,
    id: String,
    name: String,
    icon: String,
    command: String,
    patterns: String,
    minimum: String,
    maximum: String,
    shortcut: String,
    files: bool,
    folders: bool,
    toolbar: bool,
}

impl Draft {
    fn new(serial: u64, action: &CustomAction) -> Self {
        Self {
            serial,
            id: action.id.clone(),
            name: action.name.clone(),
            icon: action.icon.clone().unwrap_or_default(),
            command: action.command.clone(),
            patterns: action.conditions.patterns.join("; "),
            minimum: action.conditions.minimum_selection.to_string(),
            maximum: action.conditions.maximum_selection.unwrap_or(0).to_string(),
            shortcut: action.shortcut.clone().unwrap_or_default(),
            files: action.conditions.files,
            folders: action.conditions.directories,
            toolbar: action.toolbar,
        }
    }

    fn action(&self) -> Result<CustomAction, String> {
        let count = |value: &str, label: String| {
            value.trim().parse::<usize>().map_err(|_| {
                fl!(
                    "files-actions-count-invalid",
                    action = self.name.clone(),
                    field = label
                )
            })
        };
        let maximum = count(&self.maximum, fl!("files-actions-maximum"))?;
        let action = CustomAction {
            id: self.id.clone(),
            name: self.name.clone(),
            icon: optional(&self.icon),
            command: self.command.clone(),
            conditions: ActionConditions {
                minimum_selection: count(&self.minimum, fl!("files-actions-minimum"))?,
                maximum_selection: (maximum > 0).then_some(maximum),
                files: self.files,
                directories: self.folders,
                patterns: self
                    .patterns
                    .split(';')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned)
                    .collect(),
            },
            shortcut: optional(&self.shortcut)
                .map(|value| {
                    KeyChord::parse(&value)
                        .map(|key| key.accelerator())
                        .map_err(|error| {
                            fl!(
                                "files-actions-error",
                                action = self.name.clone(),
                                error = error.to_string()
                            )
                        })
                })
                .transpose()?,
            toolbar: self.toolbar,
        };
        action.validate().map_err(|error| {
            fl!(
                "files-actions-error",
                action = self.name.clone(),
                error = error.to_string()
            )
        })?;
        Ok(action)
    }
}

fn optional(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

pub(super) struct Editor {
    pub window: window::Id,
    base: ConfigurationSnapshot,
    drafts: Vec<Draft>,
    selected: Option<u64>,
    next_entry: u64,
    selection: u64,
    dirty: bool,
    busy: bool,
    confirmation: Option<Confirmation>,
    recording: bool,
    recording_activation: bool,
    status: String,
}

impl Editor {
    fn new(window: window::Id, base: ConfigurationSnapshot) -> Self {
        let drafts: Vec<_> = base
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| Draft::new(index as u64 + 1, action))
            .collect();
        Self {
            window,
            selected: drafts.first().map(|draft| draft.serial),
            next_entry: drafts.len() as u64,
            drafts,
            base,
            selection: 0,
            dirty: false,
            busy: false,
            confirmation: None,
            recording: false,
            recording_activation: false,
            status: fl!("files-actions-status-initial"),
        }
    }

    fn target(&self) -> Option<Target> {
        self.selected.map(|entry| Target {
            window: self.window,
            entry,
            selection: self.selection,
        })
    }
    fn accepts(&self, target: Target) -> bool {
        target.window == self.window
            && self.selected == Some(target.entry)
            && target.selection == self.selection
            && !self.busy
            && self.confirmation.is_none()
            && !self.recording
    }
    fn selected_mut(&mut self, target: Target) -> Option<&mut Draft> {
        if !self.accepts(target) {
            return None;
        }
        self.drafts
            .iter_mut()
            .find(|draft| draft.serial == target.entry)
    }
    fn select(&mut self, selected: Option<u64>) {
        self.selected = selected;
        self.selection += 1;
        self.recording = false;
    }
    fn actions(&self) -> Result<Vec<CustomAction>, String> {
        let actions = self
            .drafts
            .iter()
            .map(Draft::action)
            .collect::<Result<Vec<_>, _>>()?;
        let mut settings = self.base.settings.clone();
        settings.toolbar.retain(|id| {
            id.strip_prefix("action:").is_none_or(|id| {
                actions
                    .iter()
                    .any(|action| action.id == id && action.toolbar)
            })
        });
        settings
            .validate(&actions)
            .map_err(|error| error.to_string())?;
        Ok(actions)
    }

    fn controls(&self) -> Vec<Focus> {
        if self.busy || self.recording {
            return vec![];
        }
        if self.confirmation.is_some() {
            return vec![Focus::Keep, Focus::Discard];
        }
        let mut controls: Vec<_> = self
            .drafts
            .iter()
            .map(|draft| Focus::Select(draft.serial))
            .collect();
        if self.drafts.len() < 256 {
            controls.push(Focus::Add);
        }
        if let Some(index) = self
            .drafts
            .iter()
            .position(|draft| Some(draft.serial) == self.selected)
        {
            controls.push(Focus::Delete);
            if index > 0 {
                controls.push(Focus::Earlier);
            }
            if index + 1 < self.drafts.len() {
                controls.push(Focus::Later);
            }
            controls.extend([
                Focus::Field(Field::Name),
                Focus::Field(Field::Icon),
                Focus::Field(Field::Command),
                Focus::Field(Field::Patterns),
                Focus::Check(Check::Files),
                Focus::Check(Check::Folders),
                Focus::Field(Field::Minimum),
                Focus::Field(Field::Maximum),
                Focus::Field(Field::Shortcut),
                Focus::Record,
                Focus::Check(Check::Toolbar),
            ]);
        }
        controls.push(Focus::Reload);
        if self.dirty {
            controls.push(Focus::Save);
        }
        controls
    }

    fn focus_id(&self, focus: Focus) -> iced::widget::Id {
        iced::widget::Id::from(format!(
            "files-actions-{:?}-{}-{focus:?}",
            self.window, self.selection
        ))
    }
}

fn root_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-root-{id:?}"))
}
fn form_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-form-{id:?}"))
}
fn chooser_id(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-actions-chooser-{id:?}"))
}

impl Desktop {
    pub(super) fn show_action_editor(&mut self) -> Task<Message> {
        if let Some(editor) = &self.action_editor {
            return window::gain_focus(editor.window);
        }
        let size = if self.configuration.actions.is_empty() {
            Size::new(600.0, 332.0)
        } else {
            Size::new(720.0, 700.0)
        };
        let (id, opened) = window::open(window::Settings {
            size,
            min_size: Some(Size::new(280.0, 240.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.action_editor = Some(Editor::new(id, self.configuration.clone()));
        opened.map(Message::Opened)
    }

    pub(super) fn close_action_editor(&mut self, id: window::Id, forced: bool) -> Task<Message> {
        let Some(editor) = self
            .action_editor
            .as_mut()
            .filter(|editor| editor.window == id)
        else {
            return Task::none();
        };
        if !forced && editor.busy {
            editor.status = fl!("files-preferences-wait");
            return Task::none();
        }
        if !forced && editor.dirty {
            editor.recording = false;
            editor.confirmation = Some(Confirmation::Close);
            return Task::none();
        }
        self.action_editor = None;
        Task::batch([window::close(id), self.exit_if_empty()])
    }

    fn edit_configuration(&mut self, id: window::Id, operation: Operation) -> Task<Message> {
        let Some(editor) = self
            .action_editor
            .as_mut()
            .filter(|editor| editor.window == id && !editor.busy)
        else {
            return Task::none();
        };
        if self.configuration_pending.is_some() {
            editor.status = fl!("files-preferences-another-change");
            return Task::none();
        }
        let Some(worker) = &self.configuration_worker else {
            editor.status = fl!("files-preferences-worker-unavailable");
            return Task::none();
        };
        let receiver = match operation {
            Operation::Save => match editor.actions() {
                Ok(actions) => worker.save(editor.base.clone(), actions),
                Err(error) => {
                    editor.status = error;
                    return Task::none();
                }
            },
            Operation::Reload => worker.reload(editor.base.clone()),
        };
        let receiver = match receiver {
            Ok(receiver) => receiver,
            Err(error) => {
                editor.status = error.to_string();
                return Task::none();
            }
        };
        self.configuration_request += 1;
        let request = self.configuration_request;
        self.configuration_pending = Some((request, id, operation));
        editor.busy = true;
        editor.recording = false;
        editor.status = if operation == Operation::Save {
            fl!("files-preferences-saving")
        } else {
            fl!("files-preferences-reloading")
        };
        Task::perform(
            async move { receiver.await.ok().map(Arc::new) },
            move |result| Message::ActionEditor(Event::Finished(request, result)),
        )
    }

    pub(super) fn action_editor_event(&mut self, event: Event) -> Task<Message> {
        if let Event::Finished(request, result) = event {
            let Some((expected, id, operation)) = self.configuration_pending else {
                return Task::none();
            };
            if request != expected {
                return Task::none();
            }
            self.configuration_pending = None;
            let result = result
                .as_deref()
                .cloned()
                .unwrap_or_else(|| Err(fl!("files-preferences-worker-stopped")));
            let install = if let Ok(published) = &result {
                self.install_configuration(published.clone())
            } else {
                Task::none()
            };
            if let Some(editor) = self
                .action_editor
                .as_mut()
                .filter(|editor| editor.window == id)
            {
                editor.busy = false;
                match result {
                    Ok(publication) => {
                        let snapshot = publication.current;
                        let selected_index = editor
                            .drafts
                            .iter()
                            .position(|draft| Some(draft.serial) == editor.selected)
                            .unwrap_or(0);
                        let next = editor.next_entry;
                        let drafts: Vec<_> = snapshot
                            .actions
                            .iter()
                            .enumerate()
                            .map(|(index, action)| Draft::new(next + index as u64 + 1, action))
                            .collect();
                        editor.next_entry += drafts.len() as u64;
                        let selected = drafts
                            .get(selected_index.min(drafts.len().saturating_sub(1)))
                            .map(|draft| draft.serial);
                        editor.drafts = drafts;
                        editor.select(selected);
                        editor.base = (*snapshot).clone();
                        editor.dirty = false;
                        editor.status = if operation == Operation::Save {
                            fl!("files-actions-saved")
                        } else {
                            fl!("files-actions-reloaded")
                        };
                    }
                    Err(error) => editor.status = error,
                }
            }
            return Task::batch([install, self.exit_if_empty()]);
        }
        let id = match &event {
            Event::Select(id, _)
            | Event::Add(id)
            | Event::Save(id)
            | Event::Reload(id)
            | Event::Record(id)
            | Event::RecordKeyboard(id)
            | Event::Confirm(id, _) => *id,
            Event::Remove(target)
            | Event::Move(target, _)
            | Event::Edit(target, ..)
            | Event::Toggle(target, ..) => target.window,
            Event::Finished(..) => unreachable!(),
        };
        let Some(editor) = self
            .action_editor
            .as_mut()
            .filter(|editor| editor.window == id && !editor.busy)
        else {
            return Task::none();
        };
        if let Event::Confirm(_, discard) = event {
            let confirmation = editor.confirmation.take();
            if !discard {
                return Task::none();
            }
            return match confirmation {
                Some(Confirmation::Close) => self.close_action_editor(id, true),
                Some(Confirmation::Reload) => self.edit_configuration(id, Operation::Reload),
                None => Task::none(),
            };
        }
        if editor.confirmation.is_some() {
            return Task::none();
        }
        match event {
            Event::Select(_, serial)
                if editor.drafts.iter().any(|draft| draft.serial == serial) =>
            {
                editor.select(Some(serial))
            }
            Event::Add(_) if editor.drafts.len() < 256 => {
                let id = (1..)
                    .map(|n| format!("action-{n}"))
                    .find(|id| editor.drafts.iter().all(|draft| draft.id != *id))
                    .unwrap();
                editor.next_entry += 1;
                let serial = editor.next_entry;
                editor.drafts.push(Draft::new(
                    serial,
                    &CustomAction::new(id, fl!("files-actions-new-name"), ""),
                ));
                editor.select(Some(serial));
                editor.dirty = true;
            }
            Event::Remove(target) if editor.accepts(target) => {
                let index = editor
                    .drafts
                    .iter()
                    .position(|draft| draft.serial == target.entry)
                    .unwrap();
                editor.drafts.remove(index);
                let selected = editor
                    .drafts
                    .get(index.min(editor.drafts.len().saturating_sub(1)))
                    .map(|draft| draft.serial);
                editor.select(selected);
                editor.dirty = true;
            }
            Event::Move(target, earlier) if editor.accepts(target) => {
                let index = editor
                    .drafts
                    .iter()
                    .position(|draft| draft.serial == target.entry)
                    .unwrap();
                let next = if earlier {
                    index.checked_sub(1)
                } else {
                    (index + 1 < editor.drafts.len()).then_some(index + 1)
                };
                if let Some(next) = next {
                    editor.drafts.swap(index, next);
                    editor.dirty = true;
                }
            }
            Event::Edit(target, field, value) => {
                let maximum = match field {
                    Field::Command => 16 * 1024,
                    Field::Patterns => 64 * 257,
                    Field::Shortcut => 128,
                    Field::Minimum | Field::Maximum => 20,
                    _ => 256,
                };
                if value.len() > maximum {
                    editor.status = fl!("files-preferences-field-too-long", max = maximum);
                    return Task::none();
                }
                if let Some(draft) = editor.selected_mut(target) {
                    *match field {
                        Field::Name => &mut draft.name,
                        Field::Icon => &mut draft.icon,
                        Field::Command => &mut draft.command,
                        Field::Patterns => &mut draft.patterns,
                        Field::Minimum => &mut draft.minimum,
                        Field::Maximum => &mut draft.maximum,
                        Field::Shortcut => &mut draft.shortcut,
                    } = value;
                    editor.dirty = true;
                }
            }
            Event::Toggle(target, check, value) => {
                if let Some(draft) = editor.selected_mut(target) {
                    *match check {
                        Check::Files => &mut draft.files,
                        Check::Folders => &mut draft.folders,
                        Check::Toolbar => &mut draft.toolbar,
                    } = value;
                    editor.dirty = true;
                }
            }
            Event::Save(_) => return self.edit_configuration(id, Operation::Save),
            Event::Reload(_) => {
                if editor.dirty {
                    editor.recording = false;
                    editor.confirmation = Some(Confirmation::Reload);
                    return Task::none();
                } else {
                    return self.edit_configuration(id, Operation::Reload);
                }
            }
            Event::Record(_) | Event::RecordKeyboard(_) if editor.selected.is_some() => {
                editor.recording_activation = matches!(event, Event::RecordKeyboard(_));
                editor.recording = true;
                editor.status = fl!("files-actions-recording");
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn action_editor_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        _captured: bool,
    ) -> Task<Message> {
        let Some(editor) = self
            .action_editor
            .as_mut()
            .filter(|editor| editor.window == id)
        else {
            return Task::none();
        };
        if editor.busy {
            return Task::none();
        }
        if editor.recording {
            if std::mem::take(&mut editor.recording_activation)
                && super::focus::activation_key(&key)
            {
                return Task::none();
            }
            match &key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    editor.recording = false;
                    editor.status = fl!("files-preferences-recording-cancelled");
                }
                keyboard::Key::Named(keyboard::key::Named::Backspace) if modifiers.is_empty() => {
                    editor.recording = false;
                    if let Some(target) = editor.target() {
                        return self.action_editor_event(Event::Edit(
                            target,
                            Field::Shortcut,
                            String::new(),
                        ));
                    }
                }
                _ => {
                    if let Some(chord) = key_chord(&key, modifiers) {
                        if chord.can_bind_action() {
                            let shortcut = chord.accelerator();
                            editor.recording = false;
                            if let Some(target) = editor.target() {
                                return self.action_editor_event(Event::Edit(
                                    target,
                                    Field::Shortcut,
                                    shortcut,
                                ));
                            }
                        } else {
                            editor.status = fl!("files-actions-shortcut-modifier");
                        }
                    }
                }
            }
            return Task::none();
        }
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
            if editor.confirmation.is_some() {
                editor.confirmation = None;
                return Task::none();
            }
            return self.close_action_editor(id, false);
        }
        if modifiers.control()
            && !modifiers.alt()
            && !modifiers.logo()
            && let keyboard::Key::Character(value) = key
        {
            return match value.as_str().to_ascii_lowercase().as_str() {
                "s" => self.action_editor_event(Event::Save(id)),
                "r" => self.action_editor_event(Event::Reload(id)),
                "w" => self.close_action_editor(id, false),
                _ => Task::none(),
            };
        }
        Task::none()
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, editor: &'a Editor) -> Element<'a, Message> {
    iced::widget::responsive(move |size| contents(desktop, editor, size.width >= 720.0)).into()
}

fn contents<'a>(desktop: &'a Desktop, editor: &'a Editor, wide: bool) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().body_size.px();
    let label_size = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let id = editor.window;
    let enabled = !editor.busy && editor.confirmation.is_none() && !editor.recording;
    let action = |label: String, event: Option<Event>| -> Element<'a, Message> {
        let focus = event.as_ref().and_then(|event| {
            Some(match event {
                Event::Add(_) => Focus::Add,
                Event::Remove(_) => Focus::Delete,
                Event::Move(_, true) => Focus::Earlier,
                Event::Move(_, false) => Focus::Later,
                Event::Record(_) | Event::RecordKeyboard(_) => Focus::Record,
                Event::Reload(_) => Focus::Reload,
                Event::Save(_) => Focus::Save,
                Event::Confirm(_, false) => Focus::Keep,
                Event::Confirm(_, true) => Focus::Discard,
                _ => return None,
            })
        });
        let keyboard_message = event.as_ref().map(|event| match event {
            Event::Record(id) => Message::ActionEditor(Event::RecordKeyboard(*id)),
            other => Message::ActionEditor(other.clone()),
        });
        let message = event.map(Message::ActionEditor);
        let button = canopa_ui::styled_action(
            text(label).size(label_size).font(font),
            message.clone(),
            match focus {
                Some(Focus::Save) => canopa_ui::Action::Primary,
                Some(Focus::Add) if editor.drafts.is_empty() => canopa_ui::Action::Primary,
                Some(Focus::Add | Focus::Reload | Focus::Record) => canopa_ui::Action::Normal,
                Some(Focus::Delete) => canopa_ui::Action::Destructive,
                _ => canopa_ui::Action::Quiet,
            },
            false,
            appearance,
        );
        if let Some(focus) = focus {
            super::focus::button_with_keyboard(
                editor.focus_id(focus),
                message,
                keyboard_message,
                appearance,
                button,
            )
        } else {
            button.into()
        }
    };
    let mut chooser = row![].spacing(4);
    for draft in &editor.drafts {
        let message = enabled.then_some(Message::ActionEditor(Event::Select(id, draft.serial)));
        let selected = Some(draft.serial) == editor.selected;
        let label = chrome::section_title(appearance, &draft.name);
        let button = canopa_ui::styled_action(
            label,
            message.clone(),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        );
        chooser = chooser.push(super::focus::button(
            editor.focus_id(Focus::Select(draft.serial)),
            message,
            appearance,
            container(button).style(move |_| {
                if selected {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                } else {
                    container::Style::default()
                }
            }),
        ));
    }
    let mut body = column![
        chrome::heading_with_action(
            desktop,
            chrome::Icon::Terminal,
            fl!("files-custom-actions-title"),
            fl!("files-actions-subtitle"),
            Some(
                row![
                    action(
                        fl!("files-preferences-reload"),
                        enabled.then_some(Event::Reload(id))
                    ),
                    chrome::dismiss(desktop, id),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .into()
            ),
        ),
        scrollable(chooser)
            .id(chooser_id(id))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
            ))
    ]
    .spacing(8);
    let notice: Element<'a, Message> = if let Some(error) = &desktop.configuration_error {
        text(fl!(
            "files-configuration-not-applied",
            error = error.to_string()
        ))
        .size(label_size)
        .font(font)
        .into()
    } else if !editor.base.same_source(&desktop.configuration) {
        text(fl!("files-actions-changed-outside"))
            .size(label_size)
            .font(font)
            .into()
    } else {
        iced::widget::space().height(0).into()
    };
    // Keep the form and its native inputs in fixed sibling slots when a notice
    // appears or clears; inserting a new sibling discards their editing state.
    body = body.push(container(notice));
    let target = editor.target();
    let index = editor
        .drafts
        .iter()
        .position(|draft| Some(draft.serial) == editor.selected);
    let add = action(
        fl!("files-actions-add"),
        (enabled && editor.drafts.len() < 256).then_some(Event::Add(id)),
    );
    let empty_action = if editor.drafts.is_empty() {
        Some(add)
    } else {
        body = body.push(
            row![
                add,
                action(
                    fl!("files-actions-delete"),
                    target.filter(|_| enabled).map(Event::Remove)
                ),
                action(
                    fl!("files-preferences-earlier"),
                    target
                        .filter(|_| enabled && index.is_some_and(|n| n > 0))
                        .map(|target| Event::Move(target, true))
                ),
                action(
                    fl!("files-preferences-later"),
                    target
                        .filter(|_| enabled && index.is_some_and(|n| n + 1 < editor.drafts.len()))
                        .map(|target| Event::Move(target, false))
                )
            ]
            .spacing(6)
            .wrap(),
        );
        None
    };
    // A blocking question: shown over the editor, not inside its form.
    let question = editor.confirmation.map(|confirmation| {
        let buttons: Element<'a, Message> = row![
            action(
                fl!("files-preferences-keep-editing"),
                Some(Event::Confirm(id, false))
            ),
            action(
                if confirmation == Confirmation::Close {
                    fl!("files-preferences-discard")
                } else {
                    fl!("files-preferences-reload")
                },
                Some(Event::Confirm(id, true))
            )
        ]
        .spacing(8)
        .into();
        (
            if confirmation == Confirmation::Close {
                fl!("files-actions-discard-question")
            } else {
                fl!("files-actions-reload-question")
            },
            buttons,
        )
    });
    let mut form = column![].spacing(12);
    if let Some(draft) = editor
        .drafts
        .iter()
        .find(|draft| Some(draft.serial) == editor.selected)
    {
        let target = target.unwrap();
        let field = |label: String, hint: String, value: &'a str, which| {
            let input_appearance = appearance.clone();
            let mut input = text_input(&hint, value)
                .id(editor.focus_id(Focus::Field(which)))
                .size(size)
                .font(if which == Field::Command {
                    iced::Font::MONOSPACE
                } else {
                    font
                })
                .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
                .style(move |theme, status| view::input_style(&input_appearance, theme, status));
            if enabled {
                input = input.on_input(move |value| {
                    Message::ActionEditor(Event::Edit(target, which, value))
                });
            }
            column![text(label).size(label_size).font(font), input].spacing(4)
        };
        let section = |label: String| chrome::section_title(appearance, label);
        let divider = || iced::widget::rule::horizontal(1).style(chrome::divider);
        let hint = |label: String| {
            text(label)
                .size(label_size)
                .font(font)
                .style(canopa_ui::muted)
                .wrapping(text::Wrapping::WordOrGlyph)
        };
        let details = column![
            section(fl!("files-actions-details")),
            field(
                fl!("files-column-name"),
                fl!("files-actions-name-example"),
                &draft.name,
                Field::Name
            ),
            field(
                fl!("files-actions-icon"),
                "image-x-generic-symbolic".into(),
                &draft.icon,
                Field::Icon
            ),
            field(
                fl!("files-actions-command"),
                "my-script %F".into(),
                &draft.command,
                Field::Command
            ),
            hint(fl!("files-actions-command-hint")),
        ]
        .spacing(12);
        let conditions = column![
            section(fl!("files-actions-availability")),
            field(
                fl!("files-actions-patterns"),
                "*.png; *.jpg".into(),
                &draft.patterns,
                Field::Patterns
            ),
            hint(fl!("files-actions-patterns-hint")),
        ]
        .spacing(12);
        let toggle = |label: String, value, which| {
            let mut control = checkbox(value)
                .label(label)
                .text_size(size)
                .font(font)
                .style(canopa_ui::checkbox_style);
            if enabled {
                control = control.on_toggle(move |value| {
                    Message::ActionEditor(Event::Toggle(target, which, value))
                });
            }
            super::focus::check_box(
                editor.focus_id(Focus::Check(which)),
                value,
                enabled.then_some(Message::ActionEditor(Event::Toggle(target, which, !value))),
                appearance,
                container(control).padding(2),
            )
        };
        let conditions = conditions
            .push(
                row![
                    toggle(fl!("files-actions-files"), draft.files, Check::Files),
                    toggle(fl!("files-actions-folders"), draft.folders, Check::Folders)
                ]
                .spacing(16),
            )
            .push(
                row![
                    container(field(
                        fl!("files-actions-minimum"),
                        "1".into(),
                        &draft.minimum,
                        Field::Minimum
                    ))
                    .width(Length::FillPortion(1)),
                    container(field(
                        fl!("files-actions-maximum"),
                        "0".into(),
                        &draft.maximum,
                        Field::Maximum
                    ))
                    .width(Length::FillPortion(1)),
                ]
                .spacing(10),
            )
            .push(hint(fl!("files-actions-count-hint")))
            .push(divider())
            .push(section(fl!("files-actions-ways-to-run")))
            .push(field(
                fl!("files-actions-shortcut"),
                "<Control><Alt>i".into(),
                &draft.shortcut,
                Field::Shortcut,
            ))
            .push(action(
                fl!("files-actions-record"),
                enabled.then_some(Event::Record(id)),
            ))
            .push(toggle(
                fl!("files-actions-toolbar"),
                draft.toolbar,
                Check::Toolbar,
            ))
            .push(hint(fl!("files-actions-undo-note")));
        form = if wide {
            column![
                row![
                    chrome::card(appearance, details).width(Length::FillPortion(1)),
                    chrome::card(appearance, conditions).width(Length::FillPortion(1)),
                ]
                .spacing(12)
            ]
        } else {
            details.push(divider()).push(conditions)
        };
    }
    if editor.drafts.is_empty() {
        body = body.push(chrome::empty_state(
            desktop,
            chrome::Icon::Edit,
            fl!("files-actions-empty"),
            fl!("files-actions-empty-description"),
            empty_action,
        ));
    } else {
        body = body.push(
            scrollable(form)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(9),
                ))
                .id(form_id(id))
                .height(Length::Fill),
        );
    }
    // The initial guidance is not shown before there is an action to configure.
    let status = if editor.drafts.is_empty() && editor.status == fl!("files-actions-status-initial")
    {
        ""
    } else {
        &editor.status
    };
    let mut footer = row![
        text(status)
            .size(label_size)
            .font(font)
            .style(canopa_ui::muted)
            .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if !editor.drafts.is_empty() || editor.dirty {
        footer = footer.push(action(
            fl!("files-actions-save"),
            (enabled && editor.dirty).then_some(Event::Save(id)),
        ));
    }
    if !status.is_empty() || !editor.drafts.is_empty() || editor.dirty {
        body = body
            .push(iced::widget::rule::horizontal(1).style(chrome::divider))
            .push(footer);
    }
    let controls = editor.controls();
    let order = controls
        .iter()
        .map(|control| editor.focus_id(*control))
        .chain(std::iter::once(chrome::dismiss_id(id)))
        .collect();
    let reveal = controls
        .iter()
        .filter_map(|control| {
            let scroll = match control {
                Focus::Select(_) => chooser_id(id),
                Focus::Field(_) | Focus::Check(_) | Focus::Record => form_id(id),
                _ => return None,
            };
            Some((editor.focus_id(*control), scroll))
        })
        .collect();
    let default = editor.confirmation.map(|_| editor.focus_id(Focus::Keep));
    let appearance = appearance.clone();
    let content = chrome::centered(body, 800.0)
        .id(root_id(id))
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(&appearance, false));
    let content = chrome::modal(desktop, content.into(), question);
    super::focus::group(content, order, reveal, default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_recording_starts_after_activation_and_mouse_recording_starts_immediately() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, _) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let _ = desktop.show_action_editor();
        let id = desktop.action_editor.as_ref().unwrap().window;
        let _ = desktop.action_editor_event(Event::Add(id));
        let _ = desktop.action_editor_event(Event::RecordKeyboard(id));
        let _ = desktop.update(Message::CapturedKey(
            id,
            keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::empty(),
        ));
        assert!(desktop.action_editor.as_ref().unwrap().recording);
        let _ = desktop.update(Message::CapturedKey(
            id,
            keyboard::Key::Named(keyboard::key::Named::F8),
            keyboard::Modifiers::empty(),
        ));
        assert_eq!(
            desktop.action_editor.as_ref().unwrap().drafts[0].shortcut,
            "F8"
        );
        let _ = desktop.action_editor_event(Event::Record(id));
        let _ = desktop.update(Message::CapturedKey(
            id,
            keyboard::Key::Named(keyboard::key::Named::F9),
            keyboard::Modifiers::empty(),
        ));
        assert_eq!(
            desktop.action_editor.as_ref().unwrap().drafts[0].shortcut,
            "F9"
        );
    }

    #[test]
    fn drafts_retain_fields_and_ignore_events_from_a_previous_selection() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, _) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let _ = desktop.show_action_editor();
        let id = desktop.action_editor.as_ref().unwrap().window;
        let _ = desktop.action_editor_event(Event::Add(id));
        let first = desktop.action_editor.as_ref().unwrap().target().unwrap();
        let _ = desktop.action_editor_event(Event::Edit(first, Field::Name, "First action".into()));
        let _ =
            desktop.action_editor_event(Event::Edit(first, Field::Command, "/bin/true %F".into()));
        let _ = desktop.action_editor_event(Event::Add(id));
        let second = desktop.action_editor.as_ref().unwrap().target().unwrap();
        let _ = desktop.action_editor_event(Event::Edit(
            second,
            Field::Command,
            "/bin/false %F".into(),
        ));
        let _ = desktop.action_editor_event(Event::Edit(first, Field::Name, "Wrong action".into()));
        let _ = desktop.action_editor_event(Event::Select(id, first.entry));
        let fresh_first = desktop.action_editor.as_ref().unwrap().target().unwrap();
        let _ = desktop.action_editor_event(Event::Edit(first, Field::Name, "Stale again".into()));
        let editor = desktop.action_editor.as_ref().unwrap();
        assert_eq!(editor.drafts[0].name, "First action");
        assert_eq!(editor.drafts[1].command, "/bin/false %F");
        assert!(editor.actions().is_ok());
        let _ = desktop.close_action_editor(id, false);
        assert_eq!(
            desktop.action_editor.as_ref().unwrap().confirmation,
            Some(Confirmation::Close)
        );
        let _ = desktop.action_editor_event(Event::Confirm(id, false));
        let _ = desktop.action_editor_event(Event::Edit(
            fresh_first,
            Field::Maximum,
            "not a number".into(),
        ));
        assert!(
            desktop
                .action_editor
                .as_ref()
                .unwrap()
                .actions()
                .unwrap_err()
                .contains("whole number")
        );
        let _ = desktop.action_editor_event(Event::Reload(id));
        assert_eq!(
            desktop.action_editor.as_ref().unwrap().confirmation,
            Some(Confirmation::Reload)
        );
        let _ = desktop.action_editor_event(Event::Confirm(id, false));
        assert!(desktop.action_editor.as_ref().unwrap().dirty);
    }
}
