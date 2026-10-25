//! Immediate shared folder choices, with serialized/coalesced private-state writes.
use super::*;
use crate::preferences::{Detail, SortField, ViewMode};
use canopa_theme::UiKind;
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
    Descending(bool),
    Detail(Detail, bool),
    Width(i16),
    Defaults,
}
#[derive(Clone, Debug)]
pub(super) enum Event {
    Show(window::Id),
    Close(window::Id),
    Change(Target, Change),
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
        .map_err(|_| "The view-state worker stopped before saving.".to_owned())?
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
    pub(super) fn change_folder_mode(&mut self, id: window::Id, mode: ViewMode) -> Task<Message> {
        let Some(target) = self.folder_target(id) else {
            return Task::none();
        };
        self.folder_view_event(Event::Change(target, Change::Mode(mode)))
    }
    pub(super) fn publish_folder_views(&mut self, folder: Option<&Path>) -> Task<Message> {
        let mut changed = Vec::new();
        for (id, window) in &mut self.windows {
            let mut active_changed = false;
            for (index, tab) in window.tabs.iter_mut().enumerate() {
                let Some(browser) = tab.browser.as_mut() else {
                    continue;
                };
                if folder.is_some_and(|folder| browser.path() != folder) {
                    continue;
                }
                let view = self.preferences.for_folder(browser.path()).clone();
                if browser.view() == &view {
                    continue;
                }
                if browser.set_view(view).is_ok() {
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
                let mut view = self.preferences.for_folder(&target.path).clone();
                let before = view.clone();
                let reset = matches!(change, Change::Defaults);
                match change {
                    Change::Mode(mode) => view.mode = mode,
                    Change::Sort(sort) => view.sort = sort,
                    Change::Descending(descending) => view.descending = descending,
                    Change::Detail(detail, show) => {
                        view.details.retain(|old| *old != detail);
                        if show {
                            view.details.push(detail);
                        }
                    }
                    Change::Width(delta) => {
                        view.column_width =
                            (i32::from(view.column_width) + i32::from(delta)).clamp(160, 640) as u16
                    }
                    Change::Defaults => {}
                }
                if reset {
                    if self.preferences.folder_override(&target.path).is_none() {
                        return close;
                    }
                    self.preferences.use_defaults(&target.path);
                } else {
                    if view == before {
                        return close;
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
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let label = appearance.typography().label_size.px();
    let settings = desktop.preferences.for_folder(&target.path);
    let scroll = iced::widget::Id::from(format!("files-folder-options-scroll-{:?}", target.window));
    let mut order = Vec::new();
    let mut reveal = Vec::new();
    let mut next = 0;
    let mut action = |title: &'a str,
                      change: Option<Change>,
                      selected: bool,
                      enabled: bool,
                      in_form: bool|
     -> Element<'a, Message> {
        let id = control_id(target, next);
        next += 1;
        if enabled {
            order.push(id.clone());
            if in_form {
                reveal.push((id.clone(), scroll.clone()));
            }
        }
        let event = change.map_or(Event::Close(target.window), |change| {
            Event::Change(target.clone(), change)
        });
        let message = enabled.then_some(Message::FolderView(event));
        focus::control(
            id,
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(title).size(label).font(font),
                message,
                if selected {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                false,
                appearance,
            ),
        )
    };
    let close = action("Close", None, false, true, false);
    let mode = row![
        action(
            "Columns",
            Some(Change::Mode(ViewMode::Columns)),
            settings.mode == ViewMode::Columns,
            true,
            true
        ),
        action(
            "List",
            Some(Change::Mode(ViewMode::List)),
            settings.mode == ViewMode::List,
            true,
            true
        ),
        action(
            "Icons",
            Some(Change::Mode(ViewMode::Icons)),
            settings.mode == ViewMode::Icons,
            true,
            true
        )
    ]
    .spacing(4);
    let sort = column![
        row![
            action(
                "Name",
                Some(Change::Sort(SortField::Name)),
                settings.sort == SortField::Name,
                true,
                true
            ),
            action(
                "Size",
                Some(Change::Sort(SortField::Size)),
                settings.sort == SortField::Size,
                true,
                true
            )
        ]
        .spacing(4),
        row![
            action(
                "Modified",
                Some(Change::Sort(SortField::Modified)),
                settings.sort == SortField::Modified,
                true,
                true
            ),
            action(
                "Kind",
                Some(Change::Sort(SortField::Kind)),
                settings.sort == SortField::Kind,
                true,
                true
            )
        ]
        .spacing(4)
    ]
    .spacing(4);
    let widths = row![
        action(
            "−",
            Some(Change::Width(-20)),
            false,
            settings.column_width > 160,
            true
        ),
        container(
            text(format!("{} px", settings.column_width))
                .size(size)
                .font(font)
        )
        .width(Length::Fill),
        action(
            "+",
            Some(Change::Width(20)),
            false,
            settings.column_width < 640,
            true
        )
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let reset = action(
        "Use defaults",
        Some(Change::Defaults),
        false,
        desktop.preferences.folder_override(&target.path).is_some(),
        true,
    );
    let mut check = |title: &'a str, value: bool, change: Change| -> Element<'a, Message> {
        let id = control_id(target, next);
        next += 1;
        order.push(id.clone());
        reveal.push((id.clone(), scroll.clone()));
        let message = Message::FolderView(Event::Change(target.clone(), change));
        focus::control(
            id,
            Some(message.clone()),
            appearance,
            checkbox(value)
                .label(title)
                .size(16)
                .text_size(size)
                .font(font)
                .on_toggle(move |_| message.clone())
                .width(Length::Fill),
        )
    };
    let reverse = check(
        "Reverse order",
        settings.descending,
        Change::Descending(!settings.descending),
    );
    let mut details = column![].spacing(8);
    for (title, detail) in [
        ("Size", Detail::Size),
        ("Modified time", Detail::Modified),
        ("Kind", Detail::Kind),
    ] {
        let shown = settings.details.contains(&detail);
        details = details.push(check(title, shown, Change::Detail(detail, !shown)));
    }
    // Control order follows visual order even though closures build the groups.
    let indices = [0, 1, 2, 3, 4, 5, 6, 7, 11, 8, 9, 12, 13, 14, 10];
    order.sort_by_key(|id| {
        indices
            .iter()
            .position(|index| *id == control_id(target, *index))
            .unwrap_or(usize::MAX)
    });
    let form = column![
        text(display_location(&target.path)).size(label).font(font),
        text("View").size(size).font(font),
        mode,
        text("Sort by").size(size).font(font),
        sort,
        reverse,
        text("Column width").size(size).font(font),
        widths,
        text("List details").size(size).font(font),
        details,
        reset
    ]
    .spacing(10);
    let panel = column![
        row![
            text("This folder")
                .size(size)
                .font(font)
                .width(Length::Fill),
            close
        ]
        .align_y(iced::Alignment::Center),
        scrollable(form).id(scroll.clone()).height(Length::Fill),
        text("Choices are remembered for this folder.")
            .size(label)
            .font(font)
    ]
    .spacing(8);
    let styled = appearance.clone();
    let panel = container(panel)
        .padding(12)
        .width(Length::Fill)
        .max_width(340)
        .height(Length::Fill)
        .max_height(540)
        .style(move |_| canopa_ui::frame(&styled, styled.style(UiKind::Menu)));
    let panel = focus::group(panel, order, reveal, Some(control_id(target, 0)));
    stack![
        content,
        mouse_area(space().width(Length::Fill).height(Length::Fill))
            .on_press(Message::FolderView(Event::Close(target.window)))
            .on_right_press(Message::FolderView(Event::Close(target.window))),
        container(opaque(panel))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_right(Length::Fill)
            .align_top(Length::Fill)
    ]
    .into()
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
