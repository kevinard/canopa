//! Background application observation. The frontend retains permission and
//! process policy. Notify questions and Iced review own explicit user decisions.
mod notify;
mod observer;
mod process;
mod service;

pub use notify::NotifyPresenter;
pub use observer::Observer;
pub use service::{Background, BackgroundRequest, Decision, Presenter};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Activity {
    Background = 0,
    Running = 1,
    Active = 2,
}

pub type AppStates = BTreeMap<String, Activity>;
const MAX_WINDOWS: usize = 256;
const FRESHNESS: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Connecting,
    Updating,
    Disconnected,
    InvalidInput,
    Capacity,
    Identity,
    Stale,
    Shutdown,
    Worker,
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "background application observation unavailable ({self:?})"
        )
    }
}
impl std::error::Error for Unavailable {}

#[derive(Clone)]
pub struct Observation {
    result: Result<Arc<AppStates>, Unavailable>,
    at: Instant,
}

impl std::fmt::Debug for Observation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observation")
            .field("applications", &self.result.as_ref().map(|apps| apps.len()))
            .finish_non_exhaustive()
    }
}

impl Observation {
    pub fn available(apps: AppStates) -> Self {
        Self {
            result: Ok(Arc::new(apps)),
            at: Instant::now(),
        }
    }

    pub fn unavailable(reason: Unavailable) -> Self {
        Self {
            result: Err(reason),
            at: Instant::now(),
        }
    }

    pub fn apps(&self) -> Result<Arc<AppStates>, Unavailable> {
        if self.result.is_ok() && self.at.elapsed() > FRESHNESS {
            return Err(Unavailable::Stale);
        }
        self.result.clone()
    }
}
