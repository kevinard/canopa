use super::accessibility_ui::{self as a11y, Node, Role, semantic};
use super::row_state::keyed_row;
use super::{chrome::Icon, *};
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Alignment, Length,
    font::Weight,
    widget::{
        self, checkbox, column, container, row, rule, scrollable, sensor, space, text, text_input,
    },
};

use crate::browser_ui::entry::entry_padding;
pub(super) use crate::browser_ui::entry::row_height;

pub(super) fn entry_height(appearance: &canopa_theme::Appearance, searching: bool) -> f32 {
    row_height(appearance)
        + if searching {
            (appearance.typography().label_size.px() * 1.3).ceil() + 2.0
        } else {
            0.0
        }
}

pub(super) fn root_id(window: window::Id, tab: u64) -> widget::Id {
    widget::Id::from(format!("files-root-{window:?}-{tab}"))
}
pub(super) fn search_id(window: window::Id, tab: u64) -> widget::Id {
    widget::Id::from(format!("files-search-{window:?}-{tab}"))
}

#[derive(Clone, PartialEq)]
pub(super) enum RowKey {
    Top,
    Bottom,
    Notice,
    Entry(u64, PathBuf),
    Trash(PathBuf),
    Recovery(crate::operations::RecoveryIdentity),
    Job(u64),
    FilesystemJob(u64),
    Device(crate::local_volumes::DeviceId),
    DeviceJob(crate::local_volumes::DeviceRequestId),
    Grid(u64, usize),
}

pub(super) fn location_id(window: window::Id) -> widget::Id {
    widget::Id::from(format!("files-location-{window:?}"))
}
pub(super) fn scroll_id(tab: u64, path: &Path, epoch: u64) -> widget::Id {
    // The full byte path is a widget identity, never an operation target.
    widget::Id::from(format!(
        "files-column-{tab}-{epoch}-{:?}",
        path.as_os_str().as_bytes()
    ))
}

