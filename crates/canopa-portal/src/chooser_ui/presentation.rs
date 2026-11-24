use super::{
    Appearance, ColorRole, Element, Entry, FILES_KEY, HistoryDirection, Length, Message, Mode,
    Node, Picker, PlaceKind, UiKind, a11y, browser_ui, describe, dialog_ui, entry_key, semantic,
};
use canopa_file::mime::MimeDatabase;
use canopa_files::browse::Browser;
use canopa_files::browser_ui::{
    Icon, breadcrumb, entry as entry_ui, file_icon::FileIcon, text_layout,
};
use iced::{
    Background,
    font::Weight,
    widget::{
        button, checkbox, column, container, image, mouse_area, responsive, rule, scrollable,
        space, text, text_input, tooltip,
    },
};
use std::collections::BTreeMap;
use std::{path::Path, sync::Arc};

#[derive(Clone)]
pub(super) struct Visuals {
    pub appearance: Appearance,
    pub requester: String,
    mime: Arc<MimeDatabase>,
    icons: BTreeMap<Icon, image::Handle>,
    files: BTreeMap<FileIcon, image::Handle>,
}

impl Visuals {
    pub(super) fn new(appearance: Appearance, app_id: &str) -> Self {
        fn draw(
            bytes: &[u8],
            canopa_theme::Rgb(r, g, b): canopa_theme::Rgb,
        ) -> Option<image::Handle> {
            let raster = canopa_assets::svg::rasterize(
                bytes,
                canopa_assets::svg::Policy {
                    max_encoded_bytes: 4096,
                    max_dimension: 48,
                    max_pixels: 48 * 48,
                    output_side: 48,
                    foreground: [r, g, b],
                },
            )?;
            Some(image::Handle::from_rgba(
                raster.width,
                raster.height,
                raster.pixels,
            ))
        }
        let foreground = appearance.role(ColorRole::Text);
        let palette = appearance.palette();
        let icons = [
            Icon::Up,
            Icon::Forward,
            Icon::Back,
            Icon::Search,
            Icon::Close,
            Icon::Home,
            Icon::Desktop,
            Icon::Documents,
            Icon::Downloads,
            Icon::Music,
            Icon::Pictures,
            Icon::Videos,
            Icon::Disk,
            Icon::Folder,
            Icon::NewFolder,
            Icon::Edit,
            Icon::Warning,
        ]
        .into_iter()
        .filter_map(|icon| {
            let tint = if icon == Icon::Warning {
                palette.warning
            } else {
                foreground
            };
            draw(icon.svg().as_bytes(), tint).map(|handle| (icon, handle))
        })
        .collect();
        let files = FileIcon::ALL
            .into_iter()
            .filter_map(|icon| {
                let tint = match icon {
                    FileIcon::Folder | FileIcon::Table => palette.accent,
                    FileIcon::Image
                    | FileIcon::Document
                    | FileIcon::Audio
                    | FileIcon::Video
                    | FileIcon::Code => appearance.role(ColorRole::Brand),
                    FileIcon::Pdf => palette.danger,
                    FileIcon::Archive | FileIcon::Presentation => palette.warning,
                    _ => foreground,
                };
                draw(icon.fallback(), tint).map(|handle| (icon, handle))
            })
            .collect();
        Self {
            appearance,
            requester: dialog_ui::requester(app_id),
            mime: MimeDatabase::system(),
            icons,
            files,
        }
    }

    pub(super) fn icon<'a>(&self, icon: Icon, size: f32) -> Element<'a, Message> {
        self.image(self.icons.get(&icon.directional()), size)
    }

    fn file<'a>(&self, entry: &Entry) -> Element<'a, Message> {
        let kind = FileIcon::for_entry(entry);
        let kind = if kind == FileIcon::File {
            self.mime
                .by_name(&entry.name())
                .first()
                .map_or(kind, |mime| FileIcon::for_type(mime, Some(&self.mime)))
        } else {
            kind
        };
        self.image(self.files.get(&kind), 22.0)
    }

    fn image<'a>(&self, handle: Option<&image::Handle>, size: f32) -> Element<'a, Message> {
        handle.map_or_else(
            || space().width(size).height(size).into(),
            |handle| image(handle.clone()).width(size).height(size).into(),
        )
    }
}

