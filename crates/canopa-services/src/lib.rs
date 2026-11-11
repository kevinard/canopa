//! Toolkit-free clients for the system services Canopa presents.
//!
//! System services own state and policy; this crate only reads, observes and
//! asks them. It has no Iced, Wayland or panel-protocol dependency, so the Shell,
//! its headless providers and Settings share one client per service.
