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
            "page-Toolbar" => Some(chrome::Icon::Toolbar),
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
        let footer = matches!(name, "save" | "reload");
        let label = text(label.into())
            .font(canopa_ui::font(self.appearance))
            .size(if footer {
                self.appearance.typography().body_size.px()
            } else {
                self.appearance.typography().label_size.px() + if tab { 1.0 } else { 0.0 }
            });
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
            } else if name == "reload" && message.is_some() {
                // An unavailable Revert drops its outline so it reads as disabled.
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
        let button = if footer {
            button.padding(chrome::DIALOG_BUTTON_PADDING)
        } else {
            button
        };
        let content = container(button).style({
            let appearance = self.appearance;
            move |_| {
                if active && tab {
                    chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                } else if active && !primary {
                    container::Style::default()
                        .background(chrome::selected_fill(appearance))
                        .border(iced::Border {
                            width: 1.0,
                            color: chrome::ink(appearance, canopa_theme::ColorRole::Accent)
                                .scale_alpha(0.55),
                            radius: 11.0.into(),
                        })
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
    /// A small icon button acting on one list row, named for assistive
    /// technologies by `label`.
    fn icon_change(
        &mut self,
        icon: chrome::Icon,
        label: String,
        name: &str,
        change: Change,
        available: bool,
    ) -> Element<'a, Message> {
        let enabled = self.editor.enabled() && available;
        let id = self.register(name, true, enabled);
        let message =
            enabled.then(|| Message::Preferences(Event::Change(self.editor.target(), change)));
        let button = iced::widget::button(self.desktop.icons.chrome(
            icon,
            15.0,
            if enabled { 0.9 } else { 0.35 },
        ))
        .padding(5)
        .on_press_maybe(message.clone())
        .style(|theme, status| {
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            style.border.radius = 7.0.into();
            style
        });
        let control = focus::control(id.clone(), message.clone(), self.appearance, button);
        let control = super::accessibility_ui::named_button(
            super::accessibility_ui::focus_key(&id),
            label.clone(),
            message,
            control,
        );
        iced::widget::tooltip(
            control,
            text(label).size(self.appearance.typography().label_size.px()),
            iced::widget::tooltip::Position::Bottom,
        )
        .padding(6)
        .style(super::chrome::tooltip_style)
        .into()
    }
    /// One list row: a leading icon, its label (a button when it selects the
    /// row), its own controls at the end, and what opens under it.
    fn list_row(
        &self,
        icon: Option<chrome::Icon>,
        label: Element<'a, Message>,
        controls: Vec<Element<'a, Message>>,
        detail: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let icon = icon.map(|icon| self.desktop.icons.chrome(icon, 16.0, 0.85));
        self.list_row_with(icon, label, controls, detail)
    }
    /// A place's row, with its themed icon; `None` is a bookmark.
    fn place_row(
        &self,
        place: Option<PlaceKind>,
        label: Element<'a, Message>,
        controls: Vec<Element<'a, Message>>,
        detail: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let icon = self.desktop.icons.place(
            place.map_or(Some("folder"), super::icons_ui::place_icon_name),
            None,
            place.map_or(chrome::Icon::Folder, place_icon),
            16.0,
            1.0,
        );
        self.list_row_with(Some(icon), label, controls, detail)
    }
    fn list_row_with(
        &self,
        icon: Option<Element<'a, Message>>,
        label: Element<'a, Message>,
        controls: Vec<Element<'a, Message>>,
        detail: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let mut line: Vec<Element<'a, Message>> = Vec::new();
        if let Some(icon) = icon {
            line.push(icon);
        }
        line.push(container(label).width(Length::Fill).into());
        line.extend(controls);
        let mut content = column![
            canopa_ui::direction::row(line)
                .spacing(6)
                .align_y(iced::Alignment::Center)
        ]
        .spacing(8);
        if let Some(detail) = detail {
            content = content.push(detail);
        }
        let appearance = self.appearance;
        container(content)
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |_| chrome::catalogue_row(appearance, false))
            .into()
    }
    /// The row controls that move a row earlier or later and remove it.
    fn row_edits(
        &mut self,
        name: &str,
        index: usize,
        count: usize,
        removable: bool,
        change: impl Fn(usize, super::preferences_editor::RowEdit) -> Change,
    ) -> Vec<Element<'a, Message>> {
        use super::preferences_editor::RowEdit;
        vec![
            self.icon_change(
                chrome::Icon::Up,
                fl!("files-preferences-earlier"),
                &format!("{name}-{index}-earlier"),
                change(index, RowEdit::Earlier),
                index > 0,
            ),
            self.icon_change(
                chrome::Icon::Down,
                fl!("files-preferences-later"),
                &format!("{name}-{index}-later"),
                change(index, RowEdit::Later),
                index + 1 < count,
            ),
            self.icon_change(
                chrome::Icon::Close,
                fl!("files-preferences-remove"),
                &format!("{name}-{index}-remove"),
                change(index, RowEdit::Remove),
                removable,
            ),
        ]
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
            input = input
                .on_input(move |v| {
                    Message::Preferences(Event::Change(target, Change::Field(which, v)))
                })
                // Typed values apply when submitted.
                .on_submit(Message::Preferences(Event::Save(target.window)));
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
    // A blocking question is shown over the window, not inside the form.
    let question = editor.confirmation.map(|confirmation| {
        let buttons: Element<'a, Message> = row![
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
        .into();
        (
            if confirmation == Confirmation::Close {
                fl!("files-preferences-discard-question")
            } else {
                fl!("files-preferences-reload-question")
            },
            buttons,
        )
    });
    // The old prompt slot stays, so notices never reparent fields.
    body = body.push(container(space().height(0)));
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
    body = body.push(iced::widget::rule::horizontal(1).style(chrome::divider));
    let changed_outside = !editor.base.same_source(&desktop.configuration);
    // Changes save as they are made, so the footer reports state instead of
    // offering Save. A newer version saved elsewhere can still be loaded.
    let reload: Element<'a, Message> = if changed_outside {
        form.button(
            fl!("files-preferences-reload"),
            "reload",
            editor.enabled().then_some(Event::Reload(id)),
            false,
            false,
        )
    } else {
        iced::widget::space().width(0).into()
    };
    let status: Element<'a, Message> = if editor.dirty
        && !editor.busy
        && !editor.applying
        && matches!(editor.status.as_str(), s if s == fl!("files-preferences-status-initial")
            || s == fl!("files-preferences-saved"))
    {
        text(fl!("files-preferences-pending"))
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().label_size.px() + 1.0)
            .color(chrome::ink(appearance, canopa_theme::ColorRole::Warning))
            .into()
    } else {
        form.note(&editor.status)
    };
    body = body.push(
        row![container(status).width(Length::Fill), reload]
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
                Some(chrome::dismiss(desktop, id)),
            ),
            body,
        ]
        .spacing(8),
        760.0,
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| chrome::window_surface(&appearance, true));
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
    let content = chrome::modal(desktop, content.into(), question);
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
            .style(move |_| chrome::segmented(appearance))
    };
    let group = |title: String, control: Element<'a, Message>| -> Element<'a, Message> {
        column![chrome::section_title(appearance, title), control]
            .spacing(8)
            .into()
    };
    let descending = form.toggle(
        &fl!("files-preferences-descending"),
        "descending",
        v.descending,
        Change::Descending(!v.descending),
    );
    // Sorting applies to every view, so it lives with the order, not List view.
    let folders_first = form.toggle(
        &fl!("files-folders-first"),
        "folders-first",
        v.folders_first,
        Change::FoldersFirst(!v.folders_first),
    );
    let ordering = column![
        group(
            fl!("files-preferences-default-view"),
            column![
                segment(modes.wrap().into()),
                text(fl!("files-preferences-views-note"))
                    .font(canopa_ui::font(appearance))
                    .size(appearance.typography().label_size.px() + 1.0)
                    .style(canopa_ui::muted)
                    .wrapping(text::Wrapping::WordOrGlyph),
            ]
            .spacing(8)
            .into()
        ),
        group(
            fl!("files-preferences-sort-by"),
            column![segment(sorts.wrap().into()), descending, folders_first]
                .spacing(10)
                .into()
        ),
    ]
    .spacing(22);
    let width = column![
        container(form.field(
            &fl!("files-preferences-column-width"),
            "width",
            &form.editor.width,
            Field::Width
        ))
        .max_width(150),
        text(fl!("files-preferences-column-width-range"))
            .font(canopa_ui::font(form.appearance))
            .size(form.appearance.typography().label_size.px() + 1.0)
            .style(canopa_ui::muted),
    ]
    .spacing(6);
    let mut list_details = column![].spacing(8);
    for (detail, label) in [
        (Detail::Size, fl!("files-column-size")),
        (Detail::Modified, fl!("files-preferences-modified-date")),
        (Detail::Kind, fl!("files-column-kind")),
    ] {
        let checked = v.details.contains(&detail);
        list_details = list_details.push(form.toggle(
            &label,
            &format!("detail-{detail:?}"),
            checked,
            Change::Detail(detail, !checked),
        ));
    }
    list_details = list_details.push(form.toggle(
        &fl!("files-folder-tree"),
        "tree",
        v.tree,
        Change::Tree(!v.tree),
    ));

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
    // Registered after the existing Views controls, keeping their keyboard order.
    let mut displays = row![].spacing(4);
    for display in crate::interface::PropertiesDisplay::ALL {
        displays = displays.push(form.change(
            display.label(),
            &format!("properties-{}", display.id()),
            Change::Properties(display),
            form.editor.draft.properties == display,
            true,
        ));
    }
    let caption = |label: String| {
        text(label)
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().label_size.px() + 1.0)
            .style(canopa_ui::muted)
            .wrapping(text::Wrapping::WordOrGlyph)
    };
    // Shared browsing choices on the left; each view's own options on the
    // right under that view's name, so their scope is explicit.
    let ordering = ordering
        .push(group(
            fl!("files-preferences-thumbnails"),
            column![
                segment(show.wrap().into()),
                caption(fl!("files-preferences-thumbnails-note")),
            ]
            .spacing(8)
            .into(),
        ))
        .push(group(
            fl!("files-preferences-properties"),
            segment(displays.wrap().into()).into(),
        ));
    let details = column![
        group(fl!("files-preferences-columns-view"), width.into()),
        group(
            fl!("files-preferences-list-view"),
            column![caption(fl!("files-preferences-list-details")), list_details]
                .spacing(8)
                .into()
        ),
        group(
            fl!("files-preferences-icons-view"),
            column![
                caption(fl!("files-preferences-icon-size")),
                segment(sizes.wrap().into())
            ]
            .spacing(8)
            .into()
        ),
    ]
    .spacing(20);
    // Each column opens with what it governs: every view, or one view.
    let eyebrow = |label: String| {
        let mut font = canopa_ui::font(appearance);
        font.weight = iced::font::Weight::Semibold;
        text(label)
            .font(font)
            .size(appearance.typography().body_size.px() + 2.0)
    };
    let ordering = column![eyebrow(fl!("files-preferences-all-views")), ordering].spacing(12);
    let details = column![eyebrow(fl!("files-preferences-each-view")), details].spacing(12);
    let settings: Element<'a, Message> = if wide {
        row![
            container(ordering).width(Length::FillPortion(1)),
            container(details).width(Length::FillPortion(1)),
        ]
        .spacing(28)
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
    container(settings)
        .padding([6, 4])
        .width(Length::Fill)
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
    let appearance = form.appearance;
    // The toolbar as it will look, in its order.
    let mut preview: Vec<Element<'a, Message>> = Vec::new();
    for id in &editor.draft.toolbar {
        let chip: Element<'a, Message> = if id == "location" {
            container(form.note(fl!("files-preferences-location-field")))
                .padding([4, 10])
                .width(Length::Fill)
                .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Background))
                .into()
        } else if let Some((icon, _)) = Command::from_id(id).and_then(super::toolbar::command_icon)
        {
            container(form.desktop.icons.chrome(icon, 16.0, 0.9))
                .padding(5)
                .into()
        } else {
            container(form.note(item_label(editor, id)))
                .padding([4, 6])
                .into()
        };
        preview.push(chip);
    }
    let preview = container(
        canopa_ui::direction::row(preview)
            .spacing(6)
            .align_y(iced::Alignment::Center),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .style(move |_| chrome::bar(appearance, true));
    let mut body = column![form.note(fl!("files-preferences-toolbar-preview")), preview].spacing(8);
    let count = editor.draft.toolbar.len();
    for (index, id) in editor.draft.toolbar.iter().enumerate() {
        let icon = Command::from_id(id)
            .and_then(super::toolbar::command_icon)
            .map(|(icon, _)| icon);
        let controls = form.row_edits(
            "toolbar",
            index,
            count,
            id != "location",
            Change::ToolbarItem,
        );
        body = body.push(
            form.list_row(
                icon,
                text(item_label(editor, id))
                    .font(canopa_ui::font(appearance))
                    .size(appearance.typography().body_size.px())
                    .into(),
                controls,
                None,
            ),
        );
    }
    body = body.push(form.change(
        fl!("files-preferences-add-button"),
        "add-open",
        Change::Picker(!editor.picker),
        editor.picker,
        editor.draft.toolbar.len() < 64,
    ));
    if editor.picker {
        body = body.push(form.field(
            &fl!("files-preferences-find-command-to-add"),
            "filter",
            &editor.filter,
            Field::Filter,
        ));
        let filter = editor.filter.to_lowercase();
        let choices = Command::ALL
            .iter()
            .filter(|c| !c.never_on_toolbar())
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
            let add = form.icon_change(
                chrome::Icon::Plus,
                fl!("files-preferences-add-item", item = item_label(editor, &id)),
                &name,
                Change::AddToolbar(id.clone()),
                true,
            );
            let icon = Command::from_id(&id)
                .and_then(super::toolbar::command_icon)
                .map(|(icon, _)| icon);
            body =
                body.push(form.list_row(icon, form.note(item_label(editor, &id)), vec![add], None));
        }
    }
    body.push(form.note(fl!("files-preferences-toolbar-footer")))
        .into()
}

