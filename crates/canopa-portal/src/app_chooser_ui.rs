//! Iced application selection. The frontend retains launch and MIME policy.
use crate::accessibility_ui::{self as a11y, Node, semantic};
use crate::{
    app_chooser::{AppRequest, AppSelection},
    worker::run as worker,
};
use crate::{dialog_ui, icons};
use canopa_apps::mime_apps::{self, MimeApps};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{button, column, container, image, scrollable, text, text_input},
    window,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::watch;

#[derive(Clone, Debug)]
struct Application {
    id: String,
    name: String,
    caption: String,
    search: String,
    icon: Option<image::Handle>,
    fit: FileTypeFit,
}

/// Local file-type knowledge decorates the frontend’s offered IDs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum FileTypeFit {
    Default,
    Associated,
    #[default]
    Other,
}

fn request_type(content_type: Option<&str>, uri: Option<&str>) -> Option<String> {
    content_type
        .filter(|mime| mime_apps::valid_mime(mime))
        .map(str::to_owned)
        .or_else(|| {
            let scheme = url::Url::parse(uri?).ok()?;
            let mime = format!("x-scheme-handler/{}", scheme.scheme());
            (scheme.scheme() != "file" && mime_apps::valid_mime(&mime)).then_some(mime)
        })
}

fn desktop_id(id: &str) -> &str {
    id.strip_suffix(".desktop").unwrap_or(id)
}

fn is_last_used(application: &Application, last_choice: Option<&str>) -> bool {
    last_choice.is_some_and(|id| desktop_id(id) == desktop_id(&application.id))
}

fn applications(
    catalog: Vec<canopa_apps::DesktopApplication>,
    foreground: canopa_theme::Rgb,
    choices: &[String],
    mime: Option<&str>,
) -> Vec<Application> {
    let apps = mime.map(|_| MimeApps::load());
    let chain = mime
        .map(|mime| mime_apps::type_chain(&canopa_file::mime::MimeDatabase::system(), mime))
        .unwrap_or_default();
    let default = apps
        .as_ref()
        .and_then(|apps| chain.iter().find_map(|mime| apps.explicit_default(mime)));
    let associations: HashSet<_> = apps
        .as_ref()
        .into_iter()
        .flat_map(|apps| apps.applications_for(&chain))
        .map(|app| app.id.as_str())
        .collect();
    let visible: Vec<_> = catalog
        .iter()
        .filter(|app| {
            choices.is_empty()
                || choices
                    .iter()
                    .any(|id| desktop_id(id) == desktop_id(&app.id))
        })
        .cloned()
        .collect();
    let names: Vec<_> = visible.iter().filter_map(|app| app.icon.clone()).collect();
    let images = icons::prepare(&names, foreground);
    catalog
        .into_iter()
        .map(|app| Application {
            fit: if default.is_some_and(|default| default.id == app.id) {
                FileTypeFit::Default
            } else if associations.contains(app.id.as_str()) {
                FileTypeFit::Associated
            } else {
                FileTypeFit::Other
            },
            search: format!(
                "{} {} {} {}",
                app.name,
                app.id,
                app.generic_name.as_deref().unwrap_or_default(),
                app.keywords.join(" ")
            )
            .to_lowercase(),
            icon: app.icon.as_ref().and_then(|name| images.get(name)).cloned(),
            caption: app.generic_name.unwrap_or_default(),
            id: app.id,
            name: app.name,
        })
        .collect()
}

fn offered(choices: &[String], catalog: &[Application]) -> Vec<Application> {
    if choices.is_empty() {
        return catalog.to_owned();
    }
    let index: HashMap<_, _> = catalog
        .iter()
        .map(|app| (desktop_id(&app.id), app))
        .collect();
    choices
        .iter()
        .map(|id| {
            let mut row = index
                .get(desktop_id(id))
                .map(|row| (*row).clone())
                .unwrap_or_else(|| Application {
                    id: id.clone(),
                    name: id.clone(),
                    caption: fl!("portal-app-chooser-caption"),
                    search: id.to_lowercase(),
                    icon: None,
                    fit: FileTypeFit::Other,
                });
            row.id.clone_from(id);
            row
        })
        .collect()
}