pub(super) fn checkbox_style(theme: &iced::Theme, status: checkbox::Status) -> checkbox::Style {
    let mut style = canopa_ui::checkbox_style(theme, status);
    if let checkbox::Status::Active { is_checked: false } = status {
        style.border.color = canopa_ui::colors(theme).muted;
        style.border.width = 1.25;
    }
    style
}

impl Picker {
    pub(super) fn field(
        &self,
        placeholder: &str,
        value: &str,
    ) -> text_input::TextInput<'_, Message> {
        let appearance = self.visuals.appearance.clone();
        text_input(placeholder, value)
            .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
            .style(move |theme, status| browser_ui::input_style(&appearance, theme, status))
    }

    fn tool<'a>(
        &self,
        key: &'static str,
        label: String,
        icon: Icon,
        message: Option<Message>,
        show_label: bool,
    ) -> Element<'a, Message> {
        let mark = self.visuals.icons.get(&icon.directional()).map_or_else(
            || space().width(17).height(17).into(),
            |handle| {
                image(handle.clone())
                    .width(17)
                    .height(17)
                    .opacity(if message.is_some() { 1.0 } else { 0.35 })
                    .into()
            },
        );
        let content = if show_label {
            canopa_ui::direction::row([mark, text(label.clone()).size(14).into()])
                .align_y(iced::Center)
                .spacing(6)
                .into()
        } else {
            mark
        };
        a11y::button(
            key,
            label.clone(),
            message.clone(),
            tooltip(
                canopa_ui::action(content, message, canopa_ui::Action::Quiet, false)
                    .padding([8, 8]),
                text(label).size(13),
                tooltip::Position::Bottom,
            )
            .style(dialog_ui::card),
        )
    }

    pub(super) fn toolbar(&self, enabled: bool, focused: Option<String>) -> Element<'_, Message> {
        let location = fl!("portal-chooser-location");
        let up = self
            .browser
            .as_ref()
            .is_some_and(|browser| browser.parent().is_some());
        let history = |direction| {
            (enabled
                && self
                    .browser
                    .as_ref()
                    .and_then(|browser| browser.history_target(direction))
                    .is_some())
            .then_some(Message::History(direction))
        };
        let mut tools = vec![
            self.tool(
                "chooser-back",
                fl!("portal-chooser-back"),
                Icon::Back,
                history(HistoryDirection::Back),
                false,
            ),
            self.tool(
                "chooser-forward",
                fl!("portal-chooser-forward"),
                Icon::Forward,
                history(HistoryDirection::Forward),
                false,
            ),
            self.tool(
                "chooser-up",
                fl!("portal-chooser-up"),
                Icon::Up,
                (enabled && up).then_some(Message::Up),
                false,
            ),
        ];
        if self.editing_location {
            tools.push(semantic(
                Node::text_input(location.clone(), &self.location)
                    .key("chooser-location")
                    .placeholder(location.clone())
                    .active_descendant(focused)
                    .on_text(Message::Location),
                self.field(&location, &self.location)
                    .id("location")
                    .on_input(Message::Location)
                    .on_submit(Message::Go)
                    .width(Length::Fill),
            ));
            tools.push(a11y::button(
                "chooser-go",
                fl!("portal-chooser-go"),
                enabled.then_some(Message::Go),
                dialog_ui::action(
                    text(fl!("portal-chooser-go")),
                    enabled.then_some(Message::Go),
                    canopa_ui::Action::Normal,
                    false,
                ),
            ));
        } else if self.filtering {
            let label = fl!("portal-chooser-search");
            tools.push(semantic(
                Node::search_input(&label, &self.search)
                    .key("chooser-search")
                    .placeholder(&label)
                    .controls(FILES_KEY)
                    .active_descendant(focused)
                    .on_text(Message::Search),
                self.field(&label, &self.search)
                    .id("search")
                    .on_input(Message::Search)
                    .width(Length::Fill),
            ));
            tools.push(self.tool(
                "chooser-search-close",
                fl!("portal-chooser-close-search"),
                Icon::Close,
                enabled.then_some(Message::CloseNameFilter),
                false,
            ));
        } else {
            tools.push(self.breadcrumb(enabled));
            tools.push(self.tool(
                "chooser-edit-location",
                fl!("portal-chooser-edit-location"),
                Icon::Edit,
                enabled.then_some(Message::EditLocation),
                true,
            ));
        }
        if self.request.mode != Mode::Open {
            tools.push(self.tool(
                "chooser-new-folder-button",
                fl!("portal-chooser-new-folder"),
                Icon::NewFolder,
                enabled.then_some(Message::NewFolder),
                true,
            ));
        }
        semantic(
            Node::toolbar(fl!("portal-chooser-navigation")).key("chooser-toolbar"),
            canopa_ui::direction::row(tools)
                .spacing(4)
                .align_y(iced::Center)
                .width(Length::Fill),
        )
    }

    fn breadcrumb(&self, enabled: bool) -> Element<'_, Message> {
        let path = self
            .browser
            .as_ref()
            .map(Browser::path)
            .unwrap_or_else(|| Path::new(&self.location));
        breadcrumb::view(
            breadcrumb::Location {
                appearance: &self.visuals.appearance,
                path,
                home: &self.home,
                home_label: fl!("portal-place-home"),
                key_prefix: "chooser-crumb",
                edit_description: fl!("portal-chooser-edit-location"),
                edit: enabled.then_some(Message::EditLocation),
                compact: false,
                show_tooltips: true,
            },
            |icon, size, opacity| {
                self.visuals.icons.get(&icon.directional()).map_or_else(
                    || space().width(size).height(size).into(),
                    |handle| {
                        image(handle.clone())
                            .width(size)
                            .height(size)
                            .opacity(opacity)
                            .into()
                    },
                )
            },
            move |path| enabled.then(|| Message::Navigate(path.to_owned())),
            Some(self.tool(
                "chooser-search-open",
                fl!("portal-chooser-search"),
                Icon::Search,
                enabled.then_some(Message::ShowNameFilter),
                false,
            )),
        )
    }

    pub(super) fn places(&self, enabled: bool) -> Element<'_, Message> {
        let appearance = &self.visuals.appearance;
        let current = self.browser.as_ref().and_then(|browser| {
            self.shortcuts
                .iter()
                .filter(|(_, _, place)| browser.path().starts_with(place))
                .max_by_key(|(_, _, place)| place.components().count())
                .map(|(kind, _, _)| *kind)
        });
        let places = self.shortcuts.iter().map(|(kind, label, path)| {
            let icon = match kind {
                PlaceKind::Home => Icon::Home,
                PlaceKind::Desktop => Icon::Desktop,
                PlaceKind::Documents => Icon::Documents,
                PlaceKind::Downloads => Icon::Downloads,
                PlaceKind::Music => Icon::Music,
                PlaceKind::Pictures => Icon::Pictures,
                PlaceKind::Videos => Icon::Videos,
                PlaceKind::Filesystem => Icon::Disk,
                _ => Icon::Folder,
            };
            let active = current == Some(*kind);
            let message = enabled.then(|| Message::Navigate(path.clone()));
            let mut font = canopa_ui::font(appearance);
            if active {
                font.weight = Weight::Semibold;
            }
            semantic(
                Node::option(label, active)
                    .key(format!("chooser-place-{}", a11y::path_key(path)))
                    .disabled(message.is_none())
                    .on_click(message.clone()),
                button(
                    canopa_ui::direction::row([
                        self.visuals.icon(icon, 16.0),
                        text(label).font(font).size(14).into(),
                    ])
                    .align_y(iced::Center)
                    .spacing(9),
                )
                .padding([6, 9])
                .width(Length::Fill)
                .on_press_maybe(message)
                .style(move |theme, status| {
                    let mut style =
                        canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                    if active {
                        style.background = browser_ui::sidebar_current(appearance).background;
                    }
                    style.border.radius = 8.0.into();
                    style
                }),
            )
        });
        let label = fl!("portal-chooser-places");
        let places = semantic(
            Node::list(&label).key("chooser-places"),
            column(places).spacing(2),
        );
        container(
            scrollable(
                column![dialog_ui::section(label).style(canopa_ui::muted), places].spacing(7),
            )
            .style(canopa_ui::scrollbar)
            .spacing(5)
            .height(Length::Fill),
        )
        .width(170)
        .height(Length::Fill)
        .padding([12, 10])
        .style(|theme| container::Style::default().background(canopa_ui::colors(theme).background))
        .into()
    }

    pub(super) fn browser_panes<'a>(
        &'a self,
        enabled: bool,
        focused: Option<String>,
        status: String,
    ) -> Element<'a, Message> {
        let parent = self.parent.as_ref().filter(|parent| {
            self.browser.as_ref().is_some_and(|browser| {
                parent.entries.iter().any(|entry| {
                    entry.is_directory()
                        && entry.path != browser.path()
                        && (browser.show_hidden() || !entry.is_hidden())
                })
            })
        });
        responsive(move |bounds| {
            let mut panes = vec![
                self.places(enabled),
                rule::vertical(1).style(browser_ui::divider).into(),
            ];
            if bounds.width >= 760.0
                && let Some(parent) = parent
            {
                panes.push(self.parent_column(parent, enabled));
                panes.push(rule::vertical(1).style(browser_ui::divider).into());
            }
            panes.push(self.listing(self.entries(), enabled, &focused, &status));
            canopa_ui::direction::row(panes).height(Length::Fill).into()
        })
        .into()
    }

    fn parent_column<'a>(
        &'a self,
        parent: &'a canopa_files::browse::Directory,
        enabled: bool,
    ) -> Element<'a, Message> {
        let label = if parent.path == self.home {
            fl!("portal-place-home")
        } else {
            parent
                .path
                .file_name()
                .unwrap_or(parent.path.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        let appearance = &self.visuals.appearance;
        let current = self.browser.as_ref().map(Browser::path);
        let show_hidden = self.browser.as_ref().is_some_and(Browser::show_hidden);
        let folders = parent
            .entries
            .iter()
            .filter(|entry| {
                entry.is_directory()
                    && (show_hidden || !entry.is_hidden() || current == Some(&entry.path))
            })
            .map(|entry| {
                let name = entry.name().into_owned();
                let selected = current == Some(&entry.path);
                let message = enabled.then(|| Message::Navigate(entry.path.clone()));
                let state = entry_ui::RowState {
                    selected,
                    ancestor: true,
                    disabled: !enabled,
                    ..Default::default()
                };
                semantic(
                    Node::option(&name, selected)
                        .key(format!("chooser-parent-{}", a11y::path_key(&entry.path)))
                        .disabled(message.is_none())
                        .on_click(message.clone()),
                    button(
                        entry_ui::name_row(appearance, name, self.visuals.file(entry), state, true)
                            .push(self.visuals.icon(Icon::Forward, 16.0)),
                    )
                    .padding(entry_ui::entry_padding(appearance, false))
                    .height(entry_ui::row_height(appearance))
                    .width(Length::Fill)
                    .clip(true)
                    .on_press_maybe(message)
                    .style(move |_, status| state.button(appearance, status)),
                )
            });
        container(column![semantic(
            Node::list(label).key("chooser-parent-folders"),
            scrollable(
                container(column(folders).spacing(1))
                    .padding([4, 5])
                    .width(Length::Fill)
            )
            .style(canopa_ui::scrollbar)
            .height(Length::Fill),
        ),])
        .width(176)
        .height(Length::Fill)
        .style(|theme| container::Style::default().background(canopa_ui::colors(theme).surface))
        .into()
    }

    pub(super) fn listing<'a>(
        &'a self,
        entries: Vec<&'a Entry>,
        enabled: bool,
        focused: &Option<String>,
        status: &str,
    ) -> Element<'a, Message> {
        let appearance = &self.visuals.appearance;
        let total = entries.len();
        let mut listing = column![].spacing(1);
        for (index, entry) in entries.into_iter().enumerate() {
            let selected = self
                .browser
                .as_ref()
                .is_some_and(|b| b.selected().contains(&entry.path));
            let name = entry.name().into_owned();
            let key = entry_key(&entry.path);
            let focus = focused.as_ref() == Some(&key);
            let select = enabled.then(|| Message::Select(entry.path.clone()));
            let activate = enabled.then(|| Message::Activate(entry.path.clone()));
            let open_label = fl!("portal-chooser-open-entry", name = name.as_str());
            let replaces = self.replaces(entry);
            let show_action = entry.is_directory()
                || (!selected && self.hovered.as_ref() == Some(&entry.path))
                || (!self.search.is_empty() && total == 1);
            let open = if show_action {
                let face: Element<'_, Message> = if entry.is_directory() {
                    self.visuals.icon(Icon::Forward, 14.0)
                } else {
                    text(fl!("portal-chooser-choose-row")).size(13).into()
                };
                a11y::button(
                    format!("{key}-open"),
                    open_label.clone(),
                    activate.clone(),
                    tooltip(
                        canopa_ui::action(face, activate, canopa_ui::Action::Quiet, false)
                            .padding([5, 8]),
                        text(open_label).size(13),
                        tooltip::Position::Left,
                    )
                    .style(dialog_ui::card),
                )
            } else {
                space().width(0).into()
            };
            let state = entry_ui::RowState {
                selected,
                focused: focus,
                disabled: !enabled,
                ..Default::default()
            };
            let label: Element<'_, Message> = if replaces {
                let badge = fl!("portal-chooser-existing-row");
                let font = canopa_ui::font(appearance);
                let name_width =
                    text_layout::text_width(&name, font, appearance.typography().body_size.px())
                        + 32.0;
                let badge_width = text_layout::text_width(&badge, font, 13.0) + 25.0;
                responsive({
                    let name = name.clone();
                    move |bounds| {
                        let badge = container(text(badge.clone()).size(13))
                            .padding([4, 7])
                            .style(|theme| {
                                let warning = canopa_ui::colors(theme).warning;
                                container::Style::default()
                                    .background(warning.scale_alpha(0.22))
                                    .border(
                                        iced::Border::default()
                                            .rounded(5)
                                            .width(1)
                                            .color(warning.scale_alpha(0.62)),
                                    )
                            });
                        let file = entry_ui::name_row(
                            appearance,
                            name.clone(),
                            self.visuals.file(entry),
                            state,
                            true,
                        )
                        .width(name_width.min((bounds.width - badge_width).max(36.0)));
                        canopa_ui::direction::row([file.into(), badge.into()])
                            .align_y(iced::Center)
                            .spacing(8)
                            .into()
                    }
                })
                .height(Length::Shrink)
                .into()
            } else {
                entry_ui::name_row(
                    appearance,
                    name.clone(),
                    self.visuals.file(entry),
                    state,
                    true,
                )
                .into()
            };
            let description = if replaces {
                format!(
                    "{} · {}",
                    describe(entry),
                    fl!("portal-chooser-existing-row")
                )
            } else {
                describe(entry)
            };
            listing = listing.push(semantic(
                Node::option(name, selected)
                    .key(key)
                    .description(description)
                    .position(index + 1, total)
                    .focused(focus)
                    .disabled(select.is_none())
                    .on_click(select.clone()),
                mouse_area(
                    canopa_ui::direction::row([
                        button(label)
                            .on_press_maybe(select)
                            .width(Length::Fill)
                            .padding(entry_ui::entry_padding(appearance, false))
                            .height(entry_ui::row_height(appearance))
                            .clip(true)
                            .style(move |_, status| state.button(appearance, status))
                            .into(),
                        open,
                    ])
                    .align_y(iced::Center),
                )
                .on_enter(Message::Hover(entry.path.clone(), true))
                .on_exit(Message::Hover(entry.path.clone(), false)),
            ));
        }
        if total == 0 {
            listing =
                listing.push(container(dialog_ui::detail(status.to_owned())).padding([10, 8]));
        }
        container(semantic(
            Node::list(fl!("portal-chooser-files"))
                .key(FILES_KEY)
                .description(&self.location)
                .multiselectable(self.request.multiple)
                .busy(self.loading)
                .active_descendant(focused.clone()),
            scrollable(container(listing).padding([4, 5]).width(Length::Fill))
                .style(canopa_ui::scrollbar)
                .spacing(5)
                .height(Length::Fill),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| {
            container::Style::default()
                .background(Background::Color(canopa_ui::colors(theme).surface))
        })
        .into()
    }
}