pub(super) fn browser(desktop: &Desktop, id: window::Id) -> Element<'_, Message> {
    let Some(window) = desktop.windows.get(&id) else {
        return text("").into();
    };
    let Some(tab) = window.tabs.get(window.active) else {
        return text("").into();
    };
    let appearance = &desktop.configuration.appearance;
    let body_size = appearance.typography().body_size.px();
    let label_size = appearance.typography().label_size.px();
    let sidebar = sidebar_visible(desktop.configuration.settings.sidebar, window.width);
    let font = canopa_ui::font(appearance);
    let mut title_font = font;
    title_font.weight = Weight::Semibold;
    // The toolbar keeps its tree index, where Iced stores the active location
    // editor's focus and caret.
    let brand: Element<'_, Message> = space().width(0).into();
    let sidebar_width = desktop.sidebar_width(id);
    let properties_width = desktop.properties_width(id);
    let paths = tab.column_paths(
        window.width,
        (desktop.configuration.settings.sidebar, sidebar_width),
        window.panel_width(properties_width),
        &desktop.home,
    );
    let content_width = window.width
        - window.panel_width(properties_width)
        - if sidebar { sidebar_width + 1.0 } else { 0.0 };
    // While a divider is dragged, the current column keeps a usable width.
    let minimum = tab.browser.as_ref().map_or(0.0, |browser| {
        let fitting = if paths.len() > 1 {
            ((content_width - 220.0 - 68.0) / (paths.len() - 1) as f32).max(160.0)
        } else {
            f32::INFINITY
        };
        desktop
            .column_width(id, browser.view().column_width)
            .min(fitting.max(f32::from(browser.view().column_width)))
    });
    let divider_color = chrome::divider(&desktop.theme).color;
    let spare = (content_width - paths.len() as f32 * minimum).max(0.0);
    // Share surplus with the immediate parent while keeping the larger share
    // available for the current folder.
    let parent_extra = if paths.len() > 1 {
        (spare * 0.46).min(68.0)
    } else {
        0.0
    };
    // In Columns view the current folder, a column like its parents, takes
    // that surplus itself: its names are the ones being read.
    let columned_view = tab.browser.as_ref().is_some_and(|browser| {
        browser.search().is_none() && browser.view().mode == crate::preferences::ViewMode::Columns
    });
    let current_extra = if columned_view {
        (spare * 0.46).min(68.0)
    } else {
        0.0
    };
    let toolbar = iced::widget::Row::from_vec(vec![
        brand,
        semantic(
            Node::toolbar(fl!("files-toolbar")).key("files-toolbar"),
            toolbar::view(desktop, id, tab),
        ),
    ])
    .width(Length::Fill)
    .align_y(Alignment::Center);
    let mut body = column![
        container(toolbar).style(move |_| chrome::bar(appearance, true)),
        rule::horizontal(1).style(chrome::divider)
    ];
    let notice: Element<'_, Message> = if let Some(error) = &desktop.configuration_error {
        let notice = fl!("files-configuration-not-applied", error = error.to_string());
        semantic(
            Node::alert(notice.clone()).key("files-configuration-error"),
            container(text(notice).size(label_size)).padding([6, 12]),
        )
    } else {
        space().height(0).into()
    };
    body = body.push(container(notice));
    let persistence_notice: Element<'_, Message> = if let Some(error) = &desktop.folder_writer.error
    {
        let notice = fl!(
            "files-view-preferences-not-saved",
            error = error.to_string()
        );
        let retry = Some(Message::FolderView(folder_views::Event::Retry));
        container(row![
            semantic(
                Node::alert(notice.clone()).key("files-view-preferences-error"),
                text(notice).size(label_size).width(Length::Fill),
            ),
            a11y::button(
                "files-view-preferences-retry",
                retry.clone(),
                canopa_ui::styled_action(
                    text(fl!("files-retry")).size(label_size),
                    retry,
                    canopa_ui::Action::Quiet,
                    false,
                    appearance
                )
            )
        ])
        .padding([4, 10])
        .into()
    } else {
        space().height(0).into()
    };
    body = body.push(container(persistence_notice));
    if let Some(session) = &tab.search {
        // The field is in the toolbar and the options beside the results;
        // the filters get their own band, which narrow tiles can wrap.
        body = body.push(search_filters(desktop, id, tab, session));
        if let Some(problems) = search_parts(desktop, id, tab, session).problems {
            body = body.push(problems);
        }
    }
    // The sidebar runs from the toolbar to the status bar. Tabs head only the
    // folders they switch, in the toolbar's tone, the current one opening
    // into the folder below it.
    let mut outer = row![].height(Length::Fill);
    let mut content = row![].height(Length::Fill);
    if sidebar {
        // The divider at the sidebar's end resizes it.
        outer = outer
            .push(semantic(
                Node::navigation(fl!("files-sidebar")).key("files-sidebar"),
                places(desktop, id),
            ))
            .push(super::resize::Grip {
                window: id,
                target: super::resize::Target::Sidebar,
                width: 1.0,
                current: sidebar_width,
                height: Length::Fill,
                line: None,
                color: divider_color,
                share: 1.0,
                quiet: false,
                enabled: desktop.dividers_take_pointer(id),
            });
    }
    if let Some(browser) = &tab.browser {
        for (index, path) in paths.iter().enumerate() {
            if index != 0 {
                // Dragging any divider resizes every column left of it.
                content = content.push(super::resize::Grip {
                    window: id,
                    target: super::resize::Target::Columns,
                    width: 1.0,
                    current: f32::from(browser.view().column_width),
                    height: Length::Fill,
                    line: None,
                    color: divider_color,
                    share: 1.0 / index as f32,
                    quiet: false,
                    enabled: desktop.dividers_take_pointer(id),
                });
            }
            let current = path == browser.path();
            let searching = current && browser.search().is_some();
            let grid =
                current && !searching && browser.view().mode == crate::preferences::ViewMode::Icons;
            let directory = if current {
                Some(browser.directory())
            } else {
                tab.columns
                    .get(path)
                    .and_then(|column| column.result.as_ref())
                    .and_then(|result| result.as_ref().ok())
            };
            let child = paths.get(index + 1).map(PathBuf::as_path);
            let mut entries = column![].spacing(0);
            if let Some(directory) = directory.filter(|_| !grid) {
                let visible = if current {
                    browser.visible_entries()
                } else {
                    directory
                        .visible_entries(browser.show_hidden(), child)
                        .into_iter()
                        .filter(|entry| browser.includes(entry))
                        .collect()
                };
                if visible.is_empty() {
                    entries = entries.push(keyed_row(
                        RowKey::Notice,
                        container(
                            text(if searching {
                                if tab.search.as_ref().is_some_and(|search| search.running) {
                                    fl!("files-searching-filenames")
                                } else {
                                    fl!("files-no-matching-filenames")
                                }
                            } else {
                                fl!("files-empty-folder")
                            })
                            .size(body_size)
                            .font(font)
                            .color(chrome::ink(appearance, ColorRole::Muted)),
                        )
                        .padding([16, 18]),
                    ));
                }
                let offset = if current {
                    browser.scroll()
                } else {
                    tab.columns.get(path).map_or(0.0, |column| column.scroll)
                };
                let height = if current {
                    super::listing_ui::geometry(desktop, tab).height
                } else {
                    row_height(appearance)
                };
                let range = visible_rows(visible.len(), offset, tab.viewport_height, height);
                let total = visible.len();
                entries = entries.push(keyed_row(
                    RowKey::Top,
                    space().height(range.start as f32 * height),
                ));
                for (index, entry) in visible[range.clone()].iter().copied().enumerate() {
                    let selected = if current {
                        browser.selected().contains(&entry.path)
                    } else {
                        child == Some(entry.path.as_path())
                    };
                    let focused = current && browser.focused() == Some(entry.path.as_path());
                    entries = entries.push(keyed_row(
                        RowKey::Entry(tab.id, entry.path.clone()),
                        super::listing_ui::entry(
                            desktop,
                            id,
                            tab,
                            entry,
                            super::listing_ui::Presentation {
                                column: path,
                                selected,
                                focused,
                                searching,
                                grid: false,
                                alternate: !(range.start + index).is_multiple_of(2),
                                position: (range.start + index + 1, total),
                            },
                        ),
                    ));
                }
                entries = entries.push(keyed_row(
                    RowKey::Bottom,
                    space().height((visible.len() - range.end) as f32 * height),
                ));
            } else {
                let status = match tab
                    .columns
                    .get(path)
                    .and_then(|column| column.result.as_ref())
                {
                    Some(Err(error)) => error.clone(),
                    _ => fl!("files-loading-folder"),
                };
                entries = entries.push(keyed_row(
                    RowKey::Notice,
                    container(text(status).size(body_size)).padding(12),
                ));
            }
            let heading = if searching {
                fl!("files-search-results")
            } else if path == Path::new("/") {
                fl!("files-place-filesystem")
            } else if path == &desktop.home {
                fl!("files-place-home")
            } else {
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            };
            let path_for_drop = path.clone();
            let path = path.clone();
            let sensed_path = path.clone();
            let resized_path = path.clone();
            let tab_id = tab.id;
            let epoch = tab.list_epoch;
            let rendered = Size::new(tab.viewport_width, tab.viewport_height);
            let list = scrollable(entries)
                .style(canopa_ui::scrollbar)
                .id(scroll_id(tab.id, &path, epoch))
                .height(Length::Fill)
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
            let list = sensor(list)
                .key((tab_id, epoch, sensed_path.clone()))
                .on_show(move |size| {
                    Message::Viewport(id, tab_id, epoch, sensed_path.clone(), size)
                })
                .on_resize(move |size| {
                    Message::Viewport(id, tab_id, epoch, resized_path.clone(), size)
                });
            let list: Element<'_, Message> = if grid {
                super::listing_ui::icons(desktop, id, tab)
            } else {
                list.into()
            };
            let list = semantic(
                Node::list(heading.clone())
                    .key(a11y::list_key(&path_for_drop))
                    .description(if !current {
                        fl!("files-command-parent")
                    } else if searching {
                        String::new()
                    } else {
                        match browser.view().mode {
                            crate::preferences::ViewMode::Columns => fl!("files-column-view"),
                            crate::preferences::ViewMode::List => fl!("files-list-view"),
                            crate::preferences::ViewMode::Icons => fl!("files-icon-view"),
                        }
                    })
                    .multiselectable(current)
                    .busy(
                        current
                            && (tab.loading
                                || searching
                                    && tab.search.as_ref().is_some_and(|search| search.running)),
                    )
                    .active_descendant(
                        current
                            .then(|| browser.focused().map(a11y::entry_key))
                            .flatten(),
                    ),
                list,
            );
            let symbol = desktop.icons.chrome(
                if searching {
                    Icon::Search
                } else if path_for_drop == Path::new("/") {
                    Icon::Drive
                } else if path_for_drop == desktop.home {
                    Icon::Home
                } else {
                    Icon::Folder
                },
                18.0,
                if current { 1.0 } else { 0.82 },
            );
            // The accent underline marks the current column; its symbol
            // only stays at full strength.
            let symbol: Element<'_, Message> = container(symbol).padding(5).into();
            let inline = current
                && !searching
                && super::listing_ui::inline_details(
                    appearance,
                    browser.view(),
                    tab.viewport_width,
                );
            let name_width =
                super::listing_ui::name_width(desktop, id, browser.view(), tab.viewport_width);
            let heading = if inline {
                let trailing = entry_padding(appearance, false).right + 14.0;
                // Tree rows start after their disclosure chevron.
                let lead = if browser.tree_view() {
                    super::listing_ui::TREE_LEAD
                } else {
                    0.0
                };
                let heading_width = name_width - lead;
                row![
                    space().width(32.0 + lead),
                    container(super::listing_ui::sort_heading_button(
                        desktop,
                        id,
                        crate::preferences::SortField::Name,
                        fl!("files-column-name"),
                        chrome::single_line(
                            super::listing_ui::name_heading(browser.view()),
                            title_font,
                            label_size + 1.0,
                            Some(chrome::ink(
                                appearance,
                                if browser.view().sort == crate::preferences::SortField::Name {
                                    ColorRole::Text
                                } else {
                                    ColorRole::Muted
                                },
                            )),
                            true,
                        ),
                    ))
                    .width(heading_width),
                    super::listing_ui::detail_cells(
                        desktop,
                        id,
                        browser.view(),
                        None,
                        None,
                        name_width
                    ),
                    space().width(trailing),
                    space().width(Length::Fill),
                ]
            } else if searching {
                let mut title = row![
                    symbol,
                    chrome::single_line(
                        heading,
                        title_font,
                        body_size,
                        Some(chrome::ink(appearance, ColorRole::Text)),
                        tab.location.is_none() && desktop.action_menu.is_none(),
                    ),
                ];
                // A search's options sit beside its results.
                if let Some(session) = &tab.search {
                    title = title.push(search_parts(desktop, id, tab, session).controls);
                }
                title
            } else {
                // Without details or a search there is no heading to show.
                let _ = (symbol, heading);
                row![]
            }
            .spacing(7)
            .align_y(Alignment::Center);
            // Headings sit close to the rows' height, alike in every view.
            let heading = container(heading)
                .height(heading_height(appearance))
                .align_y(Alignment::Center)
                .padding([0, 11])
                // Headings stand on the folder's own surface, parted from
                // the rows by a hairline, so the window keeps few tones.
                .width(Length::Fill);
            let list = container(list)
                .padding(if grid { [14, 14] } else { [6, 7] })
                .height(Length::Fill);
            // Rows take their own secondary clicks; the space around them is the
            // folder's. Every column keeps the same widgets, keeping their state.
            let mut list = iced::widget::mouse_area(list);
            // A parent column's empty space goes back to its folder, the
            // child it showed selected.
            if !current && let Some(child) = child {
                list = list.on_press(Message::Select(id, child.to_owned()));
            }
            if current && !searching {
                list = list
                    .on_press(Message::Deselect(id, tab.id, tab.revision))
                    .on_right_press(Message::Action(actions_ui::Event::Background(
                        id,
                        tab.id,
                        tab.revision,
                    )));
            }
            // The toolbar's path already names an Icons view's folder.
            // Only List view's details and a search have a heading: Columns
            // and Icons views let the path name the folder.
            let folder: Element<'_, Message> = if grid || !(inline || searching) {
                column![list].into()
            } else {
                column![heading, rule::horizontal(1).style(chrome::divider), list,].into()
            };
            // In Columns view the current folder is a column like its parents,
            // so its rows stay near their names; the space after it is still
            // the folder's. The last column needs no divider after it.
            let columned = current
                && !searching
                && browser.view().mode == crate::preferences::ViewMode::Columns;
            let folder: Element<'_, Message> = if columned {
                let rest =
                    iced::widget::mouse_area(space().width(Length::Fill).height(Length::Fill))
                        .on_press(Message::Deselect(id, tab.id, tab.revision))
                        .on_right_press(Message::Action(actions_ui::Event::Background(
                            id,
                            tab.id,
                            tab.revision,
                        )));
                row![
                    container(folder)
                        .width(minimum + current_extra - 1.0)
                        .height(Length::Fill),
                    rest,
                ]
                .into()
            } else {
                folder
            };
            let folder: Element<'_, Message> = container(folder)
                .width(if current {
                    Length::Fill
                } else if index + 2 == paths.len() && !columned_view {
                    Length::Fixed(minimum + parent_extra - 1.0)
                } else {
                    Length::Fixed(minimum - 1.0)
                })
                // Parent columns share the folder's surface; their dividers
                // and selections set them apart.
                .height(Length::Fill)
                .into();
            content = content.push(super::drag_ui::target_region(
                desktop,
                id,
                tab,
                path_for_drop.clone(),
                super::drag_ui::column_binding(tab, &path_for_drop),
                1,
                folder,
            ));
        }
    } else {
        content = content.push(
            container(text(if tab.loading {
                fl!("files-loading-folder")
            } else {
                fl!("files-folder-unavailable")
            }))
            .padding(20)
            .width(Length::Fill),
        );
    }
    if let Some(panel) = &window.properties {
        let panel_appearance = appearance.clone();
        // The divider at the panel's start resizes it: moving it left
        // widens the panel.
        content = content
            .push(super::resize::Grip {
                window: id,
                target: super::resize::Target::Properties,
                width: 1.0,
                current: properties_width,
                height: Length::Fill,
                line: None,
                color: divider_color,
                share: -1.0,
                quiet: false,
                enabled: desktop.dividers_take_pointer(id),
            })
            .push(
                container(super::properties_ui::panel(desktop, id, panel))
                    .width(properties_width)
                    .height(Length::Fill)
                    .style(move |_| chrome::panel_surface(&panel_appearance)),
            );
    }
    // A tab dragged over a one-tab window shows its bar, to show where it lands.
    let outer = if window.tabs.len() > 1 || desktop.drag.tab_hover(id).is_some() {
        outer.push(
            column![tab_bar(desktop, id, window, tab, sidebar), content]
                .width(Length::Fill)
                .height(Length::Fill),
        )
    } else {
        outer.push(content)
    };
    body = body.push(outer);
    let status = if let Some(error) = &tab.error {
        error.clone()
    } else if tab.loading {
        fl!("files-loading-folder")
    } else if let Some(browser) = &tab.browser {
        // Independent facts, separated like the rest of the status bar.
        let mut parts = Vec::new();
        let selected = browser.selected().len();
        if let Some(results) = browser.search() {
            parts.push(fl!("files-status-results", count = results.entries.len()));
            parts.push(fl!("files-status-inspected", count = results.scanned));
            if tab.search.as_ref().is_some_and(|search| search.stopping) {
                parts.push(fl!("files-status-stopping"));
            } else if tab.search.as_ref().is_some_and(|search| search.running) {
                parts.push(fl!("files-status-searching"));
            } else if results.cancelled {
                parts.push(fl!("files-status-stopped"));
            }
            if selected != 0 {
                parts.push(fl!("files-status-selected", count = selected));
            }
            if results.truncated {
                parts.push(fl!("files-status-more-results"));
            }
        } else {
            let visible = browser.visible_entries();
            parts.push(fl!("files-status-items", count = visible.len()));
            // Selected files add their total size; folders have no cheap size.
            let bytes = (selected != 0 && visible.len() <= 20_000)
                .then(|| {
                    visible
                        .iter()
                        .filter(|entry| {
                            !entry.is_directory() && browser.selected().contains(&entry.path)
                        })
                        .map(|entry| entry.size)
                        .sum::<u64>()
                })
                .filter(|bytes| *bytes != 0);
            let one = (selected == 1)
                .then(|| {
                    visible
                        .iter()
                        .find(|entry| browser.selected().contains(&entry.path))
                })
                .flatten();
            match (one, bytes) {
                // One selected item is named, with its type and size.
                (Some(entry), _) => parts.push(selected_entry(desktop, entry)),
                (None, Some(bytes)) => parts.push(fl!(
                    "files-status-selected-size",
                    count = selected,
                    size = super::listing_ui::size(bytes)
                )),
                (None, None) if selected != 0 => {
                    parts.push(fl!("files-status-selected", count = selected))
                }
                (None, None) => {}
            }
        }
        parts.join(" · ")
    } else {
        String::new()
    };
    let status = desktop
        .drag
        .feedback(id)
        .map_or(status.clone(), |(_, feedback)| {
            format!("{feedback} · {status}")
        });
    let summary = [
        desktop.file_operations.summary(),
        desktop.jobs.summary(),
        desktop.devices.summary(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let wide_status = window.width >= 620.0;
    let status = if wide_status && !summary.is_empty() && tab.error.is_none() {
        format!("{status}  ·  {summary}")
    } else {
        status
    };
    let announcement = announcement(tab);
    let status_node = match &tab.error {
        Some(error) => Node::alert(error.clone()).key("files-error"),
        None => Node::new(Role::Status)
            .label(status.clone())
            .key("files-status")
            .hide_text(),
    };
    // Zero-width live regions share a spacing-free row with the status text.
    let mut footer = row![
        row![
            semantic(
                Node::status(announcement).key("files-announcement"),
                a11y::announcer_space()
            ),
            desktop.operations_announcer(id),
            semantic(
                status_node,
                container(
                    text(status)
                        .font(font)
                        .size(label_size + 1.0)
                        .color(chrome::ink(
                            appearance,
                            if tab.error.is_some() {
                                ColorRole::Danger
                            } else {
                                ColorRole::Text
                            },
                        ))
                        .wrapping(text::Wrapping::None)
                )
                .clip(true)
                .width(Length::Fill)
            )
        ]
        .width(Length::Fill)
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let operation_button = || {
        a11y::button(
            "files-operations",
            Some(Message::Command(id, Command::Operations)),
            canopa_ui::styled_action(
                row![
                    desktop.icons.chrome(Icon::Activity, 15.0, 0.90),
                    text(fl!("files-command-operations"))
                        .font(font)
                        .size(label_size),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                Some(Message::Command(id, Command::Operations)),
                canopa_ui::Action::Normal,
                false,
                appearance,
            ),
        )
    };
    if wide_status && !summary.is_empty() {
        footer = footer.push(operation_button());
    }
    let mut status_bar = column![footer].spacing(3);
    if !summary.is_empty() && (!wide_status || tab.error.is_some()) {
        let activity = row![
            text(summary)
                .font(font)
                .size(label_size)
                .style(canopa_ui::muted)
                .width(Length::Fill),
        ]
        .align_y(Alignment::Center);
        status_bar = status_bar.push(if wide_status {
            activity
        } else {
            activity.push(operation_button())
        });
    }
    body = body.push(rule::horizontal(1).style(chrome::divider)).push(
        // Status text starts where the sidebar's labels do.
        container(status_bar)
            .width(Length::Fill)
            .padding([5, 20])
            .style(move |_| chrome::bar(appearance, false)),
    );
    if let Some(picker) = &desktop.picker {
        body = body.push(picker.footer(desktop, id));
    }
    let shortcuts = if desktop
        .action_menu
        .as_ref()
        .is_some_and(|menu| menu.window() == id)
        || desktop
            .folder_options
            .as_ref()
            .is_some_and(|target| target.window == id)
    {
        vec![]
    } else {
        desktop
            .configuration
            .settings
            .bindings(Command::Location)
            .iter()
            .filter_map(|binding| KeyChord::parse(binding).ok())
            .map(|chord| (chord, Message::Command(id, Command::Location)))
            .collect()
    };
    let dimmed = desktop.recession(id);
    let appearance = appearance.clone();
    let scrim_appearance = appearance.clone();
    let surface = container(body)
        .id(root_id(id, tab.id))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::window_surface(&appearance, false));
    // The layer stays in the tree so that opening a prompt keeps every
    // browser widget's state. It only paints; input still reaches the browser.
    let scrim = container(space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| chrome::scrim(&scrim_appearance, dimmed));
    let content = iced::widget::stack![surface, scrim, drop_badge(desktop, id, window)];
    let content = focus::transitions(
        content,
        shortcuts,
        tab.location.as_ref().map(|_| location_id(id)),
    );
    if let Some(scene) = desktop.drag.scene(id) {
        super::drag_geometry::root(
            id,
            scene,
            desktop.drag.outlined(id),
            desktop.drag.grabbing(id),
            content,
        )
    } else {
        content
    }
}

const TAB_SPACING: f32 = 2.0;

/// The height of every view's heading band.
fn heading_height(appearance: &canopa_theme::Appearance) -> f32 {
    (appearance.typography().body_size.px() * 1.3 + 15.0).ceil()
}
/// A tab's room beside its name: the folder symbol, the close button and padding.
const TAB_CHROME: f32 = 100.0;
const TAB_MIN: f32 = 150.0;
const TAB_MAX: f32 = 260.0;

/// The window's tabs, once it has more than one: each names its folder, closes
/// with its cross or a middle click, and takes dropped files into that folder.
fn tab_bar<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    window: &'a BrowserWindow,
    active: &'a Tab,
    sidebar: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let label_size = appearance.typography().label_size.px() + 1.0;
    let mut tabs: Vec<(u64, Element<'_, Message>)> = Vec::new();
    // Tabs share the bar like a browser's: as wide as their folder names
    // need, narrowing together when many are open, never stretched across it.
    let count = window.tabs.len() as f32;
    let lead = if sidebar {
        desktop.sidebar_width(id) + 1.0
    } else {
        0.0
    };
    let share =
        ((window.width - lead - 16.0 - 34.0 - (count + 1.0) * TAB_SPACING) / count - 1.0).max(72.0);
    let mut semibold = font;
    semibold.weight = Weight::Semibold;
    // Every tab takes the width the longest name needs, so the strip reads
    // as one row of equal tabs.
    let tab_width = window
        .tabs
        .iter()
        .map(|candidate| {
            let path = candidate.path();
            let label = if path == desktop.home {
                fl!("files-place-home")
            } else {
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            };
            chrome::text_width(&label, semibold, label_size) + TAB_CHROME
        })
        .fold(TAB_MIN, f32::max)
        .min(TAB_MAX)
        .min(share);
    for (index, candidate) in window.tabs.iter().enumerate() {
        let current = index == window.active;
        let path = candidate.path();
        let label = if path == desktop.home {
            fl!("files-place-home")
        } else {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        let mut label_font = font;
        if current {
            label_font.weight = Weight::Semibold;
        }
        let close_message = Message::CloseTab(id, candidate.id);
        let close = a11y::named_button(
            format!("files-tab-close-{}", candidate.id),
            fl!("files-tab-close", folder = label.clone()),
            Some(close_message.clone()),
            iced::widget::button(desktop.icons.chrome(
                Icon::Close,
                16.0,
                if current { 0.9 } else { 0.7 },
            ))
            .padding(4)
            .on_press(close_message.clone())
            .style(|theme, status| {
                let mut style =
                    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                style.border.radius = 6.0.into();
                style
            }),
        );
        // Neighbouring inactive tabs are parted by a short line; the active
        // tab's own edge parts it from the others.
        let next_current = index + 1 == window.active;
        let last = index + 1 == window.tabs.len();
        let mut divider = chrome::ink(appearance, ColorRole::Border);
        divider.a = if current || next_current || last {
            0.0
        } else {
            0.45
        };
        let content = row![
            desktop
                .icons
                .chrome(Icon::Folder, 15.0, if current { 0.95 } else { 0.7 }),
            container(chrome::file_name_line(
                label.clone(),
                label_font,
                label_size,
                Some(if current {
                    chrome::ink(appearance, ColorRole::Text)
                } else {
                    chrome::ink(appearance, ColorRole::Text).scale_alpha(0.74)
                }),
                true,
            ))
            .width(Length::Fill),
            close,
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let tab_appearance = appearance.clone();
        // The current tab takes the tone of what lies below it, so the two
        // read as one piece: a list's or grid's surface, or the first
        // column's heading.
        // The current tab takes the tone right below it: List's headings, a
        // search's, the first column (an ancestor's, when there are several)
        // or the folder's own surface.
        // The current tab is cut from the tone below it, rounded only at the
        // top and flush with the bar's lower edge; a short accent bar rides
        // straight along its top, clear of the rounded corners.
        let fill_appearance = tab_appearance.clone();
        let bar_appearance = tab_appearance.clone();
        let body = container(content)
            .padding(iced::Padding {
                top: 3.0,
                bottom: if current { 5.0 } else { 3.0 },
                left: 12.0,
                right: 5.0,
            })
            .width(Length::Fill);
        let body: Element<'_, Message> = if current {
            column![
                container(
                    container(space().width(Length::Fill).height(3)).style(move |_| {
                        container::Style::default()
                            .background(chrome::ink(&bar_appearance, ColorRole::Accent))
                            .border(iced::Border::default().rounded(1.5))
                    })
                )
                .padding([0, 12]),
                body,
            ]
            .into()
        } else {
            body.into()
        };
        let card = container(body).width(tab_width).style(move |_| {
            let appearance = &fill_appearance;
            if current {
                container::Style::default()
                    .background(chrome::ink(appearance, ColorRole::Surface))
                    .border(iced::Border {
                        radius: iced::border::Radius {
                            top_left: 9.0,
                            top_right: 9.0,
                            bottom_right: 0.0,
                            bottom_left: 0.0,
                        },
                        ..Default::default()
                    })
            } else {
                // Other tabs keep a faint shape of their own.
                container::Style::default()
                    .background(chrome::ink(appearance, ColorRole::Text).scale_alpha(0.045))
                    .border(iced::Border::default().rounded(9))
            }
        });
        let card = container(super::hover::hover(
            card,
            super::listing_ui::hover_wash(appearance),
            9.0,
            !current,
        ))
        .padding(if current { 0.0 } else { 2.0 });
        let card = row![
            card,
            container(space().width(1).height(16))
                .style(move |_| container::Style::default().background(divider)),
        ]
        .align_y(Alignment::Center);
        let card = iced::widget::mouse_area(card)
            .on_press(Message::SwitchTab(id, index))
            .on_right_press(Message::Action(super::actions_ui::Event::Tab(id, index)))
            .on_middle_press(close_message);
        let card = super::drag_ui::target_region(
            desktop,
            id,
            active,
            path.to_owned(),
            None,
            2,
            card.into(),
        );
        tabs.push((
            candidate.id,
            semantic(
                Node::tab(label, current)
                    .key(format!("files-tab-{}", candidate.id))
                    .position(index + 1, window.tabs.len())
                    .on_click(Some(Message::SwitchTab(id, index))),
                card,
            ),
        ));
    }
    let add = Message::Command(id, Command::NewTab);
    let add = a11y::named_button(
        "files-tab-new",
        Command::NewTab.label(),
        Some(add.clone()),
        iced::widget::button(desktop.icons.chrome(Icon::Plus, 18.0, 0.85))
            .padding(6)
            .on_press(add)
            .style(|theme, status| {
                let mut style =
                    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                style.border.radius = 7.0.into();
                style
            }),
    );
    // A tab being dragged away leaves a ghost of itself, outlined in the
    // accent like the pin that marks where a dragged tab would land.
    let mut cover = chrome::ink(appearance, ColorRole::Surface);
    cover.a = 0.82;
    let pin = desktop.drag.tab_hover(id).and_then(|hover| {
        let geometry = window.tab_geometry.lock().ok()?.clone();
        geometry
            .landing(
                hover.point,
                hover.own.then_some(hover.tab),
                window.tabs.len(),
            )
            .map(|landing| landing.gap)
    });
    let bar_appearance = appearance.clone();
    semantic(
        Node::tab_list(fl!("files-tabs")).key("files-tabs"),
        container(super::tab_strip::tab_strip(
            tabs,
            add,
            TAB_SPACING,
            id,
            Size::new(window.width, window.height),
            desktop.drag.native(id),
            window.tab_geometry.clone(),
            pin,
            desktop.drag.lifted_tab(id),
            super::tab_strip::Colors {
                pin: chrome::ink(appearance, ColorRole::Accent),
                cover,
            },
        ))
        .padding(iced::Padding {
            top: 6.0,
            bottom: 0.0,
            left: 8.0,
            right: 8.0,
        })
        .width(Length::Fill)
        // The toolbar's tone, so the strip reads as part of it and the
        // current tab, on the folder's surface, opens out of it.
        .style(move |_| container::Style::default().background(chrome::bar_tint(&bar_appearance))),
    )
}

/// Beside the pointer of a drag over this window: what is dragged, and what
/// dropping it here would do. The layer stays, empty, between drags.
fn drop_badge<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    window: &BrowserWindow,
) -> Element<'a, Message> {
    let Some(feedback) = desktop
        .drag
        .hover_feedback(id)
        .filter(|feedback| feedback.acting)
    else {
        return space().width(0).height(0).into();
    };
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px() + 1.0;
    const EDGE: f32 = 8.0;
    let width = (chrome::text_width(&feedback.badge, font, size) + 26.0)
        .min(window.width - 2.0 * EDGE)
        .max(40.0);
    let height = (size * 1.3).ceil() + 12.0;
    let mut x = feedback.pointer.x + 16.0;
    if x + width + EDGE > window.width {
        x = feedback.pointer.x - 12.0 - width;
    }
    let mut y = feedback.pointer.y + 20.0;
    if y + height + EDGE > window.height {
        y = feedback.pointer.y - 12.0 - height;
    }
    let refused = feedback.refused;
    let badge_appearance = appearance.clone();
    let badge = container(chrome::file_name_line(
        feedback.badge,
        font,
        size,
        Some(chrome::ink(
            appearance,
            if refused {
                ColorRole::Danger
            } else {
                ColorRole::Text
            },
        )),
        false,
    ))
    .padding([6, 13])
    .width(width)
    .style(move |_| {
        let appearance = &badge_appearance;
        container::Style::default()
            .background(chrome::ink(
                appearance,
                if refused {
                    ColorRole::Raised
                } else {
                    ColorRole::AccentSurface
                },
            ))
            .border(iced::Border {
                color: chrome::ink(
                    appearance,
                    if refused {
                        ColorRole::Danger
                    } else {
                        ColorRole::Accent
                    },
                ),
                width: 1.0,
                radius: (height / 2.0).into(),
            })
    });
    container(badge)
        .padding(iced::Padding {
            left: x.max(EDGE),
            top: y.max(EDGE),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// One selected item for the status bar: "“report.pdf” · PDF document · 1.2 MiB".
fn selected_entry(desktop: &Desktop, entry: &crate::browse::Entry) -> String {
    let kind = desktop
        .icons
        .type_description(entry)
        .unwrap_or_else(|| super::listing_ui::kind(entry));
    if entry.is_directory() {
        fl!(
            "files-status-selected-folder",
            name = entry.name().into_owned(),
            kind = kind
        )
    } else {
        fl!(
            "files-status-selected-file",
            name = entry.name().into_owned(),
            kind = kind,
            size = super::listing_ui::size(entry.size)
        )
    }
}

/// Whether the keyboard cursor in the rows holds focus, rather than the
/// location or search field or an open menu.
pub(super) fn rows_have_focus(desktop: &Desktop, id: window::Id, tab: &Tab) -> bool {
    tab.location.is_none()
        && tab
            .search
            .as_ref()
            .is_none_or(|search| search.results_focused)
        && desktop
            .action_menu
            .as_ref()
            .is_none_or(|menu| menu.window() != id)
        && desktop
            .folder_options
            .as_ref()
            .is_none_or(|target| target.window != id)
}

/// The politely announced folder state: what loading, a finished search or a
/// selection change produced, without volatile scan counts.
pub(super) fn announcement(tab: &Tab) -> String {
    if tab.loading {
        return fl!("files-loading-folder");
    }
    let Some(browser) = &tab.browser else {
        return String::new();
    };
    let selected = browser.selected().len();
    if let Some(results) = browser.search() {
        if tab.search.as_ref().is_some_and(|search| search.running) {
            return fl!("files-status-searching");
        }
        let count = results.entries.len();
        return match (results.cancelled, selected) {
            (false, 0) => fl!("files-announce-results", count = count),
            (false, _) => fl!(
                "files-announce-results-selected",
                count = count,
                selected = selected
            ),
            (true, 0) => fl!("files-announce-results-stopped", count = count),
            (true, _) => fl!(
                "files-announce-results-stopped-selected",
                count = count,
                selected = selected
            ),
        };
    }
    let count = browser.visible_entries().len();
    if selected == 0 {
        fl!("files-status-items", count = count)
    } else {
        fl!(
            "files-announce-items-selected",
            count = count,
            selected = selected
        )
    }
}

/// Preserve the full scroll extent while building only nearby row widgets.
pub(super) fn visible_rows(
    count: usize,
    offset: f32,
    viewport: f32,
    height: f32,
) -> std::ops::Range<usize> {
    let viewport = if viewport > 0.0 { viewport } else { 640.0 };
    let budget = (viewport / height).ceil() as usize + 16;
    let start = ((offset / height).floor() as usize)
        .saturating_sub(8)
        .min(count.saturating_sub(budget));
    start..start.saturating_add(budget).min(count)
}

/// A search's three parts: its field, which takes the location's place in the
/// toolbar; its options and actions, beside the results' heading; and the
/// folders it could not read, under the toolbar while asked for.
pub(super) struct SearchParts<'a> {
    pub field: Element<'a, Message>,
    pub controls: Element<'a, Message>,
    pub problems: Option<Element<'a, Message>>,
}

pub(super) fn search_parts<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    session: &'a search_ui::SearchSession,
) -> SearchParts<'a> {
    use search_ui::{Event, Target};
    let target = Target {
        window: id,
        tab: tab.id,
        instance: session.instance,
    };
    let appearance = &desktop.configuration.appearance;
    let body_size = appearance.typography().body_size.px();
    let label_size = appearance.typography().label_size.px();
    let input_appearance = appearance.clone();
    let focused_result = tab
        .browser
        .as_ref()
        .filter(|browser| browser.search().is_some())
        .and_then(Browser::focused)
        .map(a11y::entry_key);
    let search_label = Command::Search.label();
    let mut input_node = Node::search_input(search_label.clone(), &session.query)
        .key("files-search")
        .placeholder(search_label.clone())
        .on_text(move |query| Message::Search(Event::Query(target, query)))
        .active_descendant(focused_result.filter(|_| !session.results_focused));
    if let Some(browser) = &tab.browser {
        input_node = input_node.controls(a11y::list_key(browser.path()));
    }
    let placeholder = if session.recent {
        fl!("files-search-recent")
    } else {
        fl!(
            "files-search-in",
            folder = tab
                .path()
                .file_name()
                .unwrap_or(tab.path().as_os_str())
                .to_string_lossy()
                .into_owned()
        )
    };
    let input = text_input(&placeholder, &session.query)
        .id(search_id(id, tab.id))
        .on_input(move |query| Message::Search(Event::Query(target, query)))
        .on_submit(Message::Search(Event::FocusResults(target)))
        .size(body_size)
        .padding({
            let mut padding = canopa_ui::content_padding(appearance.style(UiKind::Input));
            padding.right += 26.0;
            padding
        })
        .style(move |theme, status| input_style(&input_appearance, theme, status))
        .width(Length::Fill);
    let input = semantic(input_node, input);
    let action = |key: &'static str, label: String, event: Option<Event>| -> Element<'a, Message> {
        let message = event.map(Message::Search);
        a11y::button(
            key,
            message.clone(),
            canopa_ui::styled_action(
                text(label).size(label_size),
                message,
                canopa_ui::Action::Quiet,
                false,
                appearance,
            ),
        )
    };
    // The cross sits where the location field keeps its search button.
    let close_message = Message::Search(Event::Close(target));
    let close = a11y::named_button(
        "files-search-close",
        fl!("files-search-close"),
        Some(close_message.clone()),
        iced::widget::button(desktop.icons.chrome(Icon::Close, 14.0, 0.9))
            .padding(4)
            .on_press(close_message)
            .style(|theme, status| {
                let mut style =
                    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                style.border.radius = 6.0.into();
                style
            }),
    );
    let scope = a11y::check_box(
        "files-search-subfolders",
        session.recursive,
        Some(Message::Search(Event::Recursive(
            target,
            !session.recursive,
        ))),
        checkbox(session.recursive)
            .label(fl!("files-search-include-subfolders"))
            .text_size(body_size)
            .on_toggle(move |recursive| Message::Search(Event::Recursive(target, recursive))),
    );
    let results = tab.browser.as_ref().and_then(Browser::search);
    let padding = canopa_ui::content_padding(appearance.style(UiKind::Button));
    let height = ((label_size * 1.3).ceil() + padding.top + padding.bottom)
        .max(body_size * 1.3)
        .max(16.0);
    // Recent files have no folder whose subfolders could be included.
    let scope: Element<'a, Message> = if session.recent {
        space().width(0).into()
    } else {
        scope
    };
    let mut options = row![scope]
        .spacing(6)
        .align_y(Alignment::Center)
        .height(height);
    if session.running {
        options = options.push(action(
            "files-search-stop",
            if session.stopping {
                fl!("files-status-stopping")
            } else {
                fl!("files-search-stop")
            },
            (!session.stopping).then_some(Event::Stop(target)),
        ));
    } else if results.is_some_and(|results| results.truncated) {
        options = options.push(action(
            "files-search-more",
            fl!("files-search-more"),
            Some(Event::More(target)),
        ));
    }
    let field = iced::widget::stack![
        input,
        container(close)
            .padding([0, 5])
            .align_right(Length::Fill)
            .center_y(Length::Fill),
    ];
    let focused = tab
        .browser
        .as_ref()
        .filter(|browser| browser.search().is_some())
        .and_then(Browser::focused)
        .is_some();
    let failures = results.map_or(&[][..], |results| results.failures.as_slice());
    if results.is_some() || !failures.is_empty() {
        let mut actions = row![].spacing(6);
        actions = actions.push(action(
            "files-search-reveal",
            fl!("files-search-reveal"),
            focused.then_some(Event::Reveal(target)),
        ));
        if !failures.is_empty() {
            actions = actions.push(action(
                "files-search-failures",
                if session.show_failures {
                    fl!("files-search-unreadable-hide", count = failures.len())
                } else {
                    fl!("files-search-unreadable-details", count = failures.len())
                },
                Some(Event::ToggleFailures(target)),
            ));
        }
        options = options.push(actions);
    }
    let mut problems = None;
    if session.show_failures && !failures.is_empty() {
        let height = (label_size * 1.3).ceil() + 8.0;
        let range = visible_rows(failures.len(), session.failure_scroll, 100.0, height);
        let mut errors = column![space().height(range.start as f32 * height)];
        for failure in &failures[range.clone()] {
            errors = errors.push(
                container(
                    text(fl!(
                        "files-search-failure",
                        location = relative_location(&failure.path, tab.path()),
                        error = failure.message.clone()
                    ))
                    .size(label_size)
                    .wrapping(text::Wrapping::None),
                )
                .padding([4, 8])
                .height(height),
            );
        }
        errors = errors.push(space().height((failures.len() - range.end) as f32 * height));
        let serial = tab.search_serial;
        problems = Some(
            scrollable(errors)
                .height(100)
                .style(canopa_ui::scrollbar)
                .direction(scrollable::Direction::Both {
                    vertical: scrollable::Scrollbar::new(),
                    horizontal: scrollable::Scrollbar::new(),
                })
                .on_scroll(move |viewport| {
                    Message::Search(Event::FailureScroll(
                        target,
                        serial,
                        viewport.absolute_offset().y,
                    ))
                }),
        );
    }
    let problems = problems.map(|problems| {
        container(problems)
            .padding([4, 10])
            .width(Length::Fill)
            .into()
    });
    let shortcuts = [
        Command::SearchSubfolders,
        Command::StopSearch,
        Command::MoreSearchResults,
        Command::RevealSearchResult,
        Command::SearchProblems,
    ]
    .into_iter()
    .flat_map(|command| {
        desktop
            .configuration
            .settings
            .bindings(command)
            .into_iter()
            .filter_map(move |binding| {
                KeyChord::parse(&binding)
                    .ok()
                    .map(|chord| (chord, Message::Command(id, command)))
            })
    })
    .collect();
    SearchParts {
        field: semantic(
            Node::new(Role::Search)
                .label(fl!("files-toolbar-search"))
                .key("files-search-bar"),
            super::focus::editing_shortcuts(field, shortcuts),
        ),
        controls: options.into(),
        problems,
    }
}

