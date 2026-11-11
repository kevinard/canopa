//! Opening files with applications. A worker classifies the activated entry
//! (`crate::opening`) and resolves its applications (`canopa_apps::mime_apps`)
//! before anything runs; programs, scripts and untrusted desktop entries ask
//! first, in a transient dialog that also hosts the application chooser.
//! Every launch carries an activation token from the window the user acted in.
use super::*;
use crate::opening::{self, DesktopKind, Opening};
use canopa_apps::{
    DesktopApplication,
    mime_apps::{self, MimeApps, MimeAppsPaths},
};
use canopa_file::mime::MimeDatabase;
use canopa_theme::UiKind;
use iced::{
    Length,
    widget::{checkbox, column, container, row, scrollable, text, text_input},
};

/// The chooser shows at most this many matching rows.
const MAX_ROWS: usize = 200;

/// What a confirmed choice starts.
#[derive(Clone, Debug)]
pub(super) enum Launch {
    Application {
        app: Arc<DesktopApplication>,
        files: Vec<PathBuf>,
        /// Also make it the user's default for this type first.
        default_for: Option<String>,
    },
    /// A trusted or confirmed `Type=Application` desktop entry.
    Entry(PathBuf),
    /// A trusted or confirmed `Type=Link` desktop entry.
    Url(String),
    Program(PathBuf),
    Terminal(PathBuf),
    /// GIO's own choice, for files opened outside browser windows.
    Default(PathBuf),
}

/// A worker's reading of one activated file.
#[derive(Debug)]
pub(super) struct Resolved {
    path: PathBuf,
    opening: Opening,
    default: Option<Arc<DesktopApplication>>,
    text_default: Option<Arc<DesktopApplication>>,
}

#[derive(Clone, Debug)]
pub(super) struct Row {
    app: Arc<DesktopApplication>,
    recommended: bool,
    search: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Run,
    Terminal,
    /// Open a script or untrusted entry as a document.
    Open,
    /// Launch an untrusted desktop entry once.
    LaunchOnce,
}

pub(super) enum Kind {
    Chooser {
        files: Vec<PathBuf>,
        /// The one type of the files; `None` for a mixed selection.
        mime: Option<String>,
        rows: Option<Result<Vec<Row>, String>>,
        search: String,
        selected: Option<String>,
        set_default: bool,
    },
    Program {
        path: PathBuf,
        script: bool,
        open_with: Option<Arc<DesktopApplication>>,
    },
    Untrusted {
        path: PathBuf,
        name: String,
        kind: DesktopKind,
        text: Option<Arc<DesktopApplication>>,
    },
}

pub(super) struct Dialog {
    pub window: window::Id,
    pub owner: window::Id,
    /// The owner's tab that asked; errors are reported there.
    tab: Option<u64>,
    kind: Kind,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    /// Window and originating tab, so errors stay with the tab that asked.
    Resolved(window::Id, Option<u64>, Arc<Result<Resolved, String>>),
    Token(window::Id, Option<u64>, Arc<Launch>, Option<String>),
    Launched(window::Id, Option<u64>, Result<(), String>),
    Opened(window::Id, Option<window::Id>),
    Rows(window::Id, Arc<Result<Chooser, String>>),
    Search(window::Id, String),
    Select(window::Id, String),
    SetDefault(window::Id, bool),
    Accept(window::Id),
    Choose(window::Id, Choice),
    Cancel(window::Id),
}

fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-open-with-{id:?}-{name}"))
}

fn arc(app: Option<&DesktopApplication>) -> Option<Arc<DesktopApplication>> {
    app.cloned().map(Arc::new)
}

fn chain(database: &MimeDatabase, path: &Path) -> (String, Vec<String>) {
    let mime = database.detect_path(path, canopa_file::mime::DEFAULT_SNIFF_BYTES);
    let chain = mime_apps::type_chain(database, &mime);
    (chain[0].clone(), chain)
}

