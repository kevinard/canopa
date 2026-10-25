//! Shared entry presentation and geometry for lists and virtualized icon rows.
//! Geometry also drives keyboard movement, reveal and resize reconciliation.
use super::*;
use crate::{
    browse::{Entry, EntryKind},
    preferences::{Detail, ViewMode, ViewSettings},
};
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Alignment, Length,
    widget::{
        column, container, mouse_area, responsive, row, scrollable, sensor, space, text, tooltip,
    },
};

const ICON_SCROLLBAR_WIDTH: f32 = 10.0;
const ICON_SCROLLBAR_SPACING: f32 = 4.0;
const ICON_SCROLLBAR_GUTTER: f32 = ICON_SCROLLBAR_WIDTH + ICON_SCROLLBAR_SPACING;

#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub columns: usize,
    pub height: f32,
}
impl Geometry {
    pub fn new(
        appearance: &canopa_theme::Appearance,
        settings: &ViewSettings,
        searching: bool,
        width: f32,
    ) -> Self {
        let padding = canopa_ui::content_padding(appearance.style(UiKind::MenuRow));
        if !searching && settings.mode == ViewMode::Icons {
            let minimum =
                (appearance.typography().body_size.px() * 7.5 + padding.left + padding.right)
                    .max(128.0);
            Self {
                // Keep the stride stable if a height change makes the rail
                // appear. Embedded scrollbars reserve this same width.
                columns: (((width - ICON_SCROLLBAR_GUTTER).max(0.0) / minimum).floor() as usize)
                    .clamp(1, 16),
                height: 48.0
                    + 6.0
                    + (appearance.typography().body_size.px() * 1.3).ceil() * 2.0
                    + padding.top
                    + padding.bottom,
            }
        } else {
            let details =
                searching || settings.mode == ViewMode::List && !settings.details.is_empty();
            Self {
                columns: 1,
                height: super::view::entry_height(appearance, details),
            }
        }
    }
    pub fn top(self, index: usize) -> f32 {
        (index / self.columns) as f32 * self.height
    }
    pub fn visible(self, index: usize, offset: f32, viewport: f32) -> bool {
        self.top(index) >= offset && self.top(index) + self.height <= offset + viewport + 1.0
    }
    pub fn reveal(self, browser: &mut Browser, viewport: f32) -> bool {
        if viewport <= 0.0 {
            return false;
        }
        let entries = browser.visible_entries();
        let count = entries.len();
        let index = entries
            .iter()
            .position(|entry| Some(entry.path.as_path()) == browser.focused());
        let maximum = (count.div_ceil(self.columns) as f32 * self.height - viewport).max(0.0);
        let old = browser.scroll();
        let mut offset = old.min(maximum);
        if let Some(index) = index {
            let top = self.top(index);
            if top < offset {
                offset = top;
            } else if top + self.height > offset + viewport {
                offset = top + self.height - viewport;
            }
        }
        offset = offset.clamp(0.0, maximum);
        browser.set_scroll(offset);
        old != offset
    }
}

pub(super) fn geometry(desktop: &Desktop, tab: &Tab) -> Geometry {
    Geometry::new(
        &desktop.configuration.appearance,
        tab.browser
            .as_ref()
            .map_or(desktop.preferences.defaults(), Browser::view),
        tab.browser
            .as_ref()
            .is_some_and(|browser| browser.search().is_some()),
        tab.viewport_width,
    )
}

