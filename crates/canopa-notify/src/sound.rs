use crate::{
    DEFAULT_NOTIFICATION_SOUND_THEME, NotificationSound, NotificationSoundPlayback,
    NotificationSoundSettings,
};
use anyhow::Context;
use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SOUND_EXTENSIONS: [&str; 5] = ["oga", "ogg", "wav", "flac", "mp3"];
const SOUND_PROFILES: [&str; 2] = ["stereo", "mono"];

pub trait SoundPresenter: Send + Sync {
    fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()>;
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
}

impl RodioSoundPresenter {
    pub fn start() -> Self {
        Self::with_data_dirs(NotificationSoundSettings::default(), xdg_data_dirs())
    }

    fn with_data_dirs(settings: NotificationSoundSettings, data_dirs: Vec<PathBuf>) -> Self {
        let presenter = Self {
            state: Arc::new(Mutex::new(RodioSoundState {
                settings: NotificationSoundSettings {
                    enabled: false,
                    ..settings.clone()
                },
                backend: None,
                data_dirs,
            })),
        };
        if let Err(error) = presenter.apply_sound_settings(settings) {
            eprintln!("level=warn component=sound event=configuration_failed error={error:#}");
        }
        presenter
    }
}

impl SoundPresenter for RodioSoundPresenter {
    fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let was_enabled = state.settings.enabled;
        let enabled_changed = was_enabled != settings.enabled;
        state.settings = settings;

        if !state.settings.enabled {
            state.backend = None;
            return Ok(());
        }

        if state.backend.is_none() {
            match RodioBackend::open() {
                Ok(backend) => state.backend = Some(backend),
                Err(error) => {
                    eprintln!(
                        "level=warn component=sound event=backend_unavailable error={error:#}"
                    );
                }
            }
        } else if enabled_changed {
            eprintln!("level=info component=sound event=backend_enabled");
        }

        Ok(())
    }

    fn play_notification_sound(&self, playback: NotificationSoundPlayback) -> anyhow::Result<()> {
        let (settings, data_dirs, mixer) = {
            let state = self.state.lock().unwrap();
            if !state.settings.enabled {
                return Ok(());
            }
            let Some(backend) = &state.backend else {
                return Ok(());
            };
            (
                state.settings.clone(),
                state.data_dirs.clone(),
                backend.mixer.clone(),
            )
        };

        std::thread::Builder::new()
            .name("canopa-notify-sound".to_string())
            .spawn(move || {
                if let Err(error) = play_with_rodio(playback, settings, data_dirs, mixer) {
                    eprintln!("level=warn component=sound event=playback_failed error={error:#}");
                }
            })?;
        Ok(())
    }

    fn stop_all_sounds(&self) {
        let mut state = self.state.lock().unwrap();
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
    data_dirs: Vec<PathBuf>,
}

struct RodioBackend {
    _sink: rodio::MixerDeviceSink,
    mixer: rodio::mixer::Mixer,
}

impl RodioBackend {
    fn open() -> anyhow::Result<Self> {
        let mut sink = rodio::DeviceSinkBuilder::open_default_sink()
            .context("failed to open default audio output")?;
        sink.log_on_drop(false);
        let mixer = sink.mixer().clone();
        Ok(Self { _sink: sink, mixer })
    }
}

fn play_with_rodio(
    playback: NotificationSoundPlayback,
    settings: NotificationSoundSettings,
    data_dirs: Vec<PathBuf>,
    mixer: rodio::mixer::Mixer,
) -> anyhow::Result<()> {
    let resolver = SoundThemeResolver::new(data_dirs);
    let default_name = settings.default_sound.as_str();
    let requested = playback.requested_sound.as_ref();

    match requested {
        Some(NotificationSound::File(path)) => {
            if try_add_file_to_mixer(path, &mixer).is_ok() {
                return Ok(());
            }
            eprintln!(
                "level=warn component=sound event=requested_sound_failed notification_id={} kind=file path={:?}",
                playback.notification_id, path
            );
            play_default_sound(&resolver, &settings, &mixer)?;
        }
        Some(NotificationSound::Name(name)) => {
            if let Some(path) = resolver.resolve(&settings.theme, name) {
                if try_add_file_to_mixer(&path, &mixer).is_ok() {
                    return Ok(());
                }
                eprintln!(
                    "level=warn component=sound event=requested_sound_failed notification_id={} kind=name name={:?} path={}",
                    playback.notification_id,
                    name,
                    path.display()
                );
            } else {
                eprintln!(
                    "level=warn component=sound event=requested_sound_missing notification_id={} name={:?} theme={:?}",
                    playback.notification_id, name, settings.theme
                );
            }
            if name != default_name {
                play_default_sound(&resolver, &settings, &mixer)?;
            }
        }
        None => play_default_sound(&resolver, &settings, &mixer)?,
    }

    Ok(())
}

fn play_default_sound(
    resolver: &SoundThemeResolver,
    settings: &NotificationSoundSettings,
    mixer: &rodio::mixer::Mixer,
) -> anyhow::Result<()> {
    let Some(path) = resolver.resolve(&settings.theme, &settings.default_sound) else {
        eprintln!(
            "level=warn component=sound event=default_sound_missing name={:?} theme={:?}",
            settings.default_sound, settings.theme
        );
        return Ok(());
    };

    if let Err(error) = try_add_file_to_mixer(&path, mixer) {
        eprintln!(
            "level=warn component=sound event=default_sound_failed name={:?} path={} error={error:#}",
            settings.default_sound,
            path.display()
        );
    }
    Ok(())
}

