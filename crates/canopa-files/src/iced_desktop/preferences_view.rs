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
    desktop: &'a Desktop,
    wide: bool,
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
        let primary = name == "save";
        let icon = match name {
            "page-Views" => Some(chrome::Icon::Columns),
            "page-Toolbar" => Some(chrome::Icon::Edit),
            "page-Locations" => Some(chrome::Icon::Places),
            "page-Shortcuts" => Some(chrome::Icon::Terminal),
            _ => None,
        };
        let tab = icon.is_some();
        let keyboard_message = event.as_ref().map(|event| match event {
            Event::Change(target, Change::Record(false)) => {
                Message::Preferences(Event::Change(*target, Change::Record(true)))
            }
            other => Message::Preferences(other.clone()),
        });
        let message = event.map(Message::Preferences);
        let label = text(label.into())
            .font(canopa_ui::font(self.appearance))
            .size(self.appearance.typography().label_size.px() + if tab { 1.0 } else { 0.0 });
        let label: Element<'a, Message> = if let Some(icon) = icon {
            canopa_ui::direction::row([self.desktop.icons.chrome(icon, 16.0, 0.9), label.into()])
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
        } else {
            label.into()
        };
        let button = canopa_ui::styled_action(
            label,
            message.clone(),
            if primary {
                canopa_ui::Action::Primary
            } else if name == "reload" {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            false,
            self.appearance,
        )
        .width(if tab && self.wide {
            Length::Fill
        } else {
            Length::Shrink
        });
        let content = container(button).style({
            let appearance = self.appearance;
            move |_| {
                if active && tab {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                } else if active && !primary {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface).border(
                        iced::Border {
                            width: 1.0,
                            color: chrome::ink(appearance, canopa_theme::ColorRole::Accent)
                                .scale_alpha(0.60),
                            radius: 8.0.into(),
                        },
                    )
                } else {
                    container::Style::default()
                }
            }
        });
        focus::button_with_keyboard(id, message, keyboard_message, self.appearance, content)
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
        let mut node = super::accessibility_ui::Node::new(super::accessibility_ui::Role::TextInput)
            .key(format!("files-preferences-{name}"))
            .value(value)
            .read_only(!enabled);
        if enabled {
            input = input.on_input(move |v| {
                Message::Preferences(Event::Change(target, Change::Field(which, v)))
            });
            node = node.on_text(move |v| {
                Message::Preferences(Event::Change(target, Change::Field(which, v)))
            });
        }
        // Named by the note drawn before it.
        column![
            self.note(label),
            super::accessibility_ui::semantic(node, input)
        ]
        .spacing(4)
        .into()
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
            .text_size(self.appearance.typography().body_size.px())
            .style(canopa_ui::checkbox_style);
        if enabled {
            let event = event.clone();
            check = check.on_toggle(move |_| Message::Preferences(event.clone()));
        }
        focus::check_box(
            id,
            value,
            enabled.then_some(Message::Preferences(event)),
            self.appearance,
            container(check).padding(2),
        )
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, editor: &'a Editor) -> Element<'a, Message> {
    iced::widget::responsive(move |size| contents(desktop, editor, size.width >= 620.0)).into()
}

