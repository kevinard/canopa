//! Integration cases share one executable to avoid linking each source file.
//! Filter by module with `cargo test --test integration MODULE::`.

mod access;
mod account;
mod app_chooser;
mod background;
mod background_observer;
#[cfg(feature = "desktop")]
mod capture;
mod dynamic_launcher;
#[cfg(feature = "desktop")]
mod eis_reader;
mod file_chooser;
mod input_capture;
mod lockdown;
mod remote_desktop;
mod screencast;
mod screencast_capture;
mod screenshot;
mod settings;
mod usb;
mod wallpaper;
