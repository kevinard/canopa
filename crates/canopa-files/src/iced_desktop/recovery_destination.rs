//! The destination draft survives a refusal; admission alone does not close it.
use super::*;
use crate::{
    operation_jobs::{JobSnapshot, Phase, Request},
    operations::RecoveryItem,
};
use iced::{
    Length,
    widget::{column, container, row, scrollable, text, text_input},
};

pub(super) struct Dialog {
    pub browser: destination_browser::Browser,
    pub window: window::Id,
    pub owner: window::Id,
    pub busy: bool,
    pub error: Option<String>,
    pub job: Option<u64>,
    item: RecoveryItem,
    folder: String,
    name: String,
}
#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id, Option<window::Id>),
    Folder(window::Id, String),
    Browser(window::Id, destination_browser::Event),
    Name(window::Id, String),
    Submit(window::Id),
    Close(window::Id),
}
fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-recovery-destination-{id:?}-{name}"))
}
fn destination(
    folder: &str,
    name: &str,
    original: &std::ffi::OsStr,
    home: &Path,
) -> io::Result<PathBuf> {
    if folder.is_empty() || folder.contains('\0') || name.contains('\0') {
        return Err(io::Error::other(fl!("files-recovery-destination-invalid")));
    }
    if name.starts_with('/') || name.starts_with("file://") || name.starts_with("~/") {
        return local_path(Path::new(name), home, Some(home));
    }
    let folder = local_path(Path::new(folder), home, Some(home))?;
    if name.is_empty() {
        return Ok(folder.join(original));
    }
    if name == "." || name == ".." || name.contains('/') {
        return Err(io::Error::other(fl!("files-recovery-name-invalid")));
    }
    Ok(folder.join(name))
}
impl Desktop {
    pub(super) fn show_recovery_destination(
        &mut self,
        owner: window::Id,
        item: RecoveryItem,
    ) -> Task<Message> {
        if let Some(dialog) = &self.recovery_destination {
            return window::gain_focus(dialog.window);
        }
        let (id, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(540.0, 350.0),
                min_size: Some(Size::new(320.0, 300.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.recovery_destination = Some(Dialog {
            browser: destination_browser::Browser::default(),
            window: id,
            owner,
            busy: false,
            error: None,
            job: None,
            item,
            folder: "~".into(),
            name: String::new(),
        });
        opened.map(move |opened| Message::RecoveryDestination(Event::Opened(id, opened)))
    }
    pub(super) fn close_recovery_destination(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.recovery_destination.take() else {
            return Task::none();
        };
        if dialog.window != id {
            self.recovery_destination = Some(dialog);
            return Task::none();
        }
        let focus = self
            .recovery_window
            .as_ref()
            .filter(|w| w.window == dialog.owner && self.focused_window == Some(id))
            .map_or_else(Task::none, |w| window::gain_focus(w.window));
        Task::batch([focus.chain(window::close(id)), self.exit_if_empty()])
    }
    pub(super) fn recovery_destination_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            && self
                .recovery_destination
                .as_ref()
                .is_some_and(|dialog| dialog.window == id && dialog.browser.open())
        {
            return self.recovery_destination_event(Event::Browser(
                id,
                destination_browser::Event::Cancel,
            ));
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_recovery_destination(id);
        }
        Task::none()
    }
    pub(super) fn recovery_destination_completed(&mut self, job: &JobSnapshot) -> Task<Message> {
        let Some(dialog) = self
            .recovery_destination
            .as_mut()
            .filter(|d| d.job == Some(job.id))
        else {
            return Task::none();
        };
        let refused = job.phase != Phase::SaveFailed
            && job.recovery.as_ref().is_some_and(|report| {
                report.transfer.operation.is_none()
                    && report.transfer.completed == 0
                    && !report.interrupted
                    && !job.recovery_actions.retry
                    && !job.recovery_actions.remaining
            });
        if refused {
            dialog.busy = false;
            dialog.job = None;
            dialog.error = Some(
                job.error
                    .clone()
                    .unwrap_or_else(|| fl!("files-recovery-no-copy")),
            );
            if self.focused_window == Some(dialog.window) {
                iced::widget::operation::focus(control(dialog.window, "folder"))
            } else {
                Task::none()
            }
        } else {
            let id = dialog.window;
            self.close_recovery_destination(id)
        }
    }
    pub(super) fn recovery_destination_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id, _)
            | Event::Folder(id, _)
            | Event::Browser(id, _)
            | Event::Name(id, _)
            | Event::Submit(id)
            | Event::Close(id) => *id,
        };
        let Some(dialog) = self
            .recovery_destination
            .as_mut()
            .filter(|d| d.window == id)
        else {
            return if matches!(event, Event::Opened(_, Some(_))) {
                window::close(id)
            } else {
                Task::none()
            };
        };
        match event {
            Event::Opened(_, opened) => {
                if opened.is_none() {
                    self.recovery_destination = None;
                    if let Some(w) = &mut self.recovery_window {
                        w.error = Some(fl!("files-recovery-dialog-failed"));
                    }
                    return Task::none();
                }
                return window::gain_focus(id)
                    .chain(iced::widget::operation::focus(control(id, "folder")))
                    .chain(iced::widget::operation::select_all(control(id, "folder")));
            }
            Event::Browser(_, event) if !dialog.busy => {
                let using =
                    dialog.browser.open() && matches!(event, destination_browser::Event::Use(_));
                let task = dialog.browser.update(
                    id,
                    event,
                    &mut dialog.folder,
                    &self.home,
                    move |event| Message::RecoveryDestination(Event::Browser(id, event)),
                );
                if using && !dialog.browser.open() {
                    dialog.error = None;
                }
                return task;
            }
            Event::Folder(_, folder) if !dialog.busy && !dialog.browser.open() => {
                dialog.folder = folder;
                dialog.error = None;
            }
            Event::Name(_, name) if !dialog.busy => {
                dialog.name = name;
                dialog.error = None;
            }
            Event::Close(_) => return self.close_recovery_destination(id),
            Event::Submit(_) if !dialog.busy && !dialog.browser.open() => {
                if !self.file_operations.ready() {
                    dialog.error = Some(fl!("files-operation-queue-not-ready"));
                    return Task::none();
                }
                let destination =
                    match destination(&dialog.folder, &dialog.name, dialog.item.name(), &self.home)
                    {
                        Ok(path) => path,
                        Err(error) => {
                            dialog.error = Some(error.to_string());
                            return Task::none();
                        }
                    };
                dialog.busy = true;
                dialog.error = None;
                let (owner, item) = (dialog.owner, dialog.item.clone());
                return self.submit_file_operation(
                    Request::Recover {
                        item: Box::new(item),
                        destination: destination.clone(),
                    },
                    file_operations::Submission {
                        origin: None,
                        owner,
                        dialog: Some(id),
                        affected: vec![destination],
                        reveal: None,
                        refresh_all: false,
                    },
                );
            }
            _ => {}
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
    let message = move |event| Message::RecoveryDestination(Event::Browser(id, event));
    if let Some(browser) = dialog.browser.view(desktop, id, message) {
        return browser;
    }
    let browse = destination_browser::browse_button(
        desktop,
        id,
        (!dialog.busy).then_some(message(destination_browser::Event::Browse)),
    );
    let input = |placeholder: String, value: &'a str, name: &str| {
        text_input(&placeholder, value)
            .id(control(id, name))
            .font(font)
            .size(size)
            .padding(canopa_ui::content_padding(
                appearance.style(canopa_theme::UiKind::Input),
            ))
            .style(move |theme, status| view::input_style(appearance, theme, status))
    };
    let mut folder = input(fl!("files-folder-path"), &dialog.folder, "folder");
    let mut name = input(fl!("files-recovery-keep-name"), &dialog.name, "name");
    if !dialog.busy {
        folder = folder
            .on_input(move |value| Message::RecoveryDestination(Event::Folder(id, value)))
            .on_submit(Message::RecoveryDestination(Event::Submit(id)));
        name = name
            .on_input(move |value| Message::RecoveryDestination(Event::Name(id, value)))
            .on_submit(Message::RecoveryDestination(Event::Submit(id)));
    }
    let field =
        |label: String, key: &'static str, value: &str, change: fn(window::Id, String) -> Event| {
            let node = super::accessibility_ui::Node::text_input(label, value)
                .key(key)
                .read_only(dialog.busy);
            if dialog.busy {
                node
            } else {
                node.on_text(move |value| Message::RecoveryDestination(change(id, value)))
            }
        };
    let folder = super::accessibility_ui::semantic(
        field(
            fl!("files-folder-path"),
            "files-recover-folder",
            &dialog.folder,
            Event::Folder,
        ),
        folder,
    );
    let name = super::accessibility_ui::semantic(
        field(
            fl!("files-file-name"),
            "files-recover-name",
            &dialog.name,
            Event::Name,
        ),
        name,
    );
    let action = |label: String, name, event, enabled: bool| {
        let message = enabled.then_some(Message::RecoveryDestination(event));
        focus::button(
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
    let body = column![
        chrome::dialog_heading(
            desktop,
            chrome::Icon::Recovery,
            fl!("files-recover-copy-title"),
            dialog.item.name().to_string_lossy().into_owned()
        ),
        column![
            chrome::section_title(appearance, fl!("files-destination-folder")),
            row![folder, browse]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        ]
        .spacing(5),
        column![
            chrome::section_title(appearance, fl!("files-recovery-new-name")),
            name,
        ]
        .spacing(5),
        text(dialog.error.clone().unwrap_or_else(|| {
            if dialog.busy && dialog.job.is_none() {
                fl!("files-recovery-submitting")
            } else if dialog.busy {
                fl!("files-recovery-in-progress")
            } else {
                fl!("files-recovery-hint")
            }
        }))
        .font(font)
        .size(small)
        .style(canopa_ui::muted)
        .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(10);
    let actions = canopa_ui::direction::row([
        action(
            if dialog.busy {
                fl!("files-close")
            } else {
                fl!("files-cancel")
            },
            "close",
            Event::Close(id),
            true,
        ),
        action(
            fl!("files-recovery-recover-here"),
            "submit",
            Event::Submit(id),
            !dialog.busy,
        ),
    ])
    .spacing(8);
    let order = if dialog.busy {
        vec![control(id, "close")]
    } else {
        vec![
            control(id, "folder"),
            control(id, "name"),
            destination_browser::browse_id(id),
            control(id, "close"),
            control(id, "submit"),
        ]
    };
    let reveal = order
        .iter()
        .cloned()
        .map(|input| (input, control(id, "scroll")))
        .collect();
    let dialog_view = focus::group(
        container(
            column![
                scrollable(body)
                    .id(control(id, "scroll"))
                    .height(Length::Fill),
                super::question_dialog::decision_actions(actions, None),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(appearance, true)),
        order,
        reveal,
        (!dialog.busy).then(|| control(id, "folder")),
    );
    super::accessibility_ui::dialog(
        "files-recover-dialog",
        fl!("files-recover-copy-title"),
        "",
        false,
        dialog_view,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_original_names_and_explicit_destination_uris_keep_their_bytes() {
        let home = Path::new("/private/home");
        let original = std::ffi::OsStr::from_bytes(b"report-\xff.txt");
        assert_eq!(
            destination("file:///private/folder-%FE", "", original, home)
                .unwrap()
                .as_os_str()
                .as_bytes(),
            b"/private/folder-\xfe/report-\xff.txt"
        );
        assert_eq!(
            destination(
                "/private/ignored",
                "file:///private/output-%FD",
                original,
                home
            )
            .unwrap()
            .as_os_str()
            .as_bytes(),
            b"/private/output-\xfd"
        );
        assert_eq!(
            destination("~/Recovered", "renamed.txt", original, home).unwrap(),
            home.join("Recovered/renamed.txt")
        );
        assert!(destination("/private", "../elsewhere", original, home).is_err());
        assert!(destination("file://remote/private", "", original, home).is_err());
    }
}
