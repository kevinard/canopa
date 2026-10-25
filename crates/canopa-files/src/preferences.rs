//! Global defaults come from Canopa configuration; folder overrides are application state.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

const MAXIMUM_STATE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    #[default]
    Columns,
    List,
    Icons,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortField {
    #[default]
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Detail {
    Size,
    Modified,
    Kind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewSettings {
    pub mode: ViewMode,
    pub sort: SortField,
    pub descending: bool,
    pub details: Vec<Detail>,
    pub column_width: u16,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            mode: ViewMode::Columns,
            sort: SortField::Name,
            descending: false,
            details: vec![Detail::Size, Detail::Modified],
            column_width: 240,
        }
    }
}

impl ViewSettings {
    pub fn validate(&self) -> io::Result<()> {
        if !(160..=640).contains(&self.column_width)
            || self.details.iter().collect::<BTreeSet<_>>().len() != self.details.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid folder view settings",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct PreferenceStore {
    defaults: ViewSettings,
    folders: BTreeMap<PathBuf, ViewSettings>,
    overrides: BTreeMap<PathBuf, ViewOverride>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ViewOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<ViewMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort: Option<SortField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    descending: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<Detail>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column_width: Option<u16>,
}
impl ViewOverride {
    fn resolve(&self, defaults: &ViewSettings) -> ViewSettings {
        ViewSettings {
            mode: self.mode.unwrap_or(defaults.mode),
            sort: self.sort.unwrap_or(defaults.sort),
            descending: self.descending.unwrap_or(defaults.descending),
            details: self
                .details
                .clone()
                .unwrap_or_else(|| defaults.details.clone()),
            column_width: self.column_width.unwrap_or(defaults.column_width),
        }
    }
    fn full(view: &ViewSettings) -> Self {
        Self {
            mode: Some(view.mode),
            sort: Some(view.sort),
            descending: Some(view.descending),
            details: Some(view.details.clone()),
            column_width: Some(view.column_width),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPreferences {
    version: u32,
    folders: Vec<StoredFolder>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFolder {
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    view: ViewOverride,
}

impl PreferenceStore {
    pub fn new(defaults: ViewSettings) -> Self {
        Self {
            defaults,
            folders: BTreeMap::new(),
            overrides: BTreeMap::new(),
        }
    }

    pub fn defaults(&self) -> &ViewSettings {
        &self.defaults
    }

    pub fn set_defaults(&mut self, defaults: ViewSettings) -> io::Result<()> {
        defaults.validate()?;
        self.defaults = defaults;
        for (path, overrides) in &self.overrides {
            self.folders
                .insert(path.clone(), overrides.resolve(&self.defaults));
        }
        Ok(())
    }

    pub fn for_folder(&self, folder: &Path) -> &ViewSettings {
        self.folders.get(folder).unwrap_or(&self.defaults)
    }

    pub fn folder_override(&self, folder: &Path) -> Option<&ViewSettings> {
        self.folders.get(folder)
    }

    pub fn set_folder(&mut self, folder: &Path, view: ViewSettings) -> io::Result<()> {
        view.validate()?;
        if !folder.is_absolute() || folder.as_os_str().as_encoded_bytes().contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "folder preferences require an absolute path",
            ));
        }
        self.overrides
            .insert(folder.to_owned(), ViewOverride::full(&view));
        self.folders.insert(folder.to_owned(), view);
        Ok(())
    }

    /// Remember only deliberate differences from this folder's effective view.
    pub fn remember_changes(&mut self, folder: &Path, view: ViewSettings) -> io::Result<()> {
        view.validate()?;
        if !folder.is_absolute() || folder.as_os_str().as_encoded_bytes().contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "folder preferences require an absolute path",
            ));
        }
        let old = self.for_folder(folder);
        if old == &view {
            return Ok(());
        }
        let mut changes = self.overrides.get(folder).cloned().unwrap_or_default();
        if old.mode != view.mode {
            changes.mode = Some(view.mode);
        }
        if old.sort != view.sort {
            changes.sort = Some(view.sort);
        }
        if old.descending != view.descending {
            changes.descending = Some(view.descending);
        }
        if old.details != view.details {
            changes.details = Some(view.details.clone());
        }
        if old.column_width != view.column_width {
            changes.column_width = Some(view.column_width);
        }
        self.overrides.insert(folder.to_owned(), changes);
        self.folders.insert(folder.to_owned(), view);
        Ok(())
    }

    pub fn use_defaults(&mut self, folder: &Path) {
        self.folders.remove(folder);
        self.overrides.remove(folder);
    }

    pub fn load(path: &Path, defaults: ViewSettings) -> io::Result<Self> {
        defaults.validate()?;
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::new(defaults));
            }
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let stored: StoredPreferences = crate::state_io::read_json(path, MAXIMUM_STATE_BYTES)?;
        if ![1, 2].contains(&stored.version) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported view-state version",
            ));
        }
        let mut preferences = Self::new(defaults);
        let legacy_defaults = ViewSettings::default();
        for folder in stored.folders {
            if preferences.folders.contains_key(&folder.path) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate folder override",
                ));
            }
            let view = folder.view.resolve(if stored.version == 1 {
                &legacy_defaults
            } else {
                &preferences.defaults
            });
            preferences.set_folder(&folder.path, view)?;
            if stored.version == 2 {
                preferences.overrides.insert(folder.path, folder.view);
            }
        }
        Ok(preferences)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let stored = StoredPreferences {
            version: 2,
            folders: self
                .overrides
                .iter()
                .map(|(path, view)| StoredFolder {
                    path: path.clone(),
                    view: view.clone(),
                })
                .collect(),
        };
        crate::state_io::write_json(path, &stored, MAXIMUM_STATE_BYTES)
    }
}
