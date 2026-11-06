//! Shell owns durable wallpaper selections; UI and D-Bus are adapters.
mod locker;
mod runtime;
mod store;

pub(crate) use locker::lock;
pub use runtime::Event;
pub(crate) use runtime::subscription;
pub(crate) use store::Snapshot;
