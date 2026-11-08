//! A destination draft captures the selected item before the user changes views.
use super::*;
use crate::trash::TrashItem;
use iced::{
    Length,
    widget::{column, container, row, text, text_input},
};

pub(super) struct Dialog {
    pub browser: destination_browser::Browser,
    pub window: window::Id,
    pub owner: window::Id,
    pub busy: bool,
    pub error: Option<String>,
    item: TrashItem,
    folder: String,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id, Option<window::Id>),
    Folder(window::Id, String),
    Browser(window::Id, destination_browser::Event),
    Submit(window::Id),
    Close(window::Id),
}

fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-trash-restore-{id:?}-{name}"))
}

impl Desktop {
    pub(super) fn show_trash_restore(
        &mut self,
        owner: window::Id,
        item: TrashItem,
    ) -> Task<Message> {
        if let Some(dialog) = &self.trash_restore {
            return window::gain_focus(dialog.window);
        }
        let (id, opened) = window::open_transient(
            owner,
            window::Settings {
                size: Size::new(520.0, 320.0),
                min_size: Some(Size::new(320.0, 240.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.trash_restore = Some(Dialog {
            browser: destination_browser::Browser::default(),
            window: id,
            owner,
            busy: false,
            error: None,
            item,
            folder: "~".into(),
        });
        opened.map(move |opened| Message::TrashRestore(Event::Opened(id, opened)))
    }

    pub(super) fn close_trash_restore(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.trash_restore.take() else {
            return Task::none();
        };
        if dialog.window != id {
            self.trash_restore = Some(dialog);
            return Task::none();
        }
        let focus = self
            .trash_window
            .as_ref()
            .filter(|trash| trash.window == dialog.owner && self.focused_window == Some(id))
            .map_or_else(Task::none, |trash| window::gain_focus(trash.window));
        Task::batch([focus.chain(window::close(id)), self.exit_if_empty()])
    }

    pub(super) fn trash_restore_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            && self
                .trash_restore
                .as_ref()
                .is_some_and(|dialog| dialog.window == id && dialog.browser.open())
        {
            return self
                .trash_restore_event(Event::Browser(id, destination_browser::Event::Cancel));
        }
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_trash_restore(id);
        }
        Task::none()
    }

    pub(super) fn trash_restore_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id, _)
            | Event::Folder(id, _)
            | Event::Browser(id, _)
            | Event::Submit(id)
            | Event::Close(id) => *id,
        };
        let Some(dialog) = self
            .trash_restore
            .as_mut()
            .filter(|dialog| dialog.window == id)
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
                    self.trash_restore = None;
                    if let Some(trash) = &mut self.trash_window {
                        trash.error = Some("Could not open the Restore destination dialog.".into());
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
                    move |event| Message::TrashRestore(Event::Browser(id, event)),
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
            Event::Close(_) => return self.close_trash_restore(id),
            Event::Submit(_) if !dialog.busy && !dialog.browser.open() => {
                if !self.file_operations.ready() {
                    dialog.error = Some("The operation queue is not ready. Try again.".into());
                    return Task::none();
                }
                if dialog.folder.is_empty() || dialog.folder.contains('\0') {
                    dialog.error = Some("Enter a local destination folder.".into());
                    return Task::none();
                }
                let folder =
                    match local_path(Path::new(&dialog.folder), &self.home, Some(&self.home)) {
                        Ok(folder) => folder,
                        Err(error) => {
                            dialog.error = Some(error.to_string());
                            return Task::none();
                        }
                    };
                let destination = folder.join(dialog.item.name());
                dialog.busy = true;
                let (owner, item) = (dialog.owner, dialog.item.clone());
                return self.restore_trash_item(owner, item, Some(destination), Some(id));
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
    let message = move |event| Message::TrashRestore(Event::Browser(id, event));
    if let Some(browser) = dialog.browser.view(desktop, id, message) {
        return browser;
    }
    let browse = destination_browser::browse_button(
        desktop,
        id,
        (!dialog.busy).then_some(message(destination_browser::Event::Browse)),
    );
    let mut input = text_input("Folder path", &dialog.folder)
        .id(control(id, "folder"))
        .font(font)
        .size(size)
        .padding(canopa_ui::content_padding(
            appearance.style(canopa_theme::UiKind::Input),
        ))
        .style(move |theme, status| view::input_style(appearance, theme, status));
    if !dialog.busy {
        input = input
            .on_input(move |folder| Message::TrashRestore(Event::Folder(id, folder)))
            .on_submit(Message::TrashRestore(Event::Submit(id)));
    }
    let action = |label, name, event| {
        let message = (!dialog.busy).then_some(Message::TrashRestore(event));
        focus::control(
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
    let location = local_path(
        Path::new(&dialog.folder),
        &desktop.home,
        Some(&desktop.home),
    )
    .ok()
    .filter(|_| !dialog.folder.is_empty() && !dialog.busy)
    .map(|path| {
        chrome::single_line(
            format!(
                "Location · {}",
                super::question_dialog::display_location(&desktop.home, &path)
            ),
            font,
            small,
            Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted)),
            true,
        )
    });
    let facts = column![
        chrome::dialog_heading(
            desktop,
            chrome::Icon::Folder,
            "Restore to folder",
            dialog.item.name().to_string_lossy().into_owned()
        ),
        column![
            chrome::section_title(appearance, "Destination folder"),
            row![input, browse]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        ]
        .spacing(5),
        location,
        text(dialog.error.as_deref().unwrap_or(if dialog.busy {
            "Accepting operation…"
        } else {
            "If this name is already taken, the item stays in Trash."
        }))
        .font(font)
        .size(small)
        .style(canopa_ui::muted)
        .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(10);
    let actions = row![
        action("Cancel", "cancel", Event::Close(id)),
        action("Restore here", "submit", Event::Submit(id))
    ]
    .spacing(8);
    let body = container(
        column![
            iced::widget::scrollable(facts).height(Length::Fill),
            super::question_dialog::decision_actions(actions, None),
        ]
        .spacing(10),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(appearance, appearance.style(canopa_theme::UiKind::Panel)));
    let order = if dialog.busy {
        Vec::new()
    } else {
        vec![
            control(id, "folder"),
            destination_browser::browse_id(id),
            control(id, "cancel"),
            control(id, "submit"),
        ]
    };
    focus::group(
        body,
        order,
        Vec::new(),
        (!dialog.busy).then(|| control(id, "folder")),
    )
}
