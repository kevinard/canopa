use super::file_operations::{Event, Origin, Submission};
use super::*;
use crate::browser_ui::text_layout::stem_length;
use crate::{
    archives::{ArchiveRequest, Format},
    operation_jobs::Request,
    operations::OperationRequest,
};
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
    /// Set for Compress, whose value is the archive's name without its suffix,
    /// and for Extract to, whose value is the destination folder.
    pub archive: Option<Archive>,
    /// Set for New document: the template it copies, none for an empty one.
    pub document: Option<Option<PathBuf>>,
    /// Set for Rename of several items, which has its own fields and preview.
    pub batch: Option<Box<super::bulk_rename_ui::Batch>>,
}

pub(super) enum Archive {
    Compress {
        sources: Vec<PathBuf>,
        format: Format,
    },
    Extract {
        archives: Vec<PathBuf>,
        browser: destination_browser::Browser,
    },
}

impl Dialog {
    pub fn title(&self) -> String {
        if let Some(batch) = &self.batch {
            return fl!("files-rename-batch-title", count = batch.len());
        }
        if self.document.is_some() {
            return fl!("files-new-document-title");
        }
        match &self.archive {
            Some(Archive::Compress { .. }) => fl!("files-compress-title"),
            Some(Archive::Extract { browser, .. }) if browser.open() => {
                fl!("files-choose-folder")
            }
            Some(Archive::Extract { .. }) => fl!("files-extract-to-title"),
            None if self.original.is_some() => fl!("files-command-rename"),
            None => fl!("files-command-new-folder"),
        }
    }
}

