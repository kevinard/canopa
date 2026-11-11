//! The Shell's own messages to the user, shown through the session notification
//! server (Canopa Notify or any freedesktop implementation).
//!
//! One background service owns the session connection, the notification IDs and
//! the action signals. Each [`NoticeSlot`] shows at most one notification: a new
//! notice replaces the slot's previous one and clearing a slot closes it. When no
//! server is running yet, the notice waits for one to appear on the bus.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const APP_NAME: &str = "Canopa Shell";
const DEADLINE: Duration = Duration::from_secs(3);
/// Bounded retries for transient failures. A server that appears later is
/// noticed through its bus name instead of polling.
const MAX_ATTEMPTS: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NoticeSlot {
    Configuration,
    Session,
}

/// What a notice button does. The App performs it as a session-owned launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAction {
    OpenFile(PathBuf),
    OpenSettings(Option<&'static str>),
}

impl NoticeAction {
    fn key(&self) -> &'static str {
        match self {
            Self::OpenFile(_) => "open-file",
            Self::OpenSettings(_) => "open-settings",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::OpenFile(_) => "Open file",
            Self::OpenSettings(_) => "Open Settings",
        }
    }

    pub(crate) fn argv(&self) -> Vec<String> {
        match self {
            Self::OpenFile(path) => vec!["xdg-open".into(), path.display().to_string()],
            Self::OpenSettings(None) => vec!["canopa-settings".into()],
            Self::OpenSettings(Some(page)) => {
                vec!["canopa-settings".into(), "--page".into(), (*page).into()]
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) summary: String,
    pub(crate) body: String,
    /// Critical, resident and without expiry: it stays until the Shell clears it.
    pub(crate) persistent: bool,
    /// The first action is also the notification's default (body click) action.
    pub(crate) actions: Vec<NoticeAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Show(NoticeSlot, Notice),
    Clear(NoticeSlot),
}

type Receiver = Arc<Mutex<Option<mpsc::UnboundedReceiver<Command>>>>;

/// The App's handle. Commands queue until the service subscription runs.
pub(crate) struct Outbox {
    sender: mpsc::UnboundedSender<Command>,
    receiver: Receiver,
    #[cfg(test)]
    pub(crate) sent: Vec<Command>,
}

impl Default for Outbox {
    fn default() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(Some(receiver))),
            #[cfg(test)]
            sent: Vec::new(),
        }
    }
}

impl Outbox {
    pub(crate) fn show(&mut self, slot: NoticeSlot, notice: Notice) {
        self.send(Command::Show(slot, notice));
    }

    pub(crate) fn clear(&mut self, slot: NoticeSlot) {
        self.send(Command::Clear(slot));
    }

    fn send(&mut self, command: Command) {
        #[cfg(test)]
        self.sent.push(command.clone());
        let _ = self.sender.send(command);
    }

    pub(crate) fn subscription(&self) -> iced::Subscription<NoticeAction> {
        iced::Subscription::run_with(ServiceData(self.receiver.clone()), service_stream)
    }
}

struct ServiceData(Receiver);

impl std::hash::Hash for ServiceData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        "canopa-shell-notices".hash(state);
    }
}

