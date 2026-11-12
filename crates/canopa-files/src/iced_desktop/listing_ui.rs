//! Shared entry presentation and geometry for lists and virtualized icon rows.
//! Geometry also drives keyboard movement, reveal and resize reconciliation.
use super::{chrome::Icon, *};
use crate::{
    browse::{Entry, EntryKind},
    preferences::{Detail, SortField, ViewMode, ViewSettings},
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
        icon: f32,
    ) -> Self {
        let padding = canopa_ui::content_padding(appearance.style(UiKind::MenuRow));
        if !searching && settings.mode == ViewMode::Icons {
            let minimum =
                (appearance.typography().body_size.px() * 7.5 + padding.left + padding.right)
                    .max(128.0)
                    .max(icon + 32.0 + padding.left + padding.right);
            Self {
                // Keep the stride stable if a height change makes the rail
                // appear. Embedded scrollbars reserve this same width.
                columns: (((width - ICON_SCROLLBAR_GUTTER).max(0.0) / minimum).floor() as usize)
                    .clamp(1, 16),
                height: icon
                    + 6.0
                    + (appearance.typography().body_size.px() * 1.3).ceil() * 2.0
                    + padding.top
                    + padding.bottom,
            }
        } else {
            let details = searching
                || settings.mode == ViewMode::List
                    && !settings.details.is_empty()
                    && !inline_details(appearance, settings, width);
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
        desktop.icon_size(),
    )
}

fn kind(entry: &Entry) -> String {
    if entry.kind == EntryKind::Symlink {
        fl!("files-kind-link")
    } else if entry.is_directory() {
        fl!("files-kind-folder")
    } else {
        entry.path.extension().map_or_else(
            || fl!("files-kind-file"),
            |extension| {
                fl!(
                    "files-kind-extension",
                    extension = extension.to_string_lossy().to_uppercase()
                )
            },
        )
    }
}
/// A size in binary units, in the person's number format (`1.5 KiB`).
pub(super) fn size(bytes: u64) -> String {
    canopa_i18n::format::size(bytes)
}
/// `time` in the local time zone, for locale formatting.
pub(super) fn local_time(time: std::time::SystemTime) -> Option<chrono::NaiveDateTime> {
    let milliseconds = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).ok()?,
        Err(error) => i64::try_from(error.duration().as_millis())
            .ok()?
            .checked_neg()?,
    };
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(milliseconds)
        .map(|time| time.with_timezone(&chrono::Local).naive_local())
}
/// A folder's Size: its item count once counted, otherwise nothing yet.
fn folder_items(items: Option<usize>) -> Option<String> {
    items.map(|count| {
        if count == 0 {
            fl!("files-folder-empty")
        } else if count >= super::folder_counts::LIMIT {
            fl!("files-folder-items-at-least", count = count)
        } else {
            fl!("files-status-items", count = count)
        }
    })
}

fn detail_value(entry: &Entry, detail: Detail, items: Option<usize>) -> Option<String> {
    match detail {
        Detail::Size if entry.is_directory() => folder_items(items),
        Detail::Size => Some(size(entry.size)),
        Detail::Kind => Some(kind(entry)),
        Detail::Modified => modified(entry),
    }
}
/// The local modification time as the Modified detail shows it.
/// A date and time with seconds, as dialogs and properties show it.
pub(super) fn date_time_seconds(time: chrono::NaiveDateTime) -> String {
    use canopa_i18n::format;
    fl!(
        "files-date-time",
        date = format::date(time.date()),
        time = format::time_with(time.time(), true, format::HourCycleChoice::Locale)
    )
}

pub(super) fn modified(entry: &Entry) -> Option<String> {
    let today = chrono::Local::now().date_naive();
    entry
        .modified
        .and_then(local_time)
        .map(|time| relative_date_time(time, today))
}

/// Recent times read as Today or Yesterday; older ones carry their date.
pub(super) fn relative_date_time(time: chrono::NaiveDateTime, today: chrono::NaiveDate) -> String {
    use canopa_i18n::format;
    use chrono::Datelike;
    let clock = || format::time(time.time());
    if time.date() == today {
        fl!("files-date-today", time = clock())
    } else if today.pred_opt() == Some(time.date()) {
        fl!("files-date-yesterday", time = clock())
    } else if time.date().year() == today.year() {
        fl!(
            "files-date-this-year",
            date = format::month_day_short(time.date()),
            time = clock()
        )
    } else {
        format::date_with(time.date(), format::DateStyle::Medium)
    }
}

