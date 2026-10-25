//! Selection and presentation only; the catalog and operation queue own I/O.
use super::*;
use crate::{
    operation_jobs::Request,
    trash::{
        TrashItem,
        catalog::{Published, Reader, Scope},
    },
};
use iced::{
    Length,
    widget::{column, container, row, scrollable, space, text},
};

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id),
    Poll(window::Id),
    Scope(window::Id, Scope),
    Select(window::Id, u64, PathBuf),
    Refresh(window::Id),
    Restore(window::Id, bool),
    Copy(window::Id),
    Preview(window::Id),
    Scroll(window::Id, u64, f32, f32),
    Viewport(window::Id, u64, Size),
    OpenFailed(window::Id, String),
}

pub(super) struct TrashWindow {
    pub window: window::Id,
    pub snapshot: Published,
    pub selected: Option<PathBuf>,
    pub error: Option<String>,
    reader: Reader,
    notices: catalog_notices::Notices,
    offset: f32,
    viewport: f32,
}

impl TrashWindow {
    pub fn selected(&self) -> Option<&TrashItem> {
        self.selected.as_ref().and_then(|path| {
            self.snapshot
                .catalog
                .items
                .iter()
                .find(|item| &item.path == path)
        })
    }
    pub fn refresh(&self) {
        self.reader.refresh();
    }
    pub fn subscription(&self) -> Subscription<Message> {
        self.notices
            .subscription()
            .map(|id| Message::Trash(Event::Poll(id)))
    }
}

fn scroll(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-trash-scroll-{id:?}"))
}
fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-trash-{id:?}-{name}"))
}
fn height(appearance: &canopa_theme::Appearance) -> f32 {
    (appearance.typography().body_size.px() * 1.5
        + appearance.typography().label_size.px() * 3.0
        + 20.0)
        .ceil()
}

