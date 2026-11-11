//! The Notifications page's service: Canopa Notify's preferences control API
//! (`docs/notify/settings-control-api.md`) on its per-user control socket.
//!
//! Notify owns Do Not Disturb, its schedule and suppression, and the
//! per-application rules; the page reads them and asks for changes. Notify's
//! summary stream reports Do Not Disturb changes made elsewhere (the Shell, the
//! command line); the page also re-reads every few seconds for rule changes.
//! A Notify that is not running is an ordinary state.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_notify_client::{
    SummarySubscription,
    preferences::{
        self, AppPreferences, Command, Delivery, DoNotDisturbPreferences, DoNotDisturbRequest,
        NotificationPreferences, RequestError,
    },
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{path::PathBuf, time::Duration};

fn not_running() -> String {
    fl!("settings-notifications-not-running")
}
fn too_old() -> String {
    fl!("settings-notifications-too-old")
}
/// How often the page re-reads rules nobody announces.
const REREAD: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationAction {
    DoNotDisturb(DoNotDisturbRequest),
    /// The schedule, break-through and suppression as a whole; `None` returns
    /// to `config.kdl`.
    Preferences(Option<DoNotDisturbPreferences>),
    /// One application's rule and overrides, replacing the earlier ones.
    App {
        identity: String,
        delivery: Option<Delivery>,
        preferences: AppPreferences,
    },
}

impl LiveAction for NotificationAction {
    fn control(&self) -> String {
        match self {
            Self::DoNotDisturb(_) => "dnd".into(),
            Self::Preferences(_) => "dnd-preferences".into(),
            Self::App { identity, .. } => format!("app-{identity}"),
        }
    }
}

/// The App Identity Notify uses for a desktop entry id.
pub fn app_identity(desktop_id: &str) -> String {
    desktop_id
        .strip_suffix(".desktop")
        .unwrap_or(desktop_id)
        .to_owned()
}

fn failure(error: RequestError) -> ServiceError {
    match error {
        RequestError::Unavailable => ServiceError::Unavailable(not_running()),
        RequestError::Refused(reason) => {
            ServiceError::Failed(fl!("settings-notifications-refused", reason = reason))
        }
        RequestError::Invalid => ServiceError::Failed(fl!("settings-notifications-bad-reply")),
    }
}

/// Canopa Notify for the page.
#[derive(Clone, Debug, Default)]
pub struct NotificationService {
    /// The control socket; `None` is the session default.
    socket: Option<PathBuf>,
}

impl NotificationService {
    /// A service on a specific control socket (tests).
    pub fn with_socket(socket: PathBuf) -> Self {
        Self {
            socket: Some(socket),
        }
    }

    fn socket(&self) -> Result<PathBuf, ServiceError> {
        match &self.socket {
            Some(socket) => Ok(socket.clone()),
            None => canopa_notify_client::default_socket()
                .map_err(|_| ServiceError::Unavailable(not_running())),
        }
    }

    async fn read(&self) -> Result<NotificationPreferences, ServiceError> {
        preferences::read(&self.socket()?)
            .await
            .map_err(|error| match error {
                // An older Notify answers, without the preferences control.
                RequestError::Refused(reason) if reason.contains("unsupported") => {
                    ServiceError::Unavailable(too_old())
                }
                other => failure(other),
            })
    }

    async fn run(&self, action: NotificationAction) -> Result<(), ServiceError> {
        let command = match action {
            NotificationAction::DoNotDisturb(request) => Command::RequestDoNotDisturb { request },
            NotificationAction::Preferences(preferences) => {
                if let Some(schedule) = preferences.as_ref().and_then(|p| p.schedule.as_ref())
                    && schedule.days.as_ref().is_some_and(Vec::is_empty)
                {
                    return Err(ServiceError::Failed(fl!("settings-dnd-no-days")));
                }
                Command::SetDoNotDisturbPreferences { preferences }
            }
            NotificationAction::App {
                identity,
                delivery,
                preferences,
            } => Command::SetAppPreferences {
                app_identity: identity,
                delivery,
                preferences,
            },
        };
        preferences::request(&self.socket()?, &command)
            .await
            .map(drop)
            .map_err(failure)
    }
}

enum Phase {
    Connect,
    /// Read the preferences, then wait.
    Read(Box<SummarySubscription>),
    /// Wait for a summary change or the next periodic read.
    Wait(Box<SummarySubscription>),
}

impl LiveService for NotificationService {
    type State = NotificationPreferences;
    type Action = NotificationAction;