fn service_stream(
    data: &ServiceData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = NoticeAction> + Send>> {
    let receiver = data.0.clone();
    Box::pin(iced::stream::channel(
        4,
        async move |output: iced::futures::channel::mpsc::Sender<NoticeAction>| {
            let commands = receiver.lock().ok().and_then(|mut slot| slot.take());
            match commands {
                Some(commands) => run(connect_session, commands, output).await,
                // The subscription is installed once per process; nothing to own.
                None => std::future::pending().await,
            }
        },
    ))
}

async fn connect_session() -> Option<zbus::Connection> {
    tokio::time::timeout(Duration::from_secs(2), zbus::Connection::session())
        .await
        .ok()
        .and_then(Result::ok)
}

struct Shown {
    id: u32,
    notice: Notice,
}

type Signals =
    std::pin::Pin<Box<dyn iced::futures::Stream<Item = zbus::Result<zbus::Message>> + Send>>;

struct Connected {
    connection: zbus::Connection,
    signals: Signals,
}

enum Signal {
    Action(u32, String),
    Closed(u32),
    ServerAppeared,
}

/// Owns notification state for the process lifetime. Ends when the App drops
/// its [`Outbox`] or the action receiver.
pub(crate) async fn run<C, F>(
    connect: C,
    mut commands: mpsc::UnboundedReceiver<Command>,
    mut actions: iced::futures::channel::mpsc::Sender<NoticeAction>,
) where
    C: Fn() -> F,
    F: std::future::Future<Output = Option<zbus::Connection>>,
{
    let mut wanted: HashMap<NoticeSlot, Notice> = HashMap::new();
    let mut shown: HashMap<NoticeSlot, Shown> = HashMap::new();
    let mut closing: Vec<u32> = Vec::new();
    let mut bus: Option<Connected> = None;
    let mut attempts = 0;
    loop {
        if !wanted.is_empty() || !closing.is_empty() {
            if bus.is_none() {
                bus = match connect().await {
                    Some(connection) => attach(connection).await,
                    None => None,
                };
            }
            // A cloned handle: the signal stream is not shared across awaits.
            if let Some(connection) = bus.as_ref().map(|bus| bus.connection.clone()) {
                match flush(&connection, &mut wanted, &mut shown, &mut closing).await {
                    Ok(()) => attempts = 0,
                    Err(error) => {
                        attempts += 1;
                        tracing::debug!(target: "canopa_shell::notice", %error, attempts, "notification server unavailable");
                    }
                }
            } else {
                attempts += 1;
            }
        }
        let retry = (!wanted.is_empty() || !closing.is_empty()) && attempts < MAX_ATTEMPTS;
        let delay = Duration::from_millis(500) * 2u32.pow(attempts.min(MAX_ATTEMPTS));
        tokio::select! {
            command = commands.recv() => match command {
                None => return,
                Some(Command::Show(slot, notice)) => {
                    if shown.get(&slot).is_none_or(|current| current.notice != notice) {
                        wanted.insert(slot, notice);
                        attempts = 0;
                    }
                }
                Some(Command::Clear(slot)) => {
                    wanted.remove(&slot);
                    if let Some(current) = shown.remove(&slot) {
                        closing.push(current.id);
                        attempts = 0;
                    }
                }
            },
            signal = next_signal(&mut bus) => match signal {
                Some(Signal::Action(id, key)) => {
                    let action = shown.values().find(|current| current.id == id).and_then(|current| {
                        let actions = &current.notice.actions;
                        if key == "default" {
                            actions.first()
                        } else {
                            actions.iter().find(|action| action.key() == key)
                        }
                    });
                    if let Some(action) = action.cloned()
                        && actions.send(action).await.is_err()
                    {
                        return;
                    }
                }
                // Dismissed or expired. A later notice for the slot starts afresh.
                Some(Signal::Closed(id)) => shown.retain(|_, current| current.id != id),
                // A restarted server has none of our notifications: show them again.
                Some(Signal::ServerAppeared) => {
                    for (slot, current) in shown.drain() {
                        wanted.entry(slot).or_insert(current.notice);
                    }
                    attempts = 0;
                }
                None => bus = None,
            },
            _ = tokio::time::sleep(delay), if retry => {}
        }
    }
}

async fn attach(connection: zbus::Connection) -> Option<Connected> {
    // Subscribe before showing anything so no action signal can be missed.
    let notifications = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(SERVICE)
        .ok()?
        .path(PATH)
        .ok()?
        .build();
    let owner = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.freedesktop.DBus")
        .ok()?
        .interface("org.freedesktop.DBus")
        .ok()?
        .member("NameOwnerChanged")
        .ok()?
        .arg(0, SERVICE)
        .ok()?
        .build();
    let (notifications, owner) = tokio::time::timeout(DEADLINE, async {
        tokio::join!(
            zbus::MessageStream::for_match_rule(notifications, &connection, Some(16)),
            zbus::MessageStream::for_match_rule(owner, &connection, Some(4)),
        )
    })
    .await
    .ok()?;
    let signals = Box::pin(iced::futures::stream::select(
        notifications.ok()?,
        owner.ok()?,
    ));
    Some(Connected {
        connection,
        signals,
    })
}

async fn next_signal(bus: &mut Option<Connected>) -> Option<Signal> {
    let Some(connected) = bus else {
        return std::future::pending().await;
    };
    loop {
        let message = connected.signals.next().await?.ok()?;
        let header = message.header();
        match header.member().map(|member| member.as_str()) {
            Some("ActionInvoked") => {
                if let Ok((id, key)) = message.body().deserialize::<(u32, String)>() {
                    return Some(Signal::Action(id, key));
                }
            }
            Some("NotificationClosed") => {
                if let Ok((id, _reason)) = message.body().deserialize::<(u32, u32)>() {
                    return Some(Signal::Closed(id));
                }
            }
            Some("NameOwnerChanged") => {
                if let Ok((_, _, owner)) = message.body().deserialize::<(String, String, String)>()
                    && !owner.is_empty()
                {
                    return Some(Signal::ServerAppeared);
                }
            }
            _ => {}
        }
    }
}

async fn flush(
    connection: &zbus::Connection,
    wanted: &mut HashMap<NoticeSlot, Notice>,
    shown: &mut HashMap<NoticeSlot, Shown>,
    closing: &mut Vec<u32>,
) -> zbus::Result<()> {
    while let Some(id) = closing.last().copied() {
        call::<_, ()>(connection, "CloseNotification", &(id,)).await?;
        closing.pop();
    }
    let markup = capabilities(connection)
        .await
        .is_ok_and(|capabilities| capabilities.iter().any(|name| name == "body-markup"));
    let slots: Vec<_> = wanted.keys().copied().collect();
    for slot in slots {
        let notice = wanted[&slot].clone();
        let replaces = shown.get(&slot).map_or(0, |current| current.id);
        let id = match notify(connection, replaces, &notice, markup).await {
            // Some servers refuse to replace a notification they already
            // forgot; showing a new one is always correct.
            Err(zbus::Error::MethodError(..)) if replaces != 0 => {
                notify(connection, 0, &notice, markup).await?
            }
            result => result?,
        };
        wanted.remove(&slot);
        shown.insert(slot, Shown { id, notice });
    }
    Ok(())
}

async fn capabilities(connection: &zbus::Connection) -> zbus::Result<Vec<String>> {
    call(connection, "GetCapabilities", &()).await
}

async fn notify(
    connection: &zbus::Connection,
    replaces: u32,
    notice: &Notice,
    markup: bool,
) -> zbus::Result<u32> {
    use zbus::zvariant::Value;
    let body = if markup {
        escape(&notice.body)
    } else {
        notice.body.clone()
    };
    let mut actions = Vec::new();
    if let Some(first) = notice.actions.first() {
        actions.extend(["default", first.label()]);
    }
    for action in &notice.actions {
        actions.extend([action.key(), action.label()]);
    }
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("urgency", Value::U8(if notice.persistent { 2 } else { 1 }));
    if notice.persistent {
        hints.insert("resident", Value::Bool(true));
    }
    let expire: i32 = if notice.persistent { 0 } else { -1 };
    call(
        connection,
        "Notify",
        &(
            APP_NAME,
            replaces,
            "dialog-warning",
            notice.summary.as_str(),
            body.as_str(),
            actions,
            hints,
            expire,
        ),
    )
    .await
}

/// Never D-Bus-activates a server: in a Canopa session that could start an
/// unrelated daemon (dunst, mako) ahead of Canopa Notify. A server that starts
/// later is observed through its bus name.
async fn call<B, R>(connection: &zbus::Connection, method: &str, body: &B) -> zbus::Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    tokio::time::timeout(DEADLINE, async {
        let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(SERVICE)?
            .path(PATH)?
            .interface(SERVICE)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        proxy
            .call_with_flags(method, zbus::proxy::MethodFlags::NoAutoStart.into(), body)
            .await?
            .ok_or_else(|| zbus::Error::Failure("notification server sent no reply".into()))
    })
    .await
    .map_err(|_| zbus::Error::Failure("notification server timed out".into()))?
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Bounded, single-paragraph-safe text for notices built from diagnostics.
pub(crate) fn sanitize(text: &str, limit: usize) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .take(limit)
        .collect();
    if text.chars().count() > limit {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests;