/// The sidebar's places, in their order. A bookmark is a place you added for
/// a folder of your own; its name and folder are edited in its row.
fn locations<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let appearance = form.appearance;
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
        form.note(fl!("files-preferences-show-sidebar")),
        sidebar.wrap(),
        form.note(fl!("files-preferences-places"))
    ]
    .spacing(8);
    let count = editor.locations.len();
    for (index, place) in editor.locations.iter().enumerate() {
        let controls = form.row_edits("place", index, count, true, Change::PlaceItem);
        let (icon, label, detail): (_, Element<'a, Message>, Option<Element<'a, Message>>) =
            match place {
                Location::Builtin(kind) => (
                    Some(*kind),
                    text(place.label())
                        .font(canopa_ui::font(appearance))
                        .size(appearance.typography().body_size.px())
                        .into(),
                    None,
                ),
                Location::Bookmark { name, path } => {
                    let open = editor.selected_place == Some(index);
                    // A bookmark's row opens its own name and folder fields.
                    let label = form.change(
                        name.clone(),
                        &format!("place-{index}"),
                        Change::SelectPlace(index),
                        open,
                        true,
                    );
                    let detail = open.then(|| {
                        column![
                            form.field(
                                &fl!("files-preferences-bookmark-name"),
                                "bookmark-name",
                                name,
                                Field::BookmarkName,
                            ),
                            form.field(
                                &fl!("files-preferences-bookmark-folder"),
                                "bookmark-path",
                                path,
                                Field::BookmarkPath,
                            )
                        ]
                        .spacing(8)
                        .into()
                    });
                    (None, label, detail)
                }
            };
        body = body.push(form.place_row(icon, label, controls, detail));
    }
    body = body.push(form.change(
        fl!("files-preferences-add-place"),
        "add-open",
        Change::Picker(!editor.picker),
        editor.picker,
        editor.locations.len() < 256,
    ));
    if editor.picker {
        let add = form.icon_change(
            chrome::Icon::Plus,
            fl!("files-preferences-add-bookmark"),
            "add-bookmark",
            Change::AddBookmark,
            true,
        );
        body = body.push(form.place_row(
            None,
            form.note(fl!("files-preferences-add-bookmark")),
            vec![add],
            None,
        ));
        for kind in PlaceKind::ALL.iter().filter(|kind| {
            !editor
                .locations
                .iter()
                .any(|p| matches!(p, Location::Builtin(k) if k == *kind))
        }) {
            let add = form.icon_change(
                chrome::Icon::Plus,
                fl!("files-preferences-add-item", item = kind.label()),
                &format!("add-{}", kind.id()),
                Change::AddPlace(*kind),
                true,
            );
            body = body.push(form.place_row(Some(*kind), form.note(kind.label()), vec![add], None));
        }
    }
    body.push(form.note(fl!("files-preferences-locations-footer")))
        .into()
}

