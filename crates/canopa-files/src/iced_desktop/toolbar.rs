//! Configuration order is preserved across both toolbar arrangements. A narrow
//! tile gets a full-width location field and a scrollable command strip.
use super::accessibility_ui::{self as a11y, Node, Role, semantic};
use super::{chrome::Icon, *};
use crate::actions::CustomAction;
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Alignment, Length,
    font::Weight,
    widget::{button, column, container, responsive, row, scrollable, text, text_input, tooltip},
};

const SIDE_PADDING: f32 = 14.0;
const COMMAND_ICON: f32 = 20.0;

enum Item<'a> {
    Location,
    Builtin(Command),
    Action(&'a CustomAction),
}

fn items(desktop: &Desktop) -> Vec<Item<'_>> {
    desktop
        .configuration
        .settings
        .toolbar
        .iter()
        .filter_map(|id| {
            if id == "location" {
                Some(Item::Location)
            } else if let Some(id) = id.strip_prefix("action:") {
                desktop
                    .configuration
                    .actions
                    .iter()
                    .find(|action| action.id == id)
                    .map(Item::Action)
            } else {
                Command::from_id(id)
                    .filter(|command| *command != Command::QuickLook)
                    .map(Item::Builtin)
            }
        })
        .collect()
}

fn command_icon(command: Command) -> Option<(Icon, &'static str)> {
    Some(match command {
        Command::Back => (Icon::Back, ""),
        Command::Forward => (Icon::Forward, ""),
        Command::Parent => (Icon::Up, ""),
        Command::Search => (Icon::Search, "Search"),
        Command::Columns => (Icon::Columns, ""),
        Command::List => (Icon::List, ""),
        Command::Icons => (Icon::Grid, ""),
        _ => return None,
    })
}

fn navigation(item: &Item<'_>) -> bool {
    matches!(
        item,
        Item::Builtin(Command::Back | Command::Forward | Command::Parent)
    )
}

#[derive(Clone, Copy)]
enum Labels {
    Full,
    Icons,
}

