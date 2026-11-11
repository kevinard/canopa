//! Canopa Files owns browsing, local file operations, previews, and user preferences.
#[macro_use]
pub mod i18n;
pub mod action_jobs;
pub mod actions;
pub mod activation;
#[cfg(feature = "local-runtime")]
pub mod application;
pub mod browse;
pub mod configuration;
#[cfg(feature = "desktop")]
#[path = "iced_desktop/mod.rs"]
pub mod desktop;
#[cfg(any(feature = "background-actions", feature = "background-operations"))]
pub mod desktop_notifications;
pub mod file_clipboard;
#[cfg(feature = "local-runtime")]
pub mod file_manager;
pub mod file_properties;
pub mod folder_watch;
pub mod interface;
mod interface_config;
#[cfg(feature = "background-actions")]
pub mod local_actions;
pub mod local_activity;
#[cfg(feature = "local-runtime")]
pub mod local_volumes;
pub mod mounts;
pub mod network;
pub mod opening;
#[cfg(feature = "background-operations")]
pub mod operation_attention;
pub mod operation_jobs;
pub mod operations;
pub use operations::transfer as transfers;
pub use operations::trash;
mod path_serde;
pub mod personal_folders;
#[cfg(feature = "media-playback")]
pub mod playback;
pub mod preferences;
pub mod presentation;
pub mod preview;
mod process;
#[cfg(feature = "local-runtime")]
pub mod recovery_catalog;
pub mod search;
mod state_io;
pub mod storage;
mod syntax;
#[cfg(feature = "desktop")]
pub mod thumbnails;
mod time_serde;

/// Cancellation is shared by a worker and its caller; clones refer to the same job.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }
}
