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

pub(super) const TOOLBAR_MODES_WIDTH: f32 = 1120.0;

pub(super) fn entry_padding(appearance: &canopa_theme::Appearance, grid: bool) -> iced::Padding {
    let style = appearance.style(UiKind::MenuRow);
    let mut padding = canopa_ui::content_padding(style);
    if !grid && !style.custom_padding {
        padding.top -= 5.0;
        padding.bottom -= 5.0;
    }
    padding
}

pub(super) fn row_height(appearance: &canopa_theme::Appearance) -> f32 {
    let padding = entry_padding(appearance, false);
    (appearance.typography().body_size.px() * 1.3)
        .ceil()
        .max(18.0)
        + padding.top
        + padding.bottom
}

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
    // Keep the toolbar at the same tree index as the sidebar comes and goes.
    // Iced stores the active location editor's focus and caret in this tree.
    let brand: Element<'_, Message> = if sidebar {
        let mark = container(desktop.icons.chrome(Icon::Folder, 20.0, 1.0))
            .padding(7)
            .style(move |_| {
                container::Style::default()
                    .background(chrome::ink(appearance, ColorRole::AccentSurface))
                    .border(iced::Border::default().rounded(9))
            });
        container(
            row![
                mark,
                text("Files")
                    .font(title_font)
                    .size(appearance.typography().title_size.px())
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([0, 16])
        .width(chrome::SIDEBAR_WIDTH + 1.0)
        .into()
    } else {
        space().width(0).into()
    };
    let paths = tab.column_paths(window.width, desktop.configuration.settings.sidebar);
    let minimum = tab
        .browser
        .as_ref()
        .map_or(0.0, |browser| f32::from(browser.view().column_width));
    let content_width = window.width
        - if sidebar {
            chrome::SIDEBAR_WIDTH + 1.0
        } else {
            0.0
        };
    let spare = (content_width - paths.len() as f32 * minimum).max(0.0);
    // Share surplus with the immediate parent while keeping the larger share
    // available for the current folder.
    let parent_extra = if paths.len() > 1 {
        (spare * 0.46).min(68.0)
    } else {
        0.0
    };
    let toolbar = iced::widget::Row::from_vec(vec![brand, toolbar::view(desktop, id, tab)])
        .width(Length::Fill)
        .align_y(Alignment::Center);
    let mut body = column![
        container(toolbar).style(move |_| chrome::bar(appearance, true)),
        rule::horizontal(1).style(chrome::divider)
    ];
    let notice: Element<'_, Message> = if let Some(error) = &desktop.configuration_error {
        container(text(format!("Configuration was not applied: {error}")).size(label_size))
            .padding([6, 12])
            .into()
    } else {
        space().height(0).into()
    };
    body = body.push(container(notice));
    let persistence_notice: Element<'_, Message> = if let Some(error) = &desktop.folder_writer.error
    {
        container(row![
            text(format!("View preferences were not saved: {error}"))
                .size(label_size)
                .width(Length::Fill),
            canopa_ui::styled_action(
                text("Retry").size(label_size),
                Some(Message::FolderView(folder_views::Event::Retry)),
                canopa_ui::Action::Quiet,
                false,
                appearance
            )
        ])
        .padding([4, 10])
        .into()
    } else {
        space().height(0).into()
    };
    body = body.push(container(persistence_notice));
    if window.tabs.len() > 1 {
        let mut tabs = row![].spacing(4).padding([4, 8]);
        for (index, candidate) in window.tabs.iter().enumerate() {
            let label = candidate
                .path()
                .file_name()
                .unwrap_or(candidate.path().as_os_str())
                .to_string_lossy()
                .into_owned();
            tabs = tabs.push(canopa_ui::styled_action(
                text(label).size(label_size),
                Some(Message::SwitchTab(id, index)),
                if index == window.active {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                index == window.active,
                appearance,
            ));
        }
        body = body.push(
            scrollable(tabs).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new(),
            )),
        );
    }
    if let Some(session) = &tab.search {
        body = body.push(search_bar(desktop, id, tab, session));
    }
    let mut content = row![].height(Length::Fill);
    if sidebar {
        content = content
            .push(places(desktop, id))
            .push(rule::vertical(1).style(chrome::divider));
    }
    if let Some(browser) = &tab.browser {
        for (index, path) in paths.iter().enumerate() {
            if index != 0 {
                content = content.push(rule::vertical(1).style(chrome::divider));
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
                    directory.visible_entries(browser.show_hidden(), child)
                };
                if visible.is_empty() {
                    entries = entries.push(keyed_row(
                        RowKey::Notice,
                        container(
                            text(if searching {
                                if tab.search.as_ref().is_some_and(|search| search.running) {
                                    "Searching filenames…"
                                } else {
                                    "No matching filenames"
                                }
                            } else {
                                "Empty folder"
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
                    Some(Err(error)) => error.as_str(),
                    _ => "Loading folder…",
                };
                entries = entries.push(keyed_row(
                    RowKey::Notice,
                    container(text(status).size(body_size)).padding(12),
                ));
            }
            let heading = if searching {
                "Search results".to_owned()
            } else if path == Path::new("/") {
                "Filesystem".to_owned()
            } else if path == &desktop.home {
                "Home".to_owned()
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
            let symbol: Element<'_, Message> = if current {
                container(symbol)
                    .padding(5)
                    .style(move |_| chrome::highlight(appearance, ColorRole::Raised))
                    .into()
            } else {
                container(symbol).padding(5).into()
            };
            let inline = current
                && !searching
                && super::listing_ui::inline_details(
                    appearance,
                    browser.view(),
                    tab.viewport_width,
                );
            let heading = if inline {
                let trailing = entry_padding(appearance, false).right + 14.0;
                row![
                    space().width(30),
                    chrome::single_line(
                        super::listing_ui::name_heading(browser.view()),
                        title_font,
                        label_size,
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
                    super::listing_ui::detail_cells(appearance, browser.view(), None),
                    space().width(trailing),
                ]
            } else {
                row![
                    symbol,
                    chrome::single_line(
                        heading,
                        title_font,
                        if current { body_size + 4.0 } else { body_size },
                        Some(chrome::ink(appearance, ColorRole::Text)),
                        tab.location.is_none() && desktop.action_menu.is_none(),
                    ),
                ]
            }
            .spacing(7)
            .align_y(Alignment::Center);
            let heading = container(heading)
                .height((body_size * 1.3 + 23.0).ceil())
                .align_y(Alignment::Center)
                .padding([0, 11])
                .width(Length::Fill)
                .style(move |_| {
                    let role = if current && !inline {
                        ColorRole::AccentSurface
                    } else {
                        ColorRole::Raised
                    };
                    let mut background = chrome::ink(appearance, role);
                    background.a = if current { 1.0 } else { 0.58 };
                    container::Style::default().background(background)
                });
            let folder: Element<'_, Message> = container(column![
                heading,
                rule::horizontal(if current { 2 } else { 1 }).style(move |theme| {
                    let mut style = chrome::divider(theme);
                    if current && !inline {
                        style.color = chrome::ink(appearance, ColorRole::Accent);
                        style.color.a = 0.72;
                    }
                    style
                }),
                container(list).padding([6, 7]).height(Length::Fill)
            ])
            .width(if current {
                Length::Fill
            } else if index + 2 == paths.len() {
                Length::Fixed(minimum + parent_extra - 1.0)
            } else {
                Length::Fixed(minimum - 1.0)
            })
            .height(Length::Fill)
            .style(move |_| {
                if current {
                    container::Style::default()
                } else {
                    container::Style::default()
                        .background(chrome::ink(appearance, ColorRole::Spine))
                }
            })
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
                "Loading folder…"
            } else {
                "Folder unavailable"
            }))
            .padding(20)
            .width(Length::Fill),
        );
    }
    body = body.push(content);
    let status = if let Some(error) = &tab.error {
        error.clone()
    } else if tab.loading {
        "Loading folder…".into()
    } else if let Some(browser) = &tab.browser {
        let selection = if browser.selected().is_empty() {
            String::new()
        } else {
            format!(" · {} selected", browser.selected().len())
        };
        if let Some(results) = browser.search() {
            let activity = if tab.search.as_ref().is_some_and(|search| search.stopping) {
                " · Stopping…"
            } else if tab.search.as_ref().is_some_and(|search| search.running) {
                " · Searching…"
            } else if results.cancelled {
                " · Stopped"
            } else {
                ""
            };
            format!(
                "{} {} · {} inspected{activity}{selection}{}",
                results.entries.len(),
                if results.entries.len() == 1 {
                    "result"
                } else {
                    "results"
                },
                results.scanned,
                if results.truncated {
                    " · More results available"
                } else {
                    ""
                }
            )
        } else {
            format!(
                "{} {}{}",
                browser.visible_entries().len(),
                if browser.visible_entries().len() == 1 {
                    "item"
                } else {
                    "items"
                },
                if browser.selected().is_empty() {
                    String::new()
                } else {
                    format!(" · {} selected", browser.selected().len())
                }
            )
        }
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
    let mut footer = row![
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
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let operation_button = || {
        canopa_ui::styled_action(
            row![
                desktop.icons.chrome(Icon::Activity, 15.0, 0.90),
                text("Operations").font(font).size(label_size),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            Some(Message::Command(id, Command::Operations)),
            canopa_ui::Action::Normal,
            false,
            appearance,
        )
    };
    if wide_status && !summary.is_empty() {
        footer = footer.push(operation_button());
    }
    if let Some(browser) = &tab.browser
        && browser.search().is_none()
        && window.width < TOOLBAR_MODES_WIDTH
    {
        footer = footer.push(
            view_switcher(
                desktop,
                id,
                tab,
                browser.view().mode,
                window.width >= 560.0,
                false,
            )
            .1,
        );
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
        container(status_bar)
            .width(Length::Fill)
            .padding([5, 14])
            .style(move |_| chrome::bar(appearance, false)),
    );
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
    let appearance = appearance.clone();
    let content = container(body)
        .id(root_id(id, tab.id))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    let content = focus::transitions(
        content,
        shortcuts,
        tab.location.as_ref().map(|_| location_id(id)),
    );
    if let Some(scene) = desktop.drag.scene(id) {
        super::drag_geometry::root(
            id,
            scene,
            desktop.drag.feedback(id).map(|(bounds, _)| bounds),
            content,
        )
    } else {
        content
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

fn search_bar<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    session: &'a search_ui::SearchSession,
) -> Element<'a, Message> {
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
    let input = text_input("Search filenames", &session.query)
        .id(search_id(id, tab.id))
        .on_input(move |query| Message::Search(Event::Query(target, query)))
        .on_submit(Message::Search(Event::FocusResults(target)))
        .size(body_size)
        .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
        .style(move |theme, status| input_style(&input_appearance, theme, status));
    let action = |label: String, event: Option<Event>| -> Element<'a, Message> {
        canopa_ui::styled_action(
            text(label).size(label_size),
            event.map(Message::Search),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .into()
    };
    let close = action("Close".into(), Some(Event::Close(target)));
    let scope = checkbox(session.recursive)
        .label("Include subfolders")
        .text_size(body_size)
        .on_toggle(move |recursive| Message::Search(Event::Recursive(target, recursive)))
        .width(Length::Fill);
    let results = tab.browser.as_ref().and_then(Browser::search);
    let padding = canopa_ui::content_padding(appearance.style(UiKind::Button));
    let height = ((label_size * 1.3).ceil() + padding.top + padding.bottom)
        .max(body_size * 1.3)
        .max(16.0);
    let mut options = row![scope]
        .spacing(6)
        .align_y(Alignment::Center)
        .height(height);
    if session.running {
        options = options.push(action(
            if session.stopping {
                "Stopping…"
            } else {
                "Stop"
            }
            .into(),
            (!session.stopping).then_some(Event::Stop(target)),
        ));
    } else if results.is_some_and(|results| results.truncated) {
        options = options.push(action("More results".into(), Some(Event::More(target))));
    }
    let mut body = column![row![input, close].spacing(6), options].spacing(4);
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
            "Show in folder".into(),
            focused.then_some(Event::Reveal(target)),
        ));
        if !failures.is_empty() {
            actions = actions.push(action(
                format!(
                    "{} unreadable{}",
                    failures.len(),
                    if session.show_failures {
                        " · Hide"
                    } else {
                        " · Details"
                    }
                ),
                Some(Event::ToggleFailures(target)),
            ));
        }
        body = body.push(actions);
    }
    if session.show_failures && !failures.is_empty() {
        let height = (label_size * 1.3).ceil() + 8.0;
        let range = visible_rows(failures.len(), session.failure_scroll, 100.0, height);
        let mut errors = column![space().height(range.start as f32 * height)];
        for failure in &failures[range.clone()] {
            errors = errors.push(
                container(
                    text(format!(
                        "{} — {}",
                        relative_location(&failure.path, tab.path()),
                        failure.message
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
        body = body.push(
            scrollable(errors)
                .height(100)
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
    super::focus::editing_shortcuts(container(body).padding([4, 10]), shortcuts)
}

pub(super) fn view_switcher<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &Tab,
    current: crate::preferences::ViewMode,
    show_labels: bool,
    top: bool,
) -> (f32, Element<'a, Message>) {
    use crate::preferences::ViewMode;
    use iced::widget::{button, tooltip};

    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().label_size.px() + 1.0;
    let mut controls = row![].spacing(2).align_y(Alignment::Center);
    let mut width = 10.0;
    for (mode, icon, command) in [
        (ViewMode::Columns, Icon::Columns, Command::Columns),
        (ViewMode::List, Icon::List, Command::List),
        (ViewMode::Icons, Icon::Grid, Command::Icons),
    ] {
        let active = mode == current;
        width += 32.0;
        let mut content = row![
            desktop
                .icons
                .chrome(icon, 16.0, if active { 1.0 } else { 0.8 })
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        if active || show_labels {
            let label = match mode {
                ViewMode::Columns => "Columns",
                ViewMode::List => "List",
                ViewMode::Icons => "Icons",
            };
            width += 6.0 + chrome::text_width(label, font, size);
            content = content.push(text(label).font(font).size(size));
        }
        let control = button(content)
            .padding([4, 8])
            .on_press_maybe((!active && !tab.loading).then_some(Message::Command(id, command)))
            .style(move |theme, status| {
                let mut style =
                    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                style.border.radius = 6.0.into();
                if active {
                    style.text_color = chrome::ink(appearance, ColorRole::Text);
                    style.border.width = 1.0;
                    style.border.color = chrome::ink(appearance, ColorRole::Accent);
                    style.border.color.a = 0.32;
                    style.background =
                        Some(chrome::ink(appearance, ColorRole::AccentSurface).into());
                }
                style
            });
        let control: Element<'_, Message> = if active
            || show_labels
            || tab.location.is_some()
            || desktop.action_menu.is_some()
            || desktop.folder_options.is_some()
        {
            control.into()
        } else {
            tooltip(
                control,
                text(command.label()).font(font).size(size),
                if top {
                    tooltip::Position::Bottom
                } else {
                    tooltip::Position::Top
                },
            )
            .into()
        };
        controls = controls.push(control);
    }
    (
        width,
        container(controls)
            .padding(3)
            .style(move |_| chrome::highlight(appearance, ColorRole::Spine))
            .into(),
    )
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
            (path == tab.path() || path != Path::new("/") && tab.path().starts_with(&path))
                .then(|| (index, path.components().count()))
        })
        .max_by_key(|&(index, depth)| (depth, std::cmp::Reverse(index)))
        .map(|(index, _)| index);
    let mut places = column![chrome::section(appearance, "Places")]
        .spacing(2)
        .padding([10, 10]);
    for (index, place) in desktop.configuration.settings.places.iter().enumerate() {
        let icon = match place {
            Place::Builtin(PlaceKind::Home) => Icon::Home,
            Place::Builtin(PlaceKind::Desktop) => Icon::Desktop,
            Place::Builtin(PlaceKind::Documents) => Icon::Documents,
            Place::Builtin(PlaceKind::Downloads) => Icon::Downloads,
            Place::Builtin(PlaceKind::Pictures) => Icon::Pictures,
            Place::Builtin(PlaceKind::Filesystem) => Icon::Drive,
            Place::Builtin(PlaceKind::Trash) => Icon::Trash,
            Place::Builtin(PlaceKind::Recovery) => Icon::Recovery,
            Place::Bookmark { .. } => Icon::Folder,
        };
        let path = desktop.place_path(place);
        let active = active_index == Some(index);
        let message = match place {
            Place::Builtin(PlaceKind::Trash) => Message::Command(id, Command::ShowTrash),
            Place::Builtin(PlaceKind::Recovery) => Message::Command(id, Command::Recovery),
            _ => {
                let Some(path) = &path else {
                    continue;
                };
                Message::Go(id, path.clone())
            }
        };
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
        let button = canopa_ui::styled_action(
            row![
                desktop
                    .icons
                    .chrome(icon, 18.0, if active { 1.0 } else { 0.86 }),
                label
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            Some(message),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(Length::Fill);
        let button: Element<'_, Message> = if active {
            container(button)
                .width(Length::Fill)
                .style(move |_| {
                    let mut style = chrome::highlight(appearance, ColorRole::AccentSurface);
                    style.border.width = 1.0;
                    style.border.color = chrome::ink(appearance, ColorRole::Accent);
                    style.border.color.a = 0.42;
                    style
                })
                .into()
        } else {
            button.into()
        };
        let button = match path {
            Some(path) => super::drag_ui::target_region(desktop, id, tab, path, None, 2, button),
            None => button,
        };
        places = places.push(button);
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
        let button = canopa_ui::styled_action(
            row![
                desktop
                    .icons
                    .chrome(Icon::Folder, 18.0, if active { 1.0 } else { 0.86 }),
                chrome::single_line(
                    name,
                    font,
                    appearance.typography().body_size.px(),
                    None,
                    tab.location.is_none() && desktop.action_menu.is_none(),
                )
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            Some(Message::Go(id, path.clone())),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(Length::Fill);
        places = places.push(super::drag_ui::target_region(
            desktop,
            id,
            tab,
            path,
            None,
            2,
            button.into(),
        ));
    }
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
    places = places
        .push(container(rule::horizontal(1).style(chrome::divider)).padding([9, 8]))
        .push(super::network_ui::places(desktop, id));
    container(scrollable(places))
        .width(chrome::SIDEBAR_WIDTH)
        .height(Length::Fill)
        .style(move |_| {
            container::Style::default().background(chrome::ink(appearance, ColorRole::Background))
        })
        .into()
}

pub(super) fn input_style(
    appearance: &canopa_theme::Appearance,
    theme: &Theme,
    status: text_input::Status,
) -> text_input::Style {
    let hovered = matches!(
        status,
        text_input::Status::Hovered | text_input::Status::Focused { is_hovered: true }
    );
    let focused = matches!(status, text_input::Status::Focused { .. });
    let style = appearance.style(UiKind::Input).state(
        false,
        hovered,
        false,
        status == text_input::Status::Disabled,
        focused,
    );
    let frame = canopa_ui::frame(appearance, style);
    let mut result = text_input::default(theme, status);
    if let Some(background) = frame.background {
        result.background = background;
    }
    if let Some(color) = frame.text_color {
        result.value = color;
    }
    result.selection = chrome::ink(appearance, ColorRole::AccentSurface);
    result.border = frame.border;
    if focused && style.states[4].border_color.is_none() {
        result.border.width = result.border.width.max(1.0);
        result.border.color = canopa_ui::color(appearance.role(ColorRole::Focus));
    }
    result
}

pub(super) fn relative_location(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) if relative.as_os_str().is_empty() => "This folder".into(),
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}