fn action_label(command: Command, full: &'static str, labels: Labels) -> &'static str {
    match (command, labels) {
        (Command::Search, Labels::Icons) => "",
        _ => full,
    }
}

fn control_padding(
    item: &Item<'_>,
    appearance: &canopa_theme::Appearance,
    labels: Labels,
) -> iced::Padding {
    let style = appearance.style(UiKind::Button);
    let mut padding = canopa_ui::content_padding(style);
    if !style.custom_padding {
        let reduction = match item {
            _ if navigation(item) => 5.0,
            Item::Builtin(Command::Search) if matches!(labels, Labels::Icons) => 4.0,
            _ => 0.0,
        };
        padding.left -= reduction;
        padding.right -= reduction;
    }
    padding
}

fn width(item: &Item<'_>, appearance: &canopa_theme::Appearance, labels: Labels) -> f32 {
    let padding = control_padding(item, appearance, labels);
    let horizontal = padding.left + padding.right;
    let size = appearance.typography().label_size.px();
    match item {
        Item::Location => 144.0,
        Item::Builtin(command) => match command_icon(*command) {
            Some((_, label)) => {
                let label = action_label(*command, label, labels);
                let text = if label.is_empty() {
                    0.0
                } else {
                    6.0 + chrome::text_width(label, canopa_ui::font(appearance), size)
                };
                (COMMAND_ICON + text + horizontal).ceil()
            }
            None => (size * 8.4 + horizontal).ceil(),
        },
        Item::Action(action) => {
            (size * 8.4 + horizontal + if action.icon.is_some() { 24.0 } else { 0.0 }).ceil()
        }
    }
}

fn enabled(desktop: &Desktop, command: Command, tab: &Tab) -> bool {
    let Some(browser) = &tab.browser else {
        return false;
    };
    if tab.loading {
        return false;
    }
    match command {
        Command::Copy | Command::Cut => {
            desktop.file_clipboard.ready()
                && desktop.file_operations.storage.is_some()
                && desktop.file_operations.storage_error.is_none()
                && !browser.selected().is_empty()
        }
        Command::NewFolder => desktop.file_operations.ready(),
        Command::Trash | Command::DeletePermanently => {
            desktop.file_operations.ready() && !browser.selected().is_empty()
        }
        Command::Paste => desktop.file_operations.ready() && desktop.file_clipboard.can_paste(),
        Command::Recovery => desktop.file_operations.service.is_some(),
        Command::Rename => desktop.file_operations.ready() && browser.selected().len() == 1,
        Command::Undo => {
            desktop.file_operations.ready()
                && !desktop.file_operations.has_active()
                && desktop.file_operations.snapshot.undo.is_some()
        }
        Command::Back => {
            tab.search.is_some() || browser.history_target(HistoryDirection::Back).is_some()
        }
        Command::Forward => browser.history_target(HistoryDirection::Forward).is_some(),
        Command::Parent => browser.parent().is_some(),
        Command::Open => browser.focused().is_some(),
        Command::QuickLook => false,
        Command::EnterFolder => browser
            .visible_entries()
            .into_iter()
            .any(|entry| Some(entry.path.as_path()) == browser.focused() && entry.is_directory()),
        Command::NextTab
        | Command::PreviousTab
        | Command::CloseTab
        | Command::Search
        | Command::Location
        | Command::Refresh
        | Command::ShowHidden
        | Command::SelectAll
        | Command::NewWindow
        | Command::NewTab
        | Command::CustomActions
        | Command::Preferences
        | Command::BookmarkFolder
        | Command::Operations
        | Command::ShowTrash
        | Command::ConnectToServer
        | Command::Columns
        | Command::List
        | Command::Icons => true,
        Command::SearchSubfolders
        | Command::SearchProblems
        | Command::StopSearch
        | Command::MoreSearchResults
        | Command::RevealSearchResult => tab.search.is_some(),
    }
}

fn location<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    compact_path: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    if let Some(value) = &tab.location {
        let appearance = appearance.clone();
        let node = Node::text_input("Location", value.clone())
            .key("files-location")
            .on_text(move |value| Message::Location(id, value));
        let input = text_input("Location", value)
            .id(view::location_id(id))
            .on_input(move |value| Message::Location(id, value))
            .on_submit(Message::SubmitLocation(id))
            .size(appearance.typography().body_size.px())
            .font(font)
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| view::input_style(&appearance, theme, status))
            .width(Length::Fill);
        semantic(node, input)
    } else {
        semantic(
            Node::new(Role::Navigation)
                .label("Location")
                .key("files-breadcrumb"),
            breadcrumb(desktop, id, tab, compact_path),
        )
    }
}

