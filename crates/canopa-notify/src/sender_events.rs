use crate::SenderEvent;
use std::{future::Future, pin::Pin};

pub type SenderEventPublication = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

/// The authoritative, ordered transport publisher. Diagnostic subscribers are separate.
/// Implementations freeze transport identity synchronously, then enqueue with bounded
/// backpressure. Returning success means the record is owned by the transport.
pub trait SenderEventPublisher: Send + Sync {
    fn publish(&self, event: SenderEvent) -> SenderEventPublication;

    /// Includes failures that happen after enqueueing, such as a disconnected bus.
    fn error(&self) -> Option<String> {
        None
    }
}
