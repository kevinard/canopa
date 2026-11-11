//! Confirmation for permanent deletion and Empty Trash. The dialog captures the
//! exact selection or Trash items it names; confirming submits them to the
//! operation queue, which owns all removal. Cancel is the default choice.
use super::file_operations::{Origin, Submission};
use super::*;
use crate::{operation_jobs::Request, trash::TrashItem};
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{column, container, row, text},
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

fn count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "item" } else { "items" })
}

fn quoted(name: &std::ffi::OsStr) -> String {
    format!("“{}”", name.to_string_lossy())
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
        }
    }

    pub fn title(&self) -> &'static str {
        match &self.target {
            Target::Selection {
                unavailable: true, ..
            } => "Can't move to Trash",
            Target::Trash { empty: true, .. } => "Empty Trash?",
            _ => "Delete permanently?",
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
        let warning = "This can't be undone.";
        match &self.target {
            Target::Selection {
                paths, unavailable, ..
            } => {
                let subject = match paths.as_slice() {
                    [path] => quoted(path.file_name().unwrap_or(path.as_os_str())),
                    paths => count(paths.len()),
                };
                if *unavailable {
                    let (verb, pronoun) = if paths.len() == 1 {
                        ("is", "it")
                    } else {
                        ("are", "them")
                    };
                    format!(
                        "{subject} {verb} on a volume without a usable Trash, so {pronoun} cannot be moved to Trash. Delete {pronoun} permanently instead? {warning}"
                    )
                } else {
                    format!("Permanently delete {subject}? {warning}")
                }
            }
            Target::Trash {
                items,
                empty: true,
                main,
            } => format!(
                "Permanently delete all {} in {}? {warning}",
                count(items.len()),
                if *main { "Main Trash" } else { "Trash" }
            ),
            Target::Trash { items, .. } => {
                let subject = match items.as_slice() {
                    [item] => quoted(item.name()),
                    items => count(items.len()),
                };
                format!("Permanently delete {subject} from Trash? {warning}")
            }
        }
    }

    #[cfg(test)]
    pub fn summary(&self) -> String {
        format!("{} | {}", self.message(), self.confirm_label())
    }

    fn confirm_label(&self) -> String {
        match &self.target {
            Target::Trash { empty: true, .. } => "Empty Trash".into(),
            _ => match self.paths().len() {
                1 => "Delete permanently".into(),
                count => format!("Delete {count} items"),
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
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(440.0, 220.0),
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
                self.file_operations.error = Some("Could not open the confirmation.".into());
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

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let action = |label: String, name: &'static str, message: Message, kind| {
        super::focus::button(
            control(id, name),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                Some(message),
                kind,
                false,
                appearance,
            ),
        )
    };
    let body = column![
        chrome::dialog_heading(desktop, chrome::Icon::Trash, dialog.title(), ""),
        text(dialog.message())
            .font(font)
            .size(size)
            .wrapping(text::Wrapping::WordOrGlyph),
        iced::widget::space().height(Length::Fill),
        row![
            iced::widget::space().width(Length::Fill),
            action(
                "Cancel".into(),
                "cancel",
                Message::DeleteDialog(Event::Cancel(id)),
                canopa_ui::Action::Quiet,
            ),
            action(
                dialog.confirm_label(),
                "confirm",
                Message::DeleteDialog(Event::Confirm(id)),
                canopa_ui::Action::Destructive,
            ),
        ]
        .spacing(8),
    ]
    .spacing(12);
    let appearance = appearance.clone();
    let body = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
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
