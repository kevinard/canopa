//! One request owns its cache and serializes its interactive prompts. The cache
//! is never global, serialized with a grant, or written to persistent storage.
use super::{Challenge, Credentials, Kind, prompt};
use crate::{
    print::{Context, worker::Destination},
    process::DialogProcess,
};
use futures_util::future::BoxFuture;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

pub(super) const CACHE_LIMIT: usize = 8;
pub(super) const PROMPT_LIMIT: u8 = 6;

pub(super) trait Presenter: Send + Sync {
    fn show(
        &self,
        challenge: Challenge,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Credentials>>>;
}
pub(crate) struct Entry {
    pub challenge: Challenge,
    pub credentials: Credentials,
}
impl Entry {
    fn matches(&self, challenge: &Challenge) -> bool {
        self.challenge.endpoint == challenge.endpoint && self.challenge.kind == challenge.kind
    }
}
#[derive(Default)]
pub(crate) struct Retained {
    pub(crate) entries: Vec<Entry>,
    pub(super) prompts: u8,
}
impl std::fmt::Debug for Retained {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<request-owned printer credentials>")
    }
}
struct State {
    retained: Retained,
    epoch: u64,
}
pub(crate) struct Broker {
    presenter: Arc<dyn Presenter>,
    state: Mutex<State>,
    prompt: tokio::sync::Mutex<()>,
}
impl Broker {
    pub fn new(process: DialogProcess, context: Context) -> Arc<Self> {
        Self::restore(Retained::default(), process, context)
    }
    pub fn restore(retained: Retained, process: DialogProcess, context: Context) -> Arc<Self> {
        Self::with_presenter(retained, Arc::new(prompt::Dialog { process, context }))
    }
    fn with_presenter(retained: Retained, presenter: Arc<dyn Presenter>) -> Arc<Self> {
        Arc::new(Self {
            presenter,
            state: Mutex::new(State { retained, epoch: 0 }),
            prompt: tokio::sync::Mutex::new(()),
        })
    }
    pub fn clear(&self) {
        let mut state = self.state.lock().expect("printer credentials");
        state.epoch += 1;
        // Preserve the request's finite prompt budget even after changing to a
        // file or discarding credentials rejected by a previous operation.
        state.retained.entries.clear();
    }
    pub fn take(&self, destination: &Destination) -> Retained {
        let mut state = self.state.lock().expect("printer credentials");
        state.epoch += 1;
        let mut retained = std::mem::take(&mut state.retained);
        retained
            .entries
            .retain(|entry| match &entry.challenge.kind {
                Kind::Server { .. } => true,
                Kind::Queue {
                    destination: current,
                    ..
                } => current == destination,
            });
        retained
    }
    pub async fn request(
        &self,
        mut challenge: Challenge,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Credentials>> {
        challenge.validate()?;
        let _prompt = self.prompt.lock().await;
        if *cancelled.borrow() || cancelled.has_changed().is_err() {
            return Ok(None);
        }
        let epoch = {
            let mut state = self.state.lock().expect("printer credentials");
            if let Some(index) = state
                .retained
                .entries
                .iter()
                .position(|entry| entry.matches(&challenge))
            {
                // A normal interactive operation can adopt the account chosen
                // earlier for this request. Automatic retirement must keep its
                // current account, whose identity was checked against the job.
                if !challenge.interactive
                    && matches!(&challenge.kind, Kind::Server { .. })
                    && state.retained.entries[index].credentials.values[0].bytes()
                        != challenge.suggested_user.as_bytes()
                {
                    return Ok(None);
                }
                if challenge.attempt == 1 {
                    return Ok(Some(state.retained.entries[index].credentials.copy()));
                }
                let rejected = state.retained.entries.remove(index);
                if let Some(index) = challenge
                    .fields()
                    .iter()
                    .position(|field| *field == super::Field::Username)
                {
                    challenge.suggested_user =
                        std::str::from_utf8(rejected.credentials.values[index].bytes())?.to_owned();
                }
            }
            if !challenge.interactive {
                return Ok(None);
            }
            anyhow::ensure!(
                state.retained.prompts < PROMPT_LIMIT && state.retained.entries.len() < CACHE_LIMIT,
                "Printer authentication prompt limit reached"
            );
            state.retained.prompts += 1;
            state.epoch
        };
        let credentials = self
            .presenter
            .show(challenge.clone(), cancelled.clone())
            .await?;
        if *cancelled.borrow() || cancelled.has_changed().is_err() {
            return Ok(None);
        }
        let mut state = self.state.lock().expect("printer credentials");
        if state.epoch != epoch {
            return Ok(None);
        }
        if let Some(credentials) = credentials {
            credentials.validate(&challenge)?;
            let answer = credentials.copy();
            state.retained.entries.push(Entry {
                challenge,
                credentials,
            });
            Ok(Some(answer))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
