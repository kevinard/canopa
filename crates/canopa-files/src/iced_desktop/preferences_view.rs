//! Shared Canopa form controls with synchronous traversal and stable notice slots.
use super::preferences_editor::{Change, Confirmation, Editor, Event, Field, Location, Page};
use super::*;
use crate::{
    interface::PlaceKind,
    preferences::{Detail, SortField, ViewMode},
};
use canopa_theme::{Appearance, UiKind};
use iced::{
    Length,
    widget::{checkbox, column, container, row, scrollable, space, text, text_input},
};

struct Form<'a> {
    appearance: &'a Appearance,
    editor: &'a Editor,
    order: Vec<iced::widget::Id>,
    reveal: Vec<(iced::widget::Id, iced::widget::Id)>,
}
impl<'a> Form<'a> {
    fn register(&mut self, name: &str, in_form: bool, enabled: bool) -> iced::widget::Id {
        let id = self.editor.focus_id(name);
        if enabled {
            self.order.push(id.clone());
            if in_form {
                self.reveal
                    .push((id.clone(), self.editor.focus_id("scroll")));
            }
        }
        id
    }
    fn button(
        &mut self,
        label: impl Into<String>,
        name: &str,
        event: Option<Event>,
        active: bool,
        in_form: bool,
    ) -> Element<'a, Message> {
        let id = self.register(name, in_form, event.is_some());
        let keyboard_message = event.as_ref().map(|event| match event {
            Event::Change(target, Change::Record(false)) => {
                Message::Preferences(Event::Change(*target, Change::Record(true)))
            }
            other => Message::Preferences(other.clone()),
        });
        let message = event.map(Message::Preferences);
        focus::control_with_keyboard(
            id,
            message.clone(),
            keyboard_message,
            self.appearance,
            canopa_ui::styled_action(
                text(label.into())
                    .font(canopa_ui::font(self.appearance))
                    .size(self.appearance.typography().label_size.px()),
                message,
                if active {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                false,
                self.appearance,
            ),
        )
    }
    fn change(
        &mut self,
        label: impl Into<String>,
        name: &str,
        change: Change,
        active: bool,
        available: bool,
    ) -> Element<'a, Message> {
        let event = (self.editor.enabled() && available)
            .then_some(Event::Change(self.editor.target(), change));
        self.button(label, name, event, active, true)
    }
    fn field(
        &mut self,
        label: &str,
        name: &str,
        value: &str,
        which: Field,
    ) -> Element<'a, Message> {
        let enabled = self.editor.enabled();
        let id = self.register(name, true, enabled);
        let appearance = self.appearance.clone();
        let target = self.editor.target();
        let mut input = text_input("", value)
            .id(id)
            .font(canopa_ui::font(&appearance))
            .size(appearance.typography().body_size.px())
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| view::input_style(&appearance, theme, status));
        if enabled {
            input = input.on_input(move |v| {
                Message::Preferences(Event::Change(target, Change::Field(which, v)))
            });
        }
        column![self.note(label), input].spacing(4).into()
    }
    fn note(&self, label: impl Into<String>) -> Element<'a, Message> {
        text(label.into())
            .font(canopa_ui::font(self.appearance))
            .size(self.appearance.typography().label_size.px())
            .into()
    }
    fn toggle(
        &mut self,
        label: &str,
        name: &str,
        value: bool,
        change: Change,
    ) -> Element<'a, Message> {
        let enabled = self.editor.enabled();
        let id = self.register(name, true, enabled);
        let event = Event::Change(self.editor.target(), change);
        let mut check = checkbox(value)
            .label(label.to_owned())
            .font(canopa_ui::font(self.appearance))
            .text_size(self.appearance.typography().body_size.px());
        if enabled {
            let event = event.clone();
            check = check.on_toggle(move |_| Message::Preferences(event.clone()));
        }
        focus::control(
            id,
            enabled.then_some(Message::Preferences(event)),
            self.appearance,
            container(check).padding(2),
        )
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, editor: &'a Editor) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut form = Form {
        appearance,
        editor,
        order: vec![],
        reveal: vec![],
    };
    let target = editor.target();
    let id = editor.window;
    let mut tabs = row![].spacing(4);
    for page in Page::ALL {
        tabs = tabs.push(form.button(
            page.label(),
            &format!("page-{page:?}"),
            editor.enabled().then_some(Event::Page(target, page)),
            editor.page == page,
            false,
        ));
    }
    let notice = if !editor.base.is_editable() {
        "Built-in defaults are read-only. Start Files with a writable configuration to save preferences.".into()
    } else if let Some(error) = &desktop.configuration_error {
        format!("Configuration was not applied: {error}")
    } else if !editor.base.same_source(&desktop.configuration) {
        "Configuration changed outside this editor. Your draft is kept; Reload before saving."
            .into()
    } else {
        String::new()
    };
    let mut body = column![tabs.wrap(), container(form.note(notice))].spacing(8);
    // The prompt has its own fixed slot, so source/error notices never reparent fields.
    let prompt: Element<'a, Message> = if let Some(confirmation) = editor.confirmation {
        column![
            form.note(if confirmation == Confirmation::Close {
                "Discard unsaved preference changes?"
            } else {
                "Reload and discard unsaved preference changes?"
            }),
            row![
                form.button(
                    "Keep editing",
                    "keep",
                    Some(Event::Confirm(id, false)),
                    false,
                    false
                ),
                form.button(
                    if confirmation == Confirmation::Close {
                        "Discard"
                    } else {
                        "Reload"
                    },
                    "discard",
                    Some(Event::Confirm(id, true)),
                    false,
                    false
                )
            ]
            .spacing(8)
        ]
        .spacing(8)
        .into()
    } else {
        space().height(0).into()
    };
    body = body.push(container(prompt));
    let page = match editor.page {
        Page::Views => views(&mut form),
        Page::Toolbar => toolbar(&mut form),
        Page::Locations => locations(&mut form),
        Page::Shortcuts => shortcuts(&mut form),
    };
    body = body.push(
        scrollable(container(page).padding(iced::Padding {
            right: 12.0,
            ..Default::default()
        }))
        .id(editor.focus_id("scroll"))
        .height(Length::Fill),
    );
    body = body.push(form.note(&editor.status));
    body = body.push(
        row![
            form.button(
                "Reload",
                "reload",
                editor.enabled().then_some(Event::Reload(id)),
                false,
                false
            ),
            form.button(
                "Save preferences",
                "save",
                (editor.enabled() && editor.base.is_editable()).then_some(Event::Save(id)),
                false,
                false
            )
        ]
        .spacing(8),
    );
    body = body.push(form.note("Alt+1–4 pages · Ctrl+S Save · Ctrl+R Reload"));
    let default = editor.confirmation.map(|_| editor.focus_id("keep"));
    let appearance = appearance.clone();
    let content = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&appearance, appearance.style(UiKind::Panel)));
    let shortcuts = if editor.enabled() {
        Page::ALL
            .iter()
            .enumerate()
            .map(|(index, page)| {
                (
                    KeyChord::parse(&format!("<Alt>{}", index + 1)).unwrap(),
                    Message::Preferences(Event::Page(editor.target(), *page)),
                )
            })
            .collect()
    } else {
        vec![]
    };
    focus::group_with_shortcuts(content, form.order, form.reveal, default, shortcuts)
}
fn views<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let v = &form.editor.draft.view;
    let mut modes = row![].spacing(4);
    for (mode, label) in [
        (ViewMode::Columns, "Columns"),
        (ViewMode::List, "List"),
        (ViewMode::Icons, "Icons"),
    ] {
        modes = modes.push(form.change(label, label, Change::Mode(mode), v.mode == mode, true));
    }
    let mut sorts = row![].spacing(4);
    for (sort, label) in [
        (SortField::Name, "Name"),
        (SortField::Size, "Size"),
        (SortField::Modified, "Modified"),
        (SortField::Kind, "Kind"),
    ] {
        sorts = sorts.push(form.change(
            label,
            &format!("sort-{label}"),
            Change::Sort(sort),
            v.sort == sort,
            true,
        ));
    }
    let mut body = column![
        form.note("Default view"),
        modes.wrap(),
        form.note("Sort by"),
        sorts.wrap(),
        form.toggle(
            "Descending order",
            "descending",
            v.descending,
            Change::Descending(!v.descending)
        ),
        form.field(
            "Column width (160–640)",
            "width",
            &form.editor.width,
            Field::Width
        ),
        form.note("Visible file details")
    ]
    .spacing(10);
    for (detail, label) in [
        (Detail::Size, "Size"),
        (Detail::Modified, "Modified date"),
        (Detail::Kind, "Kind"),
    ] {
        let checked = v.details.contains(&detail);
        body = body.push(form.toggle(
            label,
            &format!("detail-{detail:?}"),
            checked,
            Change::Detail(detail, !checked),
        ));
    }
    body.push(form.note("Folders inherit these defaults except for choices explicitly saved for that folder. Use defaults in View options clears a folder’s choices.")).into()
}
fn item_label(editor: &Editor, id: &str) -> String {
    if id == "location" {
        "Location field".into()
    } else if let Some(command) = Command::from_id(id) {
        command.label().into()
    } else {
        id.strip_prefix("action:")
            .and_then(|id| editor.base.actions.iter().find(|a| a.id == id))
            .map(|a| format!("Action: {}", a.name))
            .unwrap_or_else(|| id.to_owned())
    }
}
fn toolbar<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut body = column![form.note("Select a toolbar item to move or remove it.")].spacing(8);
    for (i, id) in editor.draft.toolbar.iter().enumerate() {
        body = body.push(form.change(
            item_label(editor, id),
            &format!("toolbar-{i}"),
            Change::SelectToolbar(i),
            editor.selected_toolbar == Some(i),
            true,
        ));
    }
    let index = editor.selected_toolbar;
    body = body.push(
        row![
            form.change(
                "Earlier",
                "earlier",
                Change::MoveToolbar(-1),
                false,
                index.is_some_and(|i| i > 0)
            ),
            form.change(
                "Later",
                "later",
                Change::MoveToolbar(1),
                false,
                index.is_some_and(|i| i + 1 < editor.draft.toolbar.len())
            ),
            form.change(
                "Remove",
                "remove",
                Change::RemoveToolbar,
                false,
                index.is_some_and(|i| editor.draft.toolbar[i] != "location")
            )
        ]
        .spacing(4)
        .wrap(),
    );
    body = body.push(form.field(
        "Find a command to add",
        "filter",
        &editor.filter,
        Field::Filter,
    ));
    let filter = editor.filter.to_lowercase();
    let choices = Command::ALL.iter().map(|c| c.id().to_owned()).chain(
        editor
            .base
            .actions
            .iter()
            .map(|a| format!("action:{}", a.id)),
    );
    for id in choices.filter(|id| {
        !editor.draft.toolbar.contains(id)
            && item_label(editor, id).to_lowercase().contains(&filter)
    }) {
        let name = format!("add-{id}");
        body = body.push(form.change(
            format!("Add {}", item_label(editor, &id)),
            &name,
            Change::AddToolbar(id),
            false,
            editor.draft.toolbar.len() < 64,
        ));
    }
    body.push(form.note("The location field stays available. Explicitly placed custom actions appear once; unpositioned toolbar actions keep their separate row.")).into()
}
fn locations<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut sidebar = row![].spacing(4);
    for (mode, label) in [
        (SidebarMode::Auto, "Automatic"),
        (SidebarMode::Always, "When there is room"),
        (SidebarMode::Never, "Hidden"),
    ] {
        sidebar = sidebar.push(form.change(
            label,
            &format!("sidebar-{mode:?}"),
            Change::Sidebar(mode),
            editor.draft.sidebar == mode,
            true,
        ));
    }
    let mut body = column![
        form.note("Sidebar"),
        sidebar.wrap(),
        form.note("Locations and bookmarks")
    ]
    .spacing(8);
    for (i, place) in editor.locations.iter().enumerate() {
        body = body.push(form.change(
            place.label(),
            &format!("place-{i}"),
            Change::SelectPlace(i),
            editor.selected_place == Some(i),
            true,
        ));
    }
    let index = editor.selected_place;
    body = body.push(
        row![
            form.change(
                "Earlier",
                "earlier",
                Change::MovePlace(-1),
                false,
                index.is_some_and(|i| i > 0)
            ),
            form.change(
                "Later",
                "later",
                Change::MovePlace(1),
                false,
                index.is_some_and(|i| i + 1 < editor.locations.len())
            ),
            form.change(
                "Remove",
                "remove",
                Change::RemovePlace,
                false,
                index.is_some()
            ),
        ]
        .spacing(4)
        .wrap(),
    );
    if let Some(Location::Bookmark { name, path }) = index.and_then(|i| editor.locations.get(i)) {
        body = body
            .push(form.field("Bookmark name", "bookmark-name", name, Field::BookmarkName))
            .push(form.field(
                "Folder (absolute path, ~/ path or local file URI)",
                "bookmark-path",
                path,
                Field::BookmarkPath,
            ));
    }
    body = body.push(form.change(
        "Add bookmark…",
        "add-bookmark",
        Change::AddBookmark,
        false,
        editor.locations.len() < 256,
    ));
    for kind in PlaceKind::ALL.iter().filter(|kind| {
        !editor
            .locations
            .iter()
            .any(|p| matches!(p,Location::Builtin(k)if k==*kind))
    }) {
        body = body.push(form.change(
            format!("Add {}", kind.label()),
            &format!("add-{}", kind.id()),
            Change::AddPlace(*kind),
            false,
            editor.locations.len() < 256,
        ));
    }
    body.push(form.note("Missing folders remain bookmarked. Narrow tiles retain location navigation in the folder menu.")).into()
}
fn shortcuts<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut body =
        column![form.field("Find a command", "filter", &editor.filter, Field::Filter)].spacing(8);
    let filter = editor.filter.to_lowercase();
    for command in Command::ALL
        .iter()
        .filter(|c| c.label().to_lowercase().contains(&filter))
    {
        body = body.push(form.change(
            command.label(),
            &format!("command-{}", command.id()),
            Change::SelectCommand(*command),
            editor.selected_command == *command,
            true,
        ));
    }
    body = body.push(form.note(format!("{} shortcuts", editor.selected_command.label())));
    body = body.push(form.field(
        "Shortcuts (separated by ;, empty disables)",
        "shortcut",
        &editor.shortcuts[&editor.selected_command],
        Field::Shortcut,
    ));
    body = body.push(
        row![
            form.change("Record…", "record", Change::Record(false), false, true),
            form.change("Use defaults", "reset", Change::ResetShortcut, false, true)
        ]
        .spacing(8),
    );
    body.push(form.note("Selection, focus and typing keys stay available. Save checks conflicts with every command and custom action.")).into()
}
