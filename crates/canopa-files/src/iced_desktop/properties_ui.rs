//! Present prepared metadata; filesystem inspection stays in the shared core.
use super::*;
use canopa_theme::UiKind;
use gio::prelude::FileExt;
use iced::{
    Length,
    widget::{column, row, scrollable, text},
};

#[derive(Clone, Debug)]
pub(super) enum Event {
    Opened(window::Id),
    Ready(window::Id, Arc<Result<Vec<Item>, String>>),
    Copy(window::Id),
}

pub(super) struct Window {
    paths: Vec<PathBuf>,
    cancellation: Cancellation,
    result: Option<Arc<Result<Vec<Item>, String>>>,
}
impl Drop for Window {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
impl Window {
    pub fn title(&self) -> String {
        if self.result.is_none() {
            "Properties — Reading… — Canopa Files".into()
        } else {
            format!(
                "Properties — {} {} — Canopa Files",
                self.paths.len(),
                if self.paths.len() == 1 {
                    "item"
                } else {
                    "items"
                }
            )
        }
    }
}

#[derive(Debug)]
pub(super) struct Item {
    filesystem: crate::file_properties::Item,
    kind: Option<String>,
    owner: Option<String>,
    group: Option<String>,
}

impl Item {
    fn describe(filesystem: crate::file_properties::Item) -> Self {
        let kind = filesystem.metadata.as_ref().ok().and_then(|metadata| {
            if metadata.kind != crate::browse::EntryKind::File {
                return None;
            }
            let (content_type, _) = gio::content_type_guess(Some(&filesystem.path), None);
            (!gio::content_type_is_unknown(&content_type))
                .then(|| gio::content_type_get_description(&content_type).into())
        });
        let identities = filesystem.metadata.as_ref().ok().and_then(|metadata| {
            gio::File::for_path(&filesystem.path)
                .query_info(
                    "owner::user,owner::group,unix::uid,unix::gid",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                )
                .ok()
                .filter(|info| {
                    info.has_attribute("unix::uid")
                        && info.has_attribute("unix::gid")
                        && info.attribute_uint32("unix::uid") == metadata.owner
                        && info.attribute_uint32("unix::gid") == metadata.group
                })
        });
        let attribute = |name| {
            identities
                .as_ref()
                .and_then(|info| info.attribute_string(name))
                .map(Into::into)
        };
        Self {
            filesystem,
            kind,
            owner: attribute("owner::user"),
            group: attribute("owner::group"),
        }
    }
}

fn permission_label(mode: u32) -> &'static str {
    match mode & 7 {
        0 => "none",
        1 => "execute",
        2 => "write",
        3 => "write and execute",
        4 => "read",
        5 => "read and execute",
        6 => "read and write",
        _ => "read, write, and execute",
    }
}

fn control(id: window::Id, index: u8) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-properties-{id:?}-{index}"))
}
fn scroll(id: window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-properties-{id:?}-scroll"))
}
fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).map_or_else(|_| display_location(path), |url| url.to_string())
}

fn timestamp(time: std::time::SystemTime) -> Option<chrono::DateTime<chrono::Utc>> {
    let (seconds, nanos) = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => (
            i64::try_from(duration.as_secs()).ok()?,
            duration.subsec_nanos(),
        ),
        Err(error) => {
            let duration = error.duration();
            let seconds = i64::try_from(duration.as_secs()).ok()?.checked_neg()?;
            if duration.subsec_nanos() == 0 {
                (seconds, 0)
            } else {
                (
                    seconds.checked_sub(1)?,
                    1_000_000_000 - duration.subsec_nanos(),
                )
            }
        }
    };
    chrono::DateTime::from_timestamp(seconds, nanos)
}