fn contents<'a>(desktop: &'a Desktop, editor: &'a Editor, wide: bool) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut form = Form {
        desktop,
        wide,
        appearance,
        editor,
        order: vec![],
        reveal: vec![],
    };
    let target = editor.target();
    let id = editor.window;
    let mut tabs = Vec::new();
    for page in Page::ALL {
        tabs.push(form.button(
            page.label(),
            &format!("page-{page:?}"),
            editor.enabled().then_some(Event::Page(target, page)),
            editor.page == page,
            false,
        ));
    }
    let notice = if !editor.base.is_editable() {
        fl!("files-preferences-read-only")
    } else if let Some(error) = &desktop.configuration_error {
        fl!("files-configuration-not-applied", error = error.to_string())
    } else if !editor.base.same_source(&desktop.configuration) {
        fl!("files-preferences-changed-outside")
    } else {
        String::new()
    };
    let navigation: Element<'a, Message> = if wide {
        container(column(tabs).spacing(4))
            .padding(7)
            .width(142)
            .into()
    } else {
        container(row(tabs).spacing(4).wrap())
            .padding(4)
            .width(Length::Fill)
            .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Spine))
            .into()
    };
    let mut body = column![container(if notice.is_empty() {
        space().height(0).into()
    } else {
        form.note(notice)
    })]
    .spacing(8);
    // The prompt has its own fixed slot, so source/error notices never reparent fields.
    let prompt: Element<'a, Message> = if let Some(confirmation) = editor.confirmation {
        column![
            form.note(if confirmation == Confirmation::Close {
                fl!("files-preferences-discard-question")
            } else {
                fl!("files-preferences-reload-question")
            }),
            row![
                form.button(
                    fl!("files-preferences-keep-editing"),
                    "keep",
                    Some(Event::Confirm(id, false)),
                    false,
                    false
                ),
                form.button(
                    if confirmation == Confirmation::Close {
                        fl!("files-preferences-discard")
                    } else {
                        fl!("files-preferences-reload")
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
        Page::Views => views(&mut form, wide),
        Page::Toolbar => toolbar(&mut form),
        Page::Locations => locations(&mut form),
        Page::Shortcuts => shortcuts(&mut form),
    };
    let page = scrollable(column![page, space().height(8)].spacing(0))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .spacing(8),
        ))
        .id(editor.focus_id("scroll"))
        .width(Length::Fill)
        .height(Length::Fill);
    let page: Element<'a, Message> = if wide {
        row![
            navigation,
            iced::widget::rule::vertical(1).style(chrome::divider),
            page
        ]
        .spacing(10)
        .height(Length::Fill)
        .into()
    } else {
        column![navigation, page]
            .spacing(8)
            .height(Length::Fill)
            .into()
    };
    body = body.push(page);
    let reload = form.button(
        fl!("files-preferences-reload"),
        "reload",
        editor.enabled().then_some(Event::Reload(id)),
        false,
        false,
    );
    body = body.push(iced::widget::rule::horizontal(1).style(chrome::divider));
    body = body.push(
        row![
            column![
                form.note(&editor.status),
                text(fl!("files-preferences-keys"))
                    .font(canopa_ui::font(appearance))
                    .size(appearance.typography().label_size.px() + 1.0)
                    .style(canopa_ui::muted),
            ]
            .spacing(3)
            .width(Length::Fill),
            form.button(
                fl!("files-preferences-save"),
                "save",
                (editor.enabled() && editor.base.is_editable()).then_some(Event::Save(id)),
                true,
                false
            )
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8),
    );
    form.order.push(chrome::dismiss_id(id));
    let default = editor.confirmation.map(|_| editor.focus_id("keep"));
    let appearance = appearance.clone();
    let content = chrome::centered(
        column![
            chrome::heading_with_action(
                desktop,
                chrome::Icon::Settings,
                fl!("files-command-preferences"),
                fl!("files-preferences-subtitle"),
                Some(
                    row![reload, chrome::dismiss(desktop, id)]
                        .spacing(4)
                        .align_y(iced::Alignment::Center)
                        .into()
                ),
            ),
            body,
        ]
        .spacing(8),
        760.0,
    )
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
fn views<'a>(form: &mut Form<'a>, wide: bool) -> Element<'a, Message> {
    let v = &form.editor.draft.view;
    let mut modes = row![].spacing(4);
    // The second item names the control's focus ID, which stays in English.
    for (mode, name, label) in [
        (ViewMode::Columns, "Columns", fl!("files-view-columns")),
        (ViewMode::List, "List", fl!("files-view-list")),
        (ViewMode::Icons, "Icons", fl!("files-view-icons")),
    ] {
        modes = modes.push(form.change(label, name, Change::Mode(mode), v.mode == mode, true));
    }
    let mut sorts = row![].spacing(4);
    for (sort, name, label) in [
        (SortField::Name, "Name", fl!("files-column-name")),
        (SortField::Size, "Size", fl!("files-column-size")),
        (
            SortField::Modified,
            "Modified",
            fl!("files-column-modified"),
        ),
        (SortField::Kind, "Kind", fl!("files-column-kind")),
    ] {
        sorts = sorts.push(form.change(
            label,
            &format!("sort-{name}"),
            Change::Sort(sort),
            v.sort == sort,
            true,
        ));
    }
    let appearance = form.appearance;
    let segment = |choices: Element<'a, Message>| {
        container(choices)
            .padding(3)
            .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Spine))
    };
    let ordering = column![
        chrome::section_title(appearance, fl!("files-preferences-default-view")),
        segment(modes.wrap().into()),
        chrome::section_title(appearance, fl!("files-preferences-sort-by")),
        segment(sorts.wrap().into()),
        form.toggle(
            &fl!("files-preferences-descending"),
            "descending",
            v.descending,
            Change::Descending(!v.descending)
        )
    ]
    .spacing(10);
    let mut details = column![
        chrome::section_title(form.appearance, fl!("files-preferences-column-layout")),
        container(form.field(
            &fl!("files-preferences-column-width"),
            "width",
            &form.editor.width,
            Field::Width
        ))
        .max_width(220),
        text(fl!("files-preferences-column-width-range"))
            .font(canopa_ui::font(form.appearance))
            .size(form.appearance.typography().label_size.px())
            .style(canopa_ui::muted),
        chrome::section_title(form.appearance, fl!("files-preferences-list-details"))
    ]
    .spacing(10);
    for (detail, label) in [
        (Detail::Size, fl!("files-column-size")),
        (Detail::Modified, fl!("files-preferences-modified-date")),
        (Detail::Kind, fl!("files-column-kind")),
    ] {
        let checked = v.details.contains(&detail);
        details = details.push(form.toggle(
            &label,
            &format!("detail-{detail:?}"),
            checked,
            Change::Detail(detail, !checked),
        ));
    }
    let thumbnails = form.editor.draft.thumbnails;
    let mut show = row![].spacing(4);
    for policy in crate::interface::ThumbnailPolicy::ALL {
        show = show.push(form.change(
            policy.label(),
            &format!("thumbnails-{}", policy.id()),
            Change::Thumbnails(policy),
            thumbnails.show == policy,
            true,
        ));
    }
    let mut sizes = row![].spacing(4);
    for size in crate::interface::IconSize::ALL {
        sizes = sizes.push(form.change(
            size.label(),
            &format!("icon-size-{}", size.id()),
            Change::IconSize(size),
            thumbnails.size == size,
            true,
        ));
    }
    let ordering = ordering
        .push(chrome::section_title(
            appearance,
            fl!("files-preferences-thumbnails"),
        ))
        .push(segment(show.wrap().into()))
        .push(chrome::section_title(
            appearance,
            fl!("files-preferences-icon-size"),
        ))
        .push(segment(sizes.wrap().into()));
    let settings: Element<'a, Message> = if wide {
        row![
            container(ordering).width(Length::FillPortion(1)),
            container(details).width(Length::FillPortion(1)),
        ]
        .spacing(20)
        .into()
    } else {
        column![
            ordering,
            iced::widget::rule::horizontal(1).style(chrome::divider),
            details,
        ]
        .spacing(14)
        .into()
    };
    column![
        container(settings).padding([6, 4]).width(Length::Fill),
        iced::widget::rule::horizontal(1).style(chrome::divider),
        container(form.note(fl!("files-preferences-views-note"))).padding([4, 4])
    ]
    .spacing(10)
    .into()
}

