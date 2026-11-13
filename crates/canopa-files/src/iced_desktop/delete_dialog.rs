//! Confirmation for permanent deletion and Empty Trash. The dialog captures the
//! exact selection or Trash items it names; confirming submits them to the
//! operation queue, which owns all removal. Cancel is the default choice.
use super::file_operations::{Origin, Submission};
use super::*;
use crate::{operation_jobs::Request, trash::TrashItem};
use iced::{
    Length,
    widget::{column, container, text},
};

pub(super) enum Target {
    /// Browser entries. `unavailable` marks a failed Move to Trash.
    Selection {
        origin: Option<Arc<Origin>>,
        paths: Vec<PathBuf>,
        unavailable: bool,
    },
    Trash {
        items: Vec<TrashItem>,
        empty: bool,
        main: bool,
    },
}

pub(super) struct Dialog {
    pub window: window::Id,
    pub owner: window::Id,
    target: Target,
    /// The one listed entry being deleted, shown with its icon and folder.
    subject: Option<crate::browse::Entry>,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id, Option<window::Id>),
    Confirm(window::Id),
    Cancel(window::Id),
}

fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-delete-{id:?}-{name}"))
}

fn display(name: &std::ffi::OsStr) -> String {
    name.to_string_lossy().into_owned()
}

impl Dialog {
    /// A confirmation for `paths` without an operation service, for view tests.
    #[cfg(test)]
    pub fn for_selection(window: window::Id, owner: window::Id, paths: Vec<PathBuf>) -> Self {
        Self {
            window,
            owner,
            target: Target::Selection {
                origin: None,
                paths,
                unavailable: false,
            },
            subject: None,
        }
    }

    pub fn title(&self) -> String {
        match &self.target {
            Target::Selection {
                unavailable: true, ..
            } => fl!("files-delete-title-no-trash"),
            Target::Trash { empty: true, .. } => fl!("files-delete-title-empty-trash"),
            _ => fl!("files-delete-title"),
        }
    }

    fn paths(&self) -> Vec<PathBuf> {
        match &self.target {
            Target::Selection { paths, .. } => paths.clone(),
            Target::Trash { items, .. } => items.iter().map(|item| item.path.clone()).collect(),
        }
    }

    /// The confirmation names exactly what will be removed.
    fn message(&self) -> String {
        match &self.target {
            Target::Selection {
                paths, unavailable, ..
            } => match (paths.as_slice(), *unavailable) {
                ([path], true) => fl!(
                    "files-delete-no-trash-one",
                    name = display(path.file_name().unwrap_or(path.as_os_str()))
                ),
                (paths, true) => fl!("files-delete-no-trash-many", count = paths.len()),
                ([path], false) => fl!(
                    "files-delete-one",
                    name = display(path.file_name().unwrap_or(path.as_os_str()))
                ),
                (paths, false) => fl!("files-delete-many", count = paths.len()),
            },
            Target::Trash {
                items,
                empty: true,
                main,
            } => {
                if *main {
                    fl!("files-delete-empty-main-trash", count = items.len())
                } else {
                    fl!("files-delete-empty-trash", count = items.len())
                }
            }
            Target::Trash { items, .. } => match items.as_slice() {
                [item] => fl!("files-delete-from-trash-one", name = display(item.name())),
                items => fl!("files-delete-from-trash-many", count = items.len()),
            },
        }
    }

    #[cfg(test)]
    pub fn summary(&self) -> String {
        format!("{} | {}", self.message(), self.confirm_label())
    }

    fn confirm_label(&self) -> String {
        match &self.target {
            Target::Trash { empty: true, .. } => fl!("files-empty-trash"),
            _ => match self.paths().len() {
                1 => fl!("files-command-delete-permanently"),
                count => fl!("files-delete-items", count = count),
            },
        }
    }
}

impl Desktop {
    /// Shift+Delete in a Browser: confirm the current selection.
    pub(super) fn confirm_delete_selection(&mut self, owner: window::Id) -> Task<Message> {
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = file_operations::Origin::capture(self, owner)
            .filter(|origin| !origin.selected.is_empty())
        else {
            return Task::none();
        };
        let paths = origin.selected.clone();
        self.show_delete_dialog(
            owner,
            Target::Selection {
                origin: Some(origin),
                paths,
                unavailable: false,
            },
        )
    }

    /// Offered after Move to Trash failed for lack of usable Trash storage.
    pub(super) fn offer_permanent_deletion(
        &mut self,
        submission: &Submission,
        paths: Vec<PathBuf>,
    ) -> Task<Message> {
        if paths.is_empty() || !self.windows.contains_key(&submission.owner) {
            return Task::none();
        }
        self.show_delete_dialog(
            submission.owner,
            Target::Selection {
                origin: submission.origin.clone(),
                paths,
                unavailable: true,
            },
        )
    }

    pub(super) fn confirm_delete_trash(
        &mut self,
        owner: window::Id,
        items: Vec<TrashItem>,
        empty: bool,
        main: bool,
    ) -> Task<Message> {
        if items.is_empty() {
            return Task::none();
        }
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        self.show_delete_dialog(owner, Target::Trash { items, empty, main })
    }

