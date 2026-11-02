//! Unprivileged Canopa greeter. Greetd owns PAM, credentials and session lifetime.
pub mod accounts;
pub mod avatars;
pub mod catalog;
pub mod compositor;
pub mod config;
pub mod controller;
mod helper;
pub mod nss;
pub mod power;
pub mod preferences;
pub mod protocol;
pub mod sessions;
pub mod wayland;
