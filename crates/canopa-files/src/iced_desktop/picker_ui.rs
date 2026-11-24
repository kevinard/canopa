//! A request-scoped window using the regular Files browser and navigation.
use super::accessibility_ui::{Node, semantic};
use super::*;
use crate::picker::{Prepared, filters::MimeGlobs, folders};
use canopa_file::picker::{ChooserRequest, Mode, Selection, path_bytes};
use iced::{
    Alignment, Length,
    widget::{checkbox, column, container, pick_list, row, rule, space, text, text_input},
};
use std::{
    ffi::OsString,
    io::{Read, Write},
    sync::Mutex,
};

pub fn run(request: canopa_config::Request) -> io::Result<()> {
    const MAX_REQUEST: u64 = 1024 * 1024;
    let mut input = Vec::new();
    io::stdin().take(MAX_REQUEST + 1).read_to_end(&mut input)?;
    if input.len() as u64 > MAX_REQUEST {
        return Err(io::Error::other("Picker request too large"));
    }
    let request_body: ChooserRequest = serde_json::from_slice(&input)?;
    let configuration = ConfigurationSnapshot::load(&request).map_err(io::Error::other)?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from("/"));
    let config_home = canopa_config::config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
    )
    .map_err(io::Error::other)?;
    let places = PersonalFolders::load(&home, &config_home)?;
    let preference_path = crate::application::state_directory(&home).join("views.json");
    let preferences = PreferenceStore::load(&preference_path, configuration.settings.view.clone())?;
    let font = canopa_ui::font(&configuration.appearance);
    let picker = Picker::new(request_body);
    let monitor = crate::local_volumes::Monitor::observe_devices()
        .ok()
        .map(Arc::new);
    let notices = monitor
        .as_ref()
        .and_then(|monitor| monitor.take_notices())
        .map(activation_ui::Notices::new);
    let result = picker.result.clone();

    iced::daemon(
        move || {
            let (mut desktop, boot) = Desktop::boot(
                None,
                configuration.clone(),
                preferences.clone(),
                home.clone(),
                preference_path.clone(),
                places.clone(),
                Ok(None),
            );
            let suggested = picker.request.current_file.as_deref().and_then(path_bytes);
            let folder = suggested
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_owned)
                .or_else(|| {
                    picker
                        .request
                        .current_folder
                        .as_deref()
                        .and_then(path_bytes)
                })
                .unwrap_or_else(|| home.clone());
            let mut tab = desktop.new_tab(folder.clone());
            if picker.request.mode == Mode::Open {
                tab.initial_selection.extend(suggested);
            }
            desktop.picker = Some(picker.clone());
            desktop.accessibility = Some(accessibility_ui::Accessibility::new());
            desktop.network = network_ui::State::start();
            desktop.folder_watch = folder_watch_ui::State::start();
            desktop.picker_monitor.clone_from(&monitor);
            desktop.picker_device_notices.clone_from(&notices);
            if let Some(monitor) = &monitor {
                desktop.devices.catalog = monitor.snapshot();
            }
            let (id, open) = desktop.open_browser_window(vec![tab]);
            let scan = desktop.navigate(id, folder, Navigation::Visit, None);
            let bookmarks = desktop.load_gtk_bookmarks();
            (desktop, Task::batch([boot, open, scan, bookmarks]))
        },
        Desktop::update,
        Desktop::view,
    )
    .title(Desktop::title)
    .theme(|desktop: &Desktop, _| desktop.theme.clone())
    .subscription(Desktop::subscription)
    .default_font(font)
    .run()
    .map_err(io::Error::other)?;

    let bytes = serde_json::to_vec(&*result.lock().unwrap_or_else(|error| error.into_inner()))?;
    io::stdout().lock().write_all(&bytes)
}

#[derive(Clone)]
pub(super) struct Picker {
    pub request: ChooserRequest,
    result: Arc<Mutex<Option<Selection>>>,
    mime: Arc<MimeGlobs>,
    requester: String,
    name: String,
    original_name: Option<OsString>,
    writable: bool,
    busy: bool,
    error: Option<String>,
    prompt: Option<Prompt>,
}

