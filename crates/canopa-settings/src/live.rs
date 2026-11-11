//! The live service page pattern: state comes from a system service and each
//! change applies at once.
//!
//! A service client (for example from the shared services crate) implements
//! [`LiveService`] through a small adapter that picks the state the page shows
//! and the changes it can ask for. [`Live`] is the page's state machine. It
//! knows nothing about the toolkit: it turns [`Event`]s into [`Effect`]s that
//! the window runs against the service.
//!
//! - Until the first reply the page is [`Status::Loading`].
//! - A missing, stopped or vanished service makes it
//!   [`Status::Unavailable`]; "Try again" ([`Event::Reload`]) reconnects.
//! - Each change belongs to a control ([`LiveAction::control`]). While it is
//!   applied the control shows progress; at most one change per control is in
//!   flight and the latest further change is queued, so a dragged slider sends
//!   its final value. A refused change leaves an error on that control with
//!   Retry and Dismiss; other controls keep working.
use futures_util::{future::BoxFuture, stream::BoxStream};
use std::{collections::BTreeMap, fmt};

/// Why a service call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceError {
    /// The service is not installed or not running, or went away. The whole
    /// page shows that the service is not available.
    Unavailable(String),
    /// The service refused or failed this request. Shown on the control.
    Failed(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ServiceError {}

/// A change a live page asks its service for.
pub trait LiveAction: Clone + fmt::Debug + Send + 'static {
    /// A stable name for the control this change belongs to, such as
    /// `"wifi-enabled"` or `"device-<address>"`.
    fn control(&self) -> String;
}

/// What a live page needs from a system service client.
pub trait LiveService: Send + Sync + 'static {
    /// The snapshot the page shows.
    type State: Clone + fmt::Debug + Send + 'static;
    type Action: LiveAction;

    /// A user-facing service name, such as "NetworkManager", in the
    /// selected language when it is not a program's own name.
    fn name(&self) -> String;
    /// Read the current state.
    fn snapshot(&self) -> BoxFuture<'static, Result<Self::State, ServiceError>>;
    /// New states as the service reports changes. The stream ends, or yields
    /// [`ServiceError::Unavailable`], when the service goes away.
    fn updates(&self) -> BoxStream<'static, Result<Self::State, ServiceError>>;
    /// Apply one change. `Some` returns the resulting state; `None` waits for
    /// [`Self::updates`] to report it.
    fn apply(
        &self,
        action: Self::Action,
    ) -> BoxFuture<'static, Result<Option<Self::State>, ServiceError>>;
}

/// What the page shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Status<T> {
    Loading,
    Unavailable(String),
    Ready(T),
}

/// One control's progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlStatus<'a> {
    Idle,
    Applying,
    Failed(&'a str),
}

#[derive(Debug)]
struct Control<A> {
    applying: Option<A>,
    queued: Option<A>,
    failed: Option<(A, String)>,
}

impl<A> Default for Control<A> {
    fn default() -> Self {
        Self {
            applying: None,
            queued: None,
            failed: None,
        }
    }
}

/// Input to [`Live::update`].
pub enum Event<S: LiveService> {
    /// A snapshot reply.
    Loaded(Result<S::State, ServiceError>),
    /// An update from the service.
    Changed(Result<S::State, ServiceError>),
    /// The user asks for a change.
    Apply(S::Action),
    /// The service's reply to a change.
    Applied(S::Action, Result<Option<S::State>, ServiceError>),
    /// Send a control's failed change again.
    Retry(String),
    /// Hide a control's error.
    Dismiss(String),
    /// Reconnect: read the state again.
    Reload,
}

impl<S: LiveService> Clone for Event<S> {
    fn clone(&self) -> Self {
        match self {
            Self::Loaded(result) => Self::Loaded(result.clone()),
            Self::Changed(result) => Self::Changed(result.clone()),
            Self::Apply(action) => Self::Apply(action.clone()),
            Self::Applied(action, result) => Self::Applied(action.clone(), result.clone()),
            Self::Retry(control) => Self::Retry(control.clone()),
            Self::Dismiss(control) => Self::Dismiss(control.clone()),
            Self::Reload => Self::Reload,
        }
    }
}

