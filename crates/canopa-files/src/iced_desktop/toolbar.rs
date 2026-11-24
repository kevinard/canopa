//! Configuration order is preserved across both toolbar arrangements. A narrow
//! tile gets a full-width location field and a scrollable command strip.
use super::accessibility_ui::{self as a11y, Node, Role, semantic};
use super::{chrome::Icon, *};
use crate::actions::CustomAction;
use canopa_theme::{ColorRole, UiKind};
use iced::{
    Alignment, Length,
    widget::{column, container, row, scrollable, text, text_input, tooltip},
};

const SIDE_PADDING: f32 = 14.0;
/// The toolbar item showing the current view; it opens the view menu.
pub(super) const VIEW_ITEM: &str = "view";
const COMMAND_ICON: f32 = 20.0;

enum Item<'a> {
    Location,
    /// The current view, opening the view menu.
    View,
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
            } else if id == VIEW_ITEM {
                Some(Item::View)
            } else if let Some(id) = id.strip_prefix("action:") {
                if desktop.picker.is_some() {
                    return None;
                }
                desktop
                    .configuration
                    .actions
                    .iter()
                    .find(|action| action.id == id)
                    .map(Item::Action)
            } else {
                Command::from_id(id)
                    .filter(|command| {
                        !command.never_on_toolbar()
                            && desktop
                                .picker
                                .as_ref()
                                .is_none_or(|picker| picker.allows(*command))
                    })
                    .map(Item::Builtin)
            }
        })
        .collect()
}