#[derive(Clone)]
enum Prompt {
    Folder(Arc<folders::Parent>, String),
    Replace(Prepared),
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Accept,
    Cancel,
    Name(String),
    Filter(usize),
    Choice(usize, String),
    ReadOnly(bool),
    Checked(Result<Prepared, String>),
    FolderParent(Result<Arc<folders::Parent>, String>),
    FolderName(String),
    CreateFolder,
    FolderCreated(Result<PathBuf, String>),
    Replace,
    Back,
    Devices,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item(usize, String);
impl std::fmt::Display for Item {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.1)
    }
}

impl Picker {
    fn new(request: ChooserRequest) -> Self {
        let original_name = request
            .current_file
            .as_deref()
            .and_then(path_bytes)
            .and_then(|path| path.file_name().map(ToOwned::to_owned));
        let name = original_name
            .as_ref()
            .map(|name| name.to_string_lossy().into_owned())
            .or_else(|| request.current_name.clone())
            .unwrap_or_default();
        let requester = canopa_apps::desktop_entry_name(&request.app_id)
            .unwrap_or_else(|| request.app_id.clone())
            .chars()
            .filter(|ch| !ch.is_control())
            .take(120)
            .collect();
        Self {
            request,
            name,
            original_name,
            requester,
            writable: true,
            result: Arc::default(),
            mime: Arc::new(MimeGlobs::load()),
            busy: false,
            error: None,
            prompt: None,
        }
    }

    pub fn title(&self) -> String {
        let title = if self.request.title.is_empty() {
            if self.request.mode == Mode::Open {
                fl!("files-picker-accept-open")
            } else {
                fl!("files-picker-accept-save")
            }
        } else {
            self.request.title.clone()
        };
        fl!("files-window-title", title = title)
    }

    pub fn focus_name(&self) -> Task<Message> {
        if self.request.mode != Mode::Save {
            return Task::none();
        }
        use iced::widget::operation::{focus, select_all, select_range};
        let selection = match crate::browser_ui::text_layout::stem_length(&self.name) {
            Some(end) => select_range("picker-name", 0, end),
            None => select_all("picker-name"),
        };
        focus("picker-name").chain(selection)
    }

    pub fn allows(&self, command: Command) -> bool {
        match command {
            Command::NewFolder => self.request.mode != Mode::Open,
            Command::SelectAll => self.request.multiple,
            Command::ShowRecent => self.request.mode == Mode::Open && !self.request.directory,
            Command::Back
            | Command::Forward
            | Command::Parent
            | Command::EnterFolder
            | Command::Open
            | Command::Search
            | Command::SearchSubfolders
            | Command::StopSearch
            | Command::MoreSearchResults
            | Command::RevealSearchResult
            | Command::SearchProblems
            | Command::Location
            | Command::Refresh
            | Command::ShowHidden
            | Command::Columns
            | Command::List
            | Command::Icons => true,
            _ => false,
        }
    }

    pub fn blocks_browser(&self) -> bool {
        self.busy || self.prompt.is_some()
    }

    pub fn filter(&self, browser: &mut Browser) {
        let filter = self.request.current_filter.clone();
        let directories = self.request.directory;
        let mime = self.mime.clone();
        browser.set_entry_filter(move |entry| {
            entry.is_directory()
                || !directories
                    && filter
                        .as_ref()
                        .is_none_or(|filter| mime.matches(filter, &entry.path))
        });
    }

    fn selected(&self, browser: &Browser) -> Vec<PathBuf> {
        let mut paths: Vec<_> = browser
            .visible_entries()
            .into_iter()
            .filter(|entry| browser.selected().contains(&entry.path))
            .filter(|entry| entry.is_directory() == self.request.directory)
            .map(|entry| entry.path.clone())
            .collect();
        paths.sort_unstable();
        paths
    }

    fn replacement(&self, browser: &Browser) -> bool {
        let name = self.name();
        self.request.mode == Mode::Save
            && browser.directory().entries.iter().any(|entry| {
                !entry.is_directory() && entry.path.file_name() == Some(name.as_os_str())
            })
    }

    fn name(&self) -> OsString {
        self.original_name
            .clone()
            .unwrap_or_else(|| self.name.clone().into())
    }