pub fn run(
    request: AppRequest,
    appearance: canopa_theme::Appearance,
    updates: watch::Receiver<Option<Vec<String>>>,
) -> anyhow::Result<Option<AppSelection>> {
    let font = canopa_ui::font(&appearance);
    let foreground = appearance.palette().text;
    let theme = canopa_ui::theme(appearance);
    let output = Arc::new(Mutex::new(None));
    let result = output.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(620.0, 512.0),
                min_size: Some(Size::new(400.0, 320.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = request
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default();
            let (window, opening) = window::open_dialog_with_activation(
                parent.to_owned(),
                request.modal,
                request.activation_token.clone(),
                settings,
            );
            let opening = opening.map(|_| Message::Opened);
            let mut app = Chooser {
                request: request.clone(),
                window,
                pending: None,
                epoch: 0,
                theme: theme.clone(),
                foreground,
                catalog: Vec::new(),
                rows: Vec::new(),
                search: String::new(),
                selected: None,
                loading: false,
                reload_pending: false,
                initial: true,
                error: None,
                last_click: None,
                list_focus: true,
                accessibility: Some(a11y::Accessibility::new("canopa-app-chooser", WINDOW_KEY)),
                output: output.clone(),
            };
            let load = app.load();
            let controls =
                futures_util::stream::unfold(Some(updates.clone()), |state| async move {
                    let mut receiver = state?;
                    let value = if receiver.changed().await.is_ok() {
                        receiver.borrow_and_update().clone()
                    } else {
                        None
                    };
                    Some(match value {
                        Some(choices) => (Message::Update(choices), Some(receiver)),
                        None => (Message::Cancel, None),
                    })
                });
            (
                app,
                Task::batch([opening, load, Task::run(controls, |message| message)]),
            )
        },
        Chooser::update,
        Chooser::view,
    )
    .title(|_: &Chooser, _| window_title())
    .theme(|app: &Chooser, _| app.theme.clone())
    .subscription(Chooser::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("app chooser output").take())
}

/// The AT-SPI window of the application chooser and its title.
const WINDOW_KEY: &str = "app-chooser";
fn window_title() -> String {
    fl!(
        "portal-window-title",
        title = fl!("portal-app-chooser-title")
    )
}
/// The application list, controlled by the search field.
const APPLICATIONS_KEY: &str = "app-chooser-applications";

/// A row's identity: the offered application ID, never its display name.
fn application_key(id: &str) -> String {
    format!("app-chooser-application-{}", id.escape_default())
}

struct Chooser {
    request: AppRequest,
    window: window::Id,
    pending: Option<(u64, String, Instant)>,
    epoch: u64,
    theme: Theme,
    foreground: canopa_theme::Rgb,
    catalog: Vec<Application>,
    rows: Vec<Application>,
    search: String,
    selected: Option<String>,
    loading: bool,
    reload_pending: bool,
    initial: bool,
    error: Option<String>,
    last_click: Option<(String, Instant)>,
    /// Keyboard input drives the list: the search field has not been typed
    /// into or focused since the last arrow key or row click. The selected
    /// row then holds accessibility focus.
    list_focus: bool,
    accessibility: Option<a11y::Accessibility<Message>>,
    output: Arc<Mutex<Option<AppSelection>>>,
}

#[derive(Clone, Debug)]
enum Message {
    Opened,
    Loaded(Result<Vec<Application>, String>),
    Update(Vec<String>),
    Search(String),
    Select(String),
    Accept,
    Token(u64, Option<String>),
    Cancel,
    Event(Event),
    Accessibility(canopa_ui::accessibility::Event<Message>),
}

