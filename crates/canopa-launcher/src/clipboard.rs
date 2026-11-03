//! Private clipboard preferences and saved text snippets shared by Shell and Settings.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
};

pub const HISTORY_LIMIT: usize = 64;
const SNIPPET_LIMIT: usize = 64;
pub const TEXT_BYTES: usize = 16 * 1024;
const STORE_BYTES: usize = SNIPPET_LIMIT * (6 * (TEXT_BYTES + 256) + 128) + 256;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Text(String);

impl std::fmt::Debug for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClipboardText")
    }
}

impl Text {
    pub fn new(text: String) -> Option<Self> {
        let text = Self(text);
        text.valid().then_some(text)
    }

    pub fn valid(&self) -> bool {
        self.0.len() <= TEXT_BYTES && !self.0.contains('\0') && !self.0.trim().is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn preview(&self) -> String {
        self.0
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(140)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    pub id: u64,
    pub name: String,
    pub text: Text,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    version: u8,
    recording: bool,
    #[serde(default)]
    images: bool,
    #[serde(default = "default_entries")]
    max_entries: usize,
    pub snippets: Vec<Snippet>,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            recording: false,
            images: false,
            max_entries: default_entries(),
            snippets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Recording(bool),
    Add { name: String, text: Text },
    Rename { id: u64, name: String },
    Delete(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub recording: bool,
    pub images: bool,
    pub max_entries: usize,
}

const fn default_entries() -> usize {
    HISTORY_LIMIT
}

impl Default for Options {
    fn default() -> Self {
        Self {
            recording: false,
            images: false,
            max_entries: default_entries(),
        }
    }
}

impl Saved {
    pub fn options(&self) -> Options {
        Options {
            recording: self.recording,
            images: self.images,
            max_entries: self.max_entries,
        }
    }

    pub fn set_options(&mut self, options: Options) {
        self.recording = options.recording;
        self.images = options.images;
        self.max_entries = options.max_entries;
    }

    fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        if self.version != 1
            || self.snippets.len() > SNIPPET_LIMIT
            || !(1..=HISTORY_LIMIT).contains(&self.max_entries)
            || self.snippets.iter().any(|entry| {
                entry.id == 0
                    || !ids.insert(entry.id)
                    || !names.insert(entry.name.to_lowercase())
                    || !valid_name(&entry.name)
                    || !entry.text.valid()
            })
        {
            return Err("Clipboard preferences or snippets are invalid".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match canopa_file::private_state::read(path, STORE_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err("Could not read saved clipboard preferences and snippets".into()),
        };
        let state: Self =
            serde_json::from_slice(&bytes).map_err(|_| "Saved clipboard data is invalid")?;
        state.validate()?;
        Ok(state)
    }

    /// Serialize cooperating writers and merge edits with the current saved snippets.
    pub fn update(
        path: &Path,
        edit: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<Self, String> {
        let _lock = super::lock_preferences(path).map_err(|error| error.to_string())?;
        let mut state = Self::load(path)?;
        edit(&mut state)?;
        state.write(path)?;
        Ok(state)
    }

    fn write(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "Could not encode saved snippets")?;
        canopa_file::private_state::write(path, &bytes)
            .map_err(|_| "Could not save clipboard preferences and snippets".into())
    }

    pub fn edit(&mut self, edit: Edit) -> Result<(), String> {
        let name = match &edit {
            Edit::Add { name, .. } => Some((name, None)),
            Edit::Rename { id, name } => Some((name, Some(*id))),
            _ => None,
        };
        if let Some((name, id)) = name {
            if !valid_name(name) {
                return Err("Enter a snippet name up to 256 bytes".into());
            }
            if self.snippets.iter().any(|entry| {
                Some(entry.id) != id && entry.name.to_lowercase() == name.to_lowercase()
            }) {
                return Err("A snippet with that name already exists".into());
            }
        }
        match edit {
            Edit::Recording(enabled) => self.recording = enabled,
            Edit::Add { name, text } => {
                if self.snippets.len() == SNIPPET_LIMIT {
                    return Err("Delete a snippet before saving another".into());
                }
                let id = self
                    .snippets
                    .iter()
                    .map(|entry| entry.id)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or("No snippet IDs remain")?;
                self.snippets.insert(0, Snippet { id, name, text });
            }
            Edit::Rename { id, name } => {
                self.snippets
                    .iter_mut()
                    .find(|entry| entry.id == id)
                    .ok_or("That snippet is no longer available")?
                    .name = name;
            }
            Edit::Delete(id) => {
                self.snippets.retain(|entry| entry.id != id);
            }
        }
        Ok(())
    }
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name == name.trim()
        && !name.chars().any(char::is_control)
}

pub fn path() -> Result<PathBuf, String> {
    super::xdg("XDG_STATE_HOME", ".local/state")
        .map(|root| root.join("canopa/shell/clipboard.json"))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> Text {
        Text::new(value.to_owned()).unwrap()
    }

    #[test]
    fn legacy_store_defaults_new_fields_and_preserves_snippets_across_writers() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("clipboard.json");
        canopa_file::private_state::write(
            &path,
            br#"{"version":1,"recording":true,"snippets":[{"id":1,"name":"Existing","text":"old body"}]}"#,
        ).unwrap();
        let legacy = Saved::load(&path).unwrap();
        assert_eq!(
            legacy.options(),
            Options {
                recording: true,
                ..Options::default()
            }
        );
        let configured = Saved::update(&path, |state| {
            state.set_options(Options {
                recording: true,
                images: true,
                max_entries: 3,
            });
            Ok(())
        })
        .unwrap();
        let renamed = Saved::update(&path, |state| {
            state.edit(Edit::Rename {
                id: 1,
                name: "New name".into(),
            })
        })
        .unwrap();
        assert_eq!(renamed.options(), configured.options());
        assert_eq!(renamed.snippets[0].text, text("old body"));
        assert_eq!(renamed.snippets[0].name, "New name");
    }

    #[test]
    fn snippets_preserve_multiline_text_and_reject_conflicting_names() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("clipboard.json");
        let mut state = Saved::load(&path).unwrap();
        assert!(!state.options().recording);
        let body = text("first line\n  second line\n");
        state
            .edit(Edit::Add {
                name: "Invoice".into(),
                text: body.clone(),
            })
            .unwrap();
        assert!(
            state
                .edit(Edit::Add {
                    name: "invoice".into(),
                    text: text("another copy")
                })
                .is_err()
        );
        let id = state.snippets[0].id;
        state
            .edit(Edit::Rename {
                id,
                name: "Footer".into(),
            })
            .unwrap();
        state.write(&path).unwrap();
        let state = Saved::load(&path).unwrap();
        assert_eq!(state.snippets[0].text, body);
        assert_eq!(state.snippets[0].name, "Footer");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    }
}