fn breadcrumb<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    tab: &'a Tab,
    compact: bool,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let mut current_font = font;
    current_font.weight = Weight::Semibold;
    let size = appearance.typography().body_size.px();
    let path = tab.path();
    let within_home = path.starts_with(&desktop.home);
    let mut ancestors: Vec<_> = path
        .ancestors()
        .take_while(|ancestor| !within_home || ancestor.starts_with(&desktop.home))
        .map(|ancestor| {
            let name = if ancestor == desktop.home {
                "Home".to_owned()
            } else {
                ancestor
                    .file_name()
                    .unwrap_or(ancestor.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            };
            (ancestor, name)
        })
        .collect();
    let current = ancestors.remove(0).1;
    let current_width = chrome::text_width(&current, current_font, size);
    let ancestors: Vec<_> = ancestors
        .into_iter()
        .map(|(path, label)| {
            let width = chrome::text_width(&label, font, size);
            (path, label, width)
        })
        .collect();

    let input_style = appearance.style(UiKind::Input);
    let mut padding = canopa_ui::content_padding(input_style);
    let button_inset = if compact { 3.0 } else { 6.0 };
    if compact && !input_style.custom_padding {
        padding.left -= 6.0;
        padding.right -= 6.0;
    }
    let crumbs = responsive(move |bounds| {
        let origin_width = if compact { 0.0 } else { 34.0 };
        let mut available = bounds.width - current_width - origin_width - 2.0 * button_inset;
        let count = ancestors
            .iter()
            .take_while(|(_, _, width)| {
                available -= width + 20.0;
                available >= 0.0
            })
            .count();
        let origin_icon = desktop.icons.chrome(
            if within_home {
                Icon::Home
            } else {
                Icon::Folder
            },
            18.0,
            0.87,
        );
        let origin: Element<'_, Message> = if within_home && count < ancestors.len() {
            let message = Message::Go(id, desktop.home.clone());
            let home = crumb_button(origin_icon, message.clone()).padding([4, 2]);
            let home: Element<'_, Message> =
                if tab.search.is_some() || desktop.action_menu.is_some() {
                    home.into()
                } else {
                    tooltip(
                        home,
                        text("Home").font(font).size(size),
                        tooltip::Position::Bottom,
                    )
                    .into()
                };
            a11y::named_button("files-crumb-home", "Home", Some(message), home)
        } else {
            container(origin_icon).padding([0, 2]).into()
        };
        let mut trail = row![].align_y(Alignment::Center);
        // When space is tight, keep a clickable parent instead of using
        // the same room for a Home icon. Places still exposes Home.
        if !compact || count == 0 {
            trail = trail.push(origin);
            if within_home && count != 0 && count < ancestors.len() {
                trail = trail.push(desktop.icons.chrome(Icon::Forward, 12.0, 0.82));
            }
        }
        for (path, label, _) in ancestors[..count].iter().rev() {
            let message = Message::Go(id, (*path).to_owned());
            trail = trail
                .push(a11y::named_button(
                    format!("files-crumb-{}", a11y::path_key(path)),
                    label.clone(),
                    Some(message.clone()),
                    crumb_button(
                        text(label.clone())
                            .font(font)
                            .size(size)
                            .shaping(text::Shaping::Advanced)
                            .color(chrome::ink(appearance, ColorRole::Muted))
                            .wrapping(text::Wrapping::None),
                        message,
                    )
                    .padding([4, 4]),
                ))
                .push(desktop.icons.chrome(Icon::Forward, 12.0, 0.82));
        }
        let edit = Message::Command(id, Command::Location);
        trail
            .push(semantic(
                Node::button(current.clone())
                    .key("files-crumb-current")
                    .description("Current folder. Activate to edit the location")
                    .on_click(Some(edit.clone())),
                crumb_button(
                    chrome::single_line(current.clone(), current_font, size, None, false),
                    edit,
                )
                .padding([4.0, button_inset])
                .width(Length::Fill),
            ))
            .into()
    })
    .height(Length::Shrink);

    let edit = crumb_button(
        desktop.icons.chrome(Icon::Edit, 18.0, 0.93),
        Message::Command(id, Command::Location),
    )
    .padding([4.0, button_inset]);
    let edit: Element<'_, Message> = if tab.search.is_some() || desktop.action_menu.is_some() {
        edit.into()
    } else {
        tooltip(
            edit,
            text("Edit location")
                .font(font)
                .size(appearance.typography().label_size.px()),
            tooltip::Position::Bottom,
        )
        .into()
    };
    let edit = a11y::named_button(
        "files-location-edit",
        "Edit location",
        Some(Message::Command(id, Command::Location)),
        edit,
    );
    container(row![crumbs, edit].spacing(2).align_y(Alignment::Center))
        .width(Length::Fill)
        .height((size * 1.3).ceil() + padding.top + padding.bottom)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 0.0,
            right: padding.right,
            bottom: 0.0,
            left: padding.left,
        })
        .clip(true)
        .style(move |theme| {
            let input = view::input_style(appearance, theme, text_input::Status::Active);
            container::Style::default()
                .background(input.background)
                .border(input.border)
        })
        .into()
}

fn crumb_button<'a>(
    content: impl Into<Element<'a, Message>>,
    message: Message,
) -> button::Button<'a, Message> {
    button(content)
        .padding([4, 6])
        .on_press(message)
        .style(|theme, status| {
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            style.border.radius = 6.0.into();
            style
        })
}

