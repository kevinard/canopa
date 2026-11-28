//! Network status and Wi-Fi control.
//!
//! - [`bundle`] decides which daemons own status and Wi-Fi control: a unique
//!   NetworkManager owner supplies both; otherwise systemd-networkd may supply
//!   status and iwd may supply control.
//! - [`status`] observes connectivity and the connection label from that owner.
//! - [`wifi`] scans and connects through NetworkManager or iwd.
//! - [`iwd`] reads and changes iwd's devices, networks and saved networks for
//!   a settings page; [`networkd`] reads systemd-networkd's links.
//! - [`manager`] reads and changes NetworkManager's devices and saved
//!   connections for a settings page; [`profiles`] builds new ones (Wi-Fi
//!   with WPA-Enterprise, hotspots, WireGuard); [`secret_agent`] answers
//!   NetworkManager's secret questions without storing anything.
//!
//! ```no_run
//! use canopa_services::{ServiceState, network::status};
//! use futures_util::StreamExt;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let mut states = status::watch(system);
//! while let Some(state) = states.next().await {
//!     match state {
//!         ServiceState::Available(status) => println!("{:?} {:?}", status.connectivity, status.label),
//!         ServiceState::Unavailable => println!("No network service"),
//!     }
//! }
//! # Ok(()) }
//! ```
pub mod bundle;
pub mod iwd;
pub mod manager;
pub mod networkd;
pub mod profiles;
pub mod secret_agent;
pub mod status;
pub mod wifi;

/// Connectivity level reported by the active status backend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Connectivity {
    /// Full Internet access (NetworkManager).
    Full,
    /// Local or site connectivity only.
    Limited,
    /// No reachability result yet.
    #[default]
    Unknown,
    /// A link is online; Internet reachability is unmeasured (systemd-networkd).
    Connected,
    /// A captive portal was detected.
    Portal,
    /// No connectivity.
    None,
}

/// What carries the primary connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    Wired,
    Wireless,
    Vpn,
    Other,
}

impl NetworkKind {
    /// From NetworkManager's connection type, such as `802-3-ethernet`.
    pub fn from_nm(kind: &str) -> Option<Self> {
        Some(match kind {
            "" => return None,
            "802-3-ethernet" | "pppoe" => Self::Wired,
            "802-11-wireless" => Self::Wireless,
            "vpn" | "wireguard" => Self::Vpn,
            _ => Self::Other,
        })
    }
}

/// One status reading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkStatus {
    pub connectivity: Connectivity,
    /// The NetworkManager primary connection's profile name, or networkd's
    /// sorted usable link names joined with ", ".
    pub label: Option<String>,
    /// The primary connection's kind (NetworkManager only).
    pub kind: Option<NetworkKind>,
    /// Names of active VPN connections (NetworkManager only).
    pub vpn: Vec<String>,
    /// Whether Wi-Fi is switched on, when NetworkManager manages a radio
    /// that is not blocked in hardware.
    pub wireless: Option<bool>,
}
