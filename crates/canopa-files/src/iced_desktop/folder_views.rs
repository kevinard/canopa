//! Immediate shared folder choices, with serialized/coalesced private-state writes.
use super::*;
use crate::preferences::{Detail, SortField, ViewMode};
use iced::{
    Length,
    futures::channel::oneshot,
    widget::{
        checkbox, column, container, mouse_area, opaque, row, scrollable, space, stack, text,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub window: window::Id,
    tab: u64,
    navigation: u64,
    path: PathBuf,
}
#[derive(Clone, Debug)]
pub(super) enum Change {
    Mode(ViewMode),
    Sort(SortField),
    /// A List heading chose this field and direction.
    SortBy(SortField, bool),
    Descending(bool),
    Detail(Detail, bool),
    /// List view shows folders as a tree, in every folder.
    Tree(bool),
    /// Folders sort before files, in every folder.
    FoldersFirst(bool),
    Width(i16),
    Defaults,
}
#[derive(Clone, Debug)]
pub(super) enum Event {
    Show(window::Id),
    Close(window::Id),
    Change(Target, Change),
    /// A List view column heading was clicked: sort by it, or reverse it.
    Heading(window::Id, SortField),
    Saved(u64, Result<(), String>),
    Retry,
}

pub(super) struct Writer {
    path: PathBuf,
    revision: u64,
    active: Option<u64>,
    pending: Option<PreferenceStore>,
    pub error: Option<String>,
    last_folder: PathBuf,
}
impl Writer {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            revision: 0,
            active: None,
            pending: None,
            error: None,
            last_folder: PathBuf::from("/"),
        }
    }
    pub fn holds_application(&self) -> bool {
        self.active.is_some() || self.pending.is_some()
    }
    fn queue(&mut self, snapshot: PreferenceStore, folder: PathBuf) -> Task<Message> {
        self.revision += 1;
        self.error = None;
        self.last_folder = folder;
        if self.active.is_some() {
            self.pending = Some(snapshot);
            Task::none()
        } else {
            self.start(snapshot)
        }
    }
    fn start(&mut self, snapshot: PreferenceStore) -> Task<Message> {
        let revision = self.revision;
        self.active = Some(revision);
        Task::perform(persist(self.path.clone(), snapshot), move |result| {
            Message::FolderView(Event::Saved(revision, result))
        })
    }
    fn complete(&mut self, revision: u64, result: Result<(), String>) -> Task<Message> {
        if self.active != Some(revision) {
            return Task::none();
        }
        self.active = None;
        if revision == self.revision {
            self.error = result.err();
        }
        self.pending
            .take()
            .map_or_else(Task::none, |snapshot| self.start(snapshot))
    }
}
async fn persist(path: PathBuf, snapshot: PreferenceStore) -> Result<(), String> {
    let (send, receive) = oneshot::channel();
    std::thread::Builder::new()
        .name("files-folder-views".into())
        .spawn(move || {
            let _ = send.send(snapshot.save(&path).map_err(|error| error.to_string()));
        })
        .map_err(|error| error.to_string())?;
    receive
        .await
        .map_err(|_| fl!("files-view-state-worker-stopped"))?
}

