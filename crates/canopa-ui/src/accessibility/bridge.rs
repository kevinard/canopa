//! One AT-SPI adapter per window, fed from collected widget semantics.
//!
//! Nothing is collected or built until an assistive technology activates a
//! window: `accesskit_unix` connects to the accessibility bus only once
//! `org.a11y.Status.IsEnabled` is set, and the bridge does no work before.
use super::collect::{Collect, Snapshot};
use super::node::Value;
use super::tree::{self, Bound, Ids};
use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler,
    NodeId, TreeId, TreeUpdate,
};
use iced::advanced::widget::Id;
use iced::futures::channel::mpsc;
use iced::futures::{Stream, StreamExt};
use iced::{Subscription, Task, window};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The application message that carries accessibility events.
pub trait Message: Clone + Send + 'static {
    fn accessibility(event: Event<Self>) -> Self;
}

/// Something the bridge needs the application loop to pass back to it.
pub enum Event<M> {
    /// An assistive technology started (or stopped) listening to a window.
    Activated,
    Deactivated,
    /// A default action, increment, value change… as the user's input would send it.
    Action(Box<M>),
    /// Move keyboard focus to this widget.
    Focus(Id),
    /// Input that may move focus or change what is shown.
    Changed,
    WindowFocus(window::Id, bool),
    Collected(Snapshot<M>),
}

impl<M: Clone> Clone for Event<M> {
    fn clone(&self) -> Self {
        match self {
            Self::Activated => Self::Activated,
            Self::Deactivated => Self::Deactivated,
            Self::Action(message) => Self::Action(message.clone()),
            Self::Focus(id) => Self::Focus(id.clone()),
            Self::Changed => Self::Changed,
            Self::WindowFocus(window, focused) => Self::WindowFocus(*window, *focused),
            Self::Collected(snapshot) => Self::Collected(snapshot.clone()),
        }
    }
}

impl<M> std::fmt::Debug for Event<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Activated => f.write_str("Activated"),
            Self::Deactivated => f.write_str("Deactivated"),
            Self::Action(_) => f.write_str("Action(..)"),
            Self::Focus(id) => f.debug_tuple("Focus").field(id).finish(),
            Self::Changed => f.write_str("Changed"),
            Self::WindowFocus(window, focused) => f
                .debug_tuple("WindowFocus")
                .field(window)
                .field(focused)
                .finish(),
            Self::Collected(snapshot) => f.debug_tuple("Collected").field(snapshot).finish(),
        }
    }
}

struct Published<M> {
    tree: TreeUpdate,
    actions: HashMap<NodeId, Bound<M>>,
}

struct Handler<M> {
    published: Arc<Mutex<Published<M>>>,
    active: Arc<AtomicBool>,
    events: mpsc::UnboundedSender<Event<M>>,
}

impl<M: Send + 'static> ActivationHandler for Handler<M> {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.active.store(true, Ordering::SeqCst);
        let _ = self.events.unbounded_send(Event::Activated);
        Some(self.published.lock().unwrap().tree.clone())
    }
}

impl<M: Clone + Send + 'static> ActionHandler for Handler<M> {
    fn do_action(&mut self, request: ActionRequest) {
        if request.target_tree != TreeId::ROOT {
            return;
        }
        let published = self.published.lock().unwrap();
        // Retired nodes have no entry, so a stale request does nothing.
        let Some(bound) = published.actions.get(&request.target_node) else {
            return;
        };
        let actions = &bound.actions;
        let event = match request.action {
            Action::Click => actions.click.clone().map(|m| Event::Action(Box::new(m))),
            Action::Increment => actions
                .increment
                .clone()
                .map(|m| Event::Action(Box::new(m))),
            Action::Decrement => actions
                .decrement
                .clone()
                .map(|m| Event::Action(Box::new(m))),
            Action::Expand => actions.expand.clone().map(|m| Event::Action(Box::new(m))),
            Action::Collapse => actions.collapse.clone().map(|m| Event::Action(Box::new(m))),
            Action::Focus => bound.focus.clone().map(Event::Focus),
            Action::SetValue => {
                let value = match request.data {
                    Some(ActionData::Value(text)) => Some(Value::Text(text.into())),
                    Some(ActionData::NumericValue(number)) if number.is_finite() => {
                        Some(Value::Number(number))
                    }
                    _ => None,
                };
                value
                    .zip(actions.set_value.clone())
                    .and_then(|(value, set)| set(value))
                    .map(|m| Event::Action(Box::new(m)))
            }
            _ => None,
        };
        drop(published);
        if let Some(event) = event {
            let _ = self.events.unbounded_send(event);
        }
    }
}