fn place_icon(kind: PlaceKind) -> chrome::Icon {
    match kind {
        PlaceKind::Home => chrome::Icon::Home,
        PlaceKind::Desktop => chrome::Icon::Desktop,
        PlaceKind::Documents => chrome::Icon::Documents,
        PlaceKind::Downloads => chrome::Icon::Downloads,
        PlaceKind::Music => chrome::Icon::Music,
        PlaceKind::Pictures => chrome::Icon::Pictures,
        PlaceKind::Videos => chrome::Icon::Videos,
        PlaceKind::Templates => chrome::Icon::Templates,
        PlaceKind::Public => chrome::Icon::Public,
        PlaceKind::Filesystem => chrome::Icon::Drive,
        PlaceKind::Trash => chrome::Icon::Trash,
        PlaceKind::Recovery => chrome::Icon::Recovery,
    }
}

/// Every command with its shortcuts in one table. A row's Record and Reset act
/// on it; selecting a row opens its shortcuts for typing.
fn shortcuts<'a>(form: &mut Form<'a>) -> Element<'a, Message> {
    let editor = form.editor;
    let appearance = form.appearance;
    let mut body = column![form.field(
        &fl!("files-preferences-find-command"),
        "filter",
        &editor.filter,
        Field::Filter
    )]
    .spacing(6);
    let filter = editor.filter.to_lowercase();
    for command in Command::ALL
        .iter()
        .filter(|c| c.label().to_lowercase().contains(&filter))
    {
        let open = editor.selected_command == *command;
        let label = form.change(
            command.label(),
            &format!("command-{}", command.id()),
            Change::SelectCommand(*command),
            open,
            true,
        );
        let current = &editor.shortcuts[command];
        let keys = container(
            text(if current.trim().is_empty() {
                fl!("files-preferences-no-shortcut")
            } else {
                current.replace(';', " ·")
            })
            .font(iced::Font::MONOSPACE)
            .size(appearance.typography().label_size.px() + 1.0)
            .color(chrome::ink(appearance, canopa_theme::ColorRole::Muted)),
        )
        .padding([2, 8]);
        let record = form.icon_change(
            chrome::Icon::Edit,
            fl!("files-preferences-record-for", command = command.label()),
            &format!("record-{}", command.id()),
            Change::RecordFor(*command),
            true,
        );
        let reset = form.icon_change(
            chrome::Icon::Recovery,
            fl!("files-preferences-reset-for", command = command.label()),
            &format!("reset-{}", command.id()),
            Change::ResetFor(*command),
            *current != command.defaults().join("; "),
        );
        let detail = open.then(|| {
            form.field(
                &fl!("files-preferences-shortcuts-field"),
                "shortcut",
                current,
                Field::Shortcut,
            )
        });
        body = body.push(form.list_row(None, label, vec![keys.into(), record, reset], detail));
    }
    body.push(form.note(fl!("files-preferences-shortcuts-footer")))
        .into()
}