    fn finish(&self, prepared: Prepared) -> Task<Message> {
        *self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Selection {
            paths: prepared
                .paths
                .into_iter()
                .map(|path| path.into_os_string().into_vec())
                .collect(),
            choices: self
                .request
                .choices
                .iter()
                .map(|(id, _, _, value)| (id.clone(), value.clone()))
                .collect(),
            current_filter: self.request.current_filter.clone(),
            writable: self.writable,
        });
        iced::exit()
    }

    pub fn footer<'a>(&'a self, desktop: &'a Desktop, id: window::Id) -> Element<'a, Message> {
        let appearance = &desktop.configuration.appearance;
        let browser = desktop
            .active(id)
            .filter(|tab| !tab.loading)
            .and_then(|tab| tab.browser.as_ref());
        let mut fields = column![].width(Length::Fill).spacing(8);
        let replace = browser.is_some_and(|browser| self.replacement(browser));
        if self.request.mode == Mode::Save {
            let label = fl!("files-picker-file-name");
            fields = fields.push(
                column![
                    text(fl!("files-picker-name-label")).size(14),
                    input(
                        appearance,
                        "picker-name",
                        label,
                        &self.name,
                        Event::Name,
                        Event::Accept
                    ),
                ]
                .spacing(4),
            );
            if replace {
                fields = fields.push(
                    text(fl!("files-picker-existing"))
                        .size(14)
                        .color(chrome::ink(appearance, canopa_theme::ColorRole::Warning)),
                );
            }
        }
        if self.request.mode == Mode::SaveMany {
            fields = fields.push(
                text(fl!(
                    "files-picker-save-many",
                    count = self.request.files.len()
                ))
                .size(14),
            );
        }
        let mut options = row![].spacing(12);
        if self.request.mode == Mode::Open {
            options = options.push(toggle(
                fl!("files-picker-read-only"),
                !self.writable,
                Event::ReadOnly,
            ));
        }
        if !self.request.filters.is_empty() {
            let values: Vec<_> = self
                .request
                .filters
                .iter()
                .enumerate()
                .map(|(i, filter)| Item(i, filter.0.clone()))
                .collect();
            let current = self.request.current_filter.as_ref().and_then(|wanted| {
                self.request
                    .filters
                    .iter()
                    .position(|filter| filter == wanted)
            });
            options = options.push(choice(
                fl!("files-picker-file-type"),
                &values,
                current,
                Event::Filter,
            ));
        }
        for (index, (_, label, choices, selected)) in self.request.choices.iter().enumerate() {
            if choices.is_empty() {
                options = options.push(toggle(label.clone(), selected == "true", move |checked| {
                    Event::Choice(index, checked.to_string())
                }));
            } else {
                let choices = choices.clone();
                let values = choices
                    .iter()
                    .enumerate()
                    .map(|(i, choice)| Item(i, choice.1.clone()))
                    .collect::<Vec<_>>();
                let current = choices.iter().position(|choice| choice.0 == *selected);
                options = options.push(choice(label.clone(), &values, current, move |value| {
                    Event::Choice(index, choices[value].0.clone())
                }));
            }
        }
        fields = fields.push(options.wrap().vertical_spacing(6));
        if let Some(error) = &self.error {
            fields = fields.push(semantic(
                Node::alert(error),
                text(error).style(text::danger),
            ));
        }
        let ready = browser.is_some_and(|browser| match self.request.mode {
            Mode::Open => self.request.directory || !self.selected(browser).is_empty(),
            Mode::Save => !self.name.is_empty(),
            Mode::SaveMany => true,
        }) && !self.busy;
        let accept = if replace {
            fl!("files-picker-review-replacement")
        } else {
            self.request.accept_label.replace('_', "")
        };
        let mut decisions = row![].align_y(Alignment::Center).spacing(8);
        if !self.requester.is_empty() {
            decisions = decisions.push(
                text(fl!(
                    "files-picker-requested-by",
                    requester = self.requester.as_str()
                ))
                .size(13)
                .style(canopa_ui::muted),
            );
        }
        decisions = decisions.push(space().width(Length::Fill));
        if !ready && self.request.mode == Mode::Open && !self.request.directory {
            decisions = decisions.push(text(fl!("files-picker-select-file")).size(14));
        }
        fields = fields.push(
            decisions
                .push(action(
                    appearance,
                    fl!("files-cancel"),
                    Some(Event::Cancel),
                    canopa_ui::Action::Normal,
                ))
                .push(action(
                    appearance,
                    accept,
                    ready.then_some(Event::Accept),
                    canopa_ui::Action::Primary,
                )),
        );
        column![
            rule::horizontal(1).style(chrome::divider),
            container(fields).padding([12, 18])
        ]
        .into()
    }

    fn prompt_view<'a>(
        &'a self,
        appearance: &canopa_theme::Appearance,
    ) -> Option<Element<'a, Message>> {
        let (title, mut body, confirm) = match self.prompt.as_ref()? {
            Prompt::Folder(parent, name) => {
                let label = fl!("files-picker-folder-name");
                let title = fl!("files-picker-new-folder");
                let confirm = action(
                    appearance,
                    fl!("files-create"),
                    (!self.busy && !name.is_empty()).then_some(Event::CreateFolder),
                    canopa_ui::Action::Primary,
                );
                (
                    title,
                    column![
                        text(parent.path().to_string_lossy().into_owned()).size(14),
                        input(
                            appearance,
                            "picker-folder-name",
                            label,
                            name,
                            Event::FolderName,
                            Event::CreateFolder
                        )
                    ],
                    confirm,
                )
            }
            Prompt::Replace(prepared) => {
                let title = fl!("files-picker-overwrite-question");
                let mut list = column![].spacing(4);
                for path in prepared.existing.iter().take(3) {
                    list = list.push(text(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    ));
                }
                if prepared.existing.len() > 3 {
                    list = list.push(text(fl!(
                        "files-picker-overwrite-more",
                        count = prepared.existing.len().saturating_sub(3)
                    )));
                }
                let confirm = action(
                    appearance,
                    fl!("files-picker-replace"),
                    Some(Event::Replace),
                    canopa_ui::Action::Destructive,
                );
                (title, list, confirm)
            }
        };
        if let Some(error) = &self.error {
            body = body.push(semantic(
                Node::alert(error),
                text(error).style(text::danger),
            ));
        }
        let view = column![
            text(title.clone()).size(22),
            body.spacing(8),
            row![
                space().width(Length::Fill),
                action(
                    appearance,
                    fl!("files-picker-keep-choosing"),
                    (!self.busy).then_some(Event::Back),
                    canopa_ui::Action::Normal
                ),
                confirm
            ]
            .spacing(8)
        ]
        .spacing(16);
        let node = if matches!(self.prompt, Some(Prompt::Replace(_))) {
            Node::alert_dialog(fl!("files-picker-overwrite-title")).description(title)
        } else {
            Node::dialog(title)
        };
        Some(semantic(
            node.key("files-picker-prompt").busy(self.busy),
            container(container(view).padding(24).max_width(460))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        ))
    }
}