fn item_label(editor: &Editor, id: &str) -> String {
    if id == "location" {
        fl!("files-preferences-location-field")
    } else if let Some(command) = Command::from_id(id) {
        command.label()
    } else {
        id.strip_prefix("action:")
            .and_then(|id| editor.base.actions.iter().find(|a| a.id == id))
            .map(|a| fl!("files-preferences-toolbar-action", name = a.name.clone()))
            .unwrap_or_else(|| id.to_owned())
    }
}
fn toolbar<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut body = column![form.note(fl!("files-preferences-toolbar-note"))].spacing(8);
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
                fl!("files-preferences-earlier"),
                "earlier",
                Change::MoveToolbar(-1),
                false,
                index.is_some_and(|i| i > 0)
            ),
            form.change(
                fl!("files-preferences-later"),
                "later",
                Change::MoveToolbar(1),
                false,
                index.is_some_and(|i| i + 1 < editor.draft.toolbar.len())
            ),
            form.change(
                fl!("files-preferences-remove"),
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
        &fl!("files-preferences-find-command-to-add"),
        "filter",
        &editor.filter,
        Field::Filter,
    ));
    let filter = editor.filter.to_lowercase();
    let choices = Command::ALL
        .iter()
        .filter(|c| **c != Command::QuickLook)
        .map(|c| c.id().to_owned())
        .chain(
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
            fl!("files-preferences-add-item", item = item_label(editor, &id)),
            &name,
            Change::AddToolbar(id),
            false,
            editor.draft.toolbar.len() < 64,
        ));
    }
    body.push(form.note(fl!("files-preferences-toolbar-footer")))
        .into()
}
fn locations<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut sidebar = row![].spacing(4);
    for (mode, label) in [
        (SidebarMode::Auto, fl!("files-preferences-sidebar-auto")),
        (SidebarMode::Always, fl!("files-preferences-sidebar-always")),
        (SidebarMode::Never, fl!("files-preferences-sidebar-never")),
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
        form.note(fl!("files-sidebar")),
        sidebar.wrap(),
        form.note(fl!("files-preferences-locations-and-bookmarks"))
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
                fl!("files-preferences-earlier"),
                "earlier",
                Change::MovePlace(-1),
                false,
                index.is_some_and(|i| i > 0)
            ),
            form.change(
                fl!("files-preferences-later"),
                "later",
                Change::MovePlace(1),
                false,
                index.is_some_and(|i| i + 1 < editor.locations.len())
            ),
            form.change(
                fl!("files-preferences-remove"),
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
            .push(form.field(
                &fl!("files-preferences-bookmark-name"),
                "bookmark-name",
                name,
                Field::BookmarkName,
            ))
            .push(form.field(
                &fl!("files-preferences-bookmark-folder"),
                "bookmark-path",
                path,
                Field::BookmarkPath,
            ));
    }
    body = body.push(form.change(
        fl!("files-preferences-add-bookmark"),
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
            fl!("files-preferences-add-item", item = kind.label()),
            &format!("add-{}", kind.id()),
            Change::AddPlace(*kind),
            false,
            editor.locations.len() < 256,
        ));
    }
    body.push(form.note(fl!("files-preferences-locations-footer")))
        .into()
}
fn shortcuts<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let mut body = column![form.field(
        &fl!("files-preferences-find-command"),
        "filter",
        &editor.filter,
        Field::Filter
    )]
    .spacing(8);
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
    body = body.push(form.note(fl!(
        "files-preferences-command-shortcuts",
        command = editor.selected_command.label()
    )));
    body = body.push(form.field(
        &fl!("files-preferences-shortcuts-field"),
        "shortcut",
        &editor.shortcuts[&editor.selected_command],
        Field::Shortcut,
    ));
    body = body.push(
        row![
            form.change(
                fl!("files-preferences-record"),
                "record",
                Change::Record(false),
                false,
                true
            ),
            form.change(
                fl!("files-preferences-use-defaults"),
                "reset",
                Change::ResetShortcut,
                false,
                true
            )
        ]
        .spacing(8),
    );
    body.push(form.note(fl!("files-preferences-shortcuts-footer")))
        .into()
}
