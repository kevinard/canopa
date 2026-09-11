use crate::local_file::read_bounded;
use crate::{NotificationSound, NotificationSoundPlayback, NotificationSoundSettings};
use anyhow::Context;
use rodio::Source;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod theme;
use theme::SoundThemeResolver;
const MAX_SOUND_JOBS: usize = 4;
const MAX_SOUND_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DECODED_SOUND_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOUND_SECONDS: usize = 10;

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

#[derive(Clone)]
pub struct RodioSoundPresenter {
    state: Arc<Mutex<RodioSoundState>>,
    open_backend: Arc<dyn Fn() -> anyhow::Result<RodioBackend> + Send + Sync>,
}

impl RodioSoundPresenter {
    pub fn start() -> Self {
        Self::with_data_dirs(NotificationSoundSettings::default(), xdg_data_dirs())
    }

    fn with_data_dirs(settings: NotificationSoundSettings, data_dirs: Vec<PathBuf>) -> Self {
        Self::with_backend_opener(
            settings,
            SoundThemeResolver::from_environment(data_dirs),
            Arc::new(RodioBackend::open),
        )
    }

    // The output-device boundary also permits offline playback tests with a real mixer.
    fn with_backend_opener(
        settings: NotificationSoundSettings,
        resolver: SoundThemeResolver,
        open_backend: Arc<dyn Fn() -> anyhow::Result<RodioBackend> + Send + Sync>,
    ) -> Self {
        let presenter = Self {
            state: Arc::new(Mutex::new(RodioSoundState {
                settings: NotificationSoundSettings {
                    enabled: false,
                    ..settings.clone()
                },
                backend: None,
                resolver,
                jobs: Arc::new(AtomicUsize::new(0)),
                cancelled: Arc::new(AtomicBool::new(false)),
            })),
            open_backend,
        };
        if presenter.apply_sound_settings(settings).is_err() {
            eprintln!("level=warn component=sound event=configuration_failed");
        }
        presenter
    }
}

impl SoundPresenter for RodioSoundPresenter {
    fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.settings != settings {
            state.cancel_playback();
        }
        let was_enabled = state.settings.enabled;
        let enabled_changed = was_enabled != settings.enabled;
        state.settings = settings;

        if !state.settings.enabled {
            state.backend = None;
            return Ok(());
        }

        if state.backend.is_none() {
            match (self.open_backend)() {
                Ok(backend) => state.backend = Some(backend),
                Err(_) => {
                    eprintln!("level=warn component=sound event=backend_unavailable");
                }
            }
        } else if enabled_changed {
            eprintln!("level=info component=sound event=backend_enabled");
        }

        Ok(())
    }

    fn play_notification_sound(&self, playback: NotificationSoundPlayback) -> anyhow::Result<()> {
        let (settings, resolver, mixer, job) = {
            let state = self.state.lock().unwrap();
            if !state.settings.enabled {
                return Ok(());
            }
            let Some(backend) = &state.backend else {
                return Ok(());
            };
            state
                .jobs
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |jobs| {
                    (jobs < MAX_SOUND_JOBS).then_some(jobs + 1)
                })
                .map_err(|_| anyhow::anyhow!("notification sound capacity exhausted"))?;
            let job = Arc::new(SoundJob {
                jobs: state.jobs.clone(),
                cancelled: state.cancelled.clone(),
            });
            (
                state.settings.clone(),
                state.resolver.clone(),
                backend.mixer.clone(),
                job,
            )
        };

        std::thread::Builder::new()
            .name("canopa-notify-sound".to_string())
            .spawn(move || {
                if let Err(error) = play_with_rodio(playback, settings, resolver, mixer, &job) {
                    eprintln!("level=warn component=sound event=playback_failed error={error:#}");
                }
            })?;
        Ok(())
    }

    fn stop_all_sounds(&self) {
        let mut state = self.state.lock().unwrap();
        state.cancel_playback();
        state.backend = None;
    }

    fn supports_sound(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.settings.enabled && state.backend.is_some()
    }
}

struct RodioSoundState {
    settings: NotificationSoundSettings,
    backend: Option<RodioBackend>,
    resolver: SoundThemeResolver,
    jobs: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
}

impl RodioSoundState {
    fn cancel_playback(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.cancelled = Arc::new(AtomicBool::new(false));
    }
}

