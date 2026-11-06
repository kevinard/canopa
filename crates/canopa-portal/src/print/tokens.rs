//! Bounded volatile one-use approvals. Unknown tokens always require new consent.
use super::authority::Owner;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio::sync::watch;
use zbus::fdo;

const CAPACITY: usize = 64;
const PER_OWNER: usize = 8;
const LIFETIME: Duration = Duration::from_secs(300);

struct Prepared<T> {
    issuance: Arc<()>,
    owner: Owner,
    expires: Instant,
    approval: T,
    cancellation: Option<watch::Receiver<bool>>,
    _retirement: Retirement,
}
type Entries<T> = Mutex<HashMap<u32, Prepared<T>>>;

/// An approval which has not yet been returned to its caller. Dropping this
/// exact issuance retires it; consuming/publishing or token-number reuse cannot
/// let an old guard retire a later approval.
pub struct Issued<T> {
    token: u32,
    issuance: Arc<()>,
    entries: Weak<Entries<T>>,
    published: bool,
}
impl<T> Issued<T> {
    pub fn token(&self) -> u32 {
        self.token
    }
    pub fn publish(mut self) -> u32 {
        self.published = true;
        self.token
    }
}
impl<T> Drop for Issued<T> {
    fn drop(&mut self) {
        if !self.published
            && let Some(entries) = self.entries.upgrade()
        {
            retire(&entries, self.token, &self.issuance);
        }
    }
}
fn retire<T>(entries: &Entries<T>, token: u32, issuance: &Arc<()>) {
    if let Ok(mut entries) = entries.lock()
        && entries
            .get(&token)
            .is_some_and(|entry| Arc::ptr_eq(&entry.issuance, issuance))
    {
        entries.remove(&token);
    }
}

struct Retirement(tokio::task::JoinHandle<()>);
impl Drop for Retirement {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// `T` is the backend's owned approval, not caller-returned settings. It can
/// retain file descriptors; it does not need to be cloneable or serializable.
pub struct Tokens<T> {
    entries: Arc<Entries<T>>,
}
impl<T> Default for Tokens<T> {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<T: Send + 'static> Tokens<T> {
    pub async fn issue(&self, owner: Owner, approval: T) -> fdo::Result<u32> {
        self.issue_with_lifetime(owner, approval, LIFETIME).await
    }

    pub async fn issue_cancellable(
        &self,
        owner: Owner,
        approval: T,
        cancelled: watch::Receiver<bool>,
    ) -> fdo::Result<u32> {
        self.issue_inner(owner, approval, LIFETIME, Some(cancelled))
            .await
            .map(Issued::publish)
    }

    pub async fn issue_retained(
        &self,
        owner: Owner,
        approval: T,
        cancelled: watch::Receiver<bool>,
    ) -> fdo::Result<Issued<T>> {
        self.issue_inner(owner, approval, LIFETIME, Some(cancelled))
            .await
    }

    async fn issue_with_lifetime(
        &self,
        owner: Owner,
        approval: T,
        lifetime: Duration,
    ) -> fdo::Result<u32> {
        self.issue_inner(owner, approval, lifetime, None)
            .await
            .map(Issued::publish)
    }

    async fn issue_inner(
        &self,
        owner: Owner,
        approval: T,
        lifetime: Duration,
        cancellation: Option<watch::Receiver<bool>>,
    ) -> fdo::Result<Issued<T>> {
        if !owner.confirm().await {
            return Err(fdo::Error::AccessDenied("Print caller disappeared".into()));
        }
        if cancellation
            .as_ref()
            .is_some_and(|cancelled| *cancelled.borrow() || cancelled.has_changed().is_err())
        {
            return Err(fdo::Error::AccessDenied("Print approval revoked".into()));
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| fdo::Error::Failed("Print approvals unavailable".into()))?;
        let now = Instant::now();
        entries.retain(|_, entry| {
            entry.expires > now && !entry.owner.is_revoked() && !cancelled(&entry.cancellation)
        });
        if entries.len() >= CAPACITY
            || entries
                .values()
                .filter(|entry| entry.owner.same_actor(&owner))
                .count()
                >= PER_OWNER
        {
            return Err(fdo::Error::LimitsExceeded(
                "Too many print approvals".into(),
            ));
        }
        for _ in 0..16 {
            let mut bytes = [0; 4];
            let count = rustix::rand::getrandom(&mut bytes, rustix::rand::GetRandomFlags::empty())
                .map_err(|_| fdo::Error::Failed("Print token generation unavailable".into()))?;
            if count != bytes.len() {
                return Err(fdo::Error::Failed(
                    "Print token generation incomplete".into(),
                ));
            }
            let token = u32::from_ne_bytes(bytes);
            if token != 0 && !entries.contains_key(&token) {
                let expires = now + lifetime;
                let issuance = Arc::new(());
                let retirement_issuance = issuance.clone();
                let weak = Arc::downgrade(&self.entries);
                let mut cancelled = owner.cancelled();
                let retained_cancellation = cancellation.clone();
                let mut cancellation = cancellation.clone();
                let retirement = tokio::spawn(async move {
                    tokio::select! {
                        biased;
                        _ = async {
                            if !*cancelled.borrow() {
                                let _ = cancelled.changed().await;
                            }
                        } => {},
                        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(expires)) => {},
                        _ = async {
                            match &mut cancellation {
                                Some(cancelled) => { if !*cancelled.borrow() { let _ = cancelled.changed().await; } }
                                None => std::future::pending::<()>().await,
                            }
                        } => {},
                    }
                    // A weak reference avoids a task/store cycle. Removal also
                    // drops any retained target descriptors without another call.
                    if let Some(entries) = weak.upgrade() {
                        retire(&entries, token, &retirement_issuance);
                    }
                });
                entries.insert(
                    token,
                    Prepared {
                        issuance: issuance.clone(),
                        owner,
                        expires,
                        approval,
                        cancellation: retained_cancellation,
                        _retirement: Retirement(retirement),
                    },
                );
                return Ok(Issued {
                    token,
                    issuance,
                    entries: Arc::downgrade(&self.entries),
                    published: false,
                });
            }
        }
        Err(fdo::Error::Failed("Print token space unavailable".into()))
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    pub async fn consume(&self, owner: &Owner, token: u32) -> fdo::Result<Option<T>> {
        let prepared = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| fdo::Error::Failed("Print approvals unavailable".into()))?;
            let now = Instant::now();
            entries.retain(|_, entry| {
                entry.expires > now && !entry.owner.is_revoked() && !cancelled(&entry.cancellation)
            });
            if entries
                .get(&token)
                .is_some_and(|entry| entry.owner.same_identity(owner))
            {
                entries.remove(&token)
            } else {
                None
            }
        };
        // Remove before awaiting or handing off to a worker: simultaneous calls
        // cannot submit the same approval twice. A stranger cannot consume it.
        match prepared {
            Some(prepared)
                if prepared.owner.confirm().await
                    && owner.confirm().await
                    && !cancelled(&prepared.cancellation) =>
            {
                Ok(Some(prepared.approval))
            }
            _ => Ok(None),
        }
    }
}

fn cancelled(source: &Option<watch::Receiver<bool>>) -> bool {
    source
        .as_ref()
        .is_some_and(|cancelled| *cancelled.borrow() || cancelled.has_changed().is_err())
}

#[cfg(test)]
mod tests;
