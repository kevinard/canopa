//! Iced application selection. The frontend retains launch and MIME policy.
use crate::icons;
use crate::{
    app_chooser::{AppRequest, AppSelection},
    worker::run as worker,
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{button, column, container, image, row, scrollable, text, text_input},
    window,
};
use std::{
    collections::HashMap,
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
}

fn applications(
    catalog: Vec<canopa_apps::DesktopApplication>,
    foreground: canopa_theme::Rgb,
    choices: &[String],
) -> Vec<Application> {
    let visible: Vec<_> = catalog
        .iter()
        .filter(|app| {
            choices.is_empty()
                || choices
                    .iter()
                    .any(|id| id == &app.id || app.id.strip_suffix(".desktop") == Some(id.as_str()))
        })
        .cloned()
        .collect();
    let names: Vec<_> = visible.iter().filter_map(|app| app.icon.clone()).collect();
    let images = icons::prepare(&names, foreground);
    catalog
        .into_iter()
        .map(|app| Application {
            search: format!(
                "{} {} {} {}",
                app.name,
                app.id,
                app.generic_name.as_deref().unwrap_or_default(),
                app.keywords.join(" ")
            )
            .to_lowercase(),
            icon: app.icon.as_ref().and_then(|name| images.get(name)).cloned(),
            caption: app.generic_name.unwrap_or_else(|| app.id.clone()),
            id: app.id,
            name: app.name,
        })
        .collect()
}

fn offered(choices: &[String], catalog: &[Application]) -> Vec<Application> {
    if choices.is_empty() {
        return catalog.to_owned();
    }
    let index: HashMap<_, _> = catalog.iter().map(|app| (app.id.as_str(), app)).collect();
    choices
        .iter()
        .map(|id| {
            let mut row = index
                .get(id.as_str())
                .or_else(|| index.get(format!("{id}.desktop").as_str()))
                .map(|row| (*row).clone())
                .unwrap_or_else(|| Application {
                    id: id.clone(),
                    name: id.clone(),
                    caption: "Application".into(),
                    search: id.to_lowercase(),
                    icon: None,
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
                size: Size::new(620.0, 480.0),
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
    .title(|_: &Chooser, _| "Open with — Canopa".to_owned())
    .theme(|app: &Chooser, _| app.theme.clone())
    .subscription(Chooser::subscription)
    .default_font(font)
    .run()
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(result.lock().expect("app chooser output").take())
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
}

impl Chooser {
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let cancel = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (cancel || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }
    fn load(&mut self) -> Task<Message> {
        if self.loading {
            self.reload_pending = true;
            return Task::none();
        }
        self.loading = true;
        let foreground = self.foreground;
        let choices = self.request.choices.clone();
        Task::perform(
            worker(move || {
                canopa_apps::discover_choice_metadata()
                    .map(|catalog| applications(catalog, foreground, &choices))
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
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.visible().iter().any(|row| &row.id == id))
        {
            self.selected = None;
        }
        self.last_click = None;
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened => {}
            Message::Cancel => return iced::exit(),
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
                    self.selected = self
                        .request
                        .last_choice
                        .clone()
                        .filter(|id| self.rows.iter().any(|row| &row.id == id))
                        .or_else(|| self.rows.first().map(|row| row.id.clone()));
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
                self.search = value;
                self.selected = self.visible().first().map(|row| row.id.clone());
                self.last_click = None;
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
                if double {
                    return self.update(Message::Accept);
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
                        return iced::exit();
                    }
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.update(Message::Cancel);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                use keyboard::{Key, key::Named};
                match key.as_ref() {
                    Key::Named(Named::Escape) => return self.update(Message::Cancel),
                    Key::Character("f") if modifiers.control() => {
                        return iced::widget::operation::focus("applications-search");
                    }
                    Key::Named(Named::Enter) => return self.update(Message::Accept),
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
                            self.selected = Some(rows[index].id.clone());
                            self.last_click = None;
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
        let context = self
            .request
            .filename
            .as_deref()
            .or(self.request.content_type.as_deref())
            .unwrap_or("this item");
        let mut listing = column![].spacing(4);
        for application in self.visible() {
            let icon: Element<'_, Message> = application.icon.as_ref().map_or_else(
                || container(text("◇")).width(32).height(32).center(32).into(),
                |handle| image(handle.clone()).width(32).height(32).into(),
            );
            let selected = self.selected.as_deref() == Some(&application.id);
            listing = listing.push(
                button(
                    row![
                        icon,
                        column![
                            text(&application.name),
                            text(&application.caption)
                                .size(13)
                                .style(move |theme| if selected {
                                    text::Style::default()
                                } else {
                                    canopa_ui::muted(theme)
                                })
                        ]
                        .spacing(3)
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                )
                .padding(8)
                .width(Length::Fill)
                .on_press(Message::Select(application.id.clone()))
                .style(move |theme: &Theme, status| {
                    if selected {
                        button::primary(theme, status)
                    } else {
                        button::text(theme, status)
                    }
                }),
            );
        }
        if self.rows.is_empty() {
            listing = listing.push(text(if self.loading {
                "Loading applications…"
            } else {
                "No applications available"
            }));
        }
        let mut content = column![
            text(format!("Choose an application for {context}")).size(20),
            text_input("Search applications", &self.search)
                .id("applications-search")
                .on_input(Message::Search)
                .on_submit(Message::Accept),
            scrollable(listing).height(Length::Fill)
        ]
        .spacing(12);
        if let Some(error) = &self.error {
            content = content.push(text(error).style(text::danger));
        }
        let footer = row![
            iced::widget::space().width(Length::Fill),
            canopa_ui::action(
                text("Cancel"),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false
            ),
            canopa_ui::action(
                text("Open"),
                (!self.initial && self.pending.is_none() && self.selected.is_some())
                    .then_some(Message::Accept),
                canopa_ui::Action::Normal,
                false
            )
        ]
        .spacing(10);
        container(content.push(footer))
            .padding(16)
            .style(canopa_ui::surface)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(id: &str, name: &str) -> Application {
        Application {
            id: id.into(),
            name: name.into(),
            caption: id.into(),
            search: format!("{id} {name}").to_lowercase(),
            icon: None,
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
    fn chooser() -> Chooser {
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