fn input<'a>(
    appearance: &canopa_theme::Appearance,
    id: &'static str,
    label: String,
    value: &'a str,
    edit: fn(String) -> Event,
    submit: Event,
) -> Element<'a, Message> {
    let appearance = appearance.clone();
    semantic(
        Node::text_input(label.clone(), value)
            .key(id)
            .on_text(move |value| Message::Picker(edit(value))),
        text_input(&label, value)
            .id(id)
            .padding(canopa_ui::content_padding(
                appearance.style(canopa_theme::UiKind::Input),
            ))
            .style(move |theme, status| view::input_style(&appearance, theme, status))
            .on_input(move |value| Message::Picker(edit(value)))
            .on_submit(Message::Picker(submit)),
    )
}

fn toggle<'a>(
    label: String,
    checked: bool,
    edit: impl Fn(bool) -> Event + 'static,
) -> Element<'a, Message> {
    let changed = Message::Picker(edit(!checked));
    semantic(
        Node::check_box(label.clone(), checked).on_click(Some(changed)),
        checkbox(checked)
            .label(label)
            .size(17)
            .style(canopa_ui::checkbox_style)
            .on_toggle(move |checked| Message::Picker(edit(checked))),
    )
}

fn choice<'a>(
    label: String,
    values: &[Item],
    current: Option<usize>,
    edit: impl Fn(usize) -> Event + Send + Sync + 'static,
) -> Element<'a, Message> {
    let selected = current.map(|index| values[index].clone());
    let options: Vec<_> = values
        .iter()
        .map(|item| (item.1.clone(), Message::Picker(edit(item.0))))
        .collect();
    let next = current.map_or(0, |index| (index + 1) % options.len().max(1));
    let previous = current
        .and_then(|index| index.checked_sub(1))
        .unwrap_or(options.len().saturating_sub(1));
    let action = |index| {
        options
            .get(index)
            .map(|(_, message): &(String, Message)| message.clone())
    };
    let next = action(next);
    let node = Node::combo_box(
        label.clone(),
        selected.as_ref().map_or("", |item| item.1.as_str()),
    )
    .disabled(values.is_empty())
    .on_click(next.clone())
    .on_increment(next)
    .on_decrement(action(previous))
    .on_set_value(move |value| {
        let canopa_ui::accessibility::Value::Text(label) = value else {
            return None;
        };
        options
            .iter()
            .find(|item| item.0 == label)
            .map(|item| item.1.clone())
    });
    row![
        text(label.clone()).size(14),
        semantic(
            node,
            pick_list(values.to_vec(), selected, move |item| Message::Picker(
                edit(item.0)
            ))
            .placeholder(label)
        )
    ]
    .align_y(Alignment::Center)
    .spacing(6)
    .into()
}

