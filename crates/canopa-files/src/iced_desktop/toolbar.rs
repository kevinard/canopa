//! Configuration order is preserved across both toolbar arrangements. A narrow
//! tile gets a full-width location field and a scrollable command strip.
use super::*;
use crate::actions::CustomAction;
use canopa_theme::UiKind;
use iced::{
    Alignment, Length,
    widget::{column, container, row, scrollable, text, text_input, tooltip},
};

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
                Command::from_id(id).map(Item::Builtin)
            }
        })
        .collect()
}

fn label(command: Command) -> &'static str {
    match command {
        Command::Back => "‹",
        Command::Forward => "›",
        Command::Parent => "↑",
        Command::QuickLook => "Look",
        Command::Search => "Search",
        _ => command.label(),
    }
}

fn width(item: &Item<'_>, appearance: &canopa_theme::Appearance) -> f32 {
    let padding = canopa_ui::content_padding(appearance.style(UiKind::Button));
    let glyphs = match item {
        Item::Location => return 144.0,
        Item::Builtin(Command::Back | Command::Forward | Command::Parent) => 1.5,
        Item::Builtin(Command::QuickLook | Command::Search) => 5.0,
        _ => 12.0,
    };
    let icon = if matches!(item, Item::Action(action) if action.icon.is_some()) {
        24.0
    } else {
        0.0
    };
    (glyphs * appearance.typography().label_size.px() * 0.7 + padding.left + padding.right + icon)
        .ceil()
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
        Command::Trash => desktop.file_operations.ready() && !browser.selected().is_empty(),
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
        Command::QuickLook | Command::Open => browser.focused().is_some(),
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

fn location<'a>(desktop: &'a Desktop, id: window::Id, tab: &'a Tab) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    if let Some(value) = &tab.location {
        let appearance = appearance.clone();
        text_input("Location", value)
            .id(view::location_id(id))
            .on_input(move |value| Message::Location(id, value))
            .on_submit(Message::SubmitLocation(id))
            .size(appearance.typography().body_size.px())
            .font(font)
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| view::input_style(&appearance, theme, status))
            .width(Length::Fill)
            .into()
    } else {
        canopa_ui::styled_action(
            container(
                text(display_location(tab.path()))
                    .font(font)
                    .size(appearance.typography().body_size.px())
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            Some(Message::Command(id, Command::Location)),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(Length::Fill)
        .into()
    }
}

fn control<'a>(
    desktop: &'a Desktop,
    tab: &'a Tab,
    source: &Option<Arc<actions_ui::Source>>,
    item: &Item<'a>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let (title, content, message) = match item {
        Item::Location => unreachable!("location fills its own layout slot"),
        Item::Builtin(command) => (
            command.label(),
            label(*command),
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
    let icon = match item {
        Item::Action(action) => action.icon.as_deref(),
        _ => None,
    };
    let content = desktop.icons.label(
        icon,
        content,
        appearance.typography().label_size.px(),
        font,
        message.is_some(),
    );
    let button = canopa_ui::styled_action(
        container(content).width(Length::Fill).clip(true),
        message,
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .width(width(item, appearance));
    if tab.search.is_some()
        || tab.location.is_some()
        || desktop.action_menu.as_ref().is_some_and(|menu| {
            source
                .as_ref()
                .is_some_and(|source| source.window == menu.window())
        })
    {
        return button.into();
    }
    tooltip(
        button,
        text(title)
            .font(font)
            .size(appearance.typography().label_size.px()),
        tooltip::Position::Bottom,
    )
    .into()
}

pub(super) fn view<'a>(desktop: &'a Desktop, id: window::Id, tab: &'a Tab) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let items = items(desktop);
    let source = desktop.action_source(id);
    let menu_width = width(&Item::Builtin(Command::Back), appearance);
    let menu = canopa_ui::styled_action(
        text("⋮").size(19).font(canopa_ui::font(appearance)),
        source
            .as_ref()
            .map(|_| Message::Action(actions_ui::Event::Menu(id))),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .width(menu_width);
    let primary_items = items
        .iter()
        .map(|item| {
            (
                width(item, appearance),
                match item {
                    Item::Location => None,
                    _ => Some(control(desktop, tab, &source, item)),
                },
            )
        })
        .collect();
    let mut body = column![super::toolbar_layout::primary(
        location(desktop, id, tab),
        menu.into(),
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
            additional = additional.push(control(desktop, tab, &source, &Item::Action(action)));
            has_additional = true;
        }
    }
    if has_additional {
        body = body.push(horizontal(additional.into()));
    }
    container(body).padding([8, 10]).into()
}

fn horizontal(content: Element<'_, Message>) -> Element<'_, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill)
        .into()
}