/// Classify a file and find its applications. Runs on a worker.
fn resolve(path: PathBuf) -> Result<Resolved, String> {
    let paths = MimeAppsPaths::current();
    let facts = crate::mounts::Mounts::read().facts(&path);
    let opening = opening::classify(&path, facts, &paths.application_dirs())
        .map_err(|error| format!("Could not open “{}”: {error}", display_name(&path)))?;
    let database = MimeDatabase::system();
    let (_, types) = chain(&database, &path);
    let apps = MimeApps::load_from(&paths, &canopa_apps::DesktopEnvironment::current());
    Ok(Resolved {
        default: arc(apps.default_for(&types)),
        text_default: arc(apps.default_for(&mime_apps::type_chain(
            &database,
            canopa_file::mime::PLAIN_TEXT,
        ))),
        opening,
        path,
    })
}

/// Applications offered for every file, most suitable first. Runs on a worker.
pub(super) fn applications_for(paths: &[PathBuf]) -> Vec<Arc<DesktopApplication>> {
    let database = MimeDatabase::system();
    let chains: Vec<Vec<String>> = paths.iter().map(|path| chain(&database, path).1).collect();
    MimeApps::load()
        .applications_for_all(&chains)
        .into_iter()
        .map(|app| Arc::new(app.clone()))
        .collect()
}

/// Rows for the chooser, and the one type of all files (for "Set as default").
fn chooser_rows(paths: &[PathBuf]) -> Chooser {
    let database = MimeDatabase::system();
    let chains: Vec<Vec<String>> = paths.iter().map(|path| chain(&database, path).1).collect();
    let mut types: Vec<&String> = chains.iter().map(|chain| &chain[0]).collect();
    types.dedup();
    let mime = (types.len() == 1).then(|| types[0].clone());
    let apps = MimeApps::load();
    let recommended = apps.applications_for_all(&chains);
    let mut rows: Vec<Row> = recommended.iter().map(|app| row_for(app, true)).collect();
    let mut others: Vec<&DesktopApplication> = apps
        .menu_applications()
        .into_iter()
        .filter(|app| !recommended.iter().any(|known| known.id == app.id))
        .collect();
    others.sort_by_key(|app| app.name.to_lowercase());
    rows.extend(others.into_iter().map(|app| row_for(app, false)));
    Chooser { rows, mime }
}

#[derive(Clone, Debug)]
pub(super) struct Chooser {
    rows: Vec<Row>,
    mime: Option<String>,
}