fn modified(time: std::time::SystemTime) -> String {
    use chrono::Datelike;
    timestamp(time).map_or_else(
        || "Outside supported date range".into(),
        |utc| {
            if (1..=9999).contains(&utc.year()) {
                utc.with_timezone(&chrono::Local)
                    .format("%b %-d, %Y · %H:%M:%S")
                    .to_string()
            } else {
                utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        },
    )
}

impl Desktop {
    pub(super) fn show_properties(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        let settings = window::Settings {
            size: Size::new(620.0, if paths.len() == 1 { 320.0 } else { 420.0 }),
            min_size: Some(Size::new(280.0, 220.0)),
            exit_on_close_request: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "org.canopa.Files".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (id, opened) = window::open(settings);
        let cancellation = Cancellation::default();
        let worker_cancel = cancellation.clone();
        let worker_paths = paths.clone();
        self.properties.insert(
            id,
            Window {
                paths,
                cancellation,
                result: None,
            },
        );
        Task::batch([
            opened.map(move |_| Message::Properties(Event::Opened(id))),
            Task::perform(
                async move {
                    let items = crate::file_properties::read(&worker_paths, &worker_cancel)
                        .map_err(|error| error.to_string())?;
                    items
                        .into_iter()
                        .map(|item| {
                            if worker_cancel.is_cancelled() {
                                return Err("Properties inspection cancelled.".into());
                            }
                            Ok(Item::describe(item))
                        })
                        .collect()
                },
                move |result| Message::Properties(Event::Ready(id, Arc::new(result))),
            ),
        ])
    }

    pub(super) fn close_properties(&mut self, id: window::Id) -> Task<Message> {
        if self.properties.remove(&id).is_none() {
            return Task::none();
        }
        Task::batch([window::close(id), self.exit_if_empty()])
    }

    pub(super) fn properties_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Opened(id) => iced::widget::operation::focus(control(id, 1)),
            Event::Ready(id, result) => {
                if let Some(properties) = self.properties.get_mut(&id) {
                    properties.result = Some(result);
                }
                Task::none()
            }
            Event::Copy(id) => self
                .properties
                .get(&id)
                .map_or_else(Task::none, |properties| {
                    iced::clipboard::write(
                        properties
                            .paths
                            .iter()
                            .map(|p| uri(p))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                }),
        }
    }

    pub(super) fn properties_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::key::Named;
        if matches!(&key, keyboard::Key::Named(Named::Escape))
            || modifiers.control()
                && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("w"))
        {
            return self.close_properties(id);
        }
        if modifiers.control()
            && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("c"))
        {
            return self.properties_event(Event::Copy(id));
        }
        match key {
            keyboard::Key::Named(Named::Home) => iced::widget::operation::scroll_to(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
            keyboard::Key::Named(Named::End) => iced::widget::operation::snap_to_end(scroll(id)),
            keyboard::Key::Named(Named::PageDown) => iced::widget::operation::scroll_by(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 300.0 },
            ),
            keyboard::Key::Named(Named::PageUp) => iced::widget::operation::scroll_by(
                scroll(id),
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: -300.0 },
            ),
            _ => Task::none(),
        }
    }
}

pub(super) fn view<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
) -> Element<'a, Message> {
    iced::widget::responsive(move |size| contents(desktop, id, properties, size.width >= 550.0))
        .into()
}