pub(super) fn control(window: window::Id, name: &'static str) -> iced::widget::Id {
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
            if origin.selected.len() > 1 {
                return self.open_batch_dialog(origin);
            }
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

    /// A new document in the shown folder: a copy of `template`, or empty.
    /// The name proposed is the template's, numbered when it is taken.
    pub(super) fn show_document_dialog(
        &mut self,
        owner: window::Id,
        template: Option<crate::templates::Template>,
    ) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.name {
            return window::gain_focus(dialog.window);
        }
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = Origin::capture(self, owner) else {
            return Task::none();
        };
        let taken = self.listed_names(owner, &origin.directory);
        let name = template.as_ref().map_or_else(
            || std::ffi::OsString::from(fl!("files-new-document-name")),
            |template| template.name.clone(),
        );
        let value = crate::templates::free_name(&name, &taken)
            .to_string_lossy()
            .into_owned();
        self.open_dialog(origin, value, (440.0, 226.0), |dialog| {
            dialog.document = Some(template.map(|template| template.path))
        })
    }

    /// The names in `folder` as `owner`'s browser last listed them.
    pub(super) fn listed_names(
        &self,
        owner: window::Id,
        folder: &Path,
    ) -> std::collections::BTreeSet<std::ffi::OsString> {
        let Some(browser) = self.active(owner).and_then(|tab| tab.browser.as_ref()) else {
            return Default::default();
        };
        let directory = browser.directory();
        let listed = (directory.path == folder || directory.resolved_path == folder)
            .then_some(directory.entries.iter())
            .into_iter()
            .flatten()
            .chain(browser.visible_entries())
            .filter(|entry| entry.path.parent() == Some(folder));
        listed
            .filter_map(|entry| entry.path.file_name().map(ToOwned::to_owned))
            .collect()
    }

    /// Open a dialog window owned by `origin`'s browser, starting with `value`.
    pub(super) fn open_dialog(
        &mut self,
        origin: Arc<Origin>,
        value: String,
        (width, height): (f32, f32),
        configure: impl FnOnce(&mut Dialog),
    ) -> Task<Message> {
        let (window, opened) = window::open_transient(
            origin.window,
            window::Settings {
                size: Size::new(width, height),
                min_size: Some(Size::new(width.min(340.0), height.min(220.0))),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut dialog = Dialog {
            window,
            origin,
            initial: value.clone(),
            value,
            original: None,
            prepared: None,
            busy: false,
            error: None,
            archive: None,
            document: None,
            batch: None,
        };
        configure(&mut dialog);
        self.file_operations.name = Some(dialog);
        opened.map(move |opened| Message::FileOperation(Event::NameOpened(window, opened)))
    }

    /// Compress the selection beside it, after choosing a name and format.
    pub(super) fn show_compress_dialog(&mut self, owner: window::Id) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.name {
            return window::gain_focus(dialog.window);
        }
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = Origin::capture(self, owner) else {
            return Task::none();
        };
        if origin.selected.is_empty() {
            return Task::none();
        }
        let directories = self
            .active(owner)
            .and_then(|tab| tab.browser.as_ref())
            .map(|browser| {
                browser
                    .visible_entries()
                    .into_iter()
                    .filter(|entry| entry.is_directory())
                    .map(|entry| entry.path.clone())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let items: Vec<_> = origin
            .selected
            .iter()
            .filter_map(|path| Some((path.file_name()?, directories.contains(path))))
            .collect();
        let name = crate::archives::suggested_name(&items)
            .to_string_lossy()
            .into_owned();
        let sources = origin.selected.clone();
        let format = self.file_operations.archive_format;
        self.open_archive_dialog(origin, name, Archive::Compress { sources, format })
    }

    /// Extract the selected archives to a chosen folder.
    pub(super) fn show_extract_dialog(&mut self, owner: window::Id) -> Task<Message> {
        if let Some(dialog) = &self.file_operations.name {
            return window::gain_focus(dialog.window);
        }
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = Origin::capture(self, owner) else {
            return Task::none();
        };
        if origin.selected.is_empty() {
            return Task::none();
        }
        let folder = super::display_location(&origin.directory);
        let archives = origin.selected.clone();
        self.open_archive_dialog(
            origin,
            folder,
            Archive::Extract {
                archives,
                browser: destination_browser::Browser::default(),
            },
        )
    }

    /// Extract each selected archive into the folder that shows it.
    pub(super) fn extract_here(&mut self, owner: window::Id) -> Task<Message> {
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = Origin::capture(self, owner) else {
            return Task::none();
        };
        let destination = origin.directory.clone();
        self.submit_extractions(&origin, origin.selected.clone(), destination, None)
    }

    fn submit_extractions(
        &mut self,
        origin: &Arc<Origin>,
        archives: Vec<PathBuf>,
        destination: PathBuf,
        dialog: Option<window::Id>,
    ) -> Task<Message> {
        let tasks: Vec<_> = archives
            .into_iter()
            .map(|archive| {
                let stem = crate::archives::stem(archive.file_name().unwrap_or_default());
                let submission = Submission {
                    origin: Some(origin.clone()),
                    owner: origin.window,
                    dialog,
                    affected: vec![destination.join(stem)],
                    reveal: None,
                    refresh_all: false,
                };
                self.submit_file_operation(
                    Request::Archive(ArchiveRequest::extract(archive, destination.clone())),
                    submission,
                )
            })
            .collect();
        Task::batch(tasks)
    }

    fn open_archive_dialog(
        &mut self,
        origin: Arc<Origin>,
        value: String,
        archive: Archive,
    ) -> Task<Message> {
        let height = match archive {
            Archive::Compress { .. } => 284.0,
            Archive::Extract { .. } => 236.0,
        };
        self.open_dialog(origin, value, (460.0, height), |dialog| {
            dialog.archive = Some(archive)
        })
    }

    pub(super) fn name_format(&mut self, id: window::Id, chosen: Format) {
        if let Some(Dialog {
            archive: Some(Archive::Compress { format, .. }),
            ..
        }) = self
            .file_operations
            .name
            .as_mut()
            .filter(|dialog| dialog.window == id && !dialog.busy)
        {
            *format = chosen;
            // The next archive starts from the last choice.
            self.file_operations.archive_format = chosen;
        }
    }

    pub(super) fn name_browse(
        &mut self,
        id: window::Id,
        event: destination_browser::Event,
    ) -> Task<Message> {
        let home = self.home.clone();
        let Some(dialog) = self
            .file_operations
            .name
            .as_mut()
            .filter(|dialog| dialog.window == id && !dialog.busy)
        else {
            return Task::none();
        };
        let Some(Archive::Extract { browser, .. }) = dialog.archive.as_mut() else {
            return Task::none();
        };
        let task = browser.update(id, event, &mut dialog.value, &home, move |event| {
            Message::FileOperation(Event::NameBrowse(id, event))
        });
        dialog.error = None;
        task
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
            archive: None,
            document: None,
            batch: None,
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
        if let Some(field) = self
            .file_operations
            .name
            .as_ref()
            .and_then(|dialog| dialog.batch.as_ref())
            .map(|batch| batch.first_field())
        {
            let field = super::bulk_rename_ui::field_id(id, field);
            return iced::widget::operation::focus(field.clone())
                .chain(iced::widget::operation::select_all(field))
                .chain(window::gain_focus(id));
        }
        // Renaming a file selects its name without the extension, so typing
        // replaces only the part people usually change; folders and new
        // names select everything. A new document is a file too.
        let stem = self.file_operations.name.as_ref().and_then(|dialog| {
            if dialog.document.is_some() {
                return stem_length(&dialog.value);
            }
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
        if let Some(choice) = dialog.archive.as_ref().map(archive_choice) {
            return self.submit_archive_dialog(id, choice);
        }
        if dialog.batch.is_some() {
            return self.submit_batch_dialog(id);
        }
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
        // A new document never replaces anything; say so before submitting.
        if dialog.document.is_some() {
            let (owner, folder) = (dialog.origin.window, dialog.origin.directory.clone());
            let name = std::ffi::OsString::from(&dialog.value);
            if self.listed_names(owner, &folder).contains(&name) {
                if let Some(dialog) = self.file_operations.name.as_mut() {
                    dialog.error = Some(fl!("files-name-taken"));
                }
                return Task::none();
            }
        }
        let Some(dialog) = self.file_operations.name.as_mut() else {
            return Task::none();
        };
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
        } else if let Some(template) = dialog.document.clone() {
            self.submit_file_operation(
                Request::Mutation(OperationRequest::CreateFile {
                    path: destination,
                    template,
                }),
                submission,
            )
        } else {
            self.submit_file_operation(
                Request::Mutation(OperationRequest::CreateFolder { path: destination }),
                submission,
            )
        }
    }

    fn submit_archive_dialog(&mut self, id: window::Id, choice: Choice) -> Task<Message> {
        let home = self.home.clone();
        let Some(dialog) = self.file_operations.name.as_mut() else {
            return Task::none();
        };
        let origin = dialog.origin.clone();
        match choice {
            Choice::Compress(sources, format) => {
                let name = std::ffi::OsString::from(dialog.value.trim());
                if !crate::archives::valid_name(&name) {
                    dialog.error = Some(fl!("files-name-invalid"));
                    return Task::none();
                }
                let mut archive = name.clone();
                archive.push(format.suffix());
                let submission = Submission {
                    origin: Some(origin.clone()),
                    owner: origin.window,
                    dialog: Some(id),
                    affected: vec![origin.directory.join(archive)],
                    reveal: None,
                    refresh_all: false,
                };
                dialog.busy = true;
                dialog.error = None;
                self.submit_file_operation(
                    Request::Archive(ArchiveRequest::compress(
                        sources,
                        origin.directory.clone(),
                        name,
                        format,
                    )),
                    submission,
                )
            }
            Choice::Extract(archives) => {
                let folder = dialog.value.trim();
                let destination = if folder.is_empty() || folder.contains('\0') {
                    Err(io::Error::other(fl!("files-extract-folder-invalid")))
                } else {
                    super::local_path(Path::new(folder), &home, Some(&home))
                };
                let destination = match destination {
                    Ok(destination) => destination,
                    Err(error) => {
                        dialog.error = Some(error.to_string());
                        return Task::none();
                    }
                };
                dialog.busy = true;
                dialog.error = None;
                self.submit_extractions(&origin, archives, destination, Some(id))
            }
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
        let browsing = self.file_operations.name.as_ref().is_some_and(|dialog| {
            dialog.window == id
                && matches!(&dialog.archive, Some(Archive::Extract { browser, .. }) if browser.open())
        });
        if browsing && key == keyboard::Key::Named(keyboard::key::Named::Escape) {
            return self.name_browse(id, destination_browser::Event::Cancel);
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || modifiers.control() && key.as_ref() == keyboard::Key::Character("w")
        {
            return self.close_name_dialog(id, false);
        }
        Task::none()
    }
}

/// What an archive dialog submits, detached from its borrowed state.
enum Choice {
    Compress(Vec<PathBuf>, Format),
    Extract(Vec<PathBuf>),
}

fn archive_choice(archive: &Archive) -> Choice {
    match archive {
        Archive::Compress { sources, format } => Choice::Compress(sources.clone(), *format),
        Archive::Extract { archives, .. } => Choice::Extract(archives.clone()),
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    if let Some(batch) = &dialog.batch {
        return super::bulk_rename_ui::view(desktop, dialog, batch);
    }
    if let Some(Archive::Extract { browser, .. }) = &dialog.archive {
        let id = dialog.window;
        if let Some(view) = browser.view(desktop, id, move |event| {
            Message::FileOperation(Event::NameBrowse(id, event))
        }) {
            return view;
        }
    }
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let extracting = matches!(dialog.archive, Some(Archive::Extract { .. }));
    let placeholder = if extracting {
        fl!("files-folder-path")
    } else {
        fl!("files-column-name")
    };
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
    let location = match &dialog.archive {
        Some(Archive::Extract { archives, .. }) if archives.len() == 1 => archives[0]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        Some(Archive::Extract { archives, .. }) => {
            fl!("files-extract-archives", count = archives.len())
        }
        _ => folder.strip_prefix(&desktop.home).map_or_else(
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
        ),
    };
    let mut title_font = font;
    title_font.weight = iced::font::Weight::Semibold;
    let body = column![
        chrome::heading_parts(
            desktop,
            match &dialog.archive {
                Some(Archive::Compress { .. }) => chrome::Icon::Compress,
                Some(Archive::Extract { .. }) => chrome::Icon::Extract,
                None if dialog.original.is_some() => chrome::Icon::Edit,
                None if dialog.document.is_some() => chrome::Icon::Plus,
                None => chrome::Icon::Folder,
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
                    match &dialog.archive {
                        Some(Archive::Compress { .. }) => fl!("files-compress-name"),
                        Some(Archive::Extract { .. }) => fl!("files-destination-folder"),
                        None if dialog.original.is_some() => fl!("files-name-full-name"),
                        None if dialog.document.is_some() => fl!("files-name-document-name"),
                        None => fl!("files-name-folder-name"),
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
            if extracting {
                Element::from(
                    row![
                        input,
                        destination_browser::browse_button(
                            desktop,
                            id,
                            (!dialog.busy).then_some(Message::FileOperation(Event::NameBrowse(
                                id,
                                destination_browser::Event::Browse
                            ))),
                        )
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
            } else {
                input
            },
            formats(desktop, dialog),
        ]
        .spacing(6),
        canopa_ui::direction::row([
            iced::widget::space().width(Length::Fill).into(),
            action(fl!("files-cancel"), "cancel", Event::NameCancel(id)),
            action(
                match &dialog.archive {
                    Some(Archive::Compress { .. }) => fl!("files-compress-submit"),
                    Some(Archive::Extract { .. }) => fl!("files-extract-submit"),
                    None if dialog.original.is_some() => fl!("files-command-rename"),
                    None => fl!("files-create"),
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
        let mut order = vec![control(id, "name")];
        match &dialog.archive {
            Some(Archive::Compress { .. }) => order.extend(
                Format::WRITABLE
                    .iter()
                    .map(|format| control(id, format.suffix())),
            ),
            Some(Archive::Extract { .. }) => order.push(destination_browser::browse_id(id)),
            None => {}
        }
        order.extend([control(id, "cancel"), control(id, "submit")]);
        order
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

/// The Compress dialog's format choice; nothing for other dialogs.
fn formats<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let Some(Archive::Compress { format, .. }) = &dialog.archive else {
        return iced::widget::space().height(0).into();
    };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let choices = Format::WRITABLE.iter().map(|choice| {
        let selected = choice == format;
        let message =
            (!dialog.busy).then_some(Message::FileOperation(Event::NameFormat(id, *choice)));
        super::focus::radio(
            control(id, choice.suffix()),
            selected,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(choice.suffix()).font(font).size(small + 1.0),
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
    column![
        iced::widget::space().height(4),
        chrome::section_title(appearance, fl!("files-compress-format")).size(small + 1.0),
        canopa_ui::direction::row(choices).spacing(6),
    ]
    .spacing(6)
    .into()
}

/// The folder of a selection that can be renamed together: at least two
/// items, all in one folder.
pub(super) fn batch_folder(paths: &[PathBuf]) -> Option<&Path> {
    let folder = paths.first()?.parent()?;
    (paths.len() > 1 && paths.iter().all(|path| path.parent() == Some(folder))).then_some(folder)
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
