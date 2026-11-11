//! Settings is a frontend; compositor, wallpaper and idle policy keep their owners.
pub use registry::Page;
pub mod display;
pub mod file_page;
pub mod input;
pub mod instance;
pub mod launcher;
pub mod live;
pub mod niri;
pub mod power;
pub mod preferences;
pub mod profile_watch;
pub mod profiles;
pub mod registry;
pub mod session;
pub mod shortcuts;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod ui;
#[cfg(feature = "desktop")]
mod widget_layouts;
