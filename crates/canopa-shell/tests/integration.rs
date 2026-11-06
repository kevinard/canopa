//! Integration cases share one executable to avoid linking each source file.
//! Filter by module with `cargo test --test integration MODULE::`.

#[cfg(feature = "desktop")]
mod canopa_provider;
#[cfg(feature = "desktop")]
mod configuration;
#[cfg(feature = "desktop")]
mod diagnostics;
#[cfg(feature = "desktop")]
mod hardware_panel;
#[cfg(feature = "desktop")]
mod menu_cli;
mod provider_protocol;
#[cfg(feature = "desktop")]
mod wallpaper;
mod wifi_provider;