/// The icon of a toolbar command and the short caption beside it, if any.
pub(super) fn command_icon(command: Command) -> Option<(Icon, String)> {
    Some(match command {
        Command::Back => (Icon::Back, String::new()),
        Command::Forward => (Icon::Forward, String::new()),
        Command::Parent => (Icon::Up, String::new()),
        Command::Search => (Icon::Search, fl!("files-toolbar-search")),
        Command::NewFolder => (Icon::NewFolder, fl!("files-menu-new-folder-short")),
        Command::OpenTerminal => (Icon::Terminal, fl!("files-toolbar-terminal")),
        Command::Columns => (Icon::Columns, String::new()),
        Command::List => (Icon::List, String::new()),
        Command::Icons => (Icon::Grid, String::new()),
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

fn action_label(command: Command, full: String, labels: Labels) -> String {
    match (command, labels) {
        (Command::Search | Command::NewFolder, Labels::Icons) => String::new(),
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
    let size = appearance.typography().label_size.px() + 1.0;
    match item {
        Item::Location => 144.0,
        Item::View => view_item_width(padding),
        Item::Builtin(command) => match command_icon(*command) {
            Some((_, label)) => {
                let label = action_label(*command, label, labels);
                let text = if label.is_empty() {
                    0.0
                } else {
                    6.0 + chrome::text_width(&label, canopa_ui::font(appearance), size)
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
        Command::NewFolder => desktop.picker.is_some() || desktop.file_operations.ready(),
        Command::Trash | Command::DeletePermanently => {
            desktop.file_operations.ready() && !browser.selected().is_empty()
        }
        Command::Paste => desktop.file_operations.ready() && desktop.file_clipboard.can_paste(),
        Command::Recovery => desktop.file_operations.service.is_some(),
        Command::Rename => {
            desktop.file_operations.ready()
                && (browser.selected().len() == 1
                    || super::name_dialog::batch_folder(
                        &browser.selected().iter().cloned().collect::<Vec<_>>(),
                    )
                    .is_some())
        }
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
        | Command::MoveTabLeft
        | Command::MoveTabRight
        | Command::MoveTabToWindow
        | Command::CloseTab
        | Command::Search
        | Command::Location
        | Command::Refresh
        | Command::ShowHidden
        | Command::SelectAll
        | Command::NewWindow
        | Command::OpenTerminal
        | Command::NewTab
        | Command::CustomActions
        | Command::Preferences
        | Command::Properties
        | Command::BookmarkFolder
        | Command::Operations
        | Command::ShowTrash
        | Command::ShowRecent
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
        let node = Node::text_input(fl!("files-location"), value.clone())
            .key("files-location")
            .on_text(move |value| Message::Location(id, value));
        let placeholder = fl!("files-location");
        let input = text_input(&placeholder, value)
            .id(view::location_id(id))
            .on_input(move |value| Message::Location(id, value))
            .on_submit(Message::SubmitLocation(id))
            .size(appearance.typography().body_size.px())
            .font(font)
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| view::input_style(&appearance, theme, status))
            .width(Length::Fill);
        semantic(node, input)
    } else if let Some(session) = &tab.search {
        // A search takes the path's place, so there is one field to type in.
        view::search_parts(desktop, id, tab, session).field
    } else {
        semantic(
            Node::new(Role::Navigation)
                .label(fl!("files-location"))
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
    use crate::browser_ui::breadcrumb;

    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let show_tooltips = tab.search.is_none() && desktop.action_menu.is_none();
    let search = Message::Command(id, Command::Search);
    let inset = if compact { 3.0 } else { 6.0 };
    let button = breadcrumb::crumb_button(
        desktop.icons.chrome(Icon::Search, 17.0, 0.93),
        Some(search.clone()),
    )
    .padding([4.0, inset]);
    let control: Element<'_, Message> = if show_tooltips {
        tooltip(
            button,
            text(Command::Search.label())
                .font(font)
                .size(appearance.typography().label_size.px()),
            tooltip::Position::Bottom,
        )
        .padding(6)
        .style(chrome::tooltip_style)
        .into()
    } else {
        button.into()
    };
    let search = a11y::named_button(
        "files-search-open",
        Command::Search.label(),
        Some(search),
        control,
    );
    breadcrumb::view(
        breadcrumb::Location {
            appearance,
            path: tab.path(),
            home: &desktop.home,
            home_label: fl!("files-place-home"),
            key_prefix: "files-crumb",
            edit_description: fl!("files-crumb-current-description"),
            edit: Some(Message::Command(id, Command::Location)),
            compact,
            show_tooltips,
        },
        |icon, size, opacity| desktop.icons.chrome(icon, size, opacity),
        move |path| Some(Message::Go(id, path.to_owned())),
        Some(search),
    )
}

fn control<'a>(
    desktop: &'a Desktop,
    tab: &'a Tab,
    source: &Option<Arc<actions_ui::Source>>,
    item: &Item<'a>,
    labels: Labels,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    if matches!(item, Item::View) {
        return view_item(desktop, tab, source, labels);
    }
    let (title, content, message) = match item {
        Item::Location | Item::View => unreachable!("handled above"),
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
            action.name.clone(),
            action.name.clone(),
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
        Item::Location | Item::View => unreachable!("handled above"),
    };
    let node = Node::button(title.clone())
        .key(key)
        .disabled(message.is_none())
        .on_click(message.clone());
    let content: Element<'_, Message> = match item {
        Item::Builtin(command) if command_icon(*command).is_some() => {
            let (icon, label) = command_icon(*command).expect("checked above");
            let icon = desktop.icons.chrome(
                icon,
                COMMAND_ICON,
                if message.is_some() { 1.0 } else { 0.48 },
            );
            let label = action_label(*command, label, labels);
            if label.is_empty() {
                icon
            } else {
                canopa_ui::direction::row([
                    icon,
                    // Command labels match the view switcher's caption size.
                    text(label)
                        .size(appearance.typography().label_size.px() + 1.0)
                        .font(font)
                        .into(),
                ])
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
            }
        }
        Item::Action(action) => desktop.icons.label(
            action.icon.as_deref(),
            &action.name,
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
    let button = canopa_ui::styled_action(
        container(content).width(Length::Fill).clip(true),
        message,
        // Search keeps its outline only while a search is open; idle secondary
        // controls stay borderless so the location field leads the toolbar.
        if search && tab.search.is_some() {
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
        // History controls without a destination drop their tile, so the
        // available ones read as buttons at a glance.
        // History controls are borderless like the other secondary actions;
        // hover fills them and an unavailable one only dims its glyph.
        container(button)
            .style(move |_| {
                if appearance.high_contrast() {
                    chrome::highlight(appearance, ColorRole::Spine)
                        .border(iced::Border::default().rounded(10))
                } else {
                    container::Style::default()
                }
            })
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
        )
        .padding(6)
        .style(super::chrome::tooltip_style),
    )
}

pub(super) fn view<'a>(desktop: &'a Desktop, id: window::Id, tab: &'a Tab) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut items = items(desktop);
    if desktop
        .picker
        .as_ref()
        .is_some_and(|picker| picker.allows(Command::NewFolder))
        && !items
            .iter()
            .any(|item| matches!(item, Item::Builtin(Command::NewFolder)))
    {
        items.push(Item::Builtin(Command::NewFolder));
    }
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
        canopa_ui::Action::Quiet,
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
                text(fl!("files-toolbar-actions")).size(appearance.typography().label_size.px()),
                tooltip::Position::Bottom,
            )
            .padding(6)
            .style(super::chrome::tooltip_style)
            .into()
        };
    let menu = semantic(
        Node::button(fl!("files-toolbar-actions"))
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
        let places_width = chrome::text_width(&fl!("files-toolbar-places"), font, size)
            + padding.left
            + padding.right
            + 24.0;
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
                text(fl!("files-toolbar-places")).size(size).font(font)
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
    let side_padding = if show_places { 10.0 } else { SIDE_PADDING };
    let available = window_width - 2.0 * side_padding;
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
        if desktop.picker.is_none()
            && action.toolbar
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
        .style(canopa_ui::scrollbar)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill)
        .into()
}

/// The view item's width: its symbol and the menu's chevron.
fn view_item_width(padding: iced::Padding) -> f32 {
    (COMMAND_ICON + 6.0 + 12.0 + padding.left + padding.right).ceil()
}

/// The current view's symbol with a chevron; it opens the view menu below it.
/// The view's name is its tooltip and accessible name, not a caption.
fn view_item<'a>(
    desktop: &'a Desktop,
    tab: &'a Tab,
    source: &Option<Arc<actions_ui::Source>>,
    labels: Labels,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let mode = tab
        .browser
        .as_ref()
        .map(|browser| browser.view().mode)
        .unwrap_or_default();
    let message = source
        .as_ref()
        .filter(|_| !tab.loading)
        .map(|source| Message::Action(actions_ui::Event::ToolbarViewMenu(source.window)));
    let expanded = desktop.action_menu.as_ref().is_some_and(|menu| {
        source
            .as_ref()
            .is_some_and(|source| menu.window() == source.window && menu.is_view_menu())
    });
    let content = row![
        desktop.icons.chrome(
            super::view::view_icon(mode),
            COMMAND_ICON,
            if message.is_some() { 1.0 } else { 0.48 },
        ),
        desktop.icons.chrome(Icon::Down, 12.0, 0.7),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let label = fl!("files-view-current", view = super::view::view_label(mode));
    let button = canopa_ui::styled_action(
        content,
        message.clone(),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .padding(control_padding(&Item::View, appearance, labels))
    .style({
        let appearance = appearance.clone();
        move |theme, status| {
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            if expanded {
                style.background = Some(chrome::selected_fill(&appearance).into());
            }
            style
        }
    });
    let button: Element<'_, Message> = if expanded || tab.location.is_some() {
        button.into()
    } else {
        tooltip(
            button,
            text(label.clone())
                .font(font)
                .size(appearance.typography().label_size.px()),
            tooltip::Position::Bottom,
        )
        .padding(6)
        .style(super::chrome::tooltip_style)
        .into()
    };
    semantic(
        Node::button(label)
            .key("files-toolbar-view")
            .expanded(expanded)
            .disabled(message.is_none())
            .on_click(message),
        button,
    )
}