impl canopa_ui::accessibility::Message for Message {
    fn accessibility(event: canopa_ui::accessibility::Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}

impl Chooser {
    fn subscription(&self) -> Subscription<Message> {
        let input = iced::event::listen_with(|event, status, _| {
            let cancel = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (cancel || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        });
        match &self.accessibility {
            Some(state) => Subscription::batch([input, state.bridge.subscription()]),
            None => input,
        }
    }
    fn load(&mut self) -> Task<Message> {
        if self.loading {
            self.reload_pending = true;
            return Task::none();
        }
        self.loading = true;
        let foreground = self.foreground;
        let choices = self.request.choices.clone();
        let mime = request_type(
            self.request.content_type.as_deref(),
            self.request.uri.as_deref(),
        );
        Task::perform(
            worker(move || {
                canopa_apps::discover_choice_metadata()
                    .map(|catalog| applications(catalog, foreground, &choices, mime.as_deref()))
                    .map_err(|error| error.to_string())
            }),
            |result| Message::Loaded(result.and_then(|value| value)),
        )
    }
    fn visible(&self) -> Vec<&Application> {
        let query = self.search.to_lowercase();
        self.rows
            .iter()
            .filter(|row| row.search.contains(&query))
            .collect()
    }
    fn reconcile(&mut self) {
        self.rows = offered(&self.request.choices, &self.catalog);
        let last_choice = self.request.last_choice.as_deref();
        self.rows
            .sort_by_key(|row| (row.fit, !is_last_used(row, last_choice)));
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.visible().iter().any(|row| &row.id == id))
        {
            self.selected = None;
        }
        self.last_click = None;
    }
    /// Handle a message, then refresh the accessibility tree if one is read.
    fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Accessibility(event) = message {
            if matches!(event, canopa_ui::accessibility::Event::Focus(_)) {
                // An assistive technology moved focus to the search field.
                self.list_focus = false;
            }
            return a11y::update(&mut self.accessibility, event);
        }
        let task = self.apply(message);
        a11y::refresh(&mut self.accessibility, task)
    }
    fn exit(&mut self) -> Task<Message> {
        if let Some(state) = &mut self.accessibility {
            state.close();
        }
        iced::exit()
    }
    fn apply(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened => {
                let window = self.window;
                if let Some(state) = &mut self.accessibility {
                    state.opened(&window_title(), window);
                }
            }
            Message::Accessibility(_) => {}
            Message::Cancel => return self.exit(),
            Message::Loaded(result) => {
                self.loading = false;
                match result {
                    Ok(catalog) => {
                        self.catalog = catalog;
                        self.error = None;
                        self.reconcile();
                    }
                    Err(error) => self.error = Some(error),
                }
                if self.initial {
                    self.initial = false;
                    self.selected = self.visible().first().map(|row| row.id.clone());
                }
                if std::mem::take(&mut self.reload_pending) {
                    return self.load();
                }
            }
            Message::Update(choices) => {
                self.pending = None;
                self.request.choices = choices;
                self.reconcile();
                return self.load();
            }
            Message::Search(value) => {
                self.pending = None;
                self.list_focus = false;
                self.search = value;
                self.selected = self.visible().first().map(|row| row.id.clone());
                self.last_click = None;
                return iced::widget::operation::snap_to(
                    "app-chooser-results",
                    scrollable::RelativeOffset::START,
                );
            }
            Message::Select(id) => {
                self.pending = None;
                if !self.visible().iter().any(|row| row.id == id) {
                    return Task::none();
                }
                let now = Instant::now();
                let double = self.last_click.as_ref().is_some_and(|(previous, at)| {
                    previous == &id && now.duration_since(*at).as_millis() < 400
                });
                self.last_click = Some((id.clone(), now));
                self.selected = Some(id);
                self.list_focus = true;
                if double {
                    return self.apply(Message::Accept);
                }
            }
            Message::Accept => {
                if self.initial || self.pending.is_some() {
                    return Task::none();
                }
                if let Some(id) = self
                    .selected
                    .clone()
                    .filter(|id| self.visible().iter().any(|row| &row.id == id))
                {
                    self.epoch += 1;
                    let epoch = self.epoch;
                    self.pending = Some((epoch, id, Instant::now()));
                    return Task::batch([
                        window::request_activation_token(self.window)
                            .map(move |token| Message::Token(epoch, token)),
                        Task::perform(
                            worker(|| std::thread::sleep(std::time::Duration::from_millis(1100))),
                            move |_| Message::Token(epoch, None),
                        ),
                    ]);
                }
            }
            Message::Token(epoch, token) => {
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|(expected, _, _)| *expected == epoch)
                {
                    let (_, id, at) = self.pending.take().expect("matched proposal");
                    if self.visible().iter().any(|row| row.id == id)
                        && self.selected.as_ref() == Some(&id)
                    {
                        *self.output.lock().expect("app chooser output") = Some(AppSelection {
                            choice: id,
                            activation_token: token.filter(|token| {
                                !token.is_empty()
                                    && at.elapsed() < std::time::Duration::from_secs(1)
                            }),
                        });
                        return self.exit();
                    }
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.apply(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                use keyboard::{Key, key::Named};
                match key.as_ref() {
                    Key::Named(Named::Escape) => return self.apply(Message::Cancel),
                    Key::Character("f") if modifiers.control() => {
                        self.list_focus = false;
                        return iced::widget::operation::focus("applications-search");
                    }
                    Key::Named(Named::Enter) => return self.apply(Message::Accept),
                    Key::Named(Named::ArrowDown | Named::ArrowUp) => {
                        let rows = self.visible();
                        if !rows.is_empty() {
                            let current = self
                                .selected
                                .as_ref()
                                .and_then(|id| rows.iter().position(|row| &row.id == id));
                            let index = match current {
                                Some(i) if key == Key::Named(Named::ArrowUp) => i.saturating_sub(1),
                                Some(i) => (i + 1).min(rows.len() - 1),
                                None => 0,
                            };
                            let position =
                                index as f32 / rows.len().saturating_sub(1).max(1) as f32;
                            self.selected = Some(rows[index].id.clone());
                            self.last_click = None;
                            self.list_focus = true;
                            return iced::widget::operation::snap_to(
                                "app-chooser-results",
                                scrollable::RelativeOffset {
                                    x: None,
                                    y: Some(position),
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
            Message::Event(_) => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let heading = match self
            .request
            .filename
            .as_deref()
            .or(self.request.content_type.as_deref())
        {
            Some(item) => fl!("portal-app-chooser-heading", item = item),
            None => fl!("portal-app-chooser-heading-any"),
        };
        let visible = self.visible();
        let total = visible.len();
        // Arrow keys move the selection even while the search field has
        // keyboard focus, so the field points at the row they move.
        let focused_row = self
            .selected
            .as_deref()
            .filter(|id| self.list_focus && visible.iter().any(|row| row.id == *id))
            .map(application_key);
        let mut listing = column![].spacing(4);
        let has_associations = self.rows.iter().any(|row| row.fit != FileTypeFit::Other);
        let mut group = None;
        for (index, application) in visible.into_iter().enumerate() {
            let associated = application.fit != FileTypeFit::Other;
            if has_associations && group != Some(associated) {
                group = Some(associated);
                let title = if associated {
                    fl!("portal-app-chooser-associated")
                } else {
                    fl!("portal-app-chooser-other")
                };
                listing = listing.push(container(dialog_ui::section(title)).padding([8, 10]));
            }
            let icon: Element<'_, Message> = application.icon.as_ref().map_or_else(
                || {
                    let initial: String = application
                        .name
                        .chars()
                        .next()
                        .into_iter()
                        .flat_map(char::to_uppercase)
                        .collect();
                    container(dialog_ui::section(initial))
                        .width(32)
                        .height(32)
                        .center(32)
                        .style(dialog_ui::card)
                        .into()
                },
                |handle| image(handle.clone()).width(32).height(32).into(),
            );
            let selected = self.selected.as_deref() == Some(&application.id);
            let key = application_key(&application.id);
            let select = Message::Select(application.id.clone());
            let mut label = column![text(&application.name)]
                .spacing(3)
                .width(Length::Fill);
            if !application.caption.is_empty() {
                label = label.push(dialog_ui::detail(&application.caption).size(13));
            }
            let badge = if application.fit == FileTypeFit::Default {
                Some(fl!("portal-app-chooser-default"))
            } else if is_last_used(application, self.request.last_choice.as_deref()) {
                Some(fl!("portal-app-chooser-last-used"))
            } else {
                None
            };
            let description = [
                application.caption.as_str(),
                badge.as_deref().unwrap_or_default(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
            let focused = focused_row.as_ref() == Some(&key);
            listing = listing.push(semantic(
                Node::option(&application.name, selected)
                    .key(key.clone())
                    .description(description)
                    .position(index + 1, total)
                    .focused(focused_row.as_ref() == Some(&key))
                    .on_click(Some(select.clone())),
                button(
                    canopa_ui::direction::row([
                        icon,
                        label.into(),
                        badge.map_or_else(
                            || iced::widget::space().width(0).into(),
                            |badge| {
                                container(dialog_ui::detail(badge).size(12))
                                    .padding([3, 8])
                                    .style(dialog_ui::card)
                                    .into()
                            },
                        ),
                    ])
                    .spacing(10)
                    .align_y(iced::Center),
                )
                .padding([8, 10])
                .width(Length::Fill)
                .on_press(select)
                .style(move |theme, status| dialog_ui::selection(theme, status, selected, focused)),
            ));
        }
        let status = if self.rows.is_empty() && self.loading {
            fl!("portal-app-chooser-loading")
        } else if self.rows.is_empty() {
            fl!("portal-app-chooser-none")
        } else if total == 0 {
            fl!("portal-app-chooser-no-matches")
        } else {
            fl!("portal-app-chooser-count", count = total)
        };
        if total == 0 {
            listing = listing.push(container(dialog_ui::detail(status.clone())).padding(12));
        }
        let search = fl!("portal-app-chooser-search");
        let mut content = column![
            semantic(
                Node::heading(&heading, 1),
                dialog_ui::heading(heading.clone())
            ),
            semantic(
                Node::search_input(search.clone(), &self.search)
                    .key("app-chooser-search")
                    .placeholder(search.clone())
                    .controls(APPLICATIONS_KEY)
                    .active_descendant(focused_row.clone())
                    .on_text(Message::Search),
                text_input(&search, &self.search)
                    .id("applications-search")
                    .padding([8, 10])
                    .style(dialog_ui::field)
                    .on_input(Message::Search)
                    .on_submit(Message::Accept),
            ),
            semantic(
                Node::list(fl!("portal-app-chooser-applications"))
                    .key(APPLICATIONS_KEY)
                    .busy(self.loading)
                    .active_descendant(focused_row),
                container(
                    scrollable(listing)
                        .style(canopa_ui::scrollbar)
                        .id("app-chooser-results")
                        .spacing(6)
                        .height(Length::Fill)
                )
                .padding(6)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(dialog_ui::card),
            ),
            a11y::announcer("app-chooser-status", status, false),
        ]
        .spacing(12);
        if let Some(error) = &self.error {
            content = content.push(semantic(
                Node::alert(error).key("app-chooser-error"),
                text(error).style(text::danger),
            ));
        }
        let open = (!self.initial && self.pending.is_none() && self.selected.is_some())
            .then_some(Message::Accept);
        let selection = self
            .selected
            .as_deref()
            .and_then(|id| self.rows.iter().find(|row| row.id == id));
        let footer = canopa_ui::direction::row([
            dialog_ui::detail(selection.map_or_else(
                || fl!("portal-app-chooser-select"),
                |application| {
                    fl!(
                        "portal-app-chooser-selected",
                        application = application.name.as_str()
                    )
                },
            ))
            .wrapping(iced::widget::text::Wrapping::None)
            .width(Length::Fill)
            .into(),
            a11y::button(
                "app-chooser-cancel",
                fl!("portal-cancel"),
                Some(Message::Cancel),
                dialog_ui::action(
                    text(fl!("portal-cancel")),
                    Some(Message::Cancel),
                    canopa_ui::Action::Normal,
                    false,
                ),
            ),
            a11y::button(
                "app-chooser-open",
                fl!("portal-app-chooser-open"),
                open.clone(),
                dialog_ui::action(
                    text(fl!("portal-app-chooser-open")),
                    open,
                    canopa_ui::Action::Primary,
                    false,
                ),
            ),
        ])
        .spacing(8)
        .align_y(iced::Center);
        a11y::dialog_window(
            WINDOW_KEY,
            window_title(),
            Node::dialog(fl!("portal-app-chooser-title"))
                .key("app-chooser-dialog")
                .description(heading)
                .busy(self.loading || self.pending.is_some()),
            dialog_ui::frame(
                container(content)
                    .padding([20, 24])
                    .width(Length::Fill)
                    .height(Length::Fill),
                footer,
            ),
        )
    }
}

#[cfg(all(test, feature = "accessibility-testing"))]
mod accessibility_tests;
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn app(id: &str, name: &str) -> Application {
        Application {
            id: id.into(),
            name: name.into(),
            caption: id.into(),
            search: format!("{id} {name}").to_lowercase(),
            icon: None,
            fit: FileTypeFit::Other,
        }
    }
    #[test]
    fn protocol_identity_and_order_survive_metadata_lookup_and_unknown_ids() {
        let rows = offered(
            &[
                "org.example.Editor".into(),
                "unknown.desktop".into(),
                "reader.desktop".into(),
            ],
            &[
                app("reader.desktop", "Reader"),
                app("org.example.Editor.desktop", "Editor"),
            ],
        );
        assert_eq!(
            rows.iter()
                .map(|row| (row.id.as_str(), row.name.as_str()))
                .collect::<Vec<_>>(),
            [
                ("org.example.Editor", "Editor"),
                ("unknown.desktop", "unknown.desktop"),
                ("reader.desktop", "Reader")
            ]
        );
    }
    #[test]
    fn file_type_fit_outranks_recency_without_expanding_the_frontend_choices() {
        let mut chooser = chooser();
        let mut reader = app("reader.desktop", "Reader");
        reader.fit = FileTypeFit::Default;
        let mut browser = app("browser.desktop", "Browser");
        browser.fit = FileTypeFit::Associated;
        let mut omitted = app("omitted.desktop", "Another reader");
        omitted.fit = FileTypeFit::Default;
        chooser.catalog.extend([reader, browser, omitted]);
        chooser.request.choices = vec!["b.desktop".into(), "browser".into(), "reader".into()];
        chooser.request.last_choice = Some("b".into());
        chooser.initial = true;
        let _ = chooser.apply(Message::Loaded(Ok(chooser.catalog.clone())));
        assert_eq!(chooser.selected.as_deref(), Some("reader"));
        assert_eq!(
            chooser
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["reader", "browser", "b.desktop"]
        );
    }

    pub(super) fn chooser() -> Chooser {
        Chooser {
            request: AppRequest::parse(
                String::new(),
                String::new(),
                vec!["a.desktop".into(), "b.desktop".into()],
                Default::default(),
            )
            .unwrap(),
            window: window::Id::unique(),
            pending: None,
            epoch: 0,
            theme: canopa_ui::theme(Default::default()),
            foreground: canopa_theme::Rgb(255, 255, 255),
            catalog: vec![app("a.desktop", "Alpha"), app("b.desktop", "Beta")],
            rows: vec![],
            search: String::new(),
            selected: Some("a.desktop".into()),
            loading: false,
            reload_pending: false,
            initial: false,
            error: None,
            last_click: None,
            list_focus: true,
            accessibility: None,
            output: Arc::default(),
        }
    }
    #[test]
    fn a_live_update_removes_selection_before_a_later_acceptance() {
        let mut chooser = chooser();
        chooser.reconcile();
        chooser.pending = Some((1, "a.desktop".into(), Instant::now()));
        chooser.request.choices = vec!["b.desktop".into()];
        chooser.reconcile();
        assert!(chooser.selected.is_none());
        let _ = chooser.update(Message::Token(1, Some("fresh-but-removed".into())));
        assert!(chooser.output.lock().unwrap().is_none());
        let _ = chooser.update(Message::Accept);
        assert!(chooser.output.lock().unwrap().is_none());
    }
    #[test]
    fn overdue_token_is_not_forwarded_and_a_superseded_reply_cannot_complete() {
        let mut chooser = chooser();
        chooser.reconcile();
        chooser.pending = Some((
            2,
            "a.desktop".into(),
            Instant::now() - std::time::Duration::from_secs(2),
        ));
        let _ = chooser.update(Message::Token(1, Some("superseded".into())));
        assert!(chooser.output.lock().unwrap().is_none());
        assert!(chooser.pending.is_some());
        let _ = chooser.update(Message::Token(2, Some("late".into())));
        let selection = chooser.output.lock().unwrap().take().unwrap();
        assert_eq!(selection.choice, "a.desktop");
        assert!(selection.activation_token.is_none());
    }
}