    fn name(&self) -> String {
        "Canopa Notify".into()
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<NotificationPreferences, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<NotificationPreferences, ServiceError>> {
        let service = self.clone();
        let gone = || Err(ServiceError::Unavailable(not_running()));
        futures_util::stream::unfold(
            Some((Phase::Connect, None::<NotificationPreferences>)),
            move |current| {
                let service = service.clone();
                async move {
                    // After reporting an outage the stream ends; the page reconnects.
                    let (mut phase, mut last) = current?;
                    loop {
                        phase = match phase {
                            Phase::Connect => {
                                let socket = match service.socket() {
                                    Ok(socket) => socket,
                                    Err(error) => return Some((Err(error), None)),
                                };
                                match SummarySubscription::connect(&socket).await {
                                    Ok((subscription, _)) => Phase::Read(Box::new(subscription)),
                                    Err(_) => return Some((gone(), None)),
                                }
                            }
                            Phase::Read(subscription) => match service.read().await {
                                Ok(current) if last.as_ref() == Some(&current) => {
                                    Phase::Wait(subscription)
                                }
                                Ok(current) => {
                                    last = Some(current.clone());
                                    let next = Some((Phase::Wait(subscription), last));
                                    return Some((Ok(current), next));
                                }
                                Err(error) => return Some((Err(error), None)),
                            },
                            Phase::Wait(mut subscription) => {
                                match tokio::time::timeout(REREAD, subscription.next()).await {
                                    Ok(Ok(None) | Err(_)) => return Some((gone(), None)),
                                    _ => Phase::Read(subscription),
                                }
                            }
                        };
                    }
                }
            },
        )
        .boxed()
    }

    fn apply(
        &self,
        action: NotificationAction,
    ) -> BoxFuture<'static, Result<Option<NotificationPreferences>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(service.read().await.ok())
        }
        .boxed()
    }
}

/// `HH:MM` local time and whether it is today, for Unix milliseconds.
pub fn local_time(millis: i64) -> Option<(String, bool)> {
    let at = libc::time_t::try_from(millis.div_euclid(1000)).ok()?;
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let local = |seconds: libc::time_t| {
        // SAFETY: localtime_r writes only the zeroed struct it is given.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let converted = unsafe { libc::localtime_r(&seconds, &mut tm) };
        (!converted.is_null()).then_some(tm)
    };
    unsafe extern "C" {
        fn tzset();
    }
    // SAFETY: tzset only reloads the process's time zone, which may have
    // changed since the window started.
    unsafe { tzset() };
    let (then, today) = (local(at)?, local(now)?);
    let same_day = then.tm_year == today.tm_year && then.tm_yday == today.tm_yday;
    Some((format!("{:02}:{:02}", then.tm_hour, then.tm_min), same_day))
}

#[cfg(test)]
pub(crate) mod fake {
    //! A Canopa Notify control socket with an in-memory preferences store.
    use canopa_notify_client::preferences::{
        AppEntry, Command, DoNotDisturbRequest, NotificationPreferences,
    };
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    pub struct FakeNotify {
        pub socket: PathBuf,
        #[allow(dead_code)]
        pub state: Arc<Mutex<NotificationPreferences>>,
        pub commands: Arc<Mutex<Vec<Command>>>,
        _dir: tempfile::TempDir,
        task: tokio::task::JoinHandle<()>,
    }

