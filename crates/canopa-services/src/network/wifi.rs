//! Wi-Fi scanning and connection through NetworkManager or iwd.
//!
//! [`backends::choose_backend`] follows the network [`bundle`](super::bundle)
//! and never mixes NetworkManager status with iwd control. A selection is an
//! opaque device/network handle bound to the daemon owner, device and network
//! fingerprint; backends revalidate it before acting and report
//! [`WifiBackendErrorCategory::TargetChanged`] when it is stale.
//!
//! ```no_run
//! use canopa_services::network::wifi::{WifiControlBackend, backends::choose_backend};
//! # async fn page() {
//! let Some(mut wifi) = choose_backend().await else {
//!     return; // No Wi-Fi control service: show the placeholder.
//! };
//! let changes = wifi.changes().await; // Re-read the snapshot on each item.
//! let snapshot = wifi.scan().await;
//! # let _ = (changes, snapshot);
//! # }
//! ```
pub mod backends;
pub mod cleanup;
mod identity;

use std::future::Future;
use std::pin::Pin;
pub type WifiBackendFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, WifiBackendError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendName {
    Iwd,
    NetworkManager,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub can_scan_while_connecting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiBackendError {
    category: WifiBackendErrorCategory,
}

impl WifiBackendError {
    pub fn new(category: WifiBackendErrorCategory) -> Self {
        Self { category }
    }

    pub fn category(&self) -> WifiBackendErrorCategory {
        self.category
    }
}

impl std::fmt::Display for WifiBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.category {
            WifiBackendErrorCategory::TargetChanged => "selected Wi-Fi target changed",
            WifiBackendErrorCategory::NoControl => {
                "no supported Wi-Fi control backend is available"
            }
            WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed",
            WifiBackendErrorCategory::ConnectionFailed => "Wi-Fi connection failed",
            WifiBackendErrorCategory::CredentialRejected => "network credential was rejected",
            WifiBackendErrorCategory::UnsupportedNetwork => "Wi-Fi network type is unsupported",
            WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required",
        })
    }
}

impl std::error::Error for WifiBackendError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBackendErrorCategory {
    NoControl,
    ScanFailed,
    ConnectionFailed,
    CredentialRejected,
    UnsupportedNetwork,
    AuthorizationRequired,
    TargetChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurity {
    Open,
    Personal,
    Enterprise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub id: String,
    pub label: String,
    pub signal_strength: u8,
    pub security: WifiSecurity,
    pub known: bool,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiSnapshot {
    /// Opaque handle for the currently observed daemon/device pair.
    pub device_id: Option<String>,
    pub networks: Vec<WifiNetwork>,
    pub scanning: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WifiConnectRequest {
    pub device_id: String,
    pub network_id: Option<String>,
    pub label: String,
    pub credential: Option<String>,
    pub hidden: bool,
}

impl std::fmt::Debug for WifiConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WifiConnectRequest")
            .field("network_id", &self.network_id)
            .field("label", &self.label)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "[redacted]"),
            )
            .field("hidden", &self.hidden)
            .finish()
    }
}

pub trait WifiControlBackend {
    fn name(&self) -> BackendName;
    fn capabilities(&self) -> BackendCapabilities;
    fn initial_snapshot<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot>;
    fn scan<'a>(&'a mut self) -> WifiBackendFuture<'a, WifiSnapshot>;
    fn connect<'a>(
        &'a mut self,
        request: WifiConnectRequest,
    ) -> WifiBackendFuture<'a, WifiSnapshot>;
    /// Whether the Wi-Fi radio is on; `None` when the backend cannot tell.
    fn radio<'a>(&'a mut self) -> WifiBackendFuture<'a, Option<bool>> {
        Box::pin(async { Ok(None) })
    }
    /// Turn the radio on or off.
    fn set_radio<'a>(&'a mut self, _enabled: bool) -> WifiBackendFuture<'a, WifiSnapshot> {
        Box::pin(async { Err(WifiBackendError::new(WifiBackendErrorCategory::NoControl)) })
    }
    /// Leave the current network on the device of a snapshot.
    fn disconnect<'a>(&'a mut self, _device_id: String) -> WifiBackendFuture<'a, WifiSnapshot> {
        Box::pin(async { Err(WifiBackendError::new(WifiBackendErrorCategory::NoControl)) })
    }
    /// Forget a known network (`device_id` and `network_id` of the request),
    /// with the credentials its daemon stored.
    fn forget<'a>(
        &'a mut self,
        _request: WifiConnectRequest,
    ) -> WifiBackendFuture<'a, WifiSnapshot> {
        Box::pin(async { Err(WifiBackendError::new(WifiBackendErrorCategory::NoControl)) })
    }
}