    fn show_delete_dialog(&mut self, owner: window::Id, target: Target) -> Task<Message> {
        if let Some(dialog) = &self.delete_dialog {
            // One confirmation at a time; never replace what the user reads.
            return window::gain_focus(dialog.window);
        }
        // The browser's own listing supplies the entry; no filesystem call here.
        let subject = match &target {
            Target::Selection { paths, .. } if paths.len() == 1 => self
                .active(owner)
                .and_then(|tab| tab.browser.as_ref())
                .and_then(|browser| {
                    browser
                        .visible_entries()
                        .into_iter()
                        .find(|entry| entry.path == paths[0])
                        .cloned()
                }),
            _ => None,
        };
        let height = if subject.is_some() { 234.0 } else { 190.0 };
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(440.0, height),
                min_size: Some(Size::new(320.0, 200.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.delete_dialog = Some(Dialog {
            window,
            owner,
            target,
            subject,
        });
        opened.map(move |opened| Message::DeleteDialog(Event::Opened(window, opened)))
    }

    pub(super) fn close_delete_dialog(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.delete_dialog.take() else {
            return Task::none();
        };
        if dialog.window != id {
            self.delete_dialog = Some(dialog);
            return Task::none();
        }
        let focus = if self.focused_window == Some(id) {
            window::gain_focus(dialog.owner)
        } else {
            Task::none()
        };
        Task::batch([focus.chain(window::close(id)), self.exit_if_empty()])
    }

    pub(super) fn delete_dialog_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_delete_dialog(id);
        }
        Task::none()
    }

    pub(super) fn delete_dialog_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id, _) | Event::Confirm(id) | Event::Cancel(id) => *id,
        };
        if self
            .delete_dialog
            .as_ref()
            .is_none_or(|dialog| dialog.window != id)
        {
            return if matches!(event, Event::Opened(_, Some(_))) {
                window::close(id)
            } else {
                Task::none()
            };
        }
        match event {
            Event::Opened(_, None) => {
                self.delete_dialog = None;
                self.file_operations.error = Some(fl!("files-confirmation-failed"));
                Task::none()
            }
            Event::Opened(..) => {
                window::gain_focus(id).chain(iced::widget::operation::focus(control(id, "cancel")))
            }
            Event::Cancel(_) => self.close_delete_dialog(id),
            Event::Confirm(_) => {
                let dialog = self.delete_dialog.as_ref().unwrap();
                let owner = dialog.owner;
                let (request, origin) = match &dialog.target {
                    Target::Selection { origin, paths, .. } => (
                        Request::Delete {
                            sources: paths.clone(),
                        },
                        origin.clone(),
                    ),
                    Target::Trash { items, empty, .. } => (
                        Request::DeleteTrash {
                            items: items.clone(),
                            empty: *empty,
                        },
                        None,
                    ),
                };
                let affected = dialog.paths();
                let close = self.close_delete_dialog(id);
                if !self.file_operations.ready() {
                    return Task::batch([close, self.show_operations()]);
                }
                let submit = self.submit_file_operation(
                    request,
                    Submission {
                        origin,
                        owner,
                        dialog: None,
                        affected,
                        reveal: None,
                        refresh_all: false,
                    },
                );
                Task::batch([close, submit])
            }
        }
    }
}

/// The item being deleted, set apart so it can be checked at a glance.
fn subject<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let Some(entry) = &dialog.subject else {
        return iced::widget::space().height(0).into();
    };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let mut bold = font;
    bold.weight = iced::font::Weight::Semibold;
    let size = appearance.typography().body_size.px();
    let folder = entry.path.parent().unwrap_or(&entry.path);
    let mut facts = super::question_dialog::display_location(&desktop.home, folder);
    if !entry.is_directory() {
        facts = format!("{facts} · {}", super::listing_ui::size(entry.size));
    }
    chrome::section_card(
        appearance,
        iced::widget::row![
            desktop.icons.file(entry, 28.0),
            iced::widget::column![
                chrome::single_line(entry.name().into_owned(), bold, size, None, true),
                chrome::single_line(
                    facts,
                    font,
                    appearance.typography().label_size.px() + 1.0,
                    Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted)),
                    true,
                ),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 12])
    .into()
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let id = dialog.window;
    let action = |label: String, name: &'static str, message: Message, danger: bool| {
        let label = text(label).font(font).size(size);
        super::focus::button(
            control(id, name),
            Some(message.clone()),
            appearance,
            if danger {
                chrome::danger_action(label, Some(message), false, appearance)
            } else {
                canopa_ui::styled_action(
                    label,
                    Some(message),
                    canopa_ui::Action::Normal,
                    false,
                    appearance,
                )
            }
            .padding(chrome::DIALOG_BUTTON_PADDING),
        )
    };
    let body = column![
        // Like the other dialogs, the heading's subtitle states the stakes;
        // the item is set apart below it.
        chrome::toned_heading(
            desktop,
            chrome::Icon::DangerTrash,
            chrome::Tone::Danger,
            dialog.title(),
            if dialog.subject.is_some() {
                fl!("files-delete-cannot-undo")
            } else {
                dialog.message()
            }
        ),
        subject(desktop, dialog),
        iced::widget::space().height(Length::Fill),
        canopa_ui::direction::row([
            iced::widget::space().width(Length::Fill).into(),
            action(
                fl!("files-cancel"),
                "cancel",
                Message::DeleteDialog(Event::Cancel(id)),
                false,
            ),
            action(
                dialog.confirm_label(),
                "confirm",
                Message::DeleteDialog(Event::Confirm(id)),
                true,
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
    super::accessibility_ui::dialog(
        "files-delete-dialog",
        dialog.title(),
        dialog.message(),
        true,
        super::focus::group(
            body,
            vec![control(id, "cancel"), control(id, "confirm")],
            Vec::new(),
            Some(control(id, "cancel")),
        ),
    )
}