fn row_for(app: &DesktopApplication, recommended: bool) -> Row {
    Row {
        search: format!(
            "{} {} {} {}",
            app.name,
            app.id,
            app.generic_name.as_deref().unwrap_or_default(),
            app.keywords.join(" ")
        )
        .to_lowercase(),
        app: Arc::new(app.clone()),
        recommended,
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Start a confirmed launch. Runs on a worker; never through a shell.
fn execute(launch: &Launch, token: Option<&str>) -> Result<(), String> {
    let search = canopa_apps::process_search_path();
    match launch {
        Launch::Application {
            app,
            files,
            default_for,
        } => {
            if let Some(mime) = default_for {
                mime_apps::set_default(&MimeAppsPaths::current(), mime, &app.id)
                    .map_err(|error| format!("Could not make {} the default: {error}", app.name))?;
            }
            canopa_apps::check_launch(app, &search)
                .map_err(|problem| format!("Could not start {}: {problem}", app.name))?;
            opening::spawn(opening::launch_entry(&app.desktop_file, files, token), true)
                .map_err(|error| format!("Could not start {}: {error}", app.name))
        }
        Launch::Entry(path) => {
            opening::check_desktop_entry(path).map_err(|problem| {
                format!("Could not start “{}”: {problem}", display_name(path))
            })?;
            opening::spawn(opening::launch_entry(path, &[], token), true)
                .map_err(|error| format!("Could not start “{}”: {error}", display_name(path)))
        }
        Launch::Url(url) => opening::spawn(opening::open_url(url, token), true)
            .map_err(|error| format!("Could not open the link: {error}")),
        Launch::Program(path) => opening::spawn(opening::run_program(path, token), false)
            .map_err(|error| format!("Could not run “{}”: {error}", display_name(path))),
        Launch::Terminal(path) => {
            let command = opening::run_in_terminal(path, token).ok_or_else(|| {
                "Running in a terminal needs xdg-terminal-exec; install it to choose your terminal."
                    .to_owned()
            })?;
            opening::spawn(command, false)
                .map_err(|error| format!("Could not run “{}”: {error}", display_name(path)))
        }
        Launch::Default(path) => {
            let mut command = std::process::Command::new("gio");
            command.arg("open").arg(path);
            opening::spawn(command, true)
                .map_err(|error| format!("Opening the file failed: {error}"))
        }
    }
}

impl Dialog {
    pub fn title(&self) -> &'static str {
        match &self.kind {
            Kind::Chooser { .. } => "Open With",
            Kind::Program { script: true, .. } => "Run this script?",
            Kind::Program { .. } => "Run this program?",
            Kind::Untrusted { .. } => "Untrusted application launcher",
        }
    }

    fn message(&self) -> String {
        match &self.kind {
            Kind::Chooser { files, .. } => match files.as_slice() {
                [path] => format!("Choose an application for “{}”.", display_name(path)),
                files => format!("Choose an application for {} files.", files.len()),
            },
            Kind::Program { path, script, .. } => format!(
                "“{}” is an executable {}. Run it only if you trust where it came from.",
                display_name(path),
                if *script { "script" } else { "program" }
            ),
            Kind::Untrusted { path, name, .. } => format!(
                "“{name}” ({}) is not installed in an applications folder and is not marked as executable. Launch it only if you trust where it came from. Marking the file executable makes Files trust it.",
                display_name(path)
            ),
        }
    }

    #[cfg(test)]
    pub fn summary(&self) -> String {
        format!("{} | {}", self.title(), self.message())
    }

    fn visible(&self) -> Vec<&Row> {
        let Kind::Chooser { rows, search, .. } = &self.kind else {
            return Vec::new();
        };
        let query = search.trim().to_lowercase();
        rows.as_ref()
            .and_then(|rows| rows.as_ref().ok())
            .map(|rows| {
                rows.iter()
                    .filter(|row| query.is_empty() || row.search.contains(&query))
                    .take(MAX_ROWS)
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Desktop {
    /// Open an activated file in a browser window: classify first.
    pub(super) fn open_file(&mut self, id: window::Id, path: PathBuf) -> Task<Message> {
        let tab = self.active(id).map(|tab| tab.id);
        Task::perform(
            async move {
                let (send, receive) = iced::futures::channel::oneshot::channel();
                let _ = std::thread::Builder::new()
                    .name("files-open".into())
                    .spawn(move || {
                        let _ = send.send(resolve(path));
                    });
                Arc::new(
                    receive
                        .await
                        .unwrap_or_else(|_| Err("Could not open the file.".into())),
                )
            },
            move |resolved| Message::OpenWith(Event::Resolved(id, tab, resolved)),
        )
    }

    /// Launch for the window's current tab.
    pub(super) fn launch(&mut self, id: window::Id, launch: Launch) -> Task<Message> {
        let tab = self.active(id).map(|tab| tab.id);
        self.launch_for(id, tab, launch)
    }

    fn launch_for(&mut self, id: window::Id, tab: Option<u64>, launch: Launch) -> Task<Message> {
        let launch = Arc::new(launch);
        window::request_activation_token(id)
            .map(move |token| Message::OpenWith(Event::Token(id, tab, launch.clone(), token)))
    }

    /// Errors stay with the tab that asked, as other asynchronous open errors do.
    fn report(&mut self, id: window::Id, tab: Option<u64>, error: String) -> Task<Message> {
        if let Some(tab) = tab {
            return self.update(Message::Error(id, tab, error));
        }
        if self
            .trash_window
            .as_ref()
            .is_some_and(|trash| trash.window == id)
        {
            return self.trash_event(trash_ui::Event::OpenFailed(id, error));
        }
        if self
            .recovery_window
            .as_ref()
            .is_some_and(|w| w.window == id)
        {
            return self.recovery_event(recovery_ui::Event::OpenFailed(id, error));
        }
        Task::none()
    }

    pub(super) fn show_open_with_chooser(
        &mut self,
        owner: window::Id,
        files: Vec<PathBuf>,
    ) -> Task<Message> {
        let tab = self.active(owner).map(|tab| tab.id);
        self.show_chooser(owner, tab, files)
    }

    fn show_chooser(
        &mut self,
        owner: window::Id,
        tab: Option<u64>,
        files: Vec<PathBuf>,
    ) -> Task<Message> {
        if files.is_empty() {
            return Task::none();
        }
        self.show_open_dialog(
            owner,
            tab,
            Kind::Chooser {
                files,
                mime: None,
                rows: None,
                search: String::new(),
                selected: None,
                set_default: false,
            },
        )
    }

    fn show_open_dialog(
        &mut self,
        owner: window::Id,
        tab: Option<u64>,
        kind: Kind,
    ) -> Task<Message> {
        if let Some(dialog) = &self.open_dialog {
            return window::gain_focus(dialog.window);
        }
        let chooser = matches!(kind, Kind::Chooser { .. });
        let (window, opened) = window::open_transient(
            owner,
            window::Settings {
                size: if chooser {
                    Size::new(460.0, 520.0)
                } else {
                    Size::new(460.0, 230.0)
                },
                min_size: Some(Size::new(340.0, 200.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Files".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let load = if let Kind::Chooser { files, .. } = &kind {
            let files = files.clone();
            Task::perform(
                async move {
                    let (send, receive) = iced::futures::channel::oneshot::channel();
                    let _ = std::thread::Builder::new()
                        .name("files-open-with".into())
                        .spawn(move || {
                            let _ = send.send(chooser_rows(&files));
                        });
                    Arc::new(
                        receive
                            .await
                            .map_err(|_| "Applications could not be listed.".to_owned()),
                    )
                },
                move |rows| Message::OpenWith(Event::Rows(window, rows)),
            )
        } else {
            Task::none()
        };
        self.open_dialog = Some(Dialog {
            window,
            owner,
            tab,
            kind,
        });
        Task::batch([
            opened.map(move |opened| Message::OpenWith(Event::Opened(window, opened))),
            load,
        ])
    }

    pub(super) fn close_open_dialog(&mut self, id: window::Id) -> Task<Message> {
        let Some(dialog) = self.open_dialog.take_if(|dialog| dialog.window == id) else {
            return Task::none();
        };
        let focus = if self.focused_window == Some(id) {
            window::gain_focus(dialog.owner)
        } else {
            Task::none()
        };
        Task::batch([focus.chain(window::close(id)), self.exit_if_empty()])
    }

    pub(super) fn open_dialog_key(
        &mut self,
        id: window::Id,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        if key == keyboard::Key::Named(keyboard::key::Named::Escape)
            || (modifiers.control() && key.as_ref() == keyboard::Key::Character("w"))
        {
            return self.close_open_dialog(id);
        }
        Task::none()
    }

    pub(super) fn open_with_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Resolved(id, tab, resolved) => match resolved.as_ref() {
                Err(error) => self.report(id, tab, error.clone()),
                Ok(resolved) => self.resolved(id, tab, resolved),
            },
            Event::Token(id, tab, launch, token) => Task::perform(
                async move {
                    let (send, receive) = iced::futures::channel::oneshot::channel();
                    let _ = std::thread::Builder::new()
                        .name("files-launch".into())
                        .spawn(move || {
                            let _ = send.send(execute(&launch, token.as_deref()));
                        });
                    receive
                        .await
                        .unwrap_or_else(|_| Err("The launch was interrupted.".into()))
                },
                move |result| Message::OpenWith(Event::Launched(id, tab, result)),
            ),
            Event::Launched(id, tab, result) => match result {
                Err(error) => self.report(id, tab, error),
                Ok(()) => Task::none(),
            },
            event => self.open_dialog_event(event),
        }
    }

    fn resolved(&mut self, id: window::Id, tab: Option<u64>, resolved: &Resolved) -> Task<Message> {
        let path = resolved.path.clone();
        match &resolved.opening {
            Opening::Directory => self.navigate(id, path, Navigation::Visit, None),
            Opening::Document => match &resolved.default {
                Some(app) => self.launch_for(
                    id,
                    tab,
                    Launch::Application {
                        app: app.clone(),
                        files: vec![path],
                        default_for: None,
                    },
                ),
                None => self.show_chooser(id, tab, vec![path]),
            },
            Opening::Program { script } => self.show_open_dialog(
                id,
                tab,
                Kind::Program {
                    path,
                    script: *script,
                    open_with: script
                        .then(|| resolved.default.clone().or(resolved.text_default.clone()))
                        .flatten(),
                },
            ),
            Opening::DesktopEntry {
                kind,
                trusted: true,
                ..
            } => self.launch_for(
                id,
                tab,
                match kind {
                    DesktopKind::Application => Launch::Entry(path),
                    DesktopKind::Link(url) => Launch::Url(url.clone()),
                },
            ),
            Opening::DesktopEntry {
                name,
                kind,
                trusted: false,
            } => self.show_open_dialog(
                id,
                tab,
                Kind::Untrusted {
                    path,
                    name: name.clone(),
                    kind: kind.clone(),
                    text: resolved.text_default.clone(),
                },
            ),
        }
    }

    fn open_dialog_event(&mut self, event: Event) -> Task<Message> {
        let id = match &event {
            Event::Opened(id, _)
            | Event::Rows(id, _)
            | Event::Search(id, _)
            | Event::Select(id, _)
            | Event::SetDefault(id, _)
            | Event::Accept(id)
            | Event::Choose(id, _)
            | Event::Cancel(id) => *id,
            _ => return Task::none(),
        };
        let Some(dialog) = self
            .open_dialog
            .as_mut()
            .filter(|dialog| dialog.window == id)
        else {
            return if matches!(event, Event::Opened(_, Some(_))) {
                window::close(id)
            } else {
                Task::none()
            };
        };
        match event {
            Event::Opened(_, None) => {
                let (owner, tab) = (dialog.owner, dialog.tab);
                self.open_dialog = None;
                self.report(owner, tab, "Could not open the dialog.".into())
            }
            Event::Opened(..) => {
                let focus = match &dialog.kind {
                    Kind::Chooser { .. } => control(id, "search"),
                    _ => control(id, "cancel"),
                };
                window::gain_focus(id).chain(iced::widget::operation::focus(focus))
            }
            Event::Rows(_, loaded) => {
                if let Kind::Chooser {
                    rows,
                    selected,
                    mime,
                    ..
                } = &mut dialog.kind
                {
                    match loaded.as_ref() {
                        Ok(loaded) => {
                            *selected = loaded.rows.first().map(|row| row.app.id.clone());
                            mime.clone_from(&loaded.mime);
                            *rows = Some(Ok(loaded.rows.clone()));
                        }
                        Err(error) => *rows = Some(Err(error.clone())),
                    }
                }
                Task::none()
            }
            Event::Search(_, value) => {
                if let Kind::Chooser { search, .. } = &mut dialog.kind {
                    *search = value.chars().take(256).collect();
                }
                let visible: Vec<String> = dialog
                    .visible()
                    .iter()
                    .map(|row| row.app.id.clone())
                    .collect();
                if let Kind::Chooser { selected, .. } = &mut dialog.kind
                    && selected.as_ref().is_none_or(|id| !visible.contains(id))
                {
                    *selected = visible.into_iter().next();
                }
                Task::none()
            }
            Event::Select(_, app) => {
                if let Kind::Chooser { selected, .. } = &mut dialog.kind {
                    *selected = Some(app);
                }
                Task::none()
            }
            Event::SetDefault(_, value) => {
                if let Kind::Chooser {
                    set_default, mime, ..
                } = &mut dialog.kind
                {
                    *set_default = value && mime.is_some();
                }
                Task::none()
            }
            Event::Accept(_) => {
                let visible: Vec<Arc<DesktopApplication>> =
                    dialog.visible().iter().map(|row| row.app.clone()).collect();
                let Kind::Chooser {
                    files,
                    mime,
                    selected,
                    set_default,
                    ..
                } = &dialog.kind
                else {
                    return Task::none();
                };
                let Some(app) = selected
                    .as_ref()
                    .and_then(|id| visible.iter().find(|app| &app.id == id))
                    .cloned()
                else {
                    return Task::none();
                };
                let launch = Launch::Application {
                    app,
                    files: files.clone(),
                    default_for: mime.clone().filter(|_| *set_default),
                };
                self.confirm(id, launch)
            }
            Event::Choose(_, choice) => {
                let launch = match (&dialog.kind, choice) {
                    (Kind::Program { path, .. }, Choice::Run) => Launch::Program(path.clone()),
                    (Kind::Program { path, .. }, Choice::Terminal) => {
                        Launch::Terminal(path.clone())
                    }
                    (
                        Kind::Program {
                            path,
                            open_with: Some(app),
                            ..
                        },
                        Choice::Open,
                    ) => Launch::Application {
                        app: app.clone(),
                        files: vec![path.clone()],
                        default_for: None,
                    },
                    (Kind::Untrusted { path, kind, .. }, Choice::LaunchOnce) => match kind {
                        DesktopKind::Application => Launch::Entry(path.clone()),
                        DesktopKind::Link(url) => Launch::Url(url.clone()),
                    },
                    (
                        Kind::Untrusted {
                            path,
                            text: Some(app),
                            ..
                        },
                        Choice::Open,
                    ) => Launch::Application {
                        app: app.clone(),
                        files: vec![path.clone()],
                        default_for: None,
                    },
                    _ => return Task::none(),
                };
                self.confirm(id, launch)
            }
            Event::Cancel(_) => self.close_open_dialog(id),
            _ => Task::none(),
        }
    }

    /// Take the token while the dialog the user answered still has focus.
    fn confirm(&mut self, id: window::Id, launch: Launch) -> Task<Message> {
        let (owner, tab) = self
            .open_dialog
            .as_ref()
            .map_or((id, None), |dialog| (dialog.owner, dialog.tab));
        let launch = Arc::new(launch);
        let token = window::request_activation_token(id)
            .map(move |token| Message::OpenWith(Event::Token(owner, tab, launch.clone(), token)));
        token.chain(self.close_open_dialog(id))
    }
}

pub(super) fn view<'a>(desktop: &'a Desktop, dialog: &'a Dialog) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let size = appearance.typography().body_size.px();
    let small = appearance.typography().label_size.px();
    let id = dialog.window;
    let action = |label: String, name: &str, message: Option<Message>, kind| {
        super::focus::control(
            control(id, name),
            message.clone(),
            appearance,
            canopa_ui::styled_action(
                text(label).font(font).size(small),
                message,
                kind,
                false,
                appearance,
            ),
        )
    };
    let icon = match &dialog.kind {
        Kind::Chooser { .. } => chrome::Icon::Grid,
        Kind::Program { .. } => chrome::Icon::Terminal,
        Kind::Untrusted { .. } => chrome::Icon::Question,
    };
    let mut order = Vec::new();
    let mut body = column![
        chrome::dialog_heading(desktop, icon, dialog.title(), ""),
        text(dialog.message())
            .font(font)
            .size(size)
            .wrapping(text::Wrapping::WordOrGlyph),
    ]
    .spacing(12);
    let mut buttons = row![iced::widget::space().width(Length::Fill)].spacing(8);
    order.push(control(id, "cancel"));
    buttons = buttons.push(action(
        "Cancel".into(),
        "cancel",
        Some(Message::OpenWith(Event::Cancel(id))),
        canopa_ui::Action::Quiet,
    ));
    match &dialog.kind {
        Kind::Chooser {
            rows,
            search,
            selected,
            set_default,
            mime,
            ..
        } => {
            order.insert(0, control(id, "search"));
            body = body.push(
                text_input("Search applications", search)
                    .id(control(id, "search"))
                    .on_input(move |value| Message::OpenWith(Event::Search(id, value)))
                    .on_submit(Message::OpenWith(Event::Accept(id)))
                    .font(font)
                    .size(size)
                    .padding(canopa_ui::content_padding(appearance.style(UiKind::Input)))
                    .style(move |theme, status| {
                        super::view::input_style(appearance, theme, status)
                    }),
            );
            let mut list = column![].spacing(2);
            match rows {
                None => list = list.push(text("Looking for applications…").font(font).size(small)),
                Some(Err(error)) => list = list.push(text(error.clone()).font(font).size(small)),
                Some(Ok(_)) => {
                    let visible = dialog.visible();
                    if visible.is_empty() {
                        list = list.push(text("No matching applications").font(font).size(small));
                    }
                    let mut heading = None;
                    for row in visible {
                        if heading != Some(row.recommended) {
                            heading = Some(row.recommended);
                            list = list.push(
                                text(if row.recommended {
                                    "Recommended"
                                } else {
                                    "Other applications"
                                })
                                .font(font)
                                .size(small)
                                .style(canopa_ui::muted),
                            );
                        }
                        let caption = row
                            .app
                            .generic_name
                            .clone()
                            .or_else(|| row.app.comment.clone())
                            .unwrap_or_default();
                        let content = column![
                            text(row.app.name.clone()).font(font).size(size),
                            text(caption).font(font).size(small).style(canopa_ui::muted),
                        ];
                        list = list.push(
                            canopa_ui::styled_action(
                                content,
                                Some(Message::OpenWith(Event::Select(id, row.app.id.clone()))),
                                canopa_ui::Action::Quiet,
                                selected.as_deref() == Some(row.app.id.as_str()),
                                appearance,
                            )
                            .width(Length::Fill),
                        );
                    }
                }
            }
            body = body.push(scrollable(list).height(Length::Fill));
            if let Some(mime) = mime {
                order.push(control(id, "default"));
                let toggle = Message::OpenWith(Event::SetDefault(id, !set_default));
                body = body.push(super::focus::control(
                    control(id, "default"),
                    Some(toggle),
                    appearance,
                    checkbox(*set_default)
                        .label(format!("Set as default for {mime}"))
                        .font(font)
                        .text_size(small)
                        .style(canopa_ui::checkbox_style)
                        .on_toggle(move |value| Message::OpenWith(Event::SetDefault(id, value))),
                ));
            }
            order.push(control(id, "accept"));
            let ready = selected.is_some() && !dialog.visible().is_empty();
            buttons = buttons.push(action(
                "Open".into(),
                "accept",
                ready.then_some(Message::OpenWith(Event::Accept(id))),
                canopa_ui::Action::Primary,
            ));
        }
        Kind::Program {
            script, open_with, ..
        } => {
            body = body.push(iced::widget::space().height(Length::Fill));
            let choose = |choice| Some(Message::OpenWith(Event::Choose(id, choice)));
            if *script {
                order.push(control(id, "open"));
                buttons = buttons.push(action(
                    open_with
                        .as_ref()
                        .map_or("Open".into(), |app| format!("Open in {}", app.name)),
                    "open",
                    open_with.as_ref().and(choose(Choice::Open)),
                    canopa_ui::Action::Quiet,
                ));
            }
            order.push(control(id, "terminal"));
            order.push(control(id, "run"));
            buttons = buttons
                .push(action(
                    "Run in Terminal".into(),
                    "terminal",
                    choose(Choice::Terminal),
                    canopa_ui::Action::Quiet,
                ))
                .push(action(
                    "Run".into(),
                    "run",
                    choose(Choice::Run),
                    canopa_ui::Action::Primary,
                ));
        }
        Kind::Untrusted { text: editor, .. } => {
            body = body.push(iced::widget::space().height(Length::Fill));
            let choose = |choice| Some(Message::OpenWith(Event::Choose(id, choice)));
            order.push(control(id, "open"));
            order.push(control(id, "launch"));
            buttons = buttons
                .push(action(
                    "Open as Text".into(),
                    "open",
                    editor.as_ref().and(choose(Choice::Open)),
                    canopa_ui::Action::Quiet,
                ))
                .push(action(
                    "Launch".into(),
                    "launch",
                    choose(Choice::LaunchOnce),
                    canopa_ui::Action::Destructive,
                ));
        }
    }
    body = body.push(buttons);
    let style = appearance.clone();
    let body = container(body)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| canopa_ui::frame(&style, style.style(UiKind::Panel)));
    super::focus::group(body, order, Vec::new(), Some(control(id, "cancel")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(names: &[(&str, bool)]) -> Vec<Row> {
        names
            .iter()
            .map(|(name, recommended)| {
                row_for(
                    &DesktopApplication {
                        id: format!("{}.desktop", name.to_lowercase()),
                        name: (*name).into(),
                        generic_name: Some("Image Viewer".into()),
                        comment: None,
                        keywords: vec!["photo".into()],
                        icon: None,
                        desktop_file: PathBuf::from(format!("/apps/{name}.desktop")),
                        ..Default::default()
                    },
                    *recommended,
                )
            })
            .collect()
    }

    #[test]
    fn the_chooser_filters_by_name_id_and_keywords() {
        let mut dialog = Dialog {
            window: window::Id::unique(),
            owner: window::Id::unique(),
            tab: None,
            kind: Kind::Chooser {
                files: vec![PathBuf::from("/tmp/a.png")],
                mime: Some("image/png".into()),
                rows: Some(Ok(rows(&[("Loupe", true), ("GIMP", false)]))),
                search: String::new(),
                selected: None,
                set_default: false,
            },
        };
        assert_eq!(dialog.visible().len(), 2);
        if let Kind::Chooser { search, .. } = &mut dialog.kind {
            *search = "gimp".into();
        }
        assert_eq!(dialog.visible()[0].app.name, "GIMP");
        if let Kind::Chooser { search, .. } = &mut dialog.kind {
            *search = "PHOTO".into();
        }
        assert_eq!(dialog.visible().len(), 2);
        assert!(
            dialog
                .summary()
                .starts_with("Open With | Choose an application for “a.png”.")
        );
    }

    #[test]
    fn prompts_name_what_will_run() {
        let dialog = Dialog {
            window: window::Id::unique(),
            owner: window::Id::unique(),
            tab: None,
            kind: Kind::Untrusted {
                path: PathBuf::from("/home/me/Downloads/game.desktop"),
                name: "Game".into(),
                kind: DesktopKind::Application,
                text: None,
            },
        };
        assert!(
            dialog
                .summary()
                .contains("“Game” (game.desktop) is not installed")
        );
        let dialog = Dialog {
            kind: Kind::Program {
                path: PathBuf::from("/tmp/run.sh"),
                script: true,
                open_with: None,
            },
            ..dialog
        };
        assert_eq!(
            dialog.summary(),
            "Run this script? | “run.sh” is an executable script. Run it only if you trust where it came from."
        );
    }

    fn resolved(desktop: &mut Desktop, window: window::Id, path: &Path, opening: Opening) {
        let tab = desktop.active(window).map(|tab| tab.id);
        let _task = desktop.open_with_event(Event::Resolved(
            window,
            tab,
            Arc::new(Ok(Resolved {
                path: path.to_owned(),
                opening,
                default: None,
                text_default: None,
            })),
        ));
    }

    #[test]
    fn programs_and_untrusted_entries_ask_before_anything_runs() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("run.sh");
        std::fs::write(&script, "#!/bin/sh\n").unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        resolved(
            &mut desktop,
            window,
            &script,
            Opening::Program { script: true },
        );
        let dialog = desktop.open_dialog.as_ref().expect("a Run prompt");
        assert_eq!(dialog.owner, window);
        assert!(matches!(
            dialog.kind,
            Kind::Program {
                script: true,
                open_with: None,
                ..
            }
        ));
        // Without an application for the script, Open is not offered.
        let id = dialog.window;
        let _task = desktop.open_with_event(Event::Choose(id, Choice::Open));
        assert!(desktop.open_dialog.is_some());
        // One prompt at a time; a second activation does not replace it.
        let entry = root.path().join("game.desktop");
        resolved(
            &mut desktop,
            window,
            &entry,
            Opening::DesktopEntry {
                name: "Game".into(),
                kind: DesktopKind::Application,
                trusted: false,
            },
        );
        assert!(matches!(
            desktop.open_dialog.as_ref().unwrap().kind,
            Kind::Program { .. }
        ));
        let _task = desktop.open_with_event(Event::Cancel(id));
        assert!(desktop.open_dialog.is_none());
        resolved(
            &mut desktop,
            window,
            &entry,
            Opening::DesktopEntry {
                name: "Game".into(),
                kind: DesktopKind::Application,
                trusted: false,
            },
        );
        assert!(matches!(
            desktop.open_dialog.as_ref().unwrap().kind,
            Kind::Untrusted { .. }
        ));
    }

    #[test]
    fn documents_without_an_application_open_the_chooser_and_errors_stay_with_their_tab() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("data.unknown-type");
        std::fs::write(&file, "x").unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned(); 2]);
        resolved(&mut desktop, window, &file, Opening::Document);
        let dialog = desktop.open_dialog.as_ref().expect("the chooser");
        assert!(
            matches!(&dialog.kind, Kind::Chooser { files, .. } if files == std::slice::from_ref(&file))
        );
        let origin = desktop.active(window).unwrap().id;
        let _task = desktop.update(Message::SwitchTab(window, 1));
        let _task = desktop.open_with_event(Event::Launched(
            window,
            Some(origin),
            Err("Could not start Viewer".into()),
        ));
        assert!(desktop.active(window).unwrap().error.is_none());
        assert_eq!(
            desktop.windows[&window].tabs[0].error.as_deref(),
            Some("Could not start Viewer")
        );
    }
}