fn action<'a>(
    appearance: &canopa_theme::Appearance,
    label: String,
    event: Option<Event>,
    treatment: canopa_ui::Action,
) -> Element<'a, Message> {
    let message = event.map(Message::Picker);
    semantic(
        Node::button(label.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        canopa_ui::styled_action(text(label), message, treatment, false, appearance),
    )
}

impl Desktop {
    pub(super) fn picker_view(&self, id: window::Id) -> Option<Element<'_, Message>> {
        let picker = self.picker.as_ref()?;
        picker
            .prompt_view(&self.configuration.appearance)
            .or_else(|| Some(view::browser(self, id)))
    }

    pub(super) fn picker_key(
        &mut self,
        id: window::Id,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<Task<Message>> {
        let picker = self.picker.as_ref()?;
        use keyboard::{Key, key::Named};
        if *key == Key::Named(Named::Escape) {
            if picker.prompt.is_some() {
                return Some(self.picker_event(Event::Back));
            }
            if self.action_menu.is_none()
                && self.folder_options.is_none()
                && self
                    .active(id)
                    .is_some_and(|tab| tab.location.is_none() && tab.search.is_none())
            {
                return Some(iced::exit());
            }
        }
        if *key == Key::Named(Named::Enter) && modifiers.control() && !picker.blocks_browser() {
            return Some(self.picker_event(Event::Accept));
        }
        picker.blocks_browser().then(Task::none)
    }

    pub(super) fn picker_select_name(&mut self, id: window::Id, path: &Path) {
        if !self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.request.mode == Mode::Save)
        {
            return;
        }
        let listed = self
            .active(id)
            .filter(|tab| !tab.loading)
            .and_then(|tab| tab.browser.as_ref())
            .is_some_and(|browser| {
                browser
                    .visible_entries()
                    .into_iter()
                    .any(|entry| entry.path == path && !entry.is_directory())
            });
        if listed && let Some(picker) = &mut self.picker {
            picker.original_name = path.file_name().map(ToOwned::to_owned);
            picker.name = picker.name().to_string_lossy().into_owned();
            picker.error = None;
        }
    }

    pub(super) fn picker_open(&mut self, id: window::Id, path: PathBuf) -> Task<Message> {
        let Some(picker) = self.picker.as_ref() else {
            return Task::none();
        };
        if picker.blocks_browser() || picker.request.directory {
            return Task::none();
        }
        let saving = picker.request.mode == Mode::Save;
        let Some(browser) = self
            .active_mut(id)
            .filter(|tab| !tab.loading)
            .and_then(|tab| tab.browser.as_mut())
        else {
            return Task::none();
        };
        if !browser
            .visible_entries()
            .into_iter()
            .any(|entry| entry.path == path && !entry.is_directory())
        {
            return Task::none();
        }
        if saving {
            self.picker_select_name(id, &path);
        } else if !browser.selected().contains(&path) {
            browser.select(&path, SelectionMode::Replace);
        }
        self.picker_event(Event::Accept)
    }

    pub(super) fn picker_new_folder(&mut self, id: window::Id) -> Task<Message> {
        let Some(folder) = self
            .active(id)
            .filter(|tab| !tab.loading)
            .and_then(|tab| tab.browser.as_ref())
            .map(|browser| browser.path().to_owned())
        else {
            return Task::none();
        };
        let Some(picker) = self
            .picker
            .as_mut()
            .filter(|picker| !picker.blocks_browser())
        else {
            return Task::none();
        };
        picker.busy = true;
        picker.error = None;
        Task::perform(
            async move {
                folders::Parent::open(folder)
                    .map(Arc::new)
                    .map_err(|error| error.to_string())
            },
            |result| Message::Picker(Event::FolderParent(result)),
        )
    }

    pub(super) fn picker_event(&mut self, event: Event) -> Task<Message> {
        let Some(id) = self.windows.keys().next().copied() else {
            return Task::none();
        };
        let Some(picker) = self.picker.as_mut() else {
            return Task::none();
        };
        if picker.busy
            && !matches!(
                event,
                Event::Cancel
                    | Event::Checked(_)
                    | Event::FolderParent(_)
                    | Event::FolderCreated(_)
                    | Event::Devices
            )
        {
            return Task::none();
        }
        match event {
            Event::Devices => {
                if let Some(monitor) = &self.picker_monitor {
                    self.devices.catalog = monitor.snapshot();
                }
            }
            Event::Cancel => return iced::exit(),
            Event::Back => {
                picker.prompt = None;
                picker.error = None;
            }
            Event::Name(name) => {
                picker.name = name;
                picker.original_name = None;
                picker.error = None;
            }
            Event::ReadOnly(read_only) => picker.writable = !read_only,
            Event::Filter(index) => {
                picker.request.current_filter = picker.request.filters.get(index).cloned();
                let picker = picker.clone();
                if let Some(tab) = self.active_mut(id)
                    && let Some(browser) = tab.browser.as_mut()
                {
                    picker.filter(browser);
                    tab.list_epoch += 1;
                }
                return self.restore_scroll(id);
            }
            Event::Choice(index, value) => {
                if let Some(choice) = picker.request.choices.get_mut(index) {
                    choice.3 = value;
                }
            }
            Event::Accept => {
                if picker.prompt.is_some() {
                    return Task::none();
                }
                let picker = picker.clone();
                let Some(browser) = self
                    .active(id)
                    .filter(|tab| !tab.loading)
                    .and_then(|tab| tab.browser.as_ref())
                else {
                    return Task::none();
                };
                let folder = browser.path().to_owned();
                let selected = picker.selected(browser);
                if picker.request.mode == Mode::Open
                    && !picker.request.directory
                    && selected.is_empty()
                {
                    return Task::none();
                }
                let name = picker.name();
                let active = self.picker.as_mut().unwrap();
                active.busy = true;
                active.error = None;
                return Task::perform(
                    async move {
                        crate::picker::prepare(&picker.request, &folder, selected, name)
                            .map_err(|error| error.to_string())
                    },
                    |result| Message::Picker(Event::Checked(result)),
                );
            }
            Event::Checked(result) => {
                picker.busy = false;
                match result {
                    Ok(prepared) if prepared.existing.is_empty() => return picker.finish(prepared),
                    Ok(prepared) => picker.prompt = Some(Prompt::Replace(prepared)),
                    Err(error) => picker.error = Some(error),
                }
            }
            Event::Replace => {
                if let Some(Prompt::Replace(prepared)) = picker.prompt.take() {
                    return picker.finish(prepared);
                }
            }
            Event::FolderParent(result) => {
                picker.busy = false;
                match result {
                    Ok(parent) => {
                        picker.prompt = Some(Prompt::Folder(parent, String::new()));
                        return iced::widget::operation::focus("picker-folder-name");
                    }
                    Err(error) => picker.error = Some(error),
                }
            }
            Event::FolderName(name) => {
                if let Some(Prompt::Folder(_, draft)) = &mut picker.prompt {
                    *draft = name;
                    picker.error = None;
                }
            }
            Event::CreateFolder => {
                if let Some(Prompt::Folder(parent, name)) = &picker.prompt {
                    let (parent, name) = (parent.clone(), name.clone());
                    picker.error = None;
                    picker.busy = true;
                    return Task::perform(
                        async move { parent.create(&name).map_err(|error| error.to_string()) },
                        |result| Message::Picker(Event::FolderCreated(result)),
                    );
                }
            }
            Event::FolderCreated(result) => {
                picker.busy = false;
                match result {
                    Ok(path) => {
                        picker.prompt = None;
                        return self.navigate(id, path, Navigation::Visit, None);
                    }
                    Err(error) => picker.error = Some(error),
                }
            }
        }
        Task::none()
    }
}