impl Desktop {
    fn folder_target(&self, id: window::Id) -> Option<Target> {
        let tab = self.active(id).filter(|tab| !tab.loading)?;
        Some(Target {
            window: id,
            tab: tab.id,
            navigation: tab.revision,
            path: tab.browser.as_ref()?.path().to_owned(),
        })
    }
    fn matches_folder(&self, target: &Target) -> bool {
        self.folder_target(target.window).as_ref() == Some(target)
    }
    pub(super) fn sync_folder_options(&mut self) {
        if self
            .folder_options
            .as_ref()
            .is_some_and(|target| !self.matches_folder(target))
        {
            self.folder_options = None;
        }
    }
    /// Save folder views after a change made outside the View options.
    pub(super) fn save_folder_views(&mut self, folder: PathBuf) -> Task<Message> {
        if self.picker.is_some() {
            Task::none()
        } else {
            self.folder_writer.queue(self.preferences.clone(), folder)
        }
    }
    /// Show the active tab of window `id` in `mode` for now: the folder's
    /// saved view options stay as they are.
    pub(super) fn show_view_mode(&mut self, id: window::Id, mode: ViewMode) -> Task<Message> {
        let Some(tab) = self.active_mut(id).filter(|tab| !tab.loading) else {
            return Task::none();
        };
        tab.view_mode = Some(mode);
        let Some(folder) = tab
            .browser
            .as_ref()
            .map(|browser| browser.path().to_owned())
        else {
            return Task::none();
        };
        self.publish_folder_views(Some(&folder))
    }
    pub(super) fn publish_folder_views(&mut self, folder: Option<&Path>) -> Task<Message> {
        let mut changed = Vec::new();
        for (id, window) in &mut self.windows {
            let mut active_changed = false;
            for (index, tab) in window.tabs.iter_mut().enumerate() {
                let Some(browser) = tab.browser.as_mut() else {
                    continue;
                };
                if let Some(folder) = folder.filter(|folder| browser.path() != *folder) {
                    // A changed folder shown as an ancestor column is listed
                    // again in its new order.
                    if tab.columns.remove(folder).is_some() {
                        active_changed |= index == window.active;
                    }
                    continue;
                }
                let mut view = self.preferences.for_folder(browser.path()).clone();
                if let Some(mode) = tab.view_mode {
                    view.mode = mode;
                }
                if browser.view() == &view {
                    continue;
                }
                // A new column width only lays the same listings out again.
                let width_only = {
                    let mut old = browser.view().clone();
                    old.column_width = view.column_width;
                    old == view
                };
                if browser.set_view(view).is_ok() && !width_only {
                    tab.list_epoch += 1;
                    tab.reveal_on_show = true;
                    tab.columns.clear();
                    active_changed |= index == window.active;
                }
            }
            if active_changed {
                changed.push(*id);
            }
        }
        Task::batch(
            changed
                .into_iter()
                .flat_map(|id| [self.restore_scroll(id), self.load_columns(id)]),
        )
    }
    pub(super) fn folder_view_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Show(id) => {
                let Some(target) = self.folder_target(id) else {
                    return Task::none();
                };
                let tab = target.tab;
                self.folder_options = Some(target);
                self.action_menu = None;
                if let Some(tab) = self.active_mut(id) {
                    tab.location = None;
                }
                use iced::advanced::widget::{operate, operation};
                // The popup is outside this scoped Browser subtree: this cannot
                // erase its synchronous default focus after it has been laid out.
                operate(operation::scope(
                    view::root_id(id, tab),
                    operation::focusable::unfocus(),
                ))
            }
            Event::Close(id) => {
                if self
                    .folder_options
                    .as_ref()
                    .is_some_and(|target| target.window == id)
                {
                    self.folder_options = None;
                }
                Task::none()
            }
            Event::Change(target, change) => {
                if !self.matches_folder(&target) {
                    return Task::none();
                }
                let close = if matches!(change, Change::Mode(_) | Change::Defaults) {
                    self.close_search(target.window)
                } else {
                    Task::none()
                };
                // These choices belong to every folder, not to this one.
                match change {
                    Change::Tree(tree) => {
                        return self.change_shared_view(target.window, |view| view.tree = tree);
                    }
                    Change::FoldersFirst(first) => {
                        return self
                            .change_shared_view(target.window, |view| view.folders_first = first);
                    }
                    _ => {}
                }
                let temporary = matches!(change, Change::Mode(_) | Change::Defaults)
                    && self
                        .active_mut(target.window)
                        .and_then(|tab| tab.view_mode.take())
                        .is_some();
                let mut view = self.preferences.for_folder(&target.path).clone();
                let before = view.clone();
                let reset = matches!(change, Change::Defaults);
                match change {
                    Change::Mode(mode) => view.mode = mode,
                    Change::Sort(sort) => view.sort = sort,
                    Change::SortBy(sort, descending) => {
                        view.sort = sort;
                        view.descending = descending;
                    }
                    Change::Descending(descending) => view.descending = descending,
                    Change::Detail(detail, show) => {
                        view.details.retain(|old| *old != detail);
                        if show {
                            view.details.push(detail);
                        }
                    }
                    Change::Tree(_) | Change::FoldersFirst(_) => {}
                    Change::Width(delta) => {
                        view.column_width = step_width(view.column_width, delta)
                    }
                    Change::Defaults => {}
                }
                // Ending a temporary view shows the saved one even when the
                // saved options themselves do not change.
                let unchanged = if temporary {
                    self.publish_folder_views(Some(&target.path))
                } else {
                    Task::none()
                };
                if reset {
                    if self.preferences.folder_override(&target.path).is_none() {
                        return Task::batch([close, unchanged]);
                    }
                    self.preferences.use_defaults(&target.path);
                } else {
                    if view == before {
                        return Task::batch([close, unchanged]);
                    }
                    if let Err(error) = self.preferences.remember_changes(&target.path, view) {
                        self.folder_writer.error = Some(error.to_string());
                        return close;
                    }
                }
                let publish = self.publish_folder_views(Some(&target.path));
                let save = self
                    .folder_writer
                    .queue(self.preferences.clone(), target.path);
                Task::batch([close, publish, save])
            }
            Event::Heading(id, field) => {
                let Some(target) = self.folder_target(id) else {
                    return Task::none();
                };
                let view = self.preferences.for_folder(&target.path);
                // The sorting heading reverses; another one starts in the order
                // people usually want first: newest and largest at the top.
                let change = if view.sort == field {
                    Change::Descending(!view.descending)
                } else {
                    Change::SortBy(
                        field,
                        matches!(field, SortField::Size | SortField::Modified),
                    )
                };
                self.folder_view_event(Event::Change(target, change))
            }
            Event::Saved(revision, result) => {
                if self.folder_writer.active != Some(revision) {
                    return Task::none();
                }
                let save = self.folder_writer.complete(revision, result);
                let attention = if self.folder_writer.error.is_some() && self.windows.is_empty() {
                    self.new_window(vec![self.folder_writer.last_folder.clone()])
                } else {
                    self.exit_if_empty()
                };
                Task::batch([save, attention])
            }
            Event::Retry => self.folder_writer.queue(
                self.preferences.clone(),
                self.folder_writer.last_folder.clone(),
            ),
        }
    }
}