fn kind(entry: &Entry) -> String {
    if entry.kind == EntryKind::Symlink {
        "Link".into()
    } else if entry.is_directory() {
        "Folder".into()
    } else {
        entry.path.extension().map_or_else(
            || "File".into(),
            |extension| format!("{} file", extension.to_string_lossy().to_uppercase()),
        )
    }
}
fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let (amount, unit) = if bytes < 1024 * 1024 {
        (bytes as f64 / 1024.0, "KiB")
    } else if bytes < 1024 * 1024 * 1024 {
        (bytes as f64 / (1024.0 * 1024.0), "MiB")
    } else {
        (bytes as f64 / (1024.0 * 1024.0 * 1024.0), "GiB")
    };
    format!("{amount:.1} {unit}")
}
pub(super) fn details(entry: &Entry, settings: &ViewSettings) -> String {
    settings
        .details
        .iter()
        .filter_map(|detail| match detail {
            Detail::Size => (!entry.is_directory()).then(|| size(entry.size)),
            Detail::Kind => Some(kind(entry)),
            Detail::Modified => entry.modified.and_then(|time| {
                let milliseconds = match time.duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => i64::try_from(duration.as_millis()).ok()?,
                    Err(error) => i64::try_from(error.duration().as_millis())
                        .ok()?
                        .checked_neg()?,
                };
                chrono::DateTime::<chrono::Utc>::from_timestamp_millis(milliseconds).map(|time| {
                    time.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
            }),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) struct Presentation<'a> {
    pub column: &'a Path,
    pub selected: bool,
    pub focused: bool,
    pub searching: bool,
    pub grid: bool,
}

pub(super) fn entry<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    entry: &'a Entry,
    presentation: Presentation<'_>,
) -> Element<'a, Message> {
    let Presentation {
        column: column_path,
        selected,
        focused,
        searching,
        grid,
    } = presentation;
    let appearance = &desktop.configuration.appearance;
    let browser = tab.browser.as_ref().unwrap();
    let settings = browser.view();
    let current = column_path == browser.path();
    let size = appearance.typography().body_size.px();
    let label_size = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let content: Element<'a, Message> = if grid {
        let name = container(
            text(entry.name().into_owned())
                .size(size)
                .font(font)
                .align_x(iced::alignment::Horizontal::Center)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height((size * 1.3).ceil() * 2.0)
        .clip(true);
        column![desktop.icons.file(entry, 48.0), name]
            .spacing(6)
            .align_x(Alignment::Center)
            .into()
    } else {
        let name = container(
            text(entry.name().into_owned())
                .size(size)
                .font(font)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fill)
        .clip(true);
        let mut name_line = row![].spacing(6).align_y(Alignment::Center);
        if current && !searching && settings.mode == ViewMode::List {
            name_line = name_line.push(desktop.icons.file(entry, 18.0));
        }
        name_line = name_line.push(name).push(
            text(if entry.is_directory() { "›" } else { "" })
                .size(size)
                .font(font),
        );
        let mut line = column![name_line].spacing(2);
        if searching {
            line = line.push(
                text(
                    entry
                        .path
                        .parent()
                        .map(|parent| super::view::relative_location(parent, browser.path()))
                        .unwrap_or_default(),
                )
                .size(label_size)
                .font(font)
                .wrapping(text::Wrapping::None),
            );
        } else if current && settings.mode == ViewMode::List && !settings.details.is_empty() {
            line = line.push(
                text(details(entry, settings))
                    .size(label_size)
                    .font(font)
                    .wrapping(text::Wrapping::None),
            );
        }
        line.into()
    };
    let geometry = Geometry::new(appearance, settings, searching, tab.viewport_width);
    let height = if current {
        geometry.height
    } else {
        super::view::row_height(appearance)
    };
    let style_appearance = appearance.clone();
    let card = container(content)
        .padding(canopa_ui::content_padding(
            appearance.style(UiKind::MenuRow),
        ))
        .height(height)
        .width(Length::Fill)
        .clip(true)
        .style(move |_| {
            let base = style_appearance.style(UiKind::MenuRow);
            let resolved = base.state(selected, false, false, false, focused);
            let mut style = canopa_ui::frame(&style_appearance, resolved);
            if selected && base.states[0].background.is_none() {
                let mut color = canopa_ui::color(style_appearance.palette().accent);
                color.a = 0.20;
                style.background = Some(color.into());
            }
            if focused && base.states[4].border_color.is_none() {
                style.border.width = style.border.width.max(1.0);
                style.border.color = canopa_ui::color(style_appearance.role(ColorRole::Focus));
            }
            style
        });
    let activate = if entry.is_directory() {
        Message::Go(id, entry.path.clone())
    } else {
        Message::Activate(id, entry.path.clone())
    };
    let card = mouse_area(card)
        .on_press(Message::Select(id, entry.path.clone()))
        .on_right_press(Message::Action(actions_ui::Event::Row(
            actions_ui::RowTarget {
                window: id,
                tab: tab.id,
                navigation: tab.revision,
                epoch: tab.list_epoch,
                column: column_path.to_owned(),
                path: entry.path.clone(),
            },
        )))
        .on_double_click(activate);
    let card: Element<'a, Message> = card.into();
    let card = if let Some((scene, region)) =
        super::drag_ui::row_region(desktop, id, tab, entry, column_path)
    {
        super::drag_geometry::mark(scene, region, card)
    } else {
        card
    };
    if grid
        && tab.location.is_none()
        && desktop.action_menu.is_none()
        && desktop.folder_options.is_none()
    {
        tooltip(
            card,
            text(entry.name().into_owned()).size(label_size).font(font),
            tooltip::Position::Bottom,
        )
        .into()
    } else {
        card
    }
}