/// One labelled choice of a search filter list.
#[derive(Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub(super) fn kind_label(kind: crate::search::KindFilter) -> String {
    use crate::search::KindFilter;
    match kind {
        KindFilter::Any => fl!("files-search-kind-any"),
        KindFilter::Folders => fl!("files-search-kind-folders"),
        KindFilter::Documents => fl!("files-search-kind-documents"),
        KindFilter::Images => fl!("files-search-kind-images"),
        KindFilter::Audio => fl!("files-search-kind-audio"),
        KindFilter::Video => fl!("files-search-kind-video"),
        KindFilter::Archives => fl!("files-search-kind-archives"),
    }
}

pub(super) fn date_label(date: crate::search::DateFilter) -> String {
    use crate::search::DateFilter;
    match date {
        DateFilter::Any => fl!("files-search-date-any"),
        DateFilter::Today => fl!("files-search-date-today"),
        DateFilter::Week => fl!("files-search-date-week"),
        DateFilter::Month => fl!("files-search-date-month"),
        DateFilter::Year => fl!("files-search-date-year"),
    }
}

pub(super) fn size_label(size: crate::search::SizeFilter) -> String {
    use crate::search::SizeFilter;
    match size {
        SizeFilter::Any => fl!("files-search-size-any"),
        SizeFilter::Small => fl!("files-search-size-small"),
        SizeFilter::Medium => fl!("files-search-size-medium"),
        SizeFilter::Large => fl!("files-search-size-large"),
    }
}

