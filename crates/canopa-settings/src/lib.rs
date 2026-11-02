//! Settings is a frontend; compositor, wallpaper and idle policy keep their owners.
pub mod display;
pub mod input;
pub mod niri;
pub mod preferences;
pub mod session;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod ui;