fn control<'a>(
    desktop: &'a Desktop,
    tab: &'a Tab,
    source: &Option<Arc<actions_ui::Source>>,
    item: &Item<'a>,
    labels: Labels,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let (title, content, message) = match item {
        Item::Location => unreachable!("location fills its own layout slot"),
        Item::Builtin(command) => (
            command.label(),
            command.label(),
            source
                .as_ref()
                .filter(|_| enabled(desktop, *command, tab))
                .map(|source| {
                    Message::Action(actions_ui::Event::Builtin(source.clone(), *command))
                }),
        ),
        Item::Action(action) => (
            action.name.as_str(),
            action.name.as_str(),
            source
                .as_ref()
                .filter(|source| desktop.action_applicable(source, action))
                .map(|source| {
                    Message::Action(actions_ui::Event::Invoke(source.clone(), action.id.clone()))
                }),
        ),
    };
    let font = canopa_ui::font(appearance);
    let key = match item {
        Item::Builtin(command) => format!("files-toolbar-{}", command.id()),
        Item::Action(action) => format!("files-toolbar-action-{}", action.id),
        Item::Location => unreachable!("location fills its own layout slot"),
    };
    let node = Node::button(title)
        .key(key)
        .disabled(message.is_none())
        .on_click(message.clone());
    let content: Element<'_, Message> = match item {
        Item::Builtin(command) if command_icon(*command).is_some() => {
            let (icon, label) = command_icon(*command).unwrap();
            let icon = desktop.icons.chrome(
                icon,
                COMMAND_ICON,
                if message.is_some() { 1.0 } else { 0.48 },
            );
            let label = action_label(*command, label, labels);
            if label.is_empty() {
                icon
            } else {
                row![
                    icon,
                    text(label)
                        .size(appearance.typography().label_size.px())
                        .font(font)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
            }
        }
        Item::Action(action) => desktop.icons.label(
            action.icon.as_deref(),
            content,
            appearance.typography().label_size.px(),
            font,
            message.is_some(),
        ),
        _ => text(content)
            .size(appearance.typography().label_size.px())
            .font(font)
            .wrapping(text::Wrapping::None)
            .into(),
    };
    let search = matches!(item, Item::Builtin(Command::Search));
    let outlined_search = search && (tab.search.is_some() || matches!(labels, Labels::Full));
    let button = canopa_ui::styled_action(
        container(content).width(Length::Fill).clip(true),
        message,
        if outlined_search {
            canopa_ui::Action::Normal
        } else {
            canopa_ui::Action::Quiet
        },
        false,
        appearance,
    )
    .padding(control_padding(item, appearance, labels))
    .width(width(item, appearance, labels));
    let button: Element<'_, Message> = if navigation(item) {
        container(button)
            .style(move |_| chrome::highlight(appearance, ColorRole::Spine))
            .into()
    } else {
        button.into()
    };
    if tab.search.is_some()
        || tab.location.is_some()
        || desktop.action_menu.as_ref().is_some_and(|menu| {
            source
                .as_ref()
                .is_some_and(|source| source.window == menu.window())
        })
    {
        return semantic(node, button);
    }
    semantic(
        node,
        tooltip(
            button,
            text(title)
                .font(font)
                .size(appearance.typography().label_size.px()),
            tooltip::Position::Bottom,
        ),
    )
}