/// The type, date and size lists and the contents option of a search.
fn search_filters<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    session: &'a search_ui::SearchSession,
) -> Element<'a, Message> {
    use crate::search::{DateFilter, KindFilter, SearchFilters, SizeFilter};
    use search_ui::Event;
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let body_size = appearance.typography().body_size.px();
    let target = search_ui::Target {
        window: id,
        tab: tab.id,
        instance: session.instance,
    };
    let filters = session.filters;
    fn list<'a, T: Clone + PartialEq + 'a>(
        desktop: &'a Desktop,
        key: &'static str,
        name: String,
        choices: Vec<Choice<T>>,
        current: T,
        update: impl Fn(T) -> SearchFilters + 'a,
        target: search_ui::Target,
    ) -> Element<'a, Message> {
        let appearance = &desktop.configuration.appearance;
        let selected = choices
            .iter()
            .find(|choice| choice.value == current)
            .cloned();
        let value = selected
            .as_ref()
            .map(|choice| choice.label.clone())
            .unwrap_or_default();
        let list = iced::widget::pick_list(choices, selected, move |choice: Choice<T>| {
            Message::Search(search_ui::Event::Filters(target, update(choice.value)))
        })
        .font(canopa_ui::font(appearance))
        .text_size(appearance.typography().label_size.px())
        .padding(iced::Padding {
            top: 4.0,
            bottom: 4.0,
            left: 10.0,
            right: 26.0,
        })
        .handle(iced::widget::pick_list::Handle::None)
        .style(move |theme, status| {
            let mut style = iced::widget::pick_list::default(theme, status);
            style.background = chrome::ink(appearance, ColorRole::Raised).into();
            style.border.radius = 8.0.into();
            style.border.color = match status {
                iced::widget::pick_list::Status::Active => {
                    chrome::ink(appearance, ColorRole::Border).scale_alpha(0.6)
                }
                _ => chrome::ink(appearance, ColorRole::Accent),
            };
            style
        })
        .menu_style(move |theme| {
            let mut style = iced::widget::overlay::menu::default(theme);
            style.border.radius = 8.0.into();
            style.background = chrome::ink(appearance, ColorRole::Raised).into();
            style.selected_background = chrome::ink(appearance, ColorRole::AccentSurface).into();
            style.selected_text_color = chrome::ink(appearance, ColorRole::Text);
            style
        });
        let list = iced::widget::stack![
            list,
            container(desktop.icons.chrome(Icon::Down, 12.0, 0.75))
                .height(Length::Fill)
                .align_right(Length::Fill)
                .center_y(Length::Fill)
                .padding(iced::Padding {
                    right: 9.0,
                    ..Default::default()
                }),
        ];
        semantic(Node::combo_box(name, value).key(key), list)
    }
    let kinds = [
        KindFilter::Any,
        KindFilter::Folders,
        KindFilter::Documents,
        KindFilter::Images,
        KindFilter::Audio,
        KindFilter::Video,
        KindFilter::Archives,
    ]
    .map(|value| Choice {
        value,
        label: kind_label(value),
    })
    .to_vec();
    let dates = [
        DateFilter::Any,
        DateFilter::Today,
        DateFilter::Week,
        DateFilter::Month,
        DateFilter::Year,
    ]
    .map(|value| Choice {
        value,
        label: date_label(value),
    })
    .to_vec();
    let sizes = [
        SizeFilter::Any,
        SizeFilter::Small,
        SizeFilter::Medium,
        SizeFilter::Large,
    ]
    .map(|value| Choice {
        value,
        label: size_label(value),
    })
    .to_vec();
    let contents = {
        let toggled = SearchFilters {
            contents: !filters.contents,
            ..filters
        };
        a11y::check_box(
            "files-search-contents",
            filters.contents,
            Some(Message::Search(Event::Filters(target, toggled))),
            checkbox(filters.contents)
                .label(fl!("files-search-contents"))
                .text_size(body_size)
                .font(font)
                .on_toggle(move |contents| {
                    Message::Search(Event::Filters(
                        target,
                        SearchFilters {
                            contents,
                            ..filters
                        },
                    ))
                }),
        )
    };
    let lists = iced::widget::Row::from_vec(vec![
        list(
            desktop,
            "files-search-kind",
            fl!("files-search-kind"),
            kinds,
            filters.kind,
            move |kind| SearchFilters { kind, ..filters },
            target,
        ),
        list(
            desktop,
            "files-search-date",
            fl!("files-search-date"),
            dates,
            filters.modified,
            move |modified| SearchFilters {
                modified,
                ..filters
            },
            target,
        ),
        list(
            desktop,
            "files-search-size",
            fl!("files-search-size"),
            sizes,
            filters.size,
            move |size| SearchFilters { size, ..filters },
            target,
        ),
        contents,
    ])
    .spacing(8)
    .align_y(Alignment::Center)
    .wrap()
    .vertical_spacing(6);
    semantic(
        Node::new(Role::Group)
            .label(fl!("files-search-filters"))
            .key("files-search-filters"),
        container(lists).padding([6, 12]).width(Length::Fill),
    )
}

