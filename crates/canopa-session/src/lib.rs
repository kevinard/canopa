//! Independent software-rendered wallpaper and secure session-lock applications.
#[macro_use]
pub mod i18n;
pub mod auth;
pub mod background;
pub mod control;
mod file_watch;
pub mod idle;
pub mod ipc;
pub mod layout;
pub mod lock_ui;
pub mod logind;
pub mod picker;
pub mod picker_wayland;
pub mod portal_lock_image;
pub mod settings;
pub mod wallpaper_state;
pub mod wayland;