// A reservation covers the worker AND its queued/playing source. Neither may
// release capacity while the other still retains resources.
struct SoundJob {
    jobs: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for SoundJob {
    fn drop(&mut self) {
        self.jobs.fetch_sub(1, Ordering::AcqRel);
    }
}

impl SoundJob {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

struct AdmittedSound {
    source: rodio::buffer::SamplesBuffer,
    job: Arc<SoundJob>,
}

impl Iterator for AdmittedSound {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.job.is_cancelled() {
            return None;
        }
        self.source.next()
    }
}

impl rodio::Source for AdmittedSound {
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.source.channels()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.source.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

struct RodioBackend {
    _sink: Option<rodio::MixerDeviceSink>,
    mixer: rodio::mixer::Mixer,
}

impl RodioBackend {
    fn open() -> anyhow::Result<Self> {
        let mut sink = rodio::DeviceSinkBuilder::open_default_sink()
            .context("failed to open default audio output")?;
        sink.log_on_drop(false);
        let mixer = sink.mixer().clone();
        Ok(Self {
            _sink: Some(sink),
            mixer,
        })
    }
}

fn play_with_rodio(
    playback: NotificationSoundPlayback,
    settings: NotificationSoundSettings,
    resolver: SoundThemeResolver,
    mixer: rodio::mixer::Mixer,
    job: &Arc<SoundJob>,
) -> anyhow::Result<()> {
    if job.is_cancelled() {
        return Ok(());
    }
    let default_name = settings.default_sound.as_str();
    let requested = playback.requested_sound.as_ref();

    match requested {
        Some(NotificationSound::File(path)) => {
            if try_add_file_to_mixer(path, &mixer, job).is_ok() {
                return Ok(());
            }
            eprintln!(
                "level=warn component=sound event=requested_sound_failed notification_id={} kind=file",
                playback.notification_id
            );
            play_default_sound(&resolver, &settings, &mixer, job)?;
        }
        Some(NotificationSound::Name(name)) => {
            let Ok(resolved) = resolver.resolve(&settings.theme, name, &job.cancelled) else {
                return Ok(());
            };
            if let Some(path) = resolved {
                if try_add_file_to_mixer(&path, &mixer, job).is_ok() {
                    return Ok(());
                }
                eprintln!(
                    "level=warn component=sound event=requested_sound_failed notification_id={} kind=name",
                    playback.notification_id
                );
            } else {
                eprintln!(
                    "level=warn component=sound event=requested_sound_missing notification_id={} kind=name",
                    playback.notification_id
                );
            }
            if name != default_name {
                play_default_sound(&resolver, &settings, &mixer, job)?;
            }
        }
        None => play_default_sound(&resolver, &settings, &mixer, job)?,
    }

    Ok(())
}

fn play_default_sound(
    resolver: &SoundThemeResolver,
    settings: &NotificationSoundSettings,
    mixer: &rodio::mixer::Mixer,
    job: &Arc<SoundJob>,
) -> anyhow::Result<()> {
    if job.is_cancelled() {
        return Ok(());
    }
    let Ok(resolved) = resolver.resolve(&settings.theme, &settings.default_sound, &job.cancelled)
    else {
        return Ok(());
    };
    let Some(path) = resolved else {
        eprintln!("level=warn component=sound event=default_sound_missing");
        return Ok(());
    };

    if try_add_file_to_mixer(&path, mixer, job).is_err() {
        eprintln!("level=warn component=sound event=default_sound_failed");
    }
    Ok(())
}

fn try_add_file_to_mixer(
    path: impl AsRef<Path>,
    mixer: &rodio::mixer::Mixer,
    job: &Arc<SoundJob>,
) -> anyhow::Result<()> {
    if job.is_cancelled() {
        return Ok(());
    }
    let path = path.as_ref();
    let bytes = read_bounded(path, MAX_SOUND_FILE_BYTES)?;
    if job.is_cancelled() {
        return Ok(());
    }
    let mut decoder = rodio::Decoder::try_from(Cursor::new(bytes))
        .map_err(|_| anyhow::anyhow!("invalid sound data"))?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    anyhow::ensure!(
        channels.get() <= 8 && sample_rate.get() <= 192_000,
        "unsupported sound format"
    );
    let sample_limit = (channels.get() as usize * sample_rate.get() as usize * MAX_SOUND_SECONDS)
        .min(MAX_DECODED_SOUND_BYTES / size_of::<rodio::Sample>());
    let sample_limit = sample_limit - sample_limit % channels.get() as usize;
    let mut samples = Vec::with_capacity(sample_limit);
    while samples.len() < sample_limit {
        if job.is_cancelled() {
            return Ok(());
        }
        let Some(sample) = decoder.next() else {
            break;
        };
        anyhow::ensure!(
            decoder.channels() == channels && decoder.sample_rate() == sample_rate,
            "changing sound format is unsupported"
        );
        samples.push(sample);
    }
    // Only bounded PCM reaches the audio callback, never file I/O or decoding.
    let source = rodio::buffer::SamplesBuffer::new(channels, sample_rate, samples);
    mixer.add(AdmittedSound {
        source,
        job: job.clone(),
    });
    Ok(())
}

fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME") {
        Some(value) if !value.is_empty() => dirs.push(PathBuf::from(value)),
        _ => {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(home).join(".local/share"));
            }
        }
    }

    match std::env::var_os("XDG_DATA_DIRS") {
        Some(value) if !value.is_empty() => {
            dirs.extend(std::env::split_paths(&value));
        }
        _ => {
            dirs.push(PathBuf::from("/usr/local/share"));
            dirs.push(PathBuf::from("/usr/share"));
        }
    }

    dirs
}

#[cfg(test)]
mod tests;