impl Desktop {
    pub(super) fn show_trash(&mut self) -> Task<Message> {
        if let Some(trash) = &self.trash_window {
            return window::gain_focus(trash.window);
        }
        let reader = match Reader::start(crate::application::data_home(&self.home), Scope::All) {
            Ok(reader) => reader,
            Err(error) => {
                self.file_operations.error = Some(error.to_string());
                return self.show_operations();
            }
        };
        let (id, opened) = window::open(window::Settings {
            size: Size::new(760.0, 540.0),
            min_size: Some(Size::new(280.0, 240.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let notices = catalog_notices::Notices::new(reader.take_notices(), id);
        self.trash_window = Some(TrashWindow {
            window: id,
            snapshot: reader.snapshot(),
            reader,
            notices,
            selected: None,
            error: None,
            offset: 0.0,
            viewport: 360.0,
        });
        opened.map(|id| Message::Trash(Event::Opened(id)))
    }

    pub(super) fn close_trash(&mut self, id: window::Id) -> Task<Message> {
        if self
            .trash_window
            .as_ref()
            .is_none_or(|trash| trash.window != id)
        {
            return Task::none();
        }
        self.trash_window = None;
        let dialog = self
            .trash_restore
            .as_ref()
            .filter(|dialog| dialog.owner == id)
            .map(|dialog| dialog.window);
        let restore = dialog.map_or_else(Task::none, |dialog| self.close_trash_restore(dialog));
        Task::batch([
            restore,
            self.close_source_previews(id),
            self.close_operation_source(id),
            window::close(id),
            self.exit_if_empty(),
        ])
    }

    pub(super) fn trash_selection(&mut self, owner: window::Id) -> Task<Message> {
        if !self.file_operations.ready() {
            return self.show_operations();
        }
        let Some(origin) = file_operations::Origin::capture(self, owner)
            .filter(|origin| !origin.selected.is_empty())
        else {
            return Task::none();
        };
        self.submit_file_operation(
            Request::Trash {
                data_home: crate::application::data_home(&self.home),
                sources: origin.selected.clone(),
            },
            file_operations::Submission {
                origin: Some(origin.clone()),
                owner,
                dialog: None,
                affected: origin.selected.clone(),
                reveal: None,
                refresh_all: false,
            },
        )
    }

    pub(super) fn restore_trash_item(
        &mut self,
        owner: window::Id,
        item: TrashItem,
        destination: Option<PathBuf>,
        dialog: Option<window::Id>,
    ) -> Task<Message> {
        let mut affected = vec![item.path.clone()];
        affected.extend(destination.as_ref().or(item.original.as_ref()).cloned());
        self.submit_file_operation(
            Request::Restore {
                item: Box::new(item),
                destination,
            },
            file_operations::Submission {
                origin: None,
                owner,
                dialog,
                affected,
                reveal: None,
                refresh_all: false,
            },
        )
    }

    pub(super) fn trash_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id)
            | Event::Poll(id)
            | Event::Scope(id, _)
            | Event::Select(id, ..)
            | Event::Refresh(id)
            | Event::Restore(id, _)
            | Event::Copy(id)
            | Event::Preview(id)
            | Event::Scroll(id, ..)
            | Event::Viewport(id, ..)
            | Event::OpenFailed(id, _) => *id,
        };
        let Some(trash) = self
            .trash_window
            .as_mut()
            .filter(|trash| trash.window == id)
        else {
            return if matches!(event, Event::Opened(_)) {
                window::close(id)
            } else {
                Task::none()
            };
        };
        match event {
            Event::Opened(_) => return window::gain_focus(id),
            Event::Poll(_) => {
                trash.snapshot = trash.reader.snapshot();
                if trash.selected().is_none() {
                    trash.selected = None;
                }
                let maximum = (trash.snapshot.catalog.items.len() as f32
                    * height(&self.configuration.appearance)
                    - trash.viewport)
                    .max(0.0);
                if trash.offset > maximum {
                    trash.offset = maximum;
                    return iced::widget::operation::scroll_to(
                        scroll(id),
                        scrollable::AbsoluteOffset {
                            x: None,
                            y: Some(maximum),
                        },
                    );
                }
            }
            Event::Scope(_, scope) => {
                trash.reader.set_scope(scope);
                trash.snapshot = trash.reader.snapshot();
                trash.selected = None;
                trash.error = None;
                trash.offset = 0.0;
                return iced::widget::operation::scroll_to(
                    scroll(id),
                    scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(0.0),
                    },
                );
            }
            Event::Select(_, revision, path) => {
                if trash.snapshot.revision == revision
                    && trash
                        .snapshot
                        .catalog
                        .items
                        .iter()
                        .any(|item| item.path == path)
                {
                    trash.selected = Some(path);
                }
            }
            Event::Refresh(_) => {
                trash.error = None;
                trash.reader.refresh();
            }
            Event::Scroll(_, revision, offset, viewport) => {
                if revision == trash.snapshot.revision {
                    trash.offset = offset;
                    trash.viewport = viewport;
                }
            }
            Event::Viewport(_, revision, size) => {
                if revision == trash.snapshot.revision {
                    trash.viewport = size.height;
                }
            }
            Event::OpenFailed(_, error) => trash.error = Some(error),
            Event::Preview(_) => return self.toggle_preview(id),
            Event::Copy(_) => return self.copy_trash(id),
            Event::Restore(_, elsewhere) => {
                let Some(item) = trash
                    .selected()
                    .filter(|item| {
                        !trash.snapshot.loading && item.problem.is_none() && item.original.is_some()
                    })
                    .cloned()
                else {
                    return Task::none();
                };
                trash.error = None;
                if !self.file_operations.ready() {
                    return self.show_operations();
                }
                if elsewhere {
                    return self.show_trash_restore(id, item);
                }
                return self.restore_trash_item(id, item, None, None);
            }
        }
        Task::none()
    }

