use std::future::Future;
use std::pin::Pin;
pub type ActivationTokenFuture = Pin<Box<dyn Future<Output = Option<String>> + Send + 'static>>;

pub trait ActivationTokenProvider: Send + Sync {
    fn available(&self) -> bool;
    fn activation_token(&self, app_identity: String) -> ActivationTokenFuture;
}

#[derive(Clone, Debug, Default)]
pub struct NoopActivationTokenProvider;

impl ActivationTokenProvider for NoopActivationTokenProvider {
    fn available(&self) -> bool {
        false
    }

    fn activation_token(&self, _app_identity: String) -> ActivationTokenFuture {
        Box::pin(async { None })
    }
}

#[cfg(feature = "gui")]
mod wayland;
#[cfg(feature = "gui")]
pub use wayland::WaylandActivationTokenProvider;