struct Deactivate<M> {
    active: Arc<AtomicBool>,
    events: mpsc::UnboundedSender<Event<M>>,
}
impl<M: Send + 'static> DeactivationHandler for Deactivate<M> {
    fn deactivate_accessibility(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        let _ = self.events.unbounded_send(Event::Deactivated);
    }
}

struct Window<M> {
    window: Option<window::Id>,
    adapter: accesskit_unix::Adapter,
    published: Arc<Mutex<Published<M>>>,
    active: Arc<AtomicBool>,
    ids: Ids,
    focused: bool,
    focus: Option<String>,
}

impl<M> Drop for Window<M> {
    fn drop(&mut self) {
        // The adapter may still deliver a queued request; it must find nothing.
        self.published.lock().unwrap().actions.clear();
    }
}

#[derive(Clone)]
struct Events<M> {
    name: &'static str,
    receiver: Arc<iced::futures::lock::Mutex<mpsc::UnboundedReceiver<Event<M>>>>,
}
impl<M> Hash for Events<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "canopa-accessibility".hash(state);
        self.name.hash(state);
    }
}

fn events<M: Message>(source: &Events<M>) -> impl Stream<Item = M> + use<M> {
    iced::futures::stream::unfold(source.receiver.clone(), |receiver| async move {
        let event = receiver.lock().await.next().await?;
        Some((M::accessibility(event), receiver))
    })
}

fn changed<M: Message>(
    event: iced::Event,
    _: iced::event::Status,
    window: window::Id,
) -> Option<M> {
    use iced::{Event as E, keyboard, mouse};
    let event = match event {
        E::Window(window::Event::Focused) => Event::WindowFocus(window, true),
        E::Window(window::Event::Unfocused) => Event::WindowFocus(window, false),
        E::Window(window::Event::Resized(_))
        | E::Keyboard(keyboard::Event::KeyPressed { .. })
        | E::Mouse(mouse::Event::ButtonReleased(_))
        | E::Mouse(mouse::Event::WheelScrolled { .. }) => Event::Changed,
        _ => return None,
    };
    Some(M::accessibility(event))
}

/// The application's accessibility state. Keep one per process and route
/// `Event`s from [`Bridge::subscription`] back into [`Bridge::update`].
pub struct Bridge<M> {
    sender: mpsc::UnboundedSender<Event<M>>,
    events: Events<M>,
    windows: BTreeMap<String, Window<M>>,
    collecting: bool,
    dirty: bool,
    scale: f64,
}

