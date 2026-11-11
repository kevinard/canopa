//! An unprivileged polkit agent. Native polkit owns credential verification and grants.
#[macro_use]
pub mod i18n;
pub mod config;
pub mod controller;
pub mod identity;
pub mod native;
pub mod session;
pub mod wayland;
