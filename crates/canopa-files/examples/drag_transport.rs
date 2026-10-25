//! Native transport acceptance probe, deliberately without filesystem mutations.
//! Run only with the private desktop fixture; this is not a file-manager frontend.
use canopa_files::{
    file_clipboard::{Selection, URIS},
    transfers::TransferKind,
};
use iced::futures::StreamExt;
use iced::window::drag_drop::{self, Action, Actions, Content, DragDrop, Offer, Origin, Source};
use iced::{Element, Event, Subscription, Task, keyboard, mouse, window};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}
impl Notices {
    fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(self.clone(), |key| {
            let receiver = key.0.lock().unwrap().take();
            iced::futures::stream::unfold(receiver, |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((Message::Changed, Some(receiver)))
            })
        })
    }
}

#[derive(Debug, Clone)]
enum Message {
    Opened(window::Id),
    Enabled(Option<DragDrop>),
    Event(Event, window::Id),
    Changed,
    Read(u64, Result<Vec<u8>, String>),
}
struct Incoming {
    offer: Offer,
    data: Option<Result<Vec<u8>, String>>,
}
struct Probe {
    mode: String,
    id: window::Id,
    live: bool,
    close_requested: bool,
    endpoint: Option<DragDrop>,
    notices: Option<Notices>,
    contents: Vec<Content>,
    sources: Vec<Source>,
    offers: BTreeMap<u64, Incoming>,
    held_origin: Option<Origin>,
    stale_refused: bool,
    stale_attempted: bool,
    action: Action,
    finish_allowed: bool,
    errors: Vec<String>,
    reports: mpsc::Sender<Value>,
}
impl Probe {
    fn update(&mut self, message: Message) -> Task<Message> {
        let mut tasks = Vec::new();
        match message {
            Message::Opened(id) => tasks.push(drag_drop::enable(id).map(Message::Enabled)),
            Message::Enabled(endpoint) => {
                self.notices = endpoint
                    .as_ref()
                    .and_then(DragDrop::take_notices)
                    .map(|receiver| Notices(Arc::new(Mutex::new(Some(receiver)))));
                self.endpoint = endpoint;
                if self.endpoint.is_none() {
                    self.errors.push("Native drag transport unavailable".into());
                }
            }
            Message::Event(Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)), _) => {
                // Consume EVERY client left press, including ordinary receiver clicks.
                let origin = self.endpoint.as_ref().and_then(DragDrop::capture_origin);
                if self.mode == "stale" && self.held_origin.is_none() && !self.stale_attempted {
                    self.held_origin = origin;
                } else if self.mode == "source"
                    || self.mode == "source-close"
                    || self.mode == "stale"
                {
                    let origin = if self.mode == "stale" {
                        self.held_origin.take()
                    } else {
                        origin
                    };
                    if let (Some(endpoint), Some(origin)) = (&self.endpoint, origin) {
                        match endpoint.start(
                            origin,
                            self.contents.clone(),
                            Actions {
                                copy: true,
                                move_files: true,
                            },
                        ) {
                            Ok(source) => self.sources.push(source),
                            Err(error) if self.mode == "stale" => {
                                self.stale_attempted = true;
                                self.stale_refused = true;
                                self.errors.push(error);
                            }
                            Err(error) => self.errors.push(error),
                        }
                    }
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested), id) => {
                tasks.push(window::close(id))
            }
            Message::Event(Event::Window(window::Event::Closed), _) => self.live = false,
            Message::Event(
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }),
                _,
            ) => {
                for source in &self.sources {
                    let state = source.snapshot();
                    if !state.dropped && !state.finished {
                        source.cancel();
                    }
                }
            }
            Message::Event(
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Enter),
                    ..
                }),
                _,
            ) => self.finish_allowed = true,
            Message::Read(id, result) => {
                if let Some(incoming) = self.offers.get_mut(&id) {
                    incoming.data = Some(result);
                }
            }
            Message::Changed | Message::Event(_, _) => {}
        }
        if let Some(endpoint) = &self.endpoint {
            for offer in endpoint.offers() {
                let snapshot = offer.snapshot();
                if self.offers.contains_key(&snapshot.id) || snapshot.finished {
                    continue;
                }
                let supports = match self.action {
                    Action::Copy => snapshot.actions.copy,
                    Action::Move => snapshot.actions.move_files,
                };
                if offer.has_format(URIS) && supports && !snapshot.dropped && !snapshot.left {
                    let supported = Actions {
                        copy: self.action == Action::Copy,
                        move_files: self.action == Action::Move,
                    };
                    if let Err(error) =
                        offer.accept(Some(URIS.into()), supported, Some(self.action))
                    {
                        self.errors.push(error);
                        continue;
                    }
                    let read = offer.read(URIS.into(), 16 * 1024 * 1024, Duration::from_secs(5));
                    tasks.push(Task::perform(read, move |result| {
                        Message::Read(snapshot.id, result)
                    }));
                    self.offers
                        .insert(snapshot.id, Incoming { offer, data: None });
                }
            }
        }
        for incoming in self.offers.values() {
            let snapshot = incoming.offer.snapshot();
            if snapshot.dropped
                && !snapshot.finished
                && self.finish_allowed
                && incoming.data.is_some()
            {
                let success = incoming.data.as_ref().is_some_and(Result::is_ok);
                if let Err(error) = incoming.offer.finish(success) {
                    self.errors.push(error);
                }
            }
        }
        if self.mode == "source-close"
            && !self.close_requested
            && self.sources.iter().any(|source| source.snapshot().dropped)
        {
            self.close_requested = true;
            tasks.push(window::close(self.id));
        }
        self.report();
        Task::batch(tasks)
    }
    fn report(&self) {
        let sources: Vec<_> = self.sources.iter().map(|source| {
            let s = source.snapshot();
            json!({"id":s.id,"selected":format!("{:?}",s.selected),"dropped":s.dropped,"finished":s.finished,"cancelled":s.cancelled,"error":s.error})
        }).collect();
        let offers: Vec<_> = self.offers.values().map(|incoming| {
            let s = incoming.offer.snapshot();
            json!({"id":s.id,"selected":format!("{:?}",s.selected),"dropped":s.dropped,"left":s.left,"finished":s.finished,"local_source":s.local_source,
                "data":incoming.data.as_ref().and_then(|result|result.as_ref().ok()),
                "error":incoming.data.as_ref().and_then(|result|result.as_ref().err())})
        }).collect();
        let _ = self.reports.send(json!({"ready":self.endpoint.is_some(),"window_live":self.live,"sources":sources,"offers":offers,"stale_refused":self.stale_refused,"errors":self.errors}));
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        iced::widget::container(
            iced::widget::text(format!(
                "Iced drag transport\n{}\nDrop completion: {}\nReturn releases held incoming drops",
                self.mode, self.finish_allowed
            ))
            .size(24),
        )
        .center_x(iced::Fill)
        .center_y(iced::Fill)
        .into()
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::event::listen_with(|event, _, id| Some(Message::Event(event, id))),
            self.notices
                .as_ref()
                .map_or_else(Subscription::none, Notices::subscription),
        ])
    }
}
fn main() -> iced::Result {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(
        arguments.len(),
        5,
        "MODE REPORT SOURCE COPY|MOVE HOLD|FINISH"
    );
    let mode = arguments[0].to_string_lossy().into_owned();
    let report = PathBuf::from(&arguments[1]);
    let path = PathBuf::from(&arguments[2]);
    let home = PathBuf::from(std::env::var_os("HOME").expect("private HOME"));
    assert!(
        path.is_absolute()
            && path
                .canonicalize()
                .unwrap()
                .starts_with(home.canonicalize().unwrap())
    );
    let bytes = Selection::new(vec![path], TransferKind::Copy)
        .unwrap()
        .encode(URIS)
        .unwrap();
    let (reports, receiver) = mpsc::channel::<Value>();
    std::thread::Builder::new()
        .name("drag-probe-report".into())
        .spawn(move || {
            for value in receiver {
                let pending = report.with_extension("new");
                std::fs::write(&pending, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
                std::fs::rename(pending, &report).unwrap();
            }
        })
        .unwrap();
    let action = if arguments[3] == "move" {
        Action::Move
    } else {
        Action::Copy
    };
    let finish_allowed = arguments[4] != "hold";
    iced::daemon(
        move || {
            let (id, open) = window::open(window::Settings {
                size: iced::Size::new(640., 650.),
                exit_on_close_request: false,
                ..Default::default()
            });
            (
                Probe {
                    mode: mode.clone(),
                    id,
                    live: true,
                    close_requested: false,
                    endpoint: None,
                    notices: None,
                    contents: vec![Content {
                        mime: URIS.into(),
                        bytes: bytes.clone().into(),
                    }],
                    sources: Vec::new(),
                    offers: BTreeMap::new(),
                    held_origin: None,
                    stale_refused: false,
                    stale_attempted: false,
                    action,
                    finish_allowed,
                    errors: Vec::new(),
                    reports: reports.clone(),
                },
                open.map(Message::Opened),
            )
        },
        Probe::update,
        Probe::view,
    )
    .title("Iced DnD transport")
    .subscription(Probe::subscription)
    .run()
}
