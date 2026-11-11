use super::file_operations::{Event, Origin, Submission};
use super::*;
use crate::{operation_jobs::Request, operations::OperationRequest};
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{column, container, row, text, text_input},
};

pub(super) struct Dialog {
    pub window: window::Id,
    pub origin: Arc<Origin>,
    pub value: String,
    pub busy: bool,
    pub error: Option<String>,
    original: Option<PathBuf>,
    initial: String,
    prepared: Option<crate::operation_jobs::PreparedRename>,
}
impl Dialog {
    pub fn title(&self) -> &'static str {
        if self.original.is_some() {
            "Rename"
        } else {
            "New folder"
        }
    }
}

fn control(window: window::Id, name: &'static str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-name-{window:?}-{name}"))
}

impl Desktop {
    pub(super) fn show_name_dialog(&mut self, owner: window::Id, rename: bool) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.name {
            return window::gain_focus(dialog.window);
        }
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = Origin::capture(self, owner) else {
            return Task::none();
        };
        if self.file_operations.preparing_name.is_some() {
            return Task::none();
        }
        if rename {
            if origin.selected.len() != 1 {
                return Task::none();
            }
            let result = self
                .file_operations
                .service
                .as_ref()
                .unwrap()
                .prepare_rename(origin.selected[0].clone(), Some(origin.binding()));
            match result {
                Ok(receiver) => {
                    self.file_operations.preparing_name = Some(origin.clone());
                    return Task::perform(
                        async move {
                            receiver.await.unwrap_or_else(|_| {
                                Err("The operation worker stopped before preparing Rename.".into())
                            })
                        },
                        move |result| {
                            Message::FileOperation(Event::RenamePrepared(origin.clone(), result))
                        },
                    );
                }
                Err(error) => self.file_operations.error = Some(error.to_string()),
            }
            return Task::none();
        }
        self.open_name_dialog(origin, None)
    }

    pub(super) fn rename_prepared(
        &mut self,
        origin: Arc<Origin>,
        result: Result<crate::operation_jobs::PreparedRename, String>,
    ) -> Task<Message> {
        if !self
            .file_operations
            .preparing_name
            .as_ref()
            .is_some_and(|pending| Arc::ptr_eq(pending, &origin))
        {
            return Task::none();
        }
        self.file_operations.preparing_name = None;
        if !origin.current(self) || self.file_operations.name.is_some() {
            return Task::none();
        }
        match result {
            Ok(prepared) => self.open_name_dialog(origin, Some(prepared)),
            Err(error) => {
                self.file_operations.error = Some(error);
                Task::none()
            }
        }
    }

    fn open_name_dialog(
        &mut self,
        origin: Arc<Origin>,
        prepared: Option<crate::operation_jobs::PreparedRename>,
    ) -> Task<Message> {
        let owner = origin.window;
        let original = prepared.as_ref().map(|_| origin.selected[0].clone());
        let initial = original
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "New folder".into());
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(440.0, 250.0),
                min_size: Some(Size::new(320.0, 240.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.file_operations.name = Some(Dialog {
            window,
            origin,
            value: initial.clone(),
            initial,
            original,
            prepared,
            busy: false,
            error: None,
        });
        opened.map(move |opened| Message::FileOperation(Event::NameOpened(window, opened)))
    }

    pub(super) fn name_opened(
        &mut self,
        id: window::Id,
        opened: Option<window::Id>,
    ) -> Task<Message> {
        if self
            .file_operations
            .name
            .as_ref()
            .is_none_or(|dialog| dialog.window != id)
        {
            return window::close(id);
        }
        if opened.is_none() {
            self.file_operations.name = None;
            self.file_operations.error = Some("Could not open the name dialog.".into());
            return Task::none();
        }
        iced::widget::operation::focus(control(id, "name"))
            .chain(iced::widget::operation::select_all(control(id, "name")))
            .chain(window::gain_focus(id))
    }

    pub(super) fn submit_name_dialog(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self
            .file_operations
            .name
            .as_mut()
            .filter(|dialog| dialog.window == id && !dialog.busy)
        else {
            return Task::none();
        };
        if dialog.original.is_some() && dialog.value == dialog.initial {
            return self.finish_name_dialog(id);
        }
        if dialog.value.is_empty()
            || matches!(dialog.value.as_str(), "." | "..")
            || dialog.value.contains(['/', '\0'])
        {
            dialog.error =
                Some("Enter one name, without a slash. The names . and .. are reserved.".into());
            return Task::none();
        }
        let directory = dialog
            .original
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or(&dialog.origin.directory);
        let destination = directory.join(&dialog.value);
        let mut affected = vec![destination.clone()];
        if let Some(source) = &dialog.original {
            affected.push(source.clone());
        }
        let submission = Submission {
            origin: Some(dialog.origin.clone()),
            owner: dialog.origin.window,
            dialog: Some(id),
            affected,
            reveal: Some(destination.clone()),
            refresh_all: false,
        };
        dialog.busy = true;
        dialog.error = None;
        if let Some(prepared) = &dialog.prepared {
            let result = self
                .file_operations
                .service
                .as_ref()
                .ok_or_else(|| io::Error::other("The operation queue is unavailable."))
                .and_then(|service| {
                    service.submit_rename(prepared.clone(), dialog.value.clone().into())
                });
            self.await_file_admission(result, submission)
        } else {
            self.submit_file_operation(
                Request::Mutation(OperationRequest::CreateFolder { path: destination }),
                submission,
            )
        }
    }

    pub(super) fn finish_name_dialog(&mut self, id: window::Id) -> Task<Message> {
        if self
            .file_operations
            .name
            .as_ref()
            .is_some_and(|dialog| dialog.window == id)
        {
            self.file_operations.name = None;
        }
        window::close(id)
    }

    pub(super) fn close_name_dialog(&mut self, id: window::Id, forced: bool) -> Task<Message> {
        if let Some(dialog) = self
            .file_operations
            .name
            .as_mut()
            .filter(|dialog| dialog.window == id)
        {
            if dialog.busy && !forced {
                return Task::none();
            }
            self.file_operations.name = None;
            return Task::batch([window::close(id), self.exit_if_empty()]);
        }
        Task::none()
    }

    pub(super) fn name_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.close_name_dialog(id, false);
        }
        Task::none()
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let mut input = text_input("Name", &dialog.value)
        .id(control(id, "name"))
        .font(font)
        .size(size)
        .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
        .style(move |theme, status| super::view::input_style(appearance, theme, status));
    if !dialog.busy {
        input = input
            .on_input(move |value| Message::FileOperation(Event::NameChanged(id, value)))
            .on_submit(Message::FileOperation(Event::NameSubmit(id)));
    }
    // Named by the "Full name" or "Folder name" caption drawn before it.
    let mut node = super::accessibility_ui::Node::new(super::accessibility_ui::Role::TextInput)
        .key("files-name-field")
        .value(dialog.value.clone())
        .required(true)
        .invalid(dialog.error.is_some())
        .read_only(dialog.busy);
    if !dialog.busy {
        node = node.on_text(move |value| Message::FileOperation(Event::NameChanged(id, value)));
    }
    let input = super::accessibility_ui::semantic(node, input);
    let feedback = match (&dialog.error, dialog.busy) {
        (Some(error), _) => {
            super::accessibility_ui::Node::alert(error.clone()).key("files-name-error")
        }
        (None, true) => {
            super::accessibility_ui::Node::status("Accepting operation…").key("files-name-busy")
        }
        (None, false) => super::accessibility_ui::Node::status("").key("files-name-busy"),
    };
    let action = |label, name, event| {
        let message = (!dialog.busy).then_some(Message::FileOperation(event));
        super::focus::button(
            control(id, name),
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                message,
                if name == "submit" {
                    canopa_ui::Action::Primary
                } else {
                    canopa_ui::Action::Quiet
                },
                false,
                appearance,
            ),
        )
    };
    let folder = dialog
        .original
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(&dialog.origin.directory);
    let location = folder.strip_prefix(&desktop.home).map_or_else(
        |_| folder.to_string_lossy().into_owned(),
        |path| {
            if path.as_os_str().is_empty() {
                "Home".into()
            } else {
                format!("Home / {}", path.to_string_lossy().replace('/', " / "))
            }
        },
    );
    let body = column![
        chrome::dialog_heading(
            desktop,
            if dialog.original.is_some() {
                chrome::Icon::Edit
            } else {
                chrome::Icon::Folder
            },
            dialog.title(),
            ""
        ),
        chrome::single_line(
            location,
            font,
            small + 1.0,
            Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted)),
            true
        ),
        column![
            chrome::section_title(
                appearance,
                if dialog.original.is_some() {
                    "Full name"
                } else {
                    "Folder name"
                },
            )
            .size(small + 1.0),
            input,
        ]
        .spacing(4),
        super::accessibility_ui::semantic(
            feedback,
            container(
                text(dialog.error.as_deref().unwrap_or(if dialog.busy {
                    "Accepting operation…"
                } else {
                    ""
                }))
                .font(font)
                .size(small)
                .color(chrome::ink(
                    appearance,
                    if dialog.error.is_some() {
                        canopa_theme::ColorRole::Danger
                    } else {
                        canopa_theme::ColorRole::Muted
                    }
                ))
            )
            .height(if dialog.error.is_some() || dialog.busy {
                26
            } else {
                1
            })
        ),
        row![
            iced::widget::space().width(Length::Fill),
            action("Cancel", "cancel", Event::NameCancel(id)),
            action(
                if dialog.original.is_some() {
                    "Rename"
                } else {
                    "Create"
                },
                "submit",
                Event::NameSubmit(id)
            )
        ]
        .spacing(8),
    ]
    .spacing(10);
    let appearance = appearance.clone();
    let body = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    let order = if dialog.busy {
        Vec::new()
    } else {
        vec![
            control(id, "name"),
            control(id, "cancel"),
            control(id, "submit"),
        ]
    };
    super::accessibility_ui::dialog(
        "files-name-dialog",
        dialog.title(),
        "",
        false,
        super::focus::group(
            body,
            order,
            Vec::new(),
            (!dialog.busy).then(|| control(id, "name")),
        ),
    )
}