    impl FakeNotify {
        pub fn start() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("control.sock");
            let listener = tokio::net::UnixListener::bind(&socket).unwrap();
            let state = Arc::new(Mutex::new(NotificationPreferences::default()));
            let commands = Arc::new(Mutex::new(Vec::new()));
            let (store, log) = (state.clone(), commands.clone());
            let (summaries, _) = tokio::sync::broadcast::channel::<()>(8);
            let task = tokio::spawn(async move {
                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        return;
                    };
                    let (store, log) = (store.clone(), log.clone());
                    let (sender, mut changes) = (summaries.clone(), summaries.subscribe());
                    tokio::spawn(async move {
                        let mut reader = BufReader::new(stream);
                        let mut line = String::new();
                        reader.read_line(&mut line).await.unwrap();
                        let summary = |active: bool| {
                            format!(
                                "{{\"instance\":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],\"requires_attention\":0,\"do_not_disturb\":{active}}}\n"
                            )
                        };
                        if line == canopa_notify_client::SUMMARY_REQUEST {
                            // Only the listener keeps the sender, so stopping it ends this.
                            drop(sender);
                            loop {
                                let active = store.lock().unwrap().do_not_disturb.active;
                                if reader
                                    .get_mut()
                                    .write_all(summary(active).as_bytes())
                                    .await
                                    .is_err()
                                    || changes.recv().await.is_err()
                                {
                                    return;
                                }
                            }
                        }
                        let json = line.strip_prefix("control ").unwrap();
                        let command: Command = serde_json::from_str(json).unwrap();
                        log.lock().unwrap().push(command.clone());
                        let reply = {
                            let mut state = store.lock().unwrap();
                            match command {
                                Command::NotificationPreferences => format!(
                                    "{{\"reply\":\"notification-preferences\",\"value\":{}}}\n",
                                    serde_json::to_string(&*state).unwrap()
                                ),
                                Command::RequestDoNotDisturb { request } => {
                                    state.do_not_disturb.active =
                                        !matches!(request, DoNotDisturbRequest::Off);
                                    let _ = sender.send(());
                                    format!(
                                        "{{\"reply\":\"do-not-disturb-status\",\"value\":{}}}\n",
                                        serde_json::to_string(&state.do_not_disturb).unwrap()
                                    )
                                }
                                Command::SetDoNotDisturbPreferences { preferences } => {
                                    if preferences
                                        .as_ref()
                                        .and_then(|p| p.schedule.as_ref())
                                        .is_some_and(|s| s.start == s.end)
                                    {
                                        "error schedule needs a duration\n".into()
                                    } else {
                                        state.do_not_disturb_preferences =
                                            preferences.unwrap_or_default();
                                        "{\"reply\":\"command-applied\"}\n".into()
                                    }
                                }
                                Command::SetAppPreferences {
                                    app_identity,
                                    delivery,
                                    preferences,
                                } => {
                                    state.apps.retain(|app| app.app_identity != app_identity);
                                    if delivery.is_some() || preferences != Default::default() {
                                        state.apps.push(AppEntry {
                                            app_identity,
                                            delivery,
                                            configured: Default::default(),
                                            user: preferences,
                                            sound: preferences.sound.unwrap_or(true),
                                            lock_screen: preferences
                                                .lock_screen
                                                .unwrap_or_default(),
                                        });
                                    }
                                    "{\"reply\":\"command-applied\"}\n".into()
                                }
                            }
                        };
                        let _ = reader.get_mut().write_all(reply.as_bytes()).await;
                    });
                }
            });
            Self {
                socket,
                state,
                commands,
                _dir: dir,
                task,
            }
        }

        pub fn stop(&self) {
            self.task.abort();
            let _ = std::fs::remove_file(&self.socket);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fake::FakeNotify, *};
    use canopa_notify_client::preferences::{LockScreen, Schedule};

    async fn next(
        updates: &mut BoxStream<'static, Result<NotificationPreferences, ServiceError>>,
    ) -> Result<NotificationPreferences, ServiceError> {
        tokio::time::timeout(Duration::from_secs(5), updates.next())
            .await
            .expect("an update")
            .expect("the stream continues")
    }

    #[tokio::test]
    async fn the_page_reads_changes_and_follows_notify() {
        let notify = FakeNotify::start();
        let service = NotificationService::with_socket(notify.socket.clone());
        assert!(!service.snapshot().await.unwrap().do_not_disturb.active);
        let mut updates = service.updates();

        let state = service
            .apply(NotificationAction::DoNotDisturb(DoNotDisturbRequest::For {
                minutes: 60,
            }))
            .await
            .unwrap()
            .unwrap();
        assert!(state.do_not_disturb.active);
        // The summary stream announces the change; the page re-reads.
        assert!(next(&mut updates).await.unwrap().do_not_disturb.active);

        let schedule = DoNotDisturbPreferences {
            schedule: Some(Schedule {
                start: "22:00".into(),
                end: "07:00".into(),
                days: Some(vec!["mon".into(), "fri".into()]),
            }),
            ..Default::default()
        };
        let state = service
            .apply(NotificationAction::Preferences(Some(schedule.clone())))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.do_not_disturb_preferences, schedule);
        // Notify's refusals stay on the control.
        let mut empty = schedule.clone();
        empty.schedule.as_mut().unwrap().end = "22:00".into();
        assert!(matches!(
            service.apply(NotificationAction::Preferences(Some(empty))).await,
            Err(ServiceError::Failed(message)) if message.contains("needs a duration")
        ));
        let mut no_days = schedule.clone();
        no_days.schedule.as_mut().unwrap().days = Some(vec![]);
        assert!(matches!(
            service
                .apply(NotificationAction::Preferences(Some(no_days)))
                .await,
            Err(ServiceError::Failed(_))
        ));

        let state = service
            .apply(NotificationAction::App {
                identity: app_identity("org.gnome.Fractal.desktop"),
                delivery: Some(Delivery::Silence),
                preferences: AppPreferences {
                    sound: Some(false),
                    lock_screen: Some(LockScreen::Hidden),
                },
            })
            .await
            .unwrap()
            .unwrap();
        let app = state.app("org.gnome.Fractal").unwrap();
        assert_eq!(app.delivery, Some(Delivery::Silence));
        assert!(!app.sound);
        assert!(notify.commands.lock().unwrap().iter().any(|command| matches!(
            command,
            Command::SetAppPreferences { app_identity, .. } if app_identity == "org.gnome.Fractal"
        )));

        notify.stop();
        assert!(matches!(
            next(&mut updates).await,
            Err(ServiceError::Unavailable(message)) if message == not_running()
        ));
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn an_older_notify_is_a_calm_unavailable_state() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("control.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = tokio::io::BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            reader
                .get_mut()
                .write_all(b"error unsupported control request\n")
                .await
                .unwrap();
        });
        let service = NotificationService::with_socket(socket);
        assert!(matches!(
            service.snapshot().await,
            Err(ServiceError::Unavailable(message)) if message == too_old()
        ));
    }

    #[test]
    fn identities_and_times() {
        assert_eq!(app_identity("firefox.desktop"), "firefox");
        assert_eq!(app_identity("Firefox"), "Firefox");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let (clock, _) = local_time(now).unwrap();
        assert_eq!(clock.len(), 5);
        assert!(!local_time(now + 3 * 86_400_000).unwrap().1);
    }
}
