//! Settings is a frontend; compositor, wallpaper and idle policy keep their owners.
pub use registry::Page;
pub mod bluetooth;
pub mod default_apps;
pub mod display;
pub mod file_page;
pub mod input;
pub mod instance;
pub mod launcher;
pub mod live;
pub mod network;
pub mod niri;
pub mod notifications;
pub mod power;
pub mod preferences;
pub mod profile_watch;
pub mod profiles;
pub mod registry;
pub mod session;
pub mod shortcuts;
pub mod sound;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod ui;
#[cfg(feature = "desktop")]
mod widget_layouts;