pub(super) fn view_icon(mode: crate::preferences::ViewMode) -> Icon {
    match mode {
        crate::preferences::ViewMode::Columns => Icon::Columns,
        crate::preferences::ViewMode::List => Icon::List,
        crate::preferences::ViewMode::Icons => Icon::Grid,
    }
}

/// The view's symbol drawn on the accent, for a chosen segment.
pub(super) fn view_icon_on_accent(mode: crate::preferences::ViewMode) -> Icon {
    match mode {
        crate::preferences::ViewMode::Columns => Icon::ActiveColumns,
        crate::preferences::ViewMode::List => Icon::ActiveList,
        crate::preferences::ViewMode::Icons => Icon::ActiveGrid,
    }
}

pub(super) fn view_label(mode: crate::preferences::ViewMode) -> String {
    match mode {
        crate::preferences::ViewMode::Columns => fl!("files-view-columns"),
        crate::preferences::ViewMode::List => fl!("files-view-list"),
        crate::preferences::ViewMode::Icons => fl!("files-view-icons"),
    }
}

fn places(desktop: &Desktop, id: window::Id) -> Element<'_, Message> {
    let appearance = &desktop.configuration.appearance;
    use crate::interface::{Place, PlaceKind};
    let tab = desktop.active(id).expect("Places has an active tab");
    let active_index = desktop
        .configuration
        .settings
        .places
        .iter()
        .enumerate()
        .filter_map(|(index, place)| {
            let path = desktop.place_path(place)?;
            // A place stays current inside its folders, except the Filesystem
            // and Home, which hold nearly everything and would always be lit.
            (path == tab.path()
                || path != Path::new("/") && path != desktop.home && tab.path().starts_with(&path))
            .then(|| (index, path.components().count()))
        })
        .max_by_key(|&(index, depth)| (depth, std::cmp::Reverse(index)))
        .map(|(index, _)| index);
    let mut list = column![].spacing(2);
    for (index, place) in desktop.configuration.settings.places.iter().enumerate() {
        let icon = match place {
            Place::Builtin(PlaceKind::Home) => Icon::Home,
            Place::Builtin(PlaceKind::Desktop) => Icon::Desktop,
            Place::Builtin(PlaceKind::Documents) => Icon::Documents,
            Place::Builtin(PlaceKind::Downloads) => Icon::Downloads,
            Place::Builtin(PlaceKind::Music) => Icon::Music,
            Place::Builtin(PlaceKind::Pictures) => Icon::Pictures,
            Place::Builtin(PlaceKind::Videos) => Icon::Videos,
            Place::Builtin(PlaceKind::Templates) => Icon::Templates,
            Place::Builtin(PlaceKind::Public) => Icon::Public,
            Place::Builtin(PlaceKind::Filesystem) => Icon::Disk,
            Place::Builtin(PlaceKind::Recent) => Icon::Recent,
            Place::Builtin(PlaceKind::Trash) => Icon::Trash,
            Place::Builtin(PlaceKind::Recovery) => Icon::Recovery,
            Place::Bookmark { .. } => Icon::Folder,
        };
        let path = desktop.place_path(place);
        let active = active_index == Some(index);
        let message = match place {
            Place::Builtin(PlaceKind::Recent) => Message::Command(id, Command::ShowRecent),
            Place::Builtin(PlaceKind::Trash) => Message::Command(id, Command::ShowTrash),
            Place::Builtin(PlaceKind::Recovery) => Message::Command(id, Command::Recovery),
            _ => {
                let Some(path) = &path else {
                    continue;
                };
                Message::Go(id, path.clone())
            }
        };
        if desktop.picker.as_ref().is_some_and(|picker| match place {
            Place::Builtin(PlaceKind::Trash | PlaceKind::Recovery) => true,
            Place::Builtin(PlaceKind::Recent) => !picker.allows(Command::ShowRecent),
            _ => false,
        }) {
            continue;
        }
        let mut font = canopa_ui::font(appearance);
        if active {
            font.weight = Weight::Semibold;
        }
        let label = chrome::single_line(
            place.label(),
            font,
            appearance.typography().body_size.px(),
            None,
            tab.location.is_none() && desktop.action_menu.is_none(),
        );
        // The theme's place and folder icons, with the built-in glyph as
        // their stand-in.
        let (name, folder) = match place {
            Place::Builtin(kind) => (super::icons_ui::place_icon_name(*kind), None),
            Place::Bookmark { path, .. } => (None, Some(path.as_path())),
        };
        let button = canopa_ui::styled_action(
            canopa_ui::direction::row([desktop.icons.place(name, folder, icon, 18.0, 1.0), label])
                .spacing(9)
                .align_y(Alignment::Center),
            Some(message.clone()),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(Length::Fill);
        // The current place is a neutral fill, like the column trail, so the
        // accent stays with the file selection.
        let button: Element<'_, Message> = if active {
            container(button)
                .width(Length::Fill)
                .style(move |_| {
                    if appearance.high_contrast() {
                        let mut style = chrome::highlight(appearance, ColorRole::AccentSurface);
                        style.border.width = 1.0;
                        style.border.color = chrome::ink(appearance, ColorRole::Accent);
                        style
                    } else {
                        chrome::sidebar_current(appearance)
                    }
                })
                .into()
        } else {
            button.into()
        };
        let key = match place {
            Place::Builtin(kind) => format!("files-place-{}", kind.id()),
            Place::Bookmark { path, .. } => {
                format!("files-place-bookmark-{}", a11y::path_key(path))
            }
        };
        let button = semantic(
            Node::option(place.label(), active)
                .key(key)
                .on_click(Some(message)),
            button,
        );
        let button = match path {
            Some(path) => super::drag_ui::target_region(desktop, id, tab, path, None, 2, button),
            None => button,
        };
        list = list.push(button);
    }
    let configured: Vec<_> = desktop
        .configuration
        .settings
        .places
        .iter()
        .filter_map(|place| desktop.place_path(place))
        .collect();
    // GTK bookmarks are shared with file choosers and other file managers.
    for (name, path) in super::network_ui::local_bookmarks(&desktop.network.bookmarks, &configured)
    {
        let active = tab.path() == path;
        let mut font = canopa_ui::font(appearance);
        if active {
            font.weight = Weight::Semibold;
        }
        let node = Node::option(name.clone(), active)
            .key(format!("files-place-gtk-{}", a11y::path_key(&path)))
            .on_click(Some(Message::Go(id, path.clone())));
        let button = canopa_ui::styled_action(
            canopa_ui::direction::row([
                desktop
                    .icons
                    .chrome(Icon::Folder, 18.0, if active { 1.0 } else { 0.86 }),
                chrome::single_line(
                    name,
                    font,
                    appearance.typography().body_size.px(),
                    None,
                    tab.location.is_none() && desktop.action_menu.is_none(),
                ),
            ])
            .spacing(9)
            .align_y(Alignment::Center),
            Some(Message::Go(id, path.clone())),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(Length::Fill);
        list = list.push(super::drag_ui::target_region(
            desktop,
            id,
            tab,
            path,
            None,
            2,
            semantic(node, button),
        ));
    }
    let mut places = column![
        chrome::section(appearance, fl!("files-toolbar-places")),
        semantic(
            Node::list(fl!("files-toolbar-places")).key("files-places"),
            list,
        )
    ]
    .spacing(2)
    .padding([10, 10]);
    let model = super::network_ui::sidebar(&desktop.devices.catalog, &desktop.network.bookmarks);
    if !model.devices.is_empty()
        || matches!(
            desktop.devices.catalog.status,
            crate::local_volumes::DeviceStatus::Failed(_)
        )
        || desktop.devices.error.is_some()
    {
        places = places
            .push(container(rule::horizontal(1).style(chrome::divider)).padding([9, 8]))
            .push(super::devices_ui::places(desktop, id));
    }
    if desktop.picker.is_none() {
        places = places
            .push(container(rule::horizontal(1).style(chrome::divider)).padding([9, 8]))
            .push(super::network_ui::places(desktop, id));
    }
    container(scrollable(places).style(canopa_ui::scrollbar))
        .width(desktop.sidebar_width(id))
        .height(Length::Fill)
        .style(move |_| {
            container::Style::default().background(chrome::ink(appearance, ColorRole::Background))
        })
        .into()
}

pub(super) use crate::browser_ui::input_style;

pub(super) fn relative_location(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) if relative.as_os_str().is_empty() => fl!("files-this-folder"),
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}