fn contents<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    properties: &'a Window,
    wide: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px();
    let label = |value: String| {
        text(value)
            .font(font)
            .size(size)
            .wrapping(text::Wrapping::WordOrGlyph)
    };
    let fact = |key: &'static str, value: String| {
        row![
            text(key)
                .font(font)
                .size(size + 1.0)
                .color(chrome::ink(appearance, canopa_theme::ColorRole::Muted))
                .width(104),
            text(value)
                .font(font)
                .size(size + 1.0)
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph)
        ]
        .spacing(10)
    };
    let single = properties.paths.len() == 1;
    let mut facts = column![].spacing(12);
    match properties.result.as_deref() {
        None => {
            facts = facts.push(chrome::card(
                appearance,
                label("Reading properties…".into()),
            ))
        }
        Some(Err(error)) => facts = facts.push(chrome::card(appearance, label(error.clone()))),
        Some(Ok(items)) => {
            for item in items {
                let path = &item.filesystem.path;
                let name = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                let location = super::question_dialog::display_location(
                    &desktop.home,
                    path.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(path),
                );
                let mut entry = column![].spacing(10);
                if !single {
                    entry = entry
                        .push(chrome::section_title(appearance, name.into_owned()))
                        .push(iced::widget::rule::horizontal(1).style(chrome::divider));
                }
                entry = entry
                    .push(chrome::section_title(appearance, "Details"))
                    .push(fact("Location", location));
                match &item.filesystem.metadata {
                    Err(error) => entry = entry.push(fact("Unavailable", error.clone())),
                    Ok(metadata) => {
                        let kind = match metadata.kind {
                            crate::browse::EntryKind::Directory => "Folder",
                            crate::browse::EntryKind::File => {
                                item.kind.as_deref().unwrap_or("File")
                            }
                            crate::browse::EntryKind::Symlink => "Symbolic link",
                            crate::browse::EntryKind::Other => "Special file",
                        };
                        entry = entry.push(fact("Kind", kind.into()));
                        if metadata.kind == crate::browse::EntryKind::File {
                            entry = entry.push(fact("Size", format!("{} bytes", metadata.bytes)));
                        }
                        if let Some(target) = &metadata.link_target {
                            entry = entry
                                .push(fact("Link target", target.to_string_lossy().into_owned()));
                        }
                        entry = entry.push(fact(
                            "Modified",
                            metadata
                                .modified
                                .map(modified)
                                .unwrap_or_else(|| "Unavailable".into()),
                        ));
                        let access_line = |who: &str, identity: Option<String>, bits: u32| {
                            let identity = identity.map_or_else(
                                || who.to_owned(),
                                |identity| format!("{who} · {identity}"),
                            );
                            row![
                                label(identity).width(Length::Fill),
                                label(permission_label(bits).into()).style(canopa_ui::muted),
                            ]
                            .spacing(8)
                        };
                        let access = column![
                            chrome::section_title(appearance, "Access"),
                            access_line("Owner", item.owner.clone(), metadata.permissions >> 6),
                            access_line("Group", item.group.clone(), metadata.permissions >> 3),
                            access_line("Others", None, metadata.permissions),
                            label(format!(
                                "Mode {:04o} · UID {} · GID {}",
                                metadata.permissions, metadata.owner, metadata.group
                            ))
                            .style(canopa_ui::muted),
                        ]
                        .spacing(9);
                        entry = if wide {
                            column![
                                row![
                                    iced::widget::container(entry).width(Length::FillPortion(3)),
                                    iced::widget::container(access).width(Length::FillPortion(2)),
                                ]
                                .spacing(18)
                            ]
                        } else {
                            entry
                                .push(iced::widget::rule::horizontal(1).style(chrome::divider))
                                .push(access)
                        };
                    }
                }
                facts = facts.push(chrome::card(appearance, entry));
            }
        }
    }
    let action = |index, title: &'static str, message: Message| {
        super::focus::button(
            control(id, index),
            Some(message.clone()),
            appearance,
            canopa_ui::styled_action(
                text(title).font(font).size(size),
                Some(message),
                canopa_ui::Action::Normal,
                false,
                appearance,
            ),
        )
    };
    let (title, subtitle, copy_label) = if let [path] = properties.paths.as_slice() {
        (
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            "Properties".into(),
            "Copy location",
        )
    } else {
        (
            "Properties".into(),
            format!("{} items", properties.paths.len()),
            "Copy locations",
        )
    };
    let icon = if single {
        match properties
            .result
            .as_deref()
            .and_then(|result| result.as_ref().ok())
            .and_then(|items| items.first())
            .and_then(|item| item.filesystem.metadata.as_ref().ok())
            .map(|metadata| metadata.kind)
        {
            Some(crate::browse::EntryKind::Directory) => chrome::Icon::Folder,
            _ => chrome::Icon::Documents,
        }
    } else {
        chrome::Icon::Info
    };
    let actions = row![
        action(0, copy_label, Message::Properties(Event::Copy(id))),
        iced::widget::space().width(Length::Fill),
        action(1, "Close", Message::Close(id))
    ]
    .spacing(8);
    let body = chrome::centered(
        column![
            chrome::dialog_heading(desktop, icon, title, subtitle),
            scrollable(facts)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new()
                        .width(6)
                        .scroller_width(6)
                        .spacing(8),
                ))
                .id(scroll(id))
                .width(Length::Fill)
                .height(Length::Fill),
            iced::widget::rule::horizontal(1).style(chrome::divider),
            actions,
        ]
        .spacing(12),
        720.0,
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| canopa_ui::frame(appearance, appearance.style(UiKind::Panel)));
    super::focus::group(
        body,
        vec![control(id, 0), control(id, 1)],
        Vec::new(),
        Some(control(id, 1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn property_dates_preserve_pre_epoch_fraction_and_refuse_overflow_without_panicking() {
        let before = std::time::UNIX_EPOCH - Duration::from_nanos(1);
        let parsed = timestamp(before).unwrap();
        assert_eq!(parsed.timestamp(), -1);
        assert_eq!(parsed.timestamp_subsec_nanos(), 999_999_999);
        let huge = std::time::UNIX_EPOCH
            .checked_add(Duration::from_secs(i64::MAX as u64))
            .unwrap();
        assert_eq!(modified(huge), "Outside supported date range");
    }
}