impl<S: LiveService> fmt::Debug for Event<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loaded(result) => formatter.debug_tuple("Loaded").field(result).finish(),
            Self::Changed(result) => formatter.debug_tuple("Changed").field(result).finish(),
            Self::Apply(action) => formatter.debug_tuple("Apply").field(action).finish(),
            Self::Applied(action, result) => formatter
                .debug_tuple("Applied")
                .field(action)
                .field(result)
                .finish(),
            Self::Retry(control) => formatter.debug_tuple("Retry").field(control).finish(),
            Self::Dismiss(control) => formatter.debug_tuple("Dismiss").field(control).finish(),
            Self::Reload => formatter.write_str("Reload"),
        }
    }
}

/// Work for the window to run against the service.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect<A> {
    /// Call [`LiveService::snapshot`] and report [`Event::Loaded`].
    Load,
    /// Call [`LiveService::apply`] and report [`Event::Applied`].
    Apply(A),
}

/// A live page's state machine.
pub struct Live<S: LiveService> {
    status: Status<S::State>,
    controls: BTreeMap<String, Control<S::Action>>,
}

impl<S: LiveService> Default for Live<S> {
    fn default() -> Self {
        Self {
            status: Status::Loading,
            controls: BTreeMap::new(),
        }
    }
}

impl<S: LiveService> Live<S> {
    pub fn status(&self) -> &Status<S::State> {
        &self.status
    }

    /// The service's state, once it is known and available.
    pub fn state(&self) -> Option<&S::State> {
        match &self.status {
            Status::Ready(state) => Some(state),
            _ => None,
        }
    }

    /// The change being applied or queued on a control, newest first, so a
    /// view can show the value the user chose instead of the old one.
    pub fn pending(&self, control: &str) -> Option<&S::Action> {
        let control = self.controls.get(control)?;
        control.queued.as_ref().or(control.applying.as_ref())
    }

    pub fn control(&self, control: &str) -> ControlStatus<'_> {
        match self.controls.get(control) {
            Some(Control {
                applying: Some(_), ..
            }) => ControlStatus::Applying,
            Some(Control {
                failed: Some((_, error)),
                ..
            }) => ControlStatus::Failed(error),
            _ => ControlStatus::Idle,
        }
    }

    pub fn update(&mut self, event: Event<S>) -> Option<Effect<S::Action>> {
        match event {
            Event::Loaded(result) | Event::Changed(result) => {
                self.status = match result {
                    Ok(state) => Status::Ready(state),
                    Err(error) => Status::Unavailable(error.to_string()),
                };
                None
            }
            Event::Reload => {
                self.status = Status::Loading;
                self.controls.clear();
                Some(Effect::Load)
            }
            Event::Apply(action) => {
                // Changes wait for an available service.
                self.state()?;
                let control = self.controls.entry(action.control()).or_default();
                control.failed = None;
                if control.applying.is_some() {
                    control.queued = Some(action);
                    None
                } else {
                    control.applying = Some(action.clone());
                    Some(Effect::Apply(action))
                }
            }
            Event::Applied(action, result) => {
                let name = action.control();
                let control = self.controls.entry(name.clone()).or_default();
                control.applying = None;
                match result {
                    Ok(state) => {
                        if let Some(state) = state {
                            self.status = Status::Ready(state);
                        }
                    }
                    Err(ServiceError::Failed(error)) => {
                        // A queued change supersedes the refused one.
                        if control.queued.is_none() {
                            control.failed = Some((action, error));
                        }
                    }
                    Err(ServiceError::Unavailable(error)) => {
                        self.status = Status::Unavailable(error);
                        self.controls.clear();
                        return None;
                    }
                }
                let next = control.queued.take();
                if let Some(next) = &next {
                    control.applying = Some(next.clone());
                }
                if control.applying.is_none() && control.failed.is_none() {
                    self.controls.remove(&name);
                }
                next.map(Effect::Apply)
            }
            Event::Retry(name) => {
                // Changes wait for an available service.
                self.state()?;
                let control = self.controls.get_mut(&name)?;
                if control.applying.is_some() {
                    return None;
                }
                let (action, _) = control.failed.take()?;
                control.applying = Some(action.clone());
                Some(Effect::Apply(action))
            }
            Event::Dismiss(name) => {
                if let Some(control) = self.controls.get_mut(&name) {
                    control.failed = None;
                    if control.applying.is_none() {
                        self.controls.remove(&name);
                    }
                }
                None
            }
        }
    }
}

