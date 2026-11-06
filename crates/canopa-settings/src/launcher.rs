//! Settings edits launcher-owned preferences without replacing Shell's pins/aliases.
use anyhow::{Result, ensure};
use canopa_launcher::{Catalog, Preferences, clipboard};
use std::path::Path;
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub saved: Preferences,
    pub draft: Preferences,
    pub catalog: Catalog,
    pub loaded: bool,
}
impl Model {
    pub fn load() -> Result<Self> {
        Self::load_at(
            &canopa_launcher::preferences_path()?,
            &canopa_launcher::catalog_path()?,
        )
    }
    pub fn load_at(path: &Path, catalog: &Path) -> Result<Self> {
        let saved = Preferences::read(path)?;
        let catalog = Catalog::read(catalog).unwrap_or_default();
        Ok(Self {
            draft: saved.clone(),
            saved,
            catalog,
            loaded: true,
        })
    }
    pub fn save(&self) -> Result<Self> {
        self.save_at(&canopa_launcher::preferences_path()?)
    }
    pub fn save_at(&self, path: &Path) -> Result<Self> {
        ensure!(self.loaded, "Load launcher preferences before saving");
        let saved = Preferences::update(path, |current| {
            if current.extensions != self.saved.extensions
                || current.web_template != self.saved.web_template
                || current.searches != self.saved.searches
            {
                return Err(std::io::Error::other(
                    "Launcher preferences changed outside this editor; reload before saving",
                ));
            }
            current.extensions = self.draft.extensions.clone();
            current.web_template = self.draft.web_template.clone();
            current.searches = self.draft.searches.clone();
            let mut keywords = std::collections::HashSet::new();
            for ext in &self.catalog.extensions {
                let pref = current.extensions.get(&ext.id);
                if pref.map(|p| p.enabled).unwrap_or(ext.enabled)
                    && !keywords.insert(
                        pref.map(|p| p.keyword.as_str())
                            .unwrap_or(&ext.keyword)
                            .to_owned(),
                    )
                {
                    return Err(std::io::Error::other(
                        "Enabled extension keywords must be unique",
                    ));
                }
            }
            if self
                .catalog
                .extensions
                .iter()
                .find(|e| e.id == "builtin:web")
                .is_none_or(|e| {
                    current
                        .extensions
                        .get(&e.id)
                        .map(|p| p.enabled)
                        .unwrap_or(e.enabled)
                })
            {
                for search in &current.searches {
                    if !keywords.insert(search.keyword.clone()) {
                        return Err(std::io::Error::other("Search keywords must be unique"));
                    }
                }
            }
            Ok(())
        })?;
        Ok(Self {
            draft: saved.clone(),
            saved,
            catalog: self.catalog.clone(),
            loaded: true,
        })
    }
    pub fn extension(&mut self, id: &str) -> Option<&mut canopa_launcher::ExtensionPreference> {
        let ext = self.catalog.extensions.iter().find(|e| e.id == id)?;
        Some(self.draft.extensions.entry(id.into()).or_insert_with(|| {
            canopa_launcher::ExtensionPreference {
                enabled: ext.enabled,
                keyword: ext.keyword.clone(),
            }
        }))
    }
}
#[derive(Clone, Debug)]
pub struct ClipboardEditor {
    saved: clipboard::Options,
    pub recording: bool,
    pub images: bool,
    pub limit: String,
}

impl ClipboardEditor {
    fn from_options(options: clipboard::Options) -> Self {
        Self {
            saved: options,
            recording: options.recording,
            images: options.images,
            limit: options.max_entries.to_string(),
        }
    }

    pub fn load() -> Result<Self> {
        let path = clipboard::path().map_err(anyhow::Error::msg)?;
        let saved = clipboard::Saved::load(&path).map_err(anyhow::Error::msg)?;
        Ok(Self::from_options(saved.options()))
    }

    pub fn save(&self) -> Result<Self> {
        self.save_at(&clipboard::path().map_err(anyhow::Error::msg)?)
    }

    fn save_at(&self, path: &Path) -> Result<Self> {
        let limit = self.limit.parse().unwrap_or_default();
        ensure!(
            (1..=clipboard::HISTORY_LIMIT).contains(&limit),
            "Enter a history limit from 1 to {}",
            clipboard::HISTORY_LIMIT,
        );
        let options = clipboard::Options {
            recording: self.recording,
            images: self.images,
            max_entries: limit,
        };
        let saved = clipboard::Saved::update(path, |current| {
            if current.options() != self.saved {
                return Err(
                    "Clipboard preferences changed. Reload saved settings before saving.".into(),
                );
            }
            current.set_options(options);
            Ok(())
        })
        .map_err(anyhow::Error::msg)?;
        Ok(Self::from_options(saved.options()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_edits_preserve_new_snippets_and_refuse_stale_options() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("clipboard.json");
        let mut editor = ClipboardEditor::from_options(clipboard::Options::default());
        clipboard::Saved::update(&path, |saved| {
            saved.edit(clipboard::Edit::Add {
                name: "New snippet".into(),
                text: clipboard::Text::new("body".into()).unwrap(),
            })
        })
        .unwrap();

        editor.limit = "0".into();
        assert!(editor.save_at(&path).is_err());
        editor.limit = "4".into();
        editor.recording = true;
        editor.images = true;
        assert!(editor.save_at(&path).is_ok());
        let saved = clipboard::Saved::load(&path).unwrap();
        assert_eq!(saved.snippets[0].name, "New snippet");
        assert_eq!(saved.options().max_entries, 4);
        assert!(saved.options().recording && saved.options().images);

        editor.limit = "5".into();
        assert!(editor.save_at(&path).is_err());
        assert_eq!(clipboard::Saved::load(&path).unwrap(), saved);
    }

    #[test]
    fn saved_edits_preserve_shell_shortcuts_and_refuse_stale_editor_values() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("launcher.json");
        let mut model = Model::load_at(&path, &root.path().join("catalog.json")).unwrap();
        model.draft.web_template = "https://example.test/?q={query}".into();
        let shortcut = canopa_launcher::Shortcut {
            target: canopa_launcher::Target::Application {
                id: "editor.desktop".into(),
            },
            label: "Editor".into(),
            alias: "work".into(),
            pinned: false,
            icon: None,
        };
        Preferences::update(&path, |p| p.edit_shortcut(shortcut, true)).unwrap();
        let saved = model.save_at(&path).unwrap();
        assert!(saved.saved.shortcuts[0].pinned);
        assert!(model.save_at(&path).is_err());
    }
}
