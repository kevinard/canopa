//! Recovery presentation consumes backend snapshots and submits explicit work.
use super::*;
use crate::{
    operation_jobs::{Phase, Request},
    operations::{RecoveryIdentity, RecoveryItem},
    recovery_catalog::{Published, Reader},
};
use iced::{
    Length,
    widget::{column, container, row, scrollable, space, text},
};

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id),
    Poll(window::Id),
    Select(window::Id, u64, RecoveryIdentity),
    Refresh(window::Id),
    More(window::Id),
    Recover(window::Id),
    Keep(window::Id),
    Reconcile(window::Id),
    Scroll(window::Id, u64, f32, f32),
    Viewport(window::Id, u64, Size),
    OpenFailed(window::Id, String),
}
pub(super) struct RecoveryWindow {
    pub window: window::Id,
    pub snapshot: Published,
    pub selected: Option<RecoveryIdentity>,
    pub error: Option<String>,
    reader: Reader,
    notices: catalog_notices::Notices,
    offset: f32,
    viewport: f32,
}
impl RecoveryWindow {
    pub fn selected(&self) -> Option<&RecoveryItem> {
        self.selected.and_then(|identity| {
            self.snapshot
                .catalog
                .items
                .iter()
                .find(|i| i.identity() == identity)
        })
    }
    pub fn refresh(&self) {
        self.reader.refresh();
    }
    pub fn subscription(&self) -> Subscription<Message> {
        self.notices
            .subscription()
            .map(|id| Message::Recovery(Event::Poll(id)))
    }
}
fn scroll(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-recovery-scroll-{id:?}"))
}
fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-recovery-{id:?}-{name}"))
}
fn height(appearance: &canopa_theme::Appearance) -> f32 {
    (appearance.typography().body_size.px() * 1.5
        + appearance.typography().label_size.px() * 3.0
        + 20.0)
        .ceil()
}
impl Desktop {
    pub(super) fn show_recovery(&mut self) -> Task<Message> {
        if let Some(recovery) = &self.recovery_window {
            self.file_operations.recovery_presented();
            return window::gain_focus(recovery.window);
        }
        let Some(jobs) = self.file_operations.service.clone() else {
            return self.show_operations();
        };
        let reader = match Reader::start(jobs) {
            Ok(reader) => reader,
            Err(error) => {
                self.file_operations.error = Some(error.to_string());
                return self.show_operations();
            }
        };
        let (id, opened) = window::open(window::Settings {
            size: Size::new(680.0, 350.0),
            min_size: Some(Size::new(280.0, 240.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let notices = catalog_notices::Notices::new(reader.take_notices(), id);
        self.file_operations.recovery_presented();
        self.recovery_window = Some(RecoveryWindow {
            window: id,
            snapshot: reader.snapshot(),
            reader,
            notices,
            selected: None,
            error: None,
            offset: 0.0,
            viewport: 360.0,
        });
        opened.map(|id| Message::Recovery(Event::Opened(id)))
    }
    pub(super) fn close_recovery(&mut self, id: window::Id) -> Task<Message> {
        if self.recovery_window.as_ref().is_none_or(|w| w.window != id) {
            return Task::none();
        }
        self.recovery_window = None;
        let draft = self
            .recovery_destination
            .as_ref()
            .filter(|d| d.owner == id)
            .map(|d| d.window);
        let close = draft.map_or_else(Task::none, |id| self.close_recovery_destination(id));
        Task::batch([
            close,
            self.close_source_previews(id),
            self.close_operation_source(id),
            window::close(id),
            self.exit_if_empty(),
        ])
    }
    pub(super) fn recovery_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id)
            | Event::Poll(id)
            | Event::Select(id, ..)
            | Event::Refresh(id)
            | Event::More(id)
            | Event::Recover(id)
            | Event::Keep(id)
            | Event::Reconcile(id)
            | Event::Scroll(id, ..)
            | Event::Viewport(id, ..)
            | Event::OpenFailed(id, _) => *id,
        };
        let Some(recovery) = self.recovery_window.as_mut().filter(|w| w.window == id) else {
            return if matches!(event, Event::Opened(_)) {
                window::close(id)
            } else {
                Task::none()
            };
        };
        match event {
            Event::Opened(_) => return window::gain_focus(id),
            Event::Poll(_) => {
                recovery.snapshot = recovery.reader.snapshot();
                if !recovery.snapshot.loading
                    && !recovery.snapshot.catalog.has_more
                    && recovery.selected().is_none()
                {
                    recovery.selected = None;
                }
                let maximum = (recovery.snapshot.catalog.items.len() as f32
                    * height(&self.configuration.appearance)
                    - recovery.viewport)
                    .max(0.0);
                if recovery.offset > maximum {
                    recovery.offset = maximum;
                    return iced::widget::operation::scroll_to(
                        scroll(id),
                        scrollable::AbsoluteOffset {
                            x: None,
                            y: Some(maximum),
                        },
                    );
                }
            }
            Event::Select(_, revision, identity)
                if recovery.snapshot.revision == revision
                    && recovery
                        .snapshot
                        .catalog
                        .items
                        .iter()
                        .any(|i| i.identity() == identity) =>
            {
                recovery.selected = Some(identity);
            }
            Event::Refresh(_) => {
                recovery.error = None;
                recovery.reader.refresh();
            }
            Event::More(_) => recovery.reader.load_more(),
            Event::Scroll(_, revision, offset, viewport)
                if revision == recovery.snapshot.revision =>
            {
                recovery.offset = offset;
                recovery.viewport = viewport;
            }
            Event::Viewport(_, revision, size) if revision == recovery.snapshot.revision => {
                recovery.viewport = size.height
            }
            Event::OpenFailed(_, error) => recovery.error = Some(error),
            Event::Recover(_) => {
                let Some(item) = recovery
                    .selected()
                    .filter(|item| !recovery.snapshot.loading && item.entry.is_some())
                    .cloned()
                else {
                    return Task::none();
                };
                if !self.file_operations.ready() {
                    return self.show_operations();
                }
                recovery.error = None;
                return self.show_recovery_destination(id, item);
            }
            Event::Reconcile(_) => {
                if !self.file_operations.ready() {
                    return self.show_operations();
                }
                return self.submit_file_operation(
                    Request::Reconcile,
                    file_operations::Submission {
                        origin: None,
                        owner: id,
                        dialog: None,
                        affected: Vec::new(),
                        reveal: None,
                        refresh_all: true,
                    },
                );
            }
            Event::Keep(_) => {
                let Some(item) = recovery
                    .selected()
                    .filter(|item| !recovery.snapshot.loading && item.can_keep_in_recovery())
                    .cloned()
                else {
                    return Task::none();
                };
                if !self.file_operations.ready() {
                    return self.show_operations();
                }
                return self.submit_file_operation(
                    Request::KeepRecovery {
                        item: Box::new(item),
                    },
                    file_operations::Submission {
                        origin: None,
                        owner: id,
                        dialog: None,
                        affected: Vec::new(),
                        reveal: None,
                        refresh_all: true,
                    },
                );
            }
            _ => {}
        }
        Task::none()
    }
    pub(super) fn recovery_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::key::Named;
        if key == keyboard::Key::Named(Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_recovery(id);
        }
        if modifiers.control() {
            match key.as_ref() {
                keyboard::Key::Character("r" | "R") => {
                    return self.recovery_event(Event::Recover(id));
                }
                keyboard::Key::Character("j" | "J") => return self.show_operations(),
                keyboard::Key::Character("z" | "Z") => return self.command(id, Command::Undo),
                _ => {}
            }
        }
        if !modifiers.is_empty() {
            return Task::none();
        }
        match key {
            keyboard::Key::Named(Named::F5) => return self.recovery_event(Event::Refresh(id)),
            keyboard::Key::Named(Named::Space) => return self.toggle_preview(id),
            keyboard::Key::Character(ref s) if s == " " => return self.toggle_preview(id),
            keyboard::Key::Named(Named::Enter) => return self.recovery_event(Event::Recover(id)),
            _ => {}
        }
        let Some(recovery) = self.recovery_window.as_mut().filter(|w| w.window == id) else {
            return Task::none();
        };
        let items = &recovery.snapshot.catalog.items;
        if items.is_empty() {
            return Task::none();
        }
        let current = recovery
            .selected
            .and_then(|identity| items.iter().position(|i| i.identity() == identity));
        let row_height = height(&self.configuration.appearance);
        let page = (recovery.viewport / row_height).floor().max(1.0) as usize;
        let next = match key {
            keyboard::Key::Named(Named::ArrowDown) => {
                current.map_or(0, |i| (i + 1).min(items.len() - 1))
            }
            keyboard::Key::Named(Named::ArrowUp) => current.map_or(0, |i| i.saturating_sub(1)),
            keyboard::Key::Named(Named::Home) => 0,
            keyboard::Key::Named(Named::End) => items.len() - 1,
            keyboard::Key::Named(Named::PageDown) => {
                current.map_or(0, |i| (i + page).min(items.len() - 1))
            }
            keyboard::Key::Named(Named::PageUp) => current.unwrap_or(0).saturating_sub(page),
            _ => return Task::none(),
        };
        recovery.selected = Some(items[next].identity());
        let top = next as f32 * row_height;
        let offset = if top < recovery.offset {
            top
        } else if top + row_height > recovery.offset + recovery.viewport {
            top + row_height - recovery.viewport
        } else {
            recovery.offset
        };
        recovery.offset = offset;
        Task::batch([
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
        ])
    }
}
pub(super) fn view<'a>(desktop: &'a Desktop, recovery: &'a RecoveryWindow) -> Element<'a, Message> {
    iced::widget::responsive(move |size| contents(desktop, recovery, size.width >= 620.0)).into()
}

