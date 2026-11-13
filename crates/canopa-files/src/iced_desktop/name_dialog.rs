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
    pub fn title(&self) -> String {
        if self.original.is_some() {
            fl!("files-command-rename")
        } else {
            fl!("files-command-new-folder")
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
                            receiver
                                .await
                                .unwrap_or_else(|_| Err(fl!("files-rename-worker-stopped")))
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
            .unwrap_or_else(|| fl!("files-new-folder-name"));
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(440.0, 226.0),
                min_size: Some(Size::new(320.0, 220.0)),
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
            self.file_operations.error = Some(fl!("files-name-dialog-failed"));
            return Task::none();
        }
        // Renaming a file selects its name without the extension, so typing
        // replaces only the part people usually change; folders and new
        // names select everything.
        let stem = self.file_operations.name.as_ref().and_then(|dialog| {
            let original = dialog.original.as_ref()?;
            let listed_file = self
                .active(dialog.origin.window)
                .and_then(|tab| tab.browser.as_ref())
                .and_then(|browser| {
                    browser
                        .visible_entries()
                        .into_iter()
                        .find(|entry| &entry.path == original)
                        .map(|entry| !entry.is_directory())
                })
                .unwrap_or(false);
            listed_file.then(|| stem_length(&dialog.value)).flatten()
        });
        let select = match stem {
            Some(end) => iced::widget::operation::select_range(control(id, "name"), 0, end),
            None => iced::widget::operation::select_all(control(id, "name")),
        };
        iced::widget::operation::focus(control(id, "name"))
            .chain(select)
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
            dialog.error = Some(fl!("files-name-invalid"));
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
                .ok_or_else(|| io::Error::other(fl!("files-operation-queue-unavailable")))
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
    let placeholder = fl!("files-column-name");
    let mut input = text_input(&placeholder, &dialog.value)
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
        (None, true) => super::accessibility_ui::Node::status(fl!("files-accepting-operation"))
            .key("files-name-busy"),
        (None, false) => super::accessibility_ui::Node::status("").key("files-name-busy"),
    };
    let action = |label: String, name, event| {
        let message = (!dialog.busy).then_some(Message::FileOperation(event));
        super::focus::button(
            control(id, name),
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label)
                    .font(font)
                    .size(appearance.typography().body_size.px()),
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
    let folder = dialog
        .original
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(&dialog.origin.directory);
    let location = folder.strip_prefix(&desktop.home).map_or_else(
        |_| folder.to_string_lossy().into_owned(),
        |path| {
            if path.as_os_str().is_empty() {
                fl!("files-place-home")
            } else {
                fl!(
                    "files-home-path",
                    path = path.to_string_lossy().replace('/', " › ")
                )
            }
        },
    );
    let mut title_font = font;
    title_font.weight = iced::font::Weight::Semibold;
    let body = column![
        chrome::heading_parts(
            desktop,
            if dialog.original.is_some() {
                chrome::Icon::Edit
            } else {
                chrome::Icon::Folder
            },
            chrome::Tone::Accent,
            text(dialog.title())
                .font(title_font)
                .size(appearance.typography().title_size.px())
                .into(),
            Some(chrome::single_line(
                location,
                font,
                small + 1.0,
                Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted)),
                true
            )),
            None,
        ),
        column![
            // A problem or progress shows beside the field's name, so the
            // dialog keeps one compact height.
            row![
                chrome::section_title(
                    appearance,
                    if dialog.original.is_some() {
                        fl!("files-name-full-name")
                    } else {
                        fl!("files-name-folder-name")
                    },
                )
                .size(small + 1.0),
                iced::widget::space().width(Length::Fill),
                super::accessibility_ui::semantic(
                    feedback,
                    text(dialog.error.clone().unwrap_or_else(|| {
                        if dialog.busy {
                            fl!("files-accepting-operation")
                        } else {
                            String::new()
                        }
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
                    .wrapping(text::Wrapping::None),
                ),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            input,
        ]
        .spacing(6),
        canopa_ui::direction::row([
            iced::widget::space().width(Length::Fill).into(),
            action(fl!("files-cancel"), "cancel", Event::NameCancel(id)),
            action(
                if dialog.original.is_some() {
                    fl!("files-command-rename")
                } else {
                    fl!("files-create")
                },
                "submit",
                Event::NameSubmit(id),
            ),
        ])
        .spacing(8),
    ]
    .spacing(14);
    let appearance = appearance.clone();
    let body = container(body)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(&appearance, true));
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

/// The length in graphemes of `name` before its extension, counting a
/// compressed tarball's two suffixes as one; `None` without an extension.
fn stem_length(name: &str) -> Option<usize> {
    use unicode_segmentation::UnicodeSegmentation;
    const COMPOUND: [&str; 6] = [
        ".tar.gz",
        ".tar.bz2",
        ".tar.xz",
        ".tar.zst",
        ".tar.lz",
        ".tar.lzma",
    ];
    let end = COMPOUND
        .iter()
        .find_map(|suffix| {
            let start = name
                .len()
                .checked_sub(suffix.len())
                .filter(|start| *start > 0)?;
            (name.is_char_boundary(start) && name[start..].eq_ignore_ascii_case(suffix))
                .then_some(start)
        })
        .or_else(|| name.rfind('.').filter(|dot| *dot > 0))?;
    Some(name[..end].graphemes(true).count())
}

#[cfg(test)]
mod stem_tests {
    use super::stem_length;

    #[test]
    fn rename_selects_the_name_before_its_extension() {
        assert_eq!(stem_length("Report Q3.txt"), Some(9));
        assert_eq!(stem_length("backup.tar.gz"), Some(6));
        assert_eq!(stem_length("Café.png"), Some(4));
        assert_eq!(stem_length(".bashrc"), None);
        assert_eq!(stem_length("README"), None);
    }
}