/// A simulated service for tests and the debug-build example page.
#[cfg(debug_assertions)]
pub mod fake {
    use super::*;
    use futures_util::{FutureExt, StreamExt};
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::sync::watch;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct State {
        pub enabled: bool,
        pub level: u8,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Action {
        Enabled(bool),
        Level(u8),
    }

    impl LiveAction for Action {
        fn control(&self) -> String {
            match self {
                Self::Enabled(_) => "enabled",
                Self::Level(_) => "level",
            }
            .into()
        }
    }

    #[derive(Debug)]
    struct Inner {
        running: bool,
        state: State,
        refuse: u32,
        delay: Duration,
    }

    /// A service whose availability, failures and latency the caller controls.
    #[derive(Clone, Debug)]
    pub struct Service {
        inner: Arc<Mutex<Inner>>,
        changes: watch::Sender<()>,
    }

    impl Default for Service {
        fn default() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Inner {
                    running: true,
                    state: State {
                        enabled: true,
                        level: 50,
                    },
                    refuse: 0,
                    delay: Duration::ZERO,
                })),
                changes: watch::channel(()).0,
            }
        }
    }

    const STOPPED: &str = "The example service is not running.";

    impl Service {
        fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
            self.inner.lock().unwrap_or_else(|error| error.into_inner())
        }

        /// Start or stop the simulated service; subscribers see the change.
        pub fn set_running(&self, running: bool) {
            self.inner().running = running;
            self.changes.send_replace(());
        }

        pub fn running(&self) -> bool {
            self.inner().running
        }

        /// Refuse the next `count` changes.
        pub fn refuse(&self, count: u32) {
            self.inner().refuse = count;
        }

        /// Delay every reply.
        pub fn set_delay(&self, delay: Duration) {
            self.inner().delay = delay;
        }

        pub fn delay(&self) -> Duration {
            self.inner().delay
        }

        /// Change the state from "outside", as another client would.
        pub fn external(&self, state: State) {
            self.inner().state = state;
            self.changes.send_replace(());
        }

        fn current(&self) -> Result<State, ServiceError> {
            let inner = self.inner();
            if inner.running {
                Ok(inner.state.clone())
            } else {
                Err(ServiceError::Unavailable(STOPPED.into()))
            }
        }
    }

    impl LiveService for Service {
        type State = State;
        type Action = Action;

        fn name(&self) -> String {
            "Example service".into()
        }

        fn snapshot(&self) -> BoxFuture<'static, Result<State, ServiceError>> {
            let service = self.clone();
            async move {
                tokio::time::sleep(service.delay()).await;
                service.current()
            }
            .boxed()
        }

        fn updates(&self) -> BoxStream<'static, Result<State, ServiceError>> {
            let service = self.clone();
            let mut changes = self.changes.subscribe();
            changes.mark_unchanged();
            futures_util::stream::unfold(Some(changes), move |changes| {
                let service = service.clone();
                async move {
                    let mut changes = changes?;
                    changes.changed().await.ok()?;
                    let state = service.current();
                    // A stopped service ends the stream after saying so.
                    let next = state.is_ok().then_some(changes);
                    Some((state, next))
                }
            })
            .boxed()
        }

        fn apply(&self, action: Action) -> BoxFuture<'static, Result<Option<State>, ServiceError>> {
            let service = self.clone();
            async move {
                tokio::time::sleep(service.delay()).await;
                let state = {
                    let mut inner = service.inner();
                    if !inner.running {
                        return Err(ServiceError::Unavailable(STOPPED.into()));
                    }
                    if inner.refuse > 0 {
                        inner.refuse -= 1;
                        return Err(ServiceError::Failed(
                            "The example service refused the change.".into(),
                        ));
                    }
                    match action {
                        Action::Enabled(enabled) => inner.state.enabled = enabled,
                        Action::Level(level) => inner.state.level = level.min(100),
                    }
                    inner.state.clone()
                };
                service.changes.send_replace(());
                Ok(Some(state))
            }
            .boxed()
        }
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::{fake::*, *};
    use futures_util::StreamExt;

    /// Run effects against the service until the page settles.
    async fn settle(
        live: &mut Live<Service>,
        service: &Service,
        mut effect: Option<Effect<Action>>,
    ) {
        while let Some(next) = effect.take() {
            effect = match next {
                Effect::Load => live.update(Event::Loaded(service.snapshot().await)),
                Effect::Apply(action) => {
                    let result = service.apply(action.clone()).await;
                    live.update(Event::Applied(action, result))
                }
            };
        }
    }

    #[tokio::test]
    async fn pages_load_show_unavailability_and_reconnect() {
        let service = Service::default();
        let mut live = Live::<Service>::default();
        assert_eq!(live.status(), &Status::Loading);
        settle(&mut live, &service, Some(Effect::Load)).await;
        assert_eq!(live.state().unwrap().level, 50);

        service.set_running(false);
        let effect = live.update(Event::Reload);
        assert_eq!(live.status(), &Status::Loading);
        settle(&mut live, &service, effect).await;
        assert!(matches!(live.status(), Status::Unavailable(m) if m.contains("not running")));
        // Changes are ignored until the service is back.
        assert_eq!(live.update(Event::Apply(Action::Level(10))), None);

        service.set_running(true);
        let effect = live.update(Event::Reload);
        settle(&mut live, &service, effect).await;
        assert_eq!(live.state().unwrap().level, 50);
    }

    #[tokio::test]
    async fn refused_changes_stay_on_their_control_until_retried() {
        let service = Service::default();
        let mut live = Live::<Service>::default();
        settle(&mut live, &service, Some(Effect::Load)).await;
        service.refuse(1);
        let effect = live.update(Event::Apply(Action::Enabled(false)));
        assert_eq!(live.control("enabled"), ControlStatus::Applying);
        settle(&mut live, &service, effect).await;
        assert_eq!(
            live.control("enabled"),
            ControlStatus::Failed("The example service refused the change.")
        );
        assert!(live.state().unwrap().enabled);
        // Another control is unaffected.
        let effect = live.update(Event::Apply(Action::Level(80)));
        settle(&mut live, &service, effect).await;
        assert_eq!(live.state().unwrap().level, 80);
        assert_eq!(live.control("level"), ControlStatus::Idle);
        assert!(matches!(live.control("enabled"), ControlStatus::Failed(_)));

        let effect = live.update(Event::Retry("enabled".into()));
        assert_eq!(effect, Some(Effect::Apply(Action::Enabled(false))));
        settle(&mut live, &service, effect).await;
        assert!(!live.state().unwrap().enabled);
        assert_eq!(live.control("enabled"), ControlStatus::Idle);

        service.refuse(1);
        let effect = live.update(Event::Apply(Action::Level(5)));
        settle(&mut live, &service, effect).await;
        live.update(Event::Dismiss("level".into()));
        assert_eq!(live.control("level"), ControlStatus::Idle);
        assert_eq!(live.update(Event::Retry("level".into())), None);
    }

    #[tokio::test]
    async fn one_change_per_control_is_in_flight_and_the_latest_is_queued() {
        let service = Service::default();
        let mut live = Live::<Service>::default();
        settle(&mut live, &service, Some(Effect::Load)).await;
        let first = live.update(Event::Apply(Action::Level(10)));
        assert_eq!(first, Some(Effect::Apply(Action::Level(10))));
        assert_eq!(live.update(Event::Apply(Action::Level(20))), None);
        assert_eq!(live.update(Event::Apply(Action::Level(30))), None);
        assert_eq!(live.pending("level"), Some(&Action::Level(30)));
        settle(&mut live, &service, first).await;
        assert_eq!(live.state().unwrap().level, 30);
        assert_eq!(live.control("level"), ControlStatus::Idle);

        // The service vanishing during a change makes the page unavailable.
        let effect = live.update(Event::Apply(Action::Level(40)));
        service.set_running(false);
        settle(&mut live, &service, effect).await;
        assert!(matches!(live.status(), Status::Unavailable(_)));
        assert_eq!(live.control("level"), ControlStatus::Idle);
    }

    #[tokio::test]
    async fn updates_report_outside_changes_and_end_when_the_service_stops() {
        let service = Service::default();
        let mut updates = service.updates();
        let mut live = Live::<Service>::default();
        settle(&mut live, &service, Some(Effect::Load)).await;
        service.external(State {
            enabled: false,
            level: 7,
        });
        let update = updates.next().await.unwrap();
        live.update(Event::Changed(update));
        assert_eq!(
            live.state(),
            Some(&State {
                enabled: false,
                level: 7
            })
        );
        service.set_running(false);
        let update = updates.next().await.unwrap();
        assert!(matches!(update, Err(ServiceError::Unavailable(_))));
        live.update(Event::Changed(update));
        assert!(matches!(live.status(), Status::Unavailable(_)));
        assert!(updates.next().await.is_none());
    }
}