    pub(super) fn trash_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::key::Named;
        if key == keyboard::Key::Named(Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_trash(id);
        }
        if modifiers.control() {
            let event = match key.as_ref() {
                keyboard::Key::Character("0") => Some(Event::Scope(id, Scope::All)),
                keyboard::Key::Character("1") => Some(Event::Scope(id, Scope::Main)),
                keyboard::Key::Character("r" | "R") => Some(Event::Restore(id, modifiers.shift())),
                keyboard::Key::Character("c" | "C") => Some(Event::Copy(id)),
                keyboard::Key::Character("j" | "J") => return self.show_operations(),
                keyboard::Key::Character("z" | "Z") => return self.command(id, Command::Undo),
                _ => None,
            };
            if let Some(event) = event {
                return self.trash_event(event);
            }
        }
        if modifiers.is_empty() {
            if key == keyboard::Key::Named(Named::F5) {
                return self.trash_event(Event::Refresh(id));
            }
            if key == keyboard::Key::Named(Named::Space)
                || key.as_ref() == keyboard::Key::Character(" ")
            {
                return self.toggle_preview(id);
            }
            let Some(trash) = self
                .trash_window
                .as_mut()
                .filter(|trash| trash.window == id)
            else {
                return Task::none();
            };
            let items = &trash.snapshot.catalog.items;
            if items.is_empty() {
                return Task::none();
            }
            let current = trash
                .selected
                .as_ref()
                .and_then(|path| items.iter().position(|item| &item.path == path));
            let page = (trash.viewport / height(&self.configuration.appearance))
                .floor()
                .max(1.0) as usize;
            let next = match key {
                keyboard::Key::Named(Named::ArrowDown) => {
                    current.map_or(0, |index| (index + 1).min(items.len() - 1))
                }
                keyboard::Key::Named(Named::ArrowUp) => {
                    current.map_or(0, |index| index.saturating_sub(1))
                }
                keyboard::Key::Named(Named::Home) => 0,
                keyboard::Key::Named(Named::End) => items.len() - 1,
                keyboard::Key::Named(Named::PageDown) => {
                    current.map_or(0, |index| (index + page).min(items.len() - 1))
                }
                keyboard::Key::Named(Named::PageUp) => current.unwrap_or(0).saturating_sub(page),
                _ => return Task::none(),
            };
            trash.selected = Some(items[next].path.clone());
            let top = next as f32 * height(&self.configuration.appearance);
            let bottom = top + height(&self.configuration.appearance);
            let offset = if top < trash.offset {
                top
            } else if bottom > trash.offset + trash.viewport {
                bottom - trash.viewport
            } else {
                trash.offset
            };
            trash.offset = offset;
            return Task::batch([
                iced::widget::operation::scroll_to(
                    scroll(id),
                    scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(offset),
                    },
                ),
                iced::advanced::widget::operate(iced::advanced::widget::operation::scope(
                    control(id, "root"),
                    iced::advanced::widget::operation::focusable::unfocus(),
                )),
            ]);
        }
        Task::none()
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, trash: &'a TrashWindow) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = trash.window;
    let ready = desktop.file_operations.ready() && !trash.snapshot.loading;
    let selected = trash.selected();
    let restore =
        ready && selected.is_some_and(|item| item.problem.is_none() && item.original.is_some());
    let mut order = Vec::new();
    let mut action = |label: &'static str, name: &'static str, message: Message, enabled: bool| {
        let widget = control(id, name);
        if enabled {
            order.push(widget.clone());
        }
        let message = enabled.then_some(message);
        focus::control(
            widget,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                message,
                canopa_ui::Action::Quiet,
                false,
                appearance,
            ),
        )
    };
    let buttons = row![
        action(
            "Restore",
            "restore",
            Message::Trash(Event::Restore(id, false)),
            restore
        ),
        action(
            "Restore to…",
            "restore-to",
            Message::Trash(Event::Restore(id, true)),
            restore
        ),
        action(
            "Copy",
            "copy",
            Message::Trash(Event::Copy(id)),
            selected.is_some() && !trash.snapshot.loading && desktop.file_clipboard.ready()
        ),
        action(
            "Quick Look",
            "preview",
            Message::Trash(Event::Preview(id)),
            selected.is_some()
        ),
        action(
            "Refresh",
            "refresh",
            Message::Trash(Event::Refresh(id)),
            true
        ),
        action(
            "Operations",
            "operations",
            Message::Command(id, Command::Operations),
            true
        ),
    ]
    .spacing(4)
    .wrap();
    let scope = row![
        action(
            "All locations",
            "all",
            Message::Trash(Event::Scope(id, Scope::All)),
            true
        ),
        action(
            "Main Trash",
            "main",
            Message::Trash(Event::Scope(id, Scope::Main)),
            true
        ),
        text(if trash.snapshot.scope == Scope::Main {
            "Main Trash"
        } else {
            "All locations"
        })
        .font(font)
        .size(small),
    ]
    .spacing(6)
    .wrap();
    let row_height = height(appearance);
    let items = &trash.snapshot.catalog.items;
    let first = (trash.offset / row_height).floor().max(0.0) as usize;
    let first = first.saturating_sub(4).min(items.len());
    let end = (first + (trash.viewport / row_height).ceil() as usize + 12).min(items.len());
    let mut rows = column![space().height(first as f32 * row_height)].spacing(0);
    for item in &items[first..end] {
        let label = column![
            text(item.name().to_string_lossy().into_owned())
                .font(font)
                .size(size)
                .wrapping(text::Wrapping::None),
            text(
                item.original
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "Original location unknown".into())
            )
            .font(font)
            .size(small)
            .wrapping(text::Wrapping::None),
            text(
                item.problem
                    .as_deref()
                    .or(item.deletion_date.as_deref())
                    .unwrap_or("Deletion date unknown")
            )
            .font(font)
            .size(small)
            .wrapping(text::Wrapping::None),
        ]
        .spacing(2);
        rows = rows.push(row_state::keyed_row(
            view::RowKey::Trash(item.path.clone()),
            canopa_ui::styled_action(
                container(label).width(Length::Fill).clip(true),
                Some(Message::Trash(Event::Select(
                    id,
                    trash.snapshot.revision,
                    item.path.clone(),
                ))),
                canopa_ui::Action::Quiet,
                trash.selected.as_ref() == Some(&item.path),
                appearance,
            )
            .width(Length::Fill)
            .height(row_height),
        ));
    }
    rows = rows.push(space().height((items.len() - end) as f32 * row_height));
    let revision = trash.snapshot.revision;
    let listing = scrollable(rows)
        .id(scroll(id))
        .height(Length::Fill)
        .on_scroll(move |viewport| {
            Message::Trash(Event::Scroll(
                id,
                revision,
                viewport.absolute_offset().y,
                viewport.bounds().height,
            ))
        });
    let listing = iced::widget::sensor(listing)
        .key((id, trash.snapshot.scope == Scope::Main))
        .on_show(move |size| Message::Trash(Event::Viewport(id, revision, size)))
        .on_resize(move |size| Message::Trash(Event::Viewport(id, revision, size)));
    let status = trash
        .error
        .as_deref()
        .or(trash.snapshot.error.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if trash.snapshot.loading {
                "Loading Trash…".into()
            } else if selected.is_some_and(|item| item.problem.is_some()) {
                "Original information is unavailable. Copy this item to recover its contents."
                    .into()
            } else if items.is_empty() && trash.snapshot.catalog.warnings.is_empty() {
                "Trash is empty.".into()
            } else {
                format!("{} items · Select an item to restore it", items.len())
            }
        });
    let mut body = column![
        buttons,
        scope,
        listing,
        text(status)
            .font(font)
            .size(small)
            .wrapping(text::Wrapping::WordOrGlyph)
    ]
    .spacing(8)
    .padding(10)
    .height(Length::Fill);
    if !trash.snapshot.catalog.warnings.is_empty() {
        body = body.push(
            scrollable(
                text(trash.snapshot.catalog.warnings.join("\n"))
                    .font(font)
                    .size(small)
                    .wrapping(text::Wrapping::WordOrGlyph),
            )
            .height(64),
        );
    }
    focus::group(
        container(body)
            .id(control(id, "root"))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| {
                canopa_ui::frame(appearance, appearance.style(canopa_theme::UiKind::Panel))
            }),
        order,
        Vec::new(),
        None,
    )
}
