//! Interactive review stays open on conflicts; only Apply proposes a mutation.
use super::{Actions, Binding, transport::Snapshot};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[cfg(feature = "desktop")]
mod process;
#[cfg(feature = "desktop")]
pub use process::Production;
pub const MAX_FRAME: usize = 128 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub app_id: String,
    pub parent_window: String,
    pub activation_token: Option<String>,
    pub configuring: bool,
    pub actions: Actions,
    pub defaults: Vec<Binding>,
    pub current: Vec<Binding>,
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    Apply {
        generation: u64,
        bindings: Vec<Binding>,
    },
    Cancel,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub generation: u64,
    pub current: Vec<Binding>,
    pub error: Option<String>,
    pub failed: Vec<String>,
    pub busy: bool,
}
impl Feedback {
    pub(super) fn snapshot(actions: &Actions, snapshot: &Snapshot) -> anyhow::Result<Self> {
        Ok(Self {
            generation: snapshot.generation,
            current: actions.selected(&snapshot.bindings)?,
            error: None,
            failed: Vec::new(),
            busy: false,
        })
    }
}
pub trait Editor: Send {
    fn events(&mut self) -> Option<mpsc::Receiver<Decision>>;
    fn update(&self, feedback: Feedback) -> anyhow::Result<()>;
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>>;
}
pub trait Presenter: Send + Sync + 'static {
    fn open(&self, request: Request) -> BoxFuture<'_, anyhow::Result<Box<dyn Editor>>>;
}
pub struct Unavailable;
impl Presenter for Unavailable {
    fn open(&self, _: Request) -> BoxFuture<'_, anyhow::Result<Box<dyn Editor>>> {
        Box::pin(async { anyhow::bail!("Shortcut configuration unavailable") })
    }
}