impl<M: Message> Bridge<M> {
    /// `name` distinguishes bridges in one process's subscriptions.
    pub fn new(name: &'static str) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        Self {
            sender,
            events: Events {
                name,
                receiver: Arc::new(iced::futures::lock::Mutex::new(receiver)),
            },
            windows: BTreeMap::new(),
            collecting: false,
            dirty: false,
            scale: 1.0,
        }
    }

    /// Publish the window whose root node has `key`. The adapter registers on
    /// the accessibility bus only when an assistive technology is running.
    pub fn open(&mut self, key: impl Into<String>, title: &str, window: Option<window::Id>) {
        let key = key.into();
        if self.windows.contains_key(&key) {
            return;
        }
        let mut ids = Ids::default();
        let root = ids.reserve(&key);
        let published = Arc::new(Mutex::new(Published {
            tree: tree::placeholder(root, title),
            actions: HashMap::new(),
        }));
        let active = Arc::new(AtomicBool::new(false));
        let handler = || Handler {
            published: published.clone(),
            active: active.clone(),
            events: self.sender.clone(),
        };
        let adapter = accesskit_unix::Adapter::new(
            handler(),
            handler(),
            Deactivate {
                active: active.clone(),
                events: self.sender.clone(),
            },
        );
        self.windows.insert(
            key,
            Window {
                window,
                adapter,
                published,
                active,
                ids,
                focused: false,
                focus: None,
            },
        );
    }

    /// Withdraw a window's objects from the accessibility bus.
    pub fn close(&mut self, key: &str) {
        self.windows.remove(key);
    }

    pub fn is_open(&self, key: &str) -> bool {
        self.windows.contains_key(key)
    }

    /// Whether any window is being read; nothing is collected otherwise.
    pub fn is_active(&self) -> bool {
        self.windows
            .values()
            .any(|window| window.active.load(Ordering::SeqCst))
    }

    /// The logical-to-physical factor applied to every window's coordinates.
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    /// The key of the focused node in `window`, as last published.
    pub fn focused(&self, window: &str) -> Option<&str> {
        self.windows.get(window)?.focus.as_deref()
    }

    pub fn subscription(&self) -> Subscription<M> {
        let events = Subscription::run_with(self.events.clone(), events::<M>);
        if self.is_active() {
            Subscription::batch([events, iced::event::listen_raw(changed::<M>)])
        } else {
            events
        }
    }

    /// Call after the application changes what it shows. Free while inactive.
    pub fn refresh(&mut self) -> Task<M> {
        if !self.is_active() {
            return Task::none();
        }
        if self.collecting {
            self.dirty = true;
            return Task::none();
        }
        self.collecting = true;
        iced::advanced::widget::operate(Collect::<M>::new())
            .map(|snapshot| M::accessibility(Event::Collected(snapshot)))
    }

    pub fn update(&mut self, event: Event<M>) -> Task<M> {
        match event {
            Event::Activated | Event::Changed => self.refresh(),
            Event::Deactivated => Task::none(),
            Event::Action(message) => Task::done(*message),
            Event::Focus(id) => {
                let focus = iced::advanced::widget::operate(
                    iced::advanced::widget::operation::focusable::focus(id),
                );
                // Collect after focus has moved.
                self.collecting = false;
                focus.chain(self.refresh())
            }
            Event::WindowFocus(id, focused) => {
                for window in self.windows.values_mut() {
                    if window.window.is_none_or(|window| window == id) {
                        window.focused = focused;
                        window.adapter.update_window_focus_state(focused);
                    }
                }
                self.refresh()
            }
            Event::Collected(snapshot) => {
                self.collecting = false;
                self.publish(&snapshot);
                if std::mem::take(&mut self.dirty) {
                    self.refresh()
                } else {
                    Task::none()
                }
            }
        }
    }

    /// Mark a window focused without a compositor event (the first frame).
    pub fn set_window_focused(&mut self, key: &str, focused: bool) {
        if let Some(window) = self.windows.get_mut(key) {
            window.focused = focused;
            window.adapter.update_window_focus_state(focused);
        }
    }

    pub(crate) fn publish(&mut self, snapshot: &Snapshot<M>) {
        let scale = self.scale;
        for (key, window) in &mut self.windows {
            if !window.active.load(Ordering::SeqCst) {
                continue;
            }
            let Some(built) = tree::build(snapshot, key, &mut window.ids, window.focused, scale)
            else {
                continue;
            };
            window.focus = built.focus;
            let update = built.update;
            *window.published.lock().unwrap() = Published {
                tree: update.clone(),
                actions: built.actions,
            };
            // Our mutex is released: the adapter may call back on its own thread.
            window.adapter.update_if_active(|| update);
        }
    }

    /// Wait for the next event, for tests that drive a bridge directly.
    #[cfg(feature = "accessibility-testing")]
    pub async fn next_event(&self) -> Option<Event<M>> {
        self.events.receiver.lock().await.next().await
    }

    /// Publish a snapshot collected outside the application loop (tests).
    #[cfg(feature = "accessibility-testing")]
    pub fn publish_snapshot(&mut self, snapshot: &Snapshot<M>) {
        self.publish(snapshot);
    }
}