fn try_add_file_to_mixer(
    path: impl AsRef<Path>,
    mixer: &rodio::mixer::Mixer,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    let file = File::open(path)
        .with_context(|| format!("failed to open notification sound {}", path.display()))?;
    let source = rodio::Decoder::try_from(file)
        .with_context(|| format!("failed to decode notification sound {}", path.display()))?;
    mixer.add(source);
    Ok(())
}

#[derive(Clone, Debug)]
struct SoundThemeResolver {
    data_dirs: Vec<PathBuf>,
}

impl SoundThemeResolver {
    fn new(data_dirs: Vec<PathBuf>) -> Self {
        Self { data_dirs }
    }

    fn resolve(&self, theme: &str, sound_name: &str) -> Option<PathBuf> {
        let mut seen = HashSet::new();
        self.resolve_theme(theme, sound_name, &mut seen)
            .or_else(|| {
                (theme != DEFAULT_NOTIFICATION_SOUND_THEME).then(|| {
                    self.resolve_theme(DEFAULT_NOTIFICATION_SOUND_THEME, sound_name, &mut seen)
                })?
            })
    }

    fn resolve_theme(
        &self,
        theme: &str,
        sound_name: &str,
        seen: &mut HashSet<String>,
    ) -> Option<PathBuf> {
        if !seen.insert(theme.to_string()) {
            return None;
        }

        for theme_dir in self.theme_dirs(theme) {
            if let Some(path) = find_profile_sound(&theme_dir, sound_name) {
                return Some(path);
            }
        }

        for theme_dir in self.theme_dirs(theme) {
            if let Some(path) = find_any_sound(&theme_dir, sound_name) {
                return Some(path);
            }
        }

        for inherited in self.inherited_themes(theme) {
            if let Some(path) = self.resolve_theme(&inherited, sound_name, seen) {
                return Some(path);
            }
        }

        None
    }

    fn theme_dirs(&self, theme: &str) -> Vec<PathBuf> {
        self.data_dirs
            .iter()
            .map(|data_dir| data_dir.join("sounds").join(theme))
            .filter(|path| path.is_dir())
            .collect()
    }

    fn inherited_themes(&self, theme: &str) -> Vec<String> {
        let mut inherited = Vec::new();
        for theme_dir in self.theme_dirs(theme) {
            let index = theme_dir.join("index.theme");
            let Ok(contents) = std::fs::read_to_string(index) else {
                continue;
            };
            inherited.extend(parse_inherits(&contents));
        }
        inherited
    }
}

fn find_profile_sound(theme_dir: &Path, sound_name: &str) -> Option<PathBuf> {
    for profile in SOUND_PROFILES {
        for extension in SOUND_EXTENSIONS {
            let path = theme_dir
                .join(profile)
                .join(format!("{sound_name}.{extension}"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn find_any_sound(theme_dir: &Path, sound_name: &str) -> Option<PathBuf> {
    let mut dirs = vec![theme_dir.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            if path.file_stem().and_then(|stem| stem.to_str()) != Some(sound_name) {
                continue;
            }
            let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
                continue;
            };
            if SOUND_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            {
                return Some(path);
            }
        }
    }
    None
}

fn parse_inherits(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.strip_prefix("Inherits="))
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|theme| !theme.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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
mod tests {
    use super::*;

    #[test]
    fn sound_theme_resolver_prefers_profile_directories_and_extension_order() {
        let temp = tempfile::tempdir().unwrap();
        let theme = temp.path().join("sounds/freedesktop");
        std::fs::create_dir_all(theme.join("mono")).unwrap();
        std::fs::create_dir_all(theme.join("stereo")).unwrap();
        std::fs::write(theme.join("mono/message-new-instant.wav"), "").unwrap();
        std::fs::write(theme.join("stereo/message-new-instant.oga"), "").unwrap();

        let resolver = SoundThemeResolver::new(vec![temp.path().to_path_buf()]);

        assert_eq!(
            resolver
                .resolve("freedesktop", "message-new-instant")
                .unwrap(),
            theme.join("stereo/message-new-instant.oga")
        );
    }

    #[test]
    fn sound_theme_resolver_follows_inherits_and_freedesktop_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let custom = temp.path().join("sounds/custom");
        let parent = temp.path().join("sounds/parent/stereo");
        let freedesktop = temp.path().join("sounds/freedesktop/stereo");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(&freedesktop).unwrap();
        std::fs::write(
            custom.join("index.theme"),
            "[Sound Theme]\nInherits=parent\n",
        )
        .unwrap();
        std::fs::write(parent.join("complete.oga"), "").unwrap();
        std::fs::write(freedesktop.join("message-new-instant.oga"), "").unwrap();

        let resolver = SoundThemeResolver::new(vec![temp.path().to_path_buf()]);

        assert_eq!(
            resolver.resolve("custom", "complete").unwrap(),
            parent.join("complete.oga")
        );
        assert_eq!(
            resolver.resolve("custom", "message-new-instant").unwrap(),
            freedesktop.join("message-new-instant.oga")
        );
    }
}
