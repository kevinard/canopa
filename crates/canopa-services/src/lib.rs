//! Toolkit-free clients for the system services Canopa presents.
//!
//! System services own state and policy; this crate only reads, observes and
//! asks them. It has no Iced, Wayland or panel-protocol dependency, so the Shell,
//! its headless providers and Settings share one client per service
//! (see `docs/adr/0137-shared-system-service-clients.md`).
//!
//! | Module | Service | Snapshot | Changes | Commands |
//! | --- | --- | --- | --- | --- |
//! | [`network`] | NetworkManager, systemd-networkd, iwd | [`network::NetworkStatus`] (with kind and VPNs), [`network::wifi::WifiSnapshot`], [`network::manager::Overview`] | [`network::status::watch`], `RuntimeBackend::changes`, [`network::manager::watch`] | Wi-Fi scan, connect, radio, disconnect, forget; profiles (Wi-Fi with WPA-Enterprise, wired IP and DNS, VPN, WireGuard, hotspot, metered, proxy); [`network::secret_agent`] |
//! | [`bluetooth`] | BlueZ | [`bluetooth::Snapshot`] | [`bluetooth::watch`] | power, discovery, connect, [`bluetooth::pair`] |
//! | `audio` (feature `audio`) | PulseAudio protocol (PipeWire) | `audio::AudioSnapshot` (with streams and cards from `audio::mixer`), `audio::Defaults` | `audio::monitor`, `audio::mixer`, `audio::watch_defaults`, `audio::level_meter` | volume, mute, balance, default device, stream volume and mute, card profiles, `audio::change_default` |
//! | [`upower`] | UPower | [`upower::DisplayDevice`], [`upower::PowerSupply`] (batteries, health, lid) | [`upower::watch`], [`upower::watch_power_supply`] | – |
//! | [`power_profiles`] | power-profiles-daemon | [`power_profiles::PowerProfiles`] | [`power_profiles::watch`] | [`power_profiles::PowerProfiles::set`] |
//! | [`power_switches`] | logind's lid switch | [`power_switches::LidAction`] | – | [`power_switches::handle_lid`] |
//! | [`logind`] | systemd-logind | [`logind::Capabilities`], [`logind::InhibitorLock`] | [`logind::watch_capabilities`] | [`logind::request`], [`logind::inhibit`], [`logind::Session::set_brightness`] |
//! | [`backlight`] | sysfs display and keyboard backlights through logind | [`backlight::Backlight`], [`backlight::KeyboardBacklight`] | [`backlight::watch`] (polling), [`backlight::hardware_changes`] | [`backlight::Backlight::set`], `set_level`, `set_perceptual` |
//! | [`mpris`] | MPRIS 2 players | [`mpris::Player`] | [`mpris::Registry`] building blocks | [`mpris::control`] |
//!
//! Observers yield [`ServiceState`]: a missing service is `Unavailable`, not an
//! error, and streams recover by themselves. Commands return [`Error`] or a
//! service-specific error. Every client is async on Tokio; the audio client
//! runs its libpulse main loop on its own thread.
//!
//! Each module starts with an example. Tests use the fakes in `testing`,
//! enabled by the `test-support` feature.

#[cfg(feature = "audio")]
pub mod audio;
pub mod backlight;
pub mod bluetooth;
pub mod bus;
mod error;
pub mod logind;
pub mod mpris;
pub mod network;
pub mod power_profiles;
pub mod power_switches;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod upower;

pub use error::{Error, ServiceState};