pub(super) fn details(entry: &Entry, settings: &ViewSettings, items: Option<usize>) -> String {
    settings
        .details
        .iter()
        .filter_map(|&detail| detail_value(entry, detail, items))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Gap after each detail column; a header divider sits in its middle.
const DETAIL_GAP: f32 = 28.0;

fn detail_units(detail: Detail) -> f32 {
    match detail {
        Detail::Size => 6.5,
        Detail::Modified => 11.5,
        Detail::Kind => 9.0,
    }
}

/// A detail column's width: being dragged, chosen earlier, or from the text size.
pub(super) fn detail_width(desktop: &Desktop, id: window::Id, detail: Detail) -> f32 {
    let size = desktop
        .configuration
        .appearance
        .typography()
        .label_size
        .px()
        + 1.0;
    desktop
        .layouts
        .dragged(id, super::resize::Target::Detail(detail))
        .or_else(|| {
            desktop
                .configuration
                .settings
                .list_columns
                .detail(detail)
                .map(f32::from)
        })
        .unwrap_or(detail_units(detail) * size)
}

/// The Name column's width in a List `viewport` wide: it takes the room the
/// details leave, unless a width was chosen by dragging. A chosen width gives
/// way before the details would leave the window.
pub(super) fn name_width(
    desktop: &Desktop,
    id: window::Id,
    settings: &ViewSettings,
    viewport: f32,
) -> f32 {
    let details: f32 = settings
        .details
        .iter()
        .map(|&detail| detail_width(desktop, id, detail) + DETAIL_GAP + 1.0)
        .sum();
    let room = viewport - details - 72.0;
    desktop
        .layouts
        .dragged(id, super::resize::Target::Name)
        .or_else(|| {
            desktop
                .configuration
                .settings
                .list_columns
                .name
                .map(f32::from)
        })
        .map_or(room.max(260.0), |chosen| chosen.min(room).max(160.0))
}

pub(super) fn inline_details(
    appearance: &canopa_theme::Appearance,
    settings: &ViewSettings,
    width: f32,
) -> bool {
    let typography = appearance.typography();
    let minimum = typography.body_size.px() * 22.0 + typography.label_size.px() * 32.0;
    settings.mode == ViewMode::List && width >= minimum.max(620.0)
}

fn sort_heading(settings: &ViewSettings, field: SortField, label: String) -> String {
    if settings.sort == field {
        let arrow = if settings.descending { "↓" } else { "↑" };
        format!("{label}  {arrow}")
    } else {
        label
    }
}

pub(super) fn name_heading(settings: &ViewSettings) -> String {
    let (detail, label) = match settings.sort {
        SortField::Name => {
            return sort_heading(settings, SortField::Name, fl!("files-column-name"));
        }
        SortField::Size => (Detail::Size, fl!("files-column-size")),
        SortField::Modified => (Detail::Modified, fl!("files-column-modified")),
        SortField::Kind => (Detail::Kind, fl!("files-column-kind")),
    };
    if settings.details.contains(&detail) {
        fl!("files-column-name")
    } else {
        fl!(
            "files-column-name-sorted-by",
            field = sort_heading(settings, settings.sort, label)
        )
    }
}

/// Detail values for `entry`, or the headings when it is `None`. Headings
/// carry the dividers that resize the columns before them.
pub(super) fn detail_cells<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    settings: &ViewSettings,
    entry: Option<&Entry>,
    items: Option<usize>,
    name: f32,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let size = appearance.typography().label_size.px() + 1.0;
    let font = canopa_ui::font(appearance);
    let heading = entry.is_none();
    let mut divider = chrome::divider(&desktop.theme).color;
    divider.a = 0.9;
    let last = settings.details.last().copied();
    let grip = |target, current| -> Element<'a, Message> {
        super::resize::Grip {
            window: id,
            target,
            width: 1.0,
            current,
            height: Length::Fixed(28.0),
            line: Some(14.0),
            color: divider,
            share: 1.0,
            quiet: last.is_some_and(|last| target == super::resize::Target::Detail(last)),
        }
        .into()
    };
    // Generous gutters keep metadata columns distinct from each other and from Name.
    let mut cells = row![].align_y(Alignment::Center);
    // Rows keep a one-pixel space wherever the headings have a divider.
    cells = cells.push(if heading {
        grip(super::resize::Target::Name, name)
    } else {
        space().width(1).into()
    });
    for &detail in &settings.details {
        let (label, field) = match detail {
            Detail::Size => (fl!("files-column-size"), SortField::Size),
            Detail::Modified => (fl!("files-column-modified"), SortField::Modified),
            Detail::Kind => (fl!("files-column-kind"), SortField::Kind),
        };
        let width = detail_width(desktop, id, detail);
        let value = entry.map_or_else(
            || sort_heading(settings, field, label),
            |entry| detail_value(entry, detail, items).unwrap_or_else(|| "—".into()),
        );
        let active = heading && settings.sort == field;
        // Values read as secondary text; headings stay muted unless they sort.
        let foreground = if heading && !active {
            chrome::ink(appearance, ColorRole::Muted)
        } else if heading || appearance.high_contrast() {
            chrome::ink(appearance, ColorRole::Text)
        } else {
            chrome::ink(appearance, ColorRole::Text).scale_alpha(0.78)
        };
        let mut cell_font = font;
        if heading {
            cell_font.weight = iced::font::Weight::Semibold;
        }
        let cell: Element<'_, Message> = if detail == Detail::Size {
            text(value)
                .font(cell_font)
                .size(size)
                .color(foreground)
                .width(Length::Fill)
                .align_x(Alignment::End)
                .wrapping(text::Wrapping::None)
                .into()
        } else {
            chrome::single_line(value, cell_font, size, Some(foreground), entry.is_some())
        };
        cells = cells.push(container(cell).width(width).clip(true));
        cells = cells
            .push(space().width(DETAIL_GAP / 2.0))
            .push(if heading {
                grip(super::resize::Target::Detail(detail), width)
            } else {
                space().width(1).into()
            })
            .push(space().width(DETAIL_GAP / 2.0));
    }
    cells.into()
}

