use crate::NotificationLink;
use std::{future::Future, pin::Pin};

mod portal;
pub use portal::PortalNotificationLinkOpener;

pub type LinkOpeningFuture = Pin<Box<dyn Future<Output = LinkOpeningResult> + Send + 'static>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkOpeningResult {
    Opened,
    Cancelled,
    Unavailable,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkOpeningStatus {
    pub available: bool,
    pub pending: usize,
    pub last_error: Option<String>,
}

/// Opens an already validated, explicitly selected link. Implementations must
/// bound their work and never retry an uncertain launch through another opener.
pub trait NotificationLinkOpener: Send + Sync {
    fn available(&self) -> bool;
    fn open(&self, uri: NotificationLink, activation_token: Option<String>) -> LinkOpeningFuture;
    fn cancel_pending(&self) {}
}

#[derive(Clone, Debug, Default)]
pub struct NoopNotificationLinkOpener;

impl NotificationLinkOpener for NoopNotificationLinkOpener {
    fn available(&self) -> bool {
        false
    }
    fn open(&self, _: NotificationLink, _: Option<String>) -> LinkOpeningFuture {
        Box::pin(async { LinkOpeningResult::Unavailable })
    }
}