pub(super) fn icons<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
) -> Element<'a, Message> {
    use super::{row_state::keyed_row, view::RowKey};
    let browser = tab.browser.as_ref().unwrap();
    responsive(move |size| {
        let visible = browser.visible_entries();
        let geometry = Geometry::new(
            &desktop.configuration.appearance,
            browser.view(),
            false,
            size.width,
        );
        let rows = visible.len().div_ceil(geometry.columns);
        let range = super::view::visible_rows(rows, browser.scroll(), size.height, geometry.height);
        let mut items = column![keyed_row(
            RowKey::Top,
            space().height(range.start as f32 * geometry.height)
        )];
        if visible.is_empty() {
            items = items.push(keyed_row(
                RowKey::Notice,
                container(text("Empty folder")).padding(12),
            ));
        }
        for index in range.clone() {
            let mut line = row![].height(geometry.height);
            for slot in 0..geometry.columns {
                if let Some(item) = visible.get(index * geometry.columns + slot).copied() {
                    line = line.push(
                        container(keyed_row(
                            RowKey::Entry(tab.id, item.path.clone()),
                            entry(
                                desktop,
                                id,
                                tab,
                                item,
                                Presentation {
                                    column: browser.path(),
                                    selected: browser.selected().contains(&item.path),
                                    focused: browser.focused() == Some(item.path.as_path()),
                                    searching: false,
                                    grid: true,
                                },
                            ),
                        ))
                        .width(Length::FillPortion(1)),
                    );
                } else {
                    line = line.push(space().width(Length::FillPortion(1)));
                }
            }
            items = items.push(keyed_row(RowKey::Grid(tab.id, index), line));
        }
        items = items.push(keyed_row(
            RowKey::Bottom,
            space().height((rows - range.end) as f32 * geometry.height),
        ));
        let path = browser.path().to_owned();
        let sensed = path.clone();
        let resized = path.clone();
        let tab_id = tab.id;
        let epoch = tab.list_epoch;
        let rendered = Size::new(tab.viewport_width, tab.viewport_height);
        let list = scrollable(items)
            .id(super::view::scroll_id(tab_id, &path, epoch))
            .height(Length::Fill)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(ICON_SCROLLBAR_WIDTH)
                    .scroller_width(ICON_SCROLLBAR_WIDTH)
                    .spacing(ICON_SCROLLBAR_SPACING),
            ))
            .on_scroll(move |viewport| {
                Message::Scroll(
                    id,
                    tab_id,
                    epoch,
                    path.clone(),
                    ScrollUpdate {
                        offset: viewport.absolute_offset().y,
                        rendered,
                        measured: viewport.bounds().size(),
                    },
                )
            });
        sensor(list)
            .key((tab_id, epoch, sensed.clone()))
            .on_show(move |size| Message::Viewport(id, tab_id, epoch, sensed.clone(), size))
            .on_resize(move |size| Message::Viewport(id, tab_id, epoch, resized.clone(), size))
            .into()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_geometry_reveals_the_actual_file_and_preserves_deliberate_scroll() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..100 {
            std::fs::write(root.path().join(format!("{index:03}.txt")), b"x").unwrap();
        }
        let mut browser = Browser::open(root.path()).unwrap();
        let settings = ViewSettings {
            mode: ViewMode::Icons,
            ..Default::default()
        };
        browser.set_view(settings.clone()).unwrap();
        browser.select(&root.path().join("099.txt"), SelectionMode::Replace);
        let geometry = Geometry::new(&Default::default(), &settings, false, 360.0);
        assert_eq!(geometry.columns, 2);
        assert!(geometry.reveal(&mut browser, 400.0));
        assert!(geometry.visible(99, browser.scroll(), 400.0));
        assert!(!geometry.reveal(&mut browser, 400.0));
        browser.set_scroll(0.0);
        assert!(!geometry.visible(99, browser.scroll(), 400.0));
        assert_eq!(
            Geometry::new(&Default::default(), &settings, true, 360.0).columns,
            1
        );
    }
}