fn control_id(target: &Target, index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!(
        "files-folder-options-{:?}-{}-{index}",
        target.window, target.tab
    ))
}
pub(super) fn overlay<'a>(
    desktop: &'a Desktop,
    target: &'a Target,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    use canopa_theme::ColorRole;
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let label = appearance.typography().label_size.px() + 1.0;
    let mut bold = font;
    bold.weight = iced::font::Weight::Semibold;
    let settings = desktop.preferences.for_folder(&target.path);
    let scroll = iced::widget::Id::from(format!("files-folder-options-scroll-{:?}", target.window));
    let mut order = Vec::new();
    let mut reveal = Vec::new();
    let mut next = 0;
    let message_for = |change: Option<Change>| {
        Message::FolderView(change.map_or(Event::Close(target.window), |change| {
            Event::Change(target.clone(), change)
        }))
    };
    // Every control keeps its id and place in the keyboard order; only its
    // look follows the rest of Files.
    let mut register = |enabled: bool, in_form: bool| {
        let id = control_id(target, next);
        next += 1;
        if enabled {
            order.push(id.clone());
            if in_form {
                reveal.push((id.clone(), scroll.clone()));
            }
        }
        id
    };
    let close = {
        let id = register(true, false);
        let message = Some(message_for(None));
        let button = canopa_ui::styled_action(
            container(desktop.icons.chrome(chrome::Icon::Close, 16.0, 0.86))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            message.clone(),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(30)
        .height(30)
        .padding(4)
        .style(chrome::close_style(appearance));
        super::accessibility_ui::named_button(
            super::accessibility_ui::focus_key(&id),
            fl!("files-close"),
            message.clone(),
            focus::control(id, message, appearance, button),
        )
    };
    // A segment of an equal-width choice, the chosen one on the accent.
    let mut segment = |title: String, change: Change, selected: bool| -> Element<'a, Message> {
        let id = register(true, true);
        // View choices carry the view's symbol, as the toolbar's view button.
        let symbol = match change {
            Change::Mode(mode) if selected => Some(super::view::view_icon_on_accent(mode)),
            Change::Mode(mode) => Some(super::view::view_icon(mode)),
            _ => None,
        };
        let message = Some(message_for(Some(change)));
        let mut text = text(title).size(label).font(font);
        if selected {
            text = text.color(chrome::ink(appearance, ColorRole::OnAccent));
        }
        let content: Element<'a, Message> = match symbol {
            Some(symbol) => row![desktop.icons.chrome(symbol, 15.0, 0.9), text]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into(),
            None => text.into(),
        };
        let button = iced::widget::button(container(content).center_x(Length::Fill))
            .padding([5, 6])
            .width(Length::Fill)
            .on_press_maybe(message.clone());
        let button = if selected {
            button.style(chrome::selected_segment(appearance))
        } else {
            button.style(|theme, status| {
                let mut style =
                    canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
                style.border.radius = 11.0.into();
                style
            })
        };
        focus::button(id, message, appearance, button)
    };
    let track = |segments: Vec<Element<'a, Message>>| -> Element<'a, Message> {
        container(
            iced::widget::Row::from_vec(segments)
                .spacing(2)
                .width(Length::Fill),
        )
        .padding(3)
        .width(Length::Fill)
        .style(move |_| chrome::segmented(appearance))
        .into()
    };
    // The view shown is the tab's, which the view button may have changed
    // for now; choosing one here saves it for the folder.
    let shown = desktop
        .windows
        .get(&target.window)
        .and_then(|window| window.tabs.iter().find(|tab| tab.id == target.tab))
        .and_then(|tab| tab.view_mode)
        .unwrap_or(settings.mode);
    let modes = vec![
        segment(
            fl!("files-view-columns"),
            Change::Mode(ViewMode::Columns),
            shown == ViewMode::Columns,
        ),
        segment(
            fl!("files-view-list"),
            Change::Mode(ViewMode::List),
            shown == ViewMode::List,
        ),
        segment(
            fl!("files-view-icons"),
            Change::Mode(ViewMode::Icons),
            shown == ViewMode::Icons,
        ),
    ];
    let mode = track(modes);
    let sorts = vec![
        segment(
            fl!("files-column-name"),
            Change::Sort(SortField::Name),
            settings.sort == SortField::Name,
        ),
        segment(
            fl!("files-column-size"),
            Change::Sort(SortField::Size),
            settings.sort == SortField::Size,
        ),
        segment(
            fl!("files-column-modified"),
            Change::Sort(SortField::Modified),
            settings.sort == SortField::Modified,
        ),
        segment(
            fl!("files-column-kind"),
            Change::Sort(SortField::Kind),
            settings.sort == SortField::Kind,
        ),
    ];
    let sort = track(sorts);
    // The column width steps with round buttons around its value.
    let mut stepper = |glyph: chrome::Icon,
                       name: String,
                       change: Change,
                       enabled: bool|
     -> Element<'a, Message> {
        let id = register(enabled, true);
        let message = enabled.then(|| message_for(Some(change)));
        let button = canopa_ui::styled_action(
            container(
                desktop
                    .icons
                    .chrome(glyph, 14.0, if enabled { 0.9 } else { 0.35 }),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill),
            message.clone(),
            canopa_ui::Action::Quiet,
            false,
            appearance,
        )
        .width(28)
        .height(28)
        .padding(4)
        .style(chrome::close_style(appearance));
        super::accessibility_ui::named_button(
            super::accessibility_ui::focus_key(&id),
            name,
            message.clone(),
            focus::control(id, message, appearance, button),
        )
    };
    let narrower = stepper(
        chrome::Icon::Minus,
        fl!("files-column-narrower"),
        Change::Width(-WIDTH_STEP),
        settings.column_width > 160,
    );
    let wider = stepper(
        chrome::Icon::Plus,
        fl!("files-column-wider"),
        Change::Width(WIDTH_STEP),
        settings.column_width < 640,
    );
    let widths = row![
        text(fl!("files-preferences-column-width"))
            .size(size)
            .font(font)
            .width(Length::Fill),
        narrower,
        text(fl!("files-pixels", pixels = settings.column_width))
            .size(label)
            .font(font)
            .width(64)
            .align_x(iced::alignment::Horizontal::Center),
        wider,
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let has_override = desktop.preferences.folder_override(&target.path).is_some();
    let reset = {
        let id = register(has_override, true);
        let message = has_override.then(|| message_for(Some(Change::Defaults)));
        focus::button(
            id,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(fl!("files-preferences-use-defaults"))
                    .size(label)
                    .font(font),
                message,
                canopa_ui::Action::Normal,
                false,
                appearance,
            )
            .padding([5, 12]),
        )
    };
    // On/off choices are switches, as in Preferences.
    let switch = |id: iced::widget::Id,
                  title: String,
                  value: bool,
                  change: Change|
     -> Element<'a, Message> {
        let message = message_for(Some(change));
        let toggled = message.clone();
        let control = iced::widget::toggler(value)
            .size(20)
            .style(canopa_ui::switch_style)
            .on_toggle(move |_| toggled.clone());
        focus::check_box(
            id,
            value,
            Some(message.clone()),
            appearance,
            mouse_area(
                container(
                    row![
                        text(title).size(size).font(font).width(Length::Fill),
                        control
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::Center),
                )
                .padding([2, 0]),
            )
            .on_press(message),
        )
    };
    let reverse = switch(
        register(true, true),
        fl!("files-folder-reverse-order"),
        settings.descending,
        Change::Descending(!settings.descending),
    );
    let folders_first = switch(
        register(true, true),
        fl!("files-folders-first"),
        settings.folders_first,
        Change::FoldersFirst(!settings.folders_first),
    );
    let mut details = row![].spacing(14).align_y(iced::Alignment::Center);
    for (title, detail) in [
        (fl!("files-column-size"), Detail::Size),
        (fl!("files-folder-modified-time"), Detail::Modified),
        (fl!("files-column-kind"), Detail::Kind),
    ] {
        let shown = settings.details.contains(&detail);
        let id = register(true, true);
        let message = message_for(Some(Change::Detail(detail, !shown)));
        details = details.push(focus::check_box(
            id,
            shown,
            Some(message.clone()),
            appearance,
            checkbox(shown)
                .label(title)
                .size(16)
                .text_size(size)
                .font(font)
                .style(canopa_ui::checkbox_style)
                .on_toggle(move |_| message.clone()),
        ));
    }
    let tree = switch(
        register(true, true),
        fl!("files-folder-tree"),
        settings.tree,
        Change::Tree(!settings.tree),
    );
    // Control order follows visual order even though closures build the groups.
    let indices = [0, 1, 2, 3, 4, 5, 6, 7, 11, 12, 8, 9, 13, 14, 15, 16, 10];
    order.sort_by_key(|id| {
        indices
            .iter()
            .position(|index| *id == control_id(target, *index))
            .unwrap_or(usize::MAX)
    });
    let caption = |title: String| -> Element<'a, Message> {
        text(title)
            .size(label)
            .font(bold)
            .color(chrome::ink(appearance, ColorRole::Muted))
            .into()
    };
    let divider = || -> Element<'a, Message> {
        container(iced::widget::rule::horizontal(1).style(chrome::divider))
            .padding([2, 0])
            .into()
    };
    // The same settings, in the same groups and order, as Preferences' Views.
    let section =
        |title: String| -> Element<'a, Message> { text(title).size(size).font(bold).into() };
    let form = column![
        section(fl!("files-preferences-section-browsing")),
        column![caption(fl!("files-view")), mode].spacing(6),
        column![caption(fl!("files-preferences-sort-by")), sort].spacing(6),
        column![reverse, folders_first].spacing(6),
        divider(),
        section(fl!("files-preferences-section-layouts")),
        widths,
        column![caption(fl!("files-preferences-list-view")), details].spacing(6),
        tree,
    ]
    .spacing(10);
    let folder = target.path.file_name().map_or_else(
        || display_location(&target.path),
        |name| name.to_string_lossy().into_owned(),
    );
    let heading = row![
        column![
            text(fl!("files-view-options-title"))
                .size(size + 1.0)
                .font(bold),
            text(folder)
                .size(label)
                .font(font)
                .color(chrome::ink(appearance, ColorRole::Muted)),
        ]
        .spacing(2)
        .width(Length::Fill),
        close,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let footer = row![
        text(fl!("files-folder-remembered"))
            .size(label)
            .font(font)
            .color(chrome::ink(appearance, ColorRole::Muted))
            .width(Length::Fill),
        reset,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let panel = column![
        heading,
        divider(),
        scrollable(form)
            .id(scroll.clone())
            .style(canopa_ui::scrollbar)
            .height(Length::Shrink),
        divider(),
        footer,
    ]
    .spacing(12);
    let styled = appearance.clone();
    let panel = container(panel)
        .padding(16)
        .width(360)
        .max_height(600)
        .style(move |_| super::actions_ui::menu_surface(&styled));
    let panel = focus::group(panel, order, reveal, Some(control_id(target, 0)));
    stack![
        content,
        mouse_area(space().width(Length::Fill).height(Length::Fill))
            .on_press(Message::FolderView(Event::Close(target.window)))
            .on_right_press(Message::FolderView(Event::Close(target.window))),
        // Below the toolbar at the window's end, where its menus open.
        container(opaque(panel))
            .padding(iced::Padding {
                top: 62.0,
                right: 12.0,
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .align_right(Length::Fill)
            .align_top(Length::Fill)
    ]
    .into()
}

/// The column width steps by.
pub(super) const WIDTH_STEP: i16 = 20;

/// A column width one step wider or narrower, on the step's multiples: a
/// width dragged to 287 steps to 300 or 280, not to 307 or 267.
pub(super) fn step_width(width: u16, delta: i16) -> u16 {
    let step = i32::from(WIDTH_STEP);
    let width = i32::from(width);
    let snapped = if delta > 0 {
        (width.div_euclid(step) + 1) * step
    } else {
        (width + step - 1).div_euclid(step) * step - step
    };
    snapped.clamp(160, 640) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changes_publish_to_other_tabs_without_pinning_unrelated_defaults_and_old_targets_are_refused()
     {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.txt"), b"a").unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned(); 2]);
        let tabs: Vec<_> = desktop.windows[&window]
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect();
        for (index, tab) in tabs.iter().enumerate() {
            let _ = desktop.update(Message::SwitchTab(window, index));
            let revision = desktop.active(window).unwrap().revision;
            super::super::tests::loaded(&mut desktop, window, *tab, revision, root.path());
        }
        let _ = desktop.update(Message::SwitchTab(window, 0));
        let target = desktop.folder_target(window).unwrap();
        let _ =
            desktop.folder_view_event(Event::Change(target.clone(), Change::Mode(ViewMode::Icons)));
        assert!(
            desktop.windows[&window].tabs.iter().all(|tab| tab
                .browser
                .as_ref()
                .unwrap()
                .view()
                .mode
                == ViewMode::Icons)
        );
        let _ = desktop.update(Message::SwitchTab(window, 1));
        let _ =
            desktop.folder_view_event(Event::Change(target.clone(), Change::Sort(SortField::Size)));
        assert_eq!(
            desktop.preferences.for_folder(root.path()).sort,
            SortField::Name
        );
        let other = desktop.folder_target(window).unwrap();
        let _ = desktop.folder_view_event(Event::Change(
            other.clone(),
            Change::Detail(Detail::Kind, true),
        ));
        let defaults = crate::preferences::ViewSettings {
            descending: true,
            column_width: 320,
            ..Default::default()
        };
        desktop.preferences.set_defaults(defaults.clone()).unwrap();
        let _ = desktop.publish_folder_views(None);
        let view = desktop.preferences.for_folder(root.path());
        assert_eq!(view.mode, ViewMode::Icons);
        assert!(view.descending);
        assert_eq!(view.column_width, 320);
        assert!(view.details.contains(&Detail::Kind));
        let _ = desktop.folder_view_event(Event::Change(other, Change::Defaults));
        assert_eq!(desktop.preferences.for_folder(root.path()), &defaults);
        assert!(desktop.preferences.folder_override(root.path()).is_none());
    }
    #[test]
    fn the_view_switch_changes_only_this_tab_and_view_options_save_and_end_it() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let saved = desktop.preferences.for_folder(root.path()).mode;
        let shown = if saved == ViewMode::Icons {
            ViewMode::List
        } else {
            ViewMode::Icons
        };
        let command = match shown {
            ViewMode::Icons => Command::Icons,
            _ => Command::List,
        };
        let _ = desktop.update(Message::Command(window, command));
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.view().mode, shown);
        // Nothing is remembered for the folder.
        assert!(desktop.preferences.folder_override(root.path()).is_none());
        assert_eq!(desktop.preferences.for_folder(root.path()).mode, saved);
        // Choosing the saved view in View options shows it again.
        let target = desktop.folder_target(window).unwrap();
        let _ = desktop.folder_view_event(Event::Change(target, Change::Mode(saved)));
        let current = desktop.active(window).unwrap();
        assert_eq!(current.view_mode, None);
        assert_eq!(current.browser.as_ref().unwrap().view().mode, saved);
    }

    #[test]
    fn tree_and_folders_first_are_one_choice_for_every_folder() {
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join("other");
        std::fs::create_dir(&other).unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let target = desktop.folder_target(window).unwrap();
        let _ = desktop.folder_view_event(Event::Change(target.clone(), Change::Tree(true)));
        let _ = desktop.folder_view_event(Event::Change(target, Change::FoldersFirst(false)));
        // Not remembered for this folder: the defaults change, for every folder.
        assert!(desktop.preferences.folder_override(root.path()).is_none());
        for folder in [root.path(), other.as_path()] {
            let view = desktop.preferences.for_folder(folder);
            assert!(view.tree && !view.folders_first);
        }
        assert!(desktop.configuration.settings.view.tree);
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert!(browser.view().tree && !browser.view().folders_first);
    }

    #[test]
    fn list_headings_sort_by_their_field_then_reverse_it() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let view = |desktop: &Desktop| {
            let view = desktop.preferences.for_folder(root.path());
            (view.sort, view.descending)
        };
        assert_eq!(view(&desktop), (SortField::Name, false));
        let _ = desktop.folder_view_event(Event::Heading(window, SortField::Name));
        assert_eq!(view(&desktop), (SortField::Name, true));
        // Dates and sizes start with the newest and largest.
        let _ = desktop.folder_view_event(Event::Heading(window, SortField::Modified));
        assert_eq!(view(&desktop), (SortField::Modified, true));
        let _ = desktop.folder_view_event(Event::Heading(window, SortField::Modified));
        assert_eq!(view(&desktop), (SortField::Modified, false));
        let _ = desktop.folder_view_event(Event::Heading(window, SortField::Kind));
        assert_eq!(view(&desktop), (SortField::Kind, false));
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.view().sort, SortField::Kind);
    }

    #[test]
    fn serialized_writer_keeps_the_latest_snapshot_and_reports_failures_without_erasing_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("views.json");
        let mut writer = Writer::new(path.clone());
        let mut preferences = PreferenceStore::new(Default::default());
        let _first = writer.queue(preferences.clone(), root.path().to_owned());
        for mode in [ViewMode::List, ViewMode::Icons] {
            let mut view = preferences.for_folder(root.path()).clone();
            view.mode = mode;
            preferences.remember_changes(root.path(), view).unwrap();
            let _pending = writer.queue(preferences.clone(), root.path().to_owned());
        }
        assert!(writer.holds_application());
        assert_eq!(writer.active, Some(1));
        let pending = writer.pending.take().unwrap();
        iced::futures::executor::block_on(persist(path.clone(), pending)).unwrap();
        let loaded = PreferenceStore::load(&path, Default::default()).unwrap();
        assert_eq!(loaded.for_folder(root.path()).mode, ViewMode::Icons);
        let _ = writer.complete(0, Err("old".into()));
        assert_eq!(writer.active, Some(1));
        assert!(writer.error.is_none());
        writer.active = Some(3);
        let _ = writer.complete(3, Err("disk unavailable".into()));
        assert_eq!(writer.error.as_deref(), Some("disk unavailable"));
        assert!(!writer.holds_application());
        assert_eq!(preferences.for_folder(root.path()).mode, ViewMode::Icons);
    }
}
