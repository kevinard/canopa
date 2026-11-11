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
    /// Permanently delete the selected item, after confirmation.
    Delete(window::Id),
    /// Permanently delete every listed item in the current scope.
    Empty(window::Id),
    Copy(window::Id),
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
            size: Size::new(720.0, 460.0),
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
            | Event::Delete(id)
            | Event::Empty(id)
            | Event::Copy(id)
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
            Event::Copy(_) => return self.copy_trash(id),
            Event::Delete(_) | Event::Empty(_) => {
                if trash.snapshot.loading {
                    return Task::none();
                }
                let empty = matches!(event, Event::Empty(_));
                let items: Vec<_> = if empty {
                    trash.snapshot.catalog.items.clone()
                } else {
                    trash.selected().cloned().into_iter().collect()
                };
                let main = trash.snapshot.scope == Scope::Main;
                trash.error = None;
                return self.confirm_delete_trash(id, items, empty, main);
            }
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
        if modifiers == keyboard::Modifiers::SHIFT && key == keyboard::Key::Named(Named::Delete) {
            return self.trash_event(Event::Delete(id));
        }
        if modifiers.is_empty() {
            if key == keyboard::Key::Named(Named::F5) {
                return self.trash_event(Event::Refresh(id));
            }
            if key == keyboard::Key::Named(Named::Delete) {
                return self.trash_event(Event::Delete(id));
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
    iced::widget::responsive(move |size| contents(desktop, trash, size.width >= 570.0)).into()
}

fn contents<'a>(desktop: &'a Desktop, trash: &'a TrashWindow, wide: bool) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let id = trash.window;
    let ready = desktop.file_operations.ready() && !trash.snapshot.loading;
    let selected = trash.selected();
    let restore =
        ready && selected.is_some_and(|item| item.problem.is_none() && item.original.is_some());
    let mut order = Vec::new();
    let mut action = |label: String, name: &'static str, message: Message, enabled: bool| {
        let widget = control(id, name);
        if enabled {
            order.push(widget.clone());
        }
        let message = enabled.then_some(message);
        let scope_selected = (name == "all" && trash.snapshot.scope == Scope::All)
            || (name == "main" && trash.snapshot.scope == Scope::Main);
        let button = canopa_ui::styled_action(
            text(label).font(font).size(small),
            message.clone(),
            if name == "restore" {
                canopa_ui::Action::Primary
            } else if matches!(name, "delete" | "empty") && enabled {
                canopa_ui::Action::Destructive
            } else if matches!(name, "refresh" | "operations")
                || (matches!(name, "restore-to" | "copy") && enabled)
            {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            false,
            appearance,
        )
        .width(if name == "restore" && wide {
            Length::Fill
        } else {
            Length::Shrink
        });
        focus::button(
            widget,
            message,
            appearance,
            container(button).style(move |_| {
                if scope_selected {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                } else {
                    container::Style::default()
                }
            }),
        )
    };
    let restore_button = action(
        fl!("files-trash-restore"),
        "restore",
        Message::Trash(Event::Restore(id, false)),
        restore,
    );
    let alternate = action(
        fl!("files-trash-restore-to"),
        "restore-to",
        Message::Trash(Event::Restore(id, true)),
        restore,
    );
    let copy = action(
        fl!("files-trash-copy"),
        "copy",
        Message::Trash(Event::Copy(id)),
        selected.is_some() && !trash.snapshot.loading && desktop.file_clipboard.ready(),
    );
    let delete = action(
        fl!("files-menu-delete-permanently"),
        "delete",
        Message::Trash(Event::Delete(id)),
        ready && selected.is_some(),
    );
    let buttons: Element<'a, Message> = if wide {
        column![
            restore_button,
            row![alternate, copy].spacing(6).wrap(),
            delete
        ]
        .spacing(6)
        .into()
    } else {
        row![restore_button, alternate, copy, delete]
            .spacing(6)
            .wrap()
            .into()
    };
    let tools = row![
        action(
            fl!("files-trash-empty"),
            "empty",
            Message::Trash(Event::Empty(id)),
            ready && !trash.snapshot.catalog.items.is_empty(),
        ),
        action(
            fl!("files-command-refresh"),
            "refresh",
            Message::Trash(Event::Refresh(id)),
            true
        ),
        action(
            fl!("files-command-operations"),
            "operations",
            Message::Command(id, Command::Operations),
            true
        ),
    ]
    .spacing(6);
    let scope = row![
        chrome::section_title(appearance, fl!("files-preferences-page-locations")),
        action(
            fl!("files-trash-all-locations"),
            "all",
            Message::Trash(Event::Scope(id, Scope::All)),
            true
        ),
        action(
            fl!("files-trash-main"),
            "main",
            Message::Trash(Event::Scope(id, Scope::Main)),
            true
        ),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .wrap();
    let row_height = height(appearance);
    let mut item_font = font;
    item_font.weight = iced::font::Weight::Semibold;
    let muted = Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted));
    let items = &trash.snapshot.catalog.items;
    let first = (trash.offset / row_height).floor().max(0.0) as usize;
    let first = first.saturating_sub(4).min(items.len());
    let end = (first + (trash.viewport / row_height).ceil() as usize + 12).min(items.len());
    let mut rows = column![space().height(first as f32 * row_height)].spacing(0);
    for (index, item) in items[first..end].iter().enumerate() {
        let original = item
            .original
            .as_ref()
            .map(|path| {
                let location = path.parent().unwrap_or(path);
                location.strip_prefix(&desktop.home).map_or_else(
                    |_| location.to_string_lossy().into_owned(),
                    |path| {
                        fl!(
                            "files-home-path",
                            path = path.to_string_lossy().replace('/', " / ")
                        )
                    },
                )
            })
            .unwrap_or_else(|| fl!("files-trash-original-unknown"));
        let date = item
            .problem
            .clone()
            .or_else(|| {
                item.deletion_date.as_ref().map(|date| {
                    // The Trash specification's DeletionDate, in local time.
                    let display = chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S")
                        .map(canopa_i18n::format::date_time)
                        .unwrap_or_else(|_| date.replace('T', " "));
                    fl!("files-trash-deleted", date = display)
                })
            })
            .unwrap_or_else(|| fl!("files-trash-date-unknown"));
        let selected = trash.selected.as_ref() == Some(&item.path);
        let select = Message::Trash(Event::Select(
            id,
            trash.snapshot.revision,
            item.path.clone(),
        ));
        let node = super::accessibility_ui::Node::option(
            item.name().to_string_lossy().into_owned(),
            selected,
        )
        .key(format!(
            "files-trash-{}",
            super::accessibility_ui::path_key(&item.path)
        ))
        .description(fl!(
            "files-trash-item-description",
            location = original.clone(),
            date = date.clone()
        ))
        .position(first + index + 1, items.len())
        .on_click(Some(select.clone()));
        let label = canopa_ui::direction::row([
            container(desktop.icons.chrome(chrome::Icon::Documents, 18.0, 0.9))
                .padding(8)
                .style(move |_| {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                })
                .into(),
            column![
                chrome::single_line(
                    item.name().to_string_lossy().into_owned(),
                    item_font,
                    appearance.typography().body_size.px(),
                    None,
                    true,
                ),
                chrome::single_line(original, font, small, muted, true),
                chrome::single_line(date, font, small, muted, true),
            ]
            .spacing(2)
            .width(Length::Fill)
            .into(),
            if wide {
                if selected {
                    space().width(15).into()
                } else {
                    desktop.icons.chrome(chrome::Icon::Forward, 15.0, 0.82)
                }
            } else {
                chrome::selection_mark(appearance, selected)
            },
        ])
        .spacing(10)
        .align_y(iced::Alignment::Center);
        rows = rows.push(row_state::keyed_row(
            view::RowKey::Trash(item.path.clone()),
            super::accessibility_ui::semantic(
                node,
                container(
                    canopa_ui::styled_action(
                        container(label).width(Length::Fill).clip(true),
                        Some(select),
                        canopa_ui::Action::Quiet,
                        selected,
                        appearance,
                    )
                    .width(Length::Fill)
                    .height(row_height),
                )
                .width(Length::Fill)
                .style(move |_| chrome::catalogue_row(appearance, selected)),
            ),
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
    let listing =
        super::accessibility_ui::semantic(
            super::accessibility_ui::Node::list(fl!("files-trash-items"))
                .key("files-trash-items")
                .busy(trash.snapshot.loading)
                .active_descendant(trash.selected.as_ref().map(|path| {
                    format!("files-trash-{}", super::accessibility_ui::path_key(path))
                })),
            listing,
        );
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
                fl!("files-trash-loading")
            } else if selected.is_some_and(|item| item.problem.is_some()) {
                fl!("files-trash-original-unavailable")
            } else if items.is_empty() && trash.snapshot.catalog.warnings.is_empty() {
                String::new()
            } else if !wide && selected.is_none() {
                fl!("files-trash-count-select", count = items.len())
            } else {
                fl!("files-status-items", count = items.len())
            }
        });
    let listing: Element<'_, Message> = if items.is_empty() && !trash.snapshot.loading {
        chrome::empty_state(
            desktop,
            chrome::Icon::Trash,
            fl!("files-trash-empty-title"),
            fl!("files-trash-empty-description"),
            None,
        )
    } else {
        listing.into()
    };
    let subtitle = fl!("files-trash-subtitle");
    let heading: Element<'a, Message> = if wide {
        chrome::heading_with_action(
            desktop,
            chrome::Icon::Trash,
            fl!("files-place-trash"),
            subtitle.clone(),
            Some(
                tools
                    .push(chrome::dismiss(desktop, id))
                    .align_y(iced::Alignment::Center)
                    .into(),
            ),
        )
    } else {
        column![
            chrome::heading_with_action(
                desktop,
                chrome::Icon::Trash,
                fl!("files-place-trash"),
                subtitle,
                Some(chrome::dismiss(desktop, id))
            ),
            tools.wrap(),
        ]
        .spacing(6)
        .into()
    };
    let mut body = column![
        heading,
        container(scope)
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Spine)),
    ]
    .spacing(10)
    .padding(16)
    .height(Length::Fill);
    let buttons = if wide && !items.is_empty() {
        let inspector = selected.map(|item| {
            let original = item.original.as_deref().map_or_else(
                || fl!("files-trash-original-unknown"),
                |path| {
                    super::question_dialog::display_location(
                        &desktop.home,
                        path.parent().unwrap_or(path),
                    )
                },
            );
            let details = column![
                chrome::section_title(appearance, item.name().to_string_lossy().into_owned())
                    .wrapping(text::Wrapping::WordOrGlyph),
                text(original)
                    .size(small)
                    .font(font)
                    .style(canopa_ui::muted)
                    .wrapping(text::Wrapping::WordOrGlyph),
                buttons,
            ]
            .spacing(10);
            (control(id, "details-scroll"), 260.0, details.into())
        });
        body = body.push(chrome::catalogue(
            appearance,
            fl!("files-trash-items-heading"),
            fl!("files-select-item-for-restore"),
            listing,
            inspector,
        ));
        None
    } else {
        body = body.push(listing);
        (!wide).then_some(buttons)
    };
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
    let status_node =
        super::accessibility_ui::Node::status(status.clone()).key("files-trash-status");
    let status = super::accessibility_ui::semantic(
        status_node,
        text(status)
            .font(font)
            .size(small)
            .style(canopa_ui::muted)
            .wrapping(text::Wrapping::WordOrGlyph),
    );
    let footer: Element<'_, Message> = match buttons {
        Some(buttons) => column![status, buttons].spacing(4).into(),
        None => status,
    };
    body = body
        .push(iced::widget::rule::horizontal(1).style(chrome::divider))
        .push(footer);
    order.push(chrome::dismiss_id(id));
    let reveal = if wide {
        ["restore", "restore-to", "copy", "delete"]
            .into_iter()
            .map(|name| (control(id, name), control(id, "details-scroll")))
            .collect()
    } else {
        Vec::new()
    };
    focus::group(
        chrome::centered(body, 940.0)
            .id(control(id, "root"))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| {
                canopa_ui::frame(appearance, appearance.style(canopa_theme::UiKind::Panel))
            }),
        order,
        reveal,
        None,
    )
}
