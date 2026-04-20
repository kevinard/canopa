//! StatusNotifierItem (SNI) tray protocol implementation.
//!
//! Split into:
//! - `host`: SNI bus ownership and item lifecycle
//! - `subscription`: iced subscription wiring (built on `crate::dbus::subscription`)
//! - `pixmap`: pixmap validation, size/byte bounds, ARGB->RGBA conversion
//! - `menu`: DBusMenu fetch and bounds
//! - `proxy`: zbus proxy trait definitions

pub mod host;
pub(super) mod menu;
pub(super) mod pixmap;
pub(crate) mod proxy;
// pub mod subscription;

// Re-exports for future use when sub-modules stabilise.
// Uncomment when callers are updated to prefer the sni:: prefix:
// pub use host::{TrayEvent, TrayItem, ItemStatus, sni_subscription, menu_layout_subscription};