pub(super) struct Presentation<'a> {
    pub column: &'a Path,
    pub selected: bool,
    pub focused: bool,
    pub searching: bool,
    pub grid: bool,
    pub alternate: bool,
    /// One-based position among all rows of the virtualized list.
    pub position: (usize, usize),
}

/// High contrast keeps the bold selected name as a second, non-colour cue.
fn style_contrast(appearance: &canopa_theme::Appearance) -> bool {
    appearance.high_contrast()
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
        alternate,
        position,
    } = presentation;
    let appearance = &desktop.configuration.appearance;
    let browser = tab.browser.as_ref().unwrap();
    let settings = browser.view();
    let current = column_path == browser.path();
    let current_folder = !current && entry.path == browser.path();
    let size = appearance.typography().body_size.px();
    let label_size = appearance.typography().label_size.px();
    let font = canopa_ui::font(appearance);
    let inline = current && !searching && inline_details(appearance, settings, tab.viewport_width);
    let items = entry
        .is_directory()
        .then(|| desktop.folder_counts.items(entry, browser.show_hidden()))
        .flatten();
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
        column![desktop.icons.file(entry, desktop.icon_size()), name]
            .spacing(6)
            .align_x(Alignment::Center)
            .into()
    } else {
        // Selection is carried by the row's fill; the name keeps its weight
        // so it never shifts as the selection moves.
        let mut name_font = font;
        if selected && style_contrast(appearance) {
            name_font.weight = iced::font::Weight::Semibold;
        }
        let name = chrome::single_line(
            entry.name().into_owned(),
            name_font,
            size,
            None,
            tab.location.is_none()
                && desktop.action_menu.is_none()
                && desktop.folder_options.is_none(),
        );
        let file_icon = desktop.icons.file(entry, 22.0);
        let arrow = || {
            desktop
                .icons
                .chrome(Icon::Forward, 16.0, if selected { 1.0 } else { 0.83 })
        };
        // Inline details follow a bounded Name column, so a row's facts sit
        // beside its name instead of across the window.
        let name_width = name_width(desktop, id, settings, tab.viewport_width);
        let name: Element<'a, Message> = if inline {
            container(name).width(name_width).into()
        } else {
            name
        };
        let mut name_line = row![file_icon, name].spacing(8).align_y(Alignment::Center);
        // Chevrons promise a column to the right; List view opens folders in place.
        let drills = entry.is_directory() && (!current || settings.mode == ViewMode::Columns);
        if inline {
            let trailing = if drills {
                arrow()
            } else {
                space().width(16).into()
            };
            name_line = name_line
                .push(detail_cells(
                    desktop,
                    id,
                    settings,
                    Some(entry),
                    items,
                    name_width,
                ))
                .push(trailing)
                .push(space().width(Length::Fill));
        } else if drills {
            name_line = name_line.push(arrow());
        }
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
                .color(chrome::ink(appearance, ColorRole::Muted))
                .wrapping(text::Wrapping::None),
            );
        } else if current
            && !inline
            && settings.mode == ViewMode::List
            && !settings.details.is_empty()
        {
            line = line.push(
                text(details(entry, settings, items))
                    .size(label_size)
                    .font(font)
                    .wrapping(text::Wrapping::None),
            );
        }
        line.into()
    };
    let geometry = Geometry::new(
        appearance,
        settings,
        searching,
        tab.viewport_width,
        desktop.icon_size(),
    );
    let height = if current {
        geometry.height
    } else {
        super::view::row_height(appearance)
    };
    let style_appearance = appearance.clone();
    // The keyboard cursor's ring belongs to the active window only; the
    // accessible focus state below does not depend on it.
    let ring = focused && desktop.focused_window == Some(id);
    let card = container(content)
        .padding(super::view::entry_padding(appearance, grid))
        .height(height)
        .width(Length::Fill)
        .clip(true)
        .style(move |_| {
            let base = style_appearance.style(UiKind::MenuRow);
            let resolved = base.state(selected, false, false, false, ring);
            let mut style = canopa_ui::frame(&style_appearance, resolved);
            // High contrast keeps selection and its edge opaque in every column.
            let contrast = style_appearance.high_contrast();
            if selected && base.states[0].background.is_none() {
                // Ancestors share one neutral fill; the accent stays with the
                // current folder's selection and keyboard focus.
                let color = if current || contrast {
                    chrome::ink(&style_appearance, ColorRole::AccentSurface)
                } else {
                    chrome::ink(&style_appearance, ColorRole::Raised)
                };
                // The current folder's selection is the accent surface without an
                // outline, so it never reads as the keyboard focus ring.
                style.background = Some(color.into());
                if !base.custom_border && base.states[0].border_color.is_none() {
                    let mut edge = chrome::ink(&style_appearance, ColorRole::Accent);
                    edge.a = if contrast {
                        1.0
                    } else if current {
                        0.0
                    } else if current_folder {
                        0.42
                    } else {
                        0.0
                    };
                    style.border.width = if contrast { 1.0 } else { 0.0 };
                    style.border.color = edge;
                }
            }
            if inline
                && alternate
                && !contrast
                && !selected
                && !ring
                && canopa_ui::resolve_color(&style_appearance, base.background).a == 0.0
            {
                style.background = Some(
                    chrome::ink(&style_appearance, ColorRole::Raised)
                        .scale_alpha(0.22)
                        .into(),
                );
            }
            if !base.custom_radius {
                style.border.radius = 7.0.into();
            }
            if ring && base.states[4].border_color.is_none() {
                style.border.width = style.border.width.max(1.0);
                // A quieter ring than controls': rows are focused continuously.
                style.border.color = canopa_ui::color(style_appearance.role(ColorRole::Focus))
                    .scale_alpha(if contrast { 1.0 } else { 0.6 });
            }
            style
        });
    let activate = if entry.is_directory() {
        Message::Go(id, entry.path.clone())
    } else {
        Message::Activate(id, entry.path.clone())
    };
    let node = super::accessibility_ui::row(super::accessibility_ui::Row {
        entry,
        settings,
        selected,
        focused: focused && super::view::rows_have_focus(desktop, id, tab),
        position,
        select: Message::Select(id, entry.path.clone()),
    });
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
    let card = if grid
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
    };
    super::accessibility_ui::semantic(node, card)
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
            desktop.icon_size(),
        );
        let rows = visible.len().div_ceil(geometry.columns);
        let total = visible.len();
        let range = super::view::visible_rows(rows, browser.scroll(), size.height, geometry.height);
        let mut items = column![keyed_row(
            RowKey::Top,
            space().height(range.start as f32 * geometry.height)
        )];
        if visible.is_empty() {
            items = items.push(keyed_row(
                RowKey::Notice,
                container(text(fl!("files-empty-folder"))).padding(12),
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
                                    alternate: false,
                                    position: (index * geometry.columns + slot + 1, total),
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
    use chrono::Datelike;

    #[test]
    fn modified_times_read_relative_recently_and_drop_the_current_year() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let at = |date: chrono::NaiveDate| date.and_hms_opt(9, 30, 0).unwrap();
        assert!(relative_date_time(at(today), today).starts_with("Today, "));
        let yesterday = today.pred_opt().unwrap();
        assert!(relative_date_time(at(yesterday), today).starts_with("Yesterday, "));
        let this_year = relative_date_time(at(today.with_day(1).unwrap()), today);
        assert!(!this_year.contains("2026"), "{this_year}");
        let older = relative_date_time(
            at(chrono::NaiveDate::from_ymd_opt(2025, 3, 4).unwrap()),
            today,
        );
        assert!(older.contains("2025"), "{older}");
    }
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
        let geometry = Geometry::new(&Default::default(), &settings, false, 360.0, 48.0);
        assert_eq!(geometry.columns, 2);
        assert!(geometry.reveal(&mut browser, 400.0));
        assert!(geometry.visible(99, browser.scroll(), 400.0));
        assert!(!geometry.reveal(&mut browser, 400.0));
        browser.set_scroll(0.0);
        assert!(!geometry.visible(99, browser.scroll(), 400.0));
        assert_eq!(
            Geometry::new(&Default::default(), &settings, true, 360.0, 48.0).columns,
            1
        );
    }
}