pub(super) fn view<'a>(desktop: &'a Desktop, id: window::Id, tab: &'a Tab) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let items = items(desktop);
    let source = desktop.action_source(id);
    let button_style = appearance.style(UiKind::Button);
    let mut menu_padding = canopa_ui::content_padding(button_style);
    if !button_style.custom_padding {
        menu_padding.left -= 6.0;
        menu_padding.right -= 6.0;
    }
    let menu_width = 18.0 + menu_padding.left + menu_padding.right;
    let open_menu = source
        .as_ref()
        .map(|_| Message::Action(actions_ui::Event::Menu(id)));
    let menu = canopa_ui::styled_action(
        desktop.icons.chrome(Icon::More, 18.0, 1.0),
        open_menu.clone(),
        canopa_ui::Action::Normal,
        false,
        appearance,
    )
    .padding(menu_padding)
    .width(menu_width);
    let menu: Element<'_, Message> =
        if tab.location.is_some() || tab.search.is_some() || desktop.action_menu.is_some() {
            menu.into()
        } else {
            tooltip(
                menu,
                text("Actions").size(appearance.typography().label_size.px()),
                tooltip::Position::Bottom,
            )
            .into()
        };
    let menu = semantic(
        Node::button("Actions")
            .key("files-toolbar-menu")
            .disabled(open_menu.is_none())
            .on_click(open_menu),
        menu,
    );
    let window_width = desktop.windows.get(&id).map_or(0.0, |window| window.width);
    let show_places = !sidebar_visible(desktop.configuration.settings.sidebar, window_width);
    let (menu, menu_width) = if show_places {
        let mut padding = canopa_ui::content_padding(button_style);
        if !button_style.custom_padding {
            padding.left -= 5.0;
            padding.right -= 5.0;
        }
        let font = canopa_ui::font(appearance);
        let size = appearance.typography().label_size.px();
        let places_width =
            chrome::text_width("Places", font, size) + padding.left + padding.right + 24.0;
        let open_places = source
            .as_ref()
            .map(|_| Message::Action(actions_ui::Event::Locations(id)));
        let places = canopa_ui::styled_action(
            row![
                desktop.icons.chrome(
                    Icon::Places,
                    18.0,
                    if source.is_some() { 1.0 } else { 0.45 }
                ),
                text("Places").size(size).font(font)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            open_places.clone(),
            canopa_ui::Action::Normal,
            false,
            appearance,
        )
        .padding(padding)
        .width(places_width);
        let places = a11y::button("files-toolbar-places", open_places, places);
        (
            row![places, menu]
                .spacing(4)
                .align_y(Alignment::Center)
                .into(),
            places_width + 4.0 + menu_width,
        )
    } else {
        (menu, menu_width)
    };
    let (menu, menu_width) = if let Some(browser) = &tab.browser
        && browser.search().is_none()
        && window_width >= super::view::TOOLBAR_MODES_WIDTH
    {
        let (width, modes) =
            super::view::view_switcher(desktop, id, tab, browser.view().mode, true, true);
        (
            row![modes, menu]
                .spacing(6)
                .align_y(Alignment::Center)
                .into(),
            width + 6.0 + menu_width,
        )
    } else {
        (menu, menu_width)
    };
    let side_padding = if show_places { 10.0 } else { SIDE_PADDING };
    let available = window_width
        - 2.0 * side_padding
        - if show_places {
            0.0
        } else {
            chrome::SIDEBAR_WIDTH + 1.0
        };
    let required = |items: &[Item<'_>], labels| {
        let commands = items
            .iter()
            .filter(|item| !matches!(item, Item::Location))
            .map(|item| width(item, appearance, labels))
            .sum();
        super::toolbar_layout::required(commands, menu_width, items.len())
    };
    let labels = if available < required(&items, Labels::Full) + 68.0
        && available >= required(&items, Labels::Icons)
    {
        Labels::Icons
    } else {
        Labels::Full
    };
    let primary_items = items
        .iter()
        .map(|item| {
            (
                width(item, appearance, labels),
                match item {
                    Item::Location => None,
                    _ => Some(control(desktop, tab, &source, item, labels)),
                },
            )
        })
        .collect();
    let mut body = column![super::toolbar_layout::primary(
        location(desktop, id, tab, matches!(labels, Labels::Icons)),
        menu,
        menu_width,
        primary_items
    )]
    .spacing(4);
    let mut additional = row![].spacing(6).align_y(Alignment::Center);
    let mut has_additional = false;
    for action in &desktop.configuration.actions {
        if action.toolbar
            && !items
                .iter()
                .any(|item| matches!(item, Item::Action(existing) if existing.id == action.id))
        {
            additional = additional.push(control(
                desktop,
                tab,
                &source,
                &Item::Action(action),
                Labels::Full,
            ));
            has_additional = true;
        }
    }
    if has_additional {
        body = body.push(horizontal(additional.into()));
    }
    container(body)
        .padding([10.0, side_padding])
        .width(Length::Fill)
        .into()
}

fn horizontal(content: Element<'_, Message>) -> Element<'_, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill)
        .into()
}
