use crate::{NotificationSoundPlayback, NotificationSoundSettings};

pub trait SoundPresenter: Send + Sync {
    /// Failure must leave the previously installed settings intact.
    fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()>;
    /// Best-effort presentation: an admission or playback failure must not undo
    /// the notification receipt. Adapters need not queue overloaded requests.
    fn play_notification_sound(&self, playback: NotificationSoundPlayback) -> anyhow::Result<()>;
    fn stop_all_sounds(&self);
    fn supports_sound(&self) -> bool;
}

#[derive(Clone, Default)]
pub struct NoopSoundPresenter;

impl SoundPresenter for NoopSoundPresenter {
    fn apply_sound_settings(&self, _settings: NotificationSoundSettings) -> anyhow::Result<()> {
        Ok(())
    }

    fn play_notification_sound(&self, _playback: NotificationSoundPlayback) -> anyhow::Result<()> {
        Ok(())
    }

    fn stop_all_sounds(&self) {}

    fn supports_sound(&self) -> bool {
        false
    }
}

#[cfg(feature = "audio")]
mod playback;
#[cfg(feature = "audio")]
mod theme;
#[cfg(feature = "audio")]
pub use playback::RodioSoundPresenter;
