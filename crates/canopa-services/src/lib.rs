//! Toolkit-free clients for the system services Canopa presents.
//!
//! System services own state and policy; this crate only reads, observes and
//! asks them. It has no Iced, Wayland or panel-protocol dependency, so the Shell,
//! its headless providers and Settings share one client per service.

pub mod backlight;
pub mod bus;
mod error;
pub mod logind;
pub mod network;
pub mod power_profiles;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod upower;

pub use error::{Error, ServiceState};