fn contents<'a>(
    desktop: &'a Desktop,
    recovery: &'a RecoveryWindow,
    wide: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let small = appearance.typography().label_size.px();
    let id = recovery.window;
    let selected = recovery.selected();
    let ready = desktop.file_operations.ready() && !recovery.snapshot.loading;
    let mut order = Vec::new();
    let mut action = |label: &'static str, name: &str, event: Event, enabled: bool| {
        let widget = control(id, name);
        if enabled {
            order.push(widget.clone());
        }
        let message = enabled.then_some(Message::Recovery(event));
        focus::control(
            widget,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                message,
                if name == "recover" {
                    canopa_ui::Action::Primary
                } else if name == "refresh" {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                false,
                appearance,
            ),
        )
    };
    let interrupted = !recovery.snapshot.catalog.interrupted.is_empty()
        || desktop
            .file_operations
            .snapshot
            .jobs
            .iter()
            .any(|j| j.phase == Phase::Interrupted);
    let has_items = !recovery.snapshot.catalog.items.is_empty();
    let show_empty_action = !has_items && !recovery.snapshot.loading && !interrupted;
    let mut item_actions = row![].spacing(6);
    if has_items {
        item_actions = item_actions.push(action(
            "Recover a copy…",
            "recover",
            Event::Recover(id),
            ready && selected.is_some_and(|item| item.entry.is_some()),
        ));
    }
    if selected.is_some_and(RecoveryItem::can_keep_in_recovery) {
        item_actions =
            item_actions.push(action("Keep in Recovery", "keep", Event::Keep(id), ready));
    }
    let refresh = action("Refresh", "refresh", Event::Refresh(id), true);
    let mut buttons = row![].spacing(6);
    if recovery.snapshot.catalog.has_more {
        buttons = buttons.push(action(
            "Load more",
            "more",
            Event::More(id),
            !recovery.snapshot.loading,
        ));
    }
    if interrupted {
        buttons = buttons.push(action(
            "Reconcile interrupted work",
            "reconcile",
            Event::Reconcile(id),
            ready,
        ));
    }
    let operations = focus::control(
        control(id, "operations"),
        Some(Message::Command(id, Command::Operations)),
        appearance,
        canopa_ui::styled_action(
            text(if show_empty_action {
                "View Operations"
            } else {
                "Operations"
            })
            .font(font)
            .size(small),
            Some(Message::Command(id, Command::Operations)),
            if show_empty_action {
                canopa_ui::Action::Primary
            } else {
                canopa_ui::Action::Normal
            },
            false,
            appearance,
        ),
    );
    order.push(control(id, "operations"));
    let row_height = height(appearance);
    let items = &recovery.snapshot.catalog.items;
    let first = ((recovery.offset / row_height).floor().max(0.0) as usize)
        .saturating_sub(4)
        .min(items.len());
    let end = (first + (recovery.viewport / row_height).ceil() as usize + 12).min(items.len());
    let mut item_font = font;
    item_font.weight = iced::font::Weight::Semibold;
    let muted = Some(chrome::ink(appearance, canopa_theme::ColorRole::Muted));
    let mut rows = column![space().height(first as f32 * row_height)].spacing(0);
    for item in &items[first..end] {
        let selected = recovery.selected == Some(item.identity());
        let original = item.original.as_ref().map_or_else(
            || "Original location unknown".into(),
            |path| {
                super::question_dialog::display_location(
                    &desktop.home,
                    path.parent().unwrap_or(path),
                )
            },
        );
        let summary = if item.held_for_undo {
            item.entry
                .as_ref()
                .and_then(|entry| entry.modified)
                .map(chrono::DateTime::<chrono::Local>::from)
                .map_or_else(
                    || "Held for Undo".into(),
                    |time| {
                        format!(
                            "Modified {}  ·  Held for Undo",
                            time.format("%b %-d, %Y · %H:%M")
                        )
                    },
                )
        } else {
            item.message.clone()
        };
        let metadata = column![
            chrome::single_line(
                item.name().to_string_lossy().into_owned(),
                item_font,
                appearance.typography().body_size.px(),
                None,
                true,
            ),
            chrome::single_line(original, font, small, muted, true),
            chrome::single_line(summary, font, small, muted, true),
        ]
        .spacing(2)
        .width(Length::Fill);
        let identity = item.entry.as_ref().map_or_else(
            || desktop.icons.chrome(chrome::Icon::Documents, 20.0, 0.9),
            |entry| desktop.icons.file(entry, 20.0),
        );
        let label = row![
            container(identity)
                .padding(7)
                .style(move |_| chrome::highlight(
                    appearance,
                    canopa_theme::ColorRole::AccentSurface
                )),
            metadata,
            if wide {
                if selected {
                    space().width(15).into()
                } else {
                    desktop.icons.chrome(chrome::Icon::Forward, 15.0, 0.82)
                }
            } else {
                chrome::selection_mark(appearance, selected)
            },
        ]
        .spacing(9)
        .align_y(iced::Alignment::Center);
        rows = rows.push(row_state::keyed_row(
            view::RowKey::Recovery(item.identity()),
            container(
                canopa_ui::styled_action(
                    container(label).width(Length::Fill).clip(true),
                    Some(Message::Recovery(Event::Select(
                        id,
                        recovery.snapshot.revision,
                        item.identity(),
                    ))),
                    canopa_ui::Action::Quiet,
                    selected,
                    appearance,
                )
                .width(Length::Fill)
                .height(row_height),
            )
            .style(move |_| chrome::catalogue_row(appearance, selected)),
        ));
    }
    rows = rows.push(space().height((items.len() - end) as f32 * row_height));
    let revision = recovery.snapshot.revision;
    let listing = scrollable(rows)
        .id(scroll(id))
        .height(Length::Fill)
        .on_scroll(move |v| {
            Message::Recovery(Event::Scroll(
                id,
                revision,
                v.absolute_offset().y,
                v.bounds().height,
            ))
        });
    let listing = iced::widget::sensor(listing)
        .key(id)
        .on_show(move |size| Message::Recovery(Event::Viewport(id, revision, size)))
        .on_resize(move |size| Message::Recovery(Event::Viewport(id, revision, size)));
    let status = recovery
        .error
        .as_deref()
        .or(recovery.snapshot.error.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if recovery.snapshot.loading {
                "Loading Recovery…".into()
            } else if items.is_empty() {
                String::new()
            } else {
                format!(
                    "{} retained {}{} · Recovery creates a copy and keeps the saved version.",
                    items.len(),
                    if items.len() == 1 {
                        "version"
                    } else {
                        "versions"
                    },
                    if recovery.snapshot.catalog.has_more {
                        " loaded"
                    } else {
                        ""
                    }
                )
            }
        });
    let (empty_action, tools_action) = if show_empty_action {
        (Some(operations), None)
    } else {
        (None, Some(operations))
    };
    let listing: Element<'_, Message> = if items.is_empty() && !recovery.snapshot.loading {
        chrome::empty_state(
            desktop,
            chrome::Icon::Documents,
            "No saved versions",
            "Saved versions from file replacements appear here. Operations shows your recent file changes.",
            empty_action,
        )
    } else {
        listing.into()
    };
    let tools = row![refresh, tools_action].spacing(6);
    let subtitle = if interrupted {
        "Saved versions and interrupted operations"
    } else {
        "Recover a copy of an earlier version."
    };
    let heading: Element<'a, Message> = if wide {
        chrome::heading_with_action(
            desktop,
            chrome::Icon::Recovery,
            "Recovery",
            subtitle,
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
                chrome::Icon::Recovery,
                "Recovery",
                subtitle,
                Some(chrome::dismiss(desktop, id))
            ),
            tools.wrap(),
        ]
        .spacing(6)
        .into()
    };
    let (listing, narrow_actions): (Element<'a, Message>, _) = if wide && has_items {
        let note = |value: String| {
            text(value)
                .font(font)
                .size(small)
                .style(canopa_ui::muted)
                .wrapping(text::Wrapping::WordOrGlyph)
        };
        let inspector = selected.map(|item| {
            let name = item.name().to_string_lossy().into_owned();
            let mut details = column![
                chrome::section_title(appearance, name).wrapping(text::Wrapping::WordOrGlyph)
            ]
            .spacing(9);
            if let Some(original) = &item.original {
                details = details.push(note(super::question_dialog::display_location(
                    &desktop.home,
                    original,
                )));
            }
            if let Some(entry) = &item.entry {
                details = details.push(note(super::question_dialog::entry_metadata(entry)));
            }
            details = details
                .push(note(item.message.clone()))
                .push(item_actions.wrap());
            (control(id, "details-scroll"), 260.0, details.into())
        });
        (
            chrome::catalogue(
                appearance,
                "Saved versions",
                "Select a version to choose where to recover it",
                listing,
                inspector,
            ),
            None,
        )
    } else {
        (listing, has_items.then(|| item_actions.wrap()))
    };
    let mut body = column![
        heading,
        listing,
        text(status)
            .font(font)
            .size(small)
            .wrapping(text::Wrapping::WordOrGlyph)
    ]
    .spacing(10)
    .padding(16)
    .height(Length::Fill);
    if narrow_actions.is_some() || recovery.snapshot.catalog.has_more || interrupted {
        body = body
            .push(iced::widget::rule::horizontal(1).style(chrome::divider))
            .push(
                container(column![narrow_actions, buttons.wrap()].spacing(4))
                    .width(Length::Fill)
                    .align_x(iced::Alignment::End),
            );
    }
    let mut notices = recovery.snapshot.catalog.warnings.clone();
    notices.extend(
        recovery
            .snapshot
            .catalog
            .interrupted
            .iter()
            .map(|i| format!("{}: {}", i.label, i.message)),
    );
    if !notices.is_empty() {
        body = body.push(
            scrollable(
                text(notices.join("\n"))
                    .font(font)
                    .size(small)
                    .wrapping(text::Wrapping::WordOrGlyph),
            )
            .height(64),
        );
    }
    order.push(chrome::dismiss_id(id));
    let reveal = if wide {
        ["recover", "keep"]
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
