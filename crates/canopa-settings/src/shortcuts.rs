//! Edit the packaged keyboard actions in the Canopa session. Niri validates
//! the whole candidate, including user includes, before the editor publishes it.
use crate::{niri, session::Paths, storage::Snapshot};
use anyhow::{Context, Result, ensure};
use kdl::{KdlDocument, KdlNode};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

const DEFAULTS: &str = include_str!("../../../packaging/niri/defaults.kdl");
const MAX_INCLUDES: usize = 32;

#[derive(Clone, Debug)]
pub struct Shortcut {
    pub label: String,
    pub key: String,
    pub saved_key: String,
    pub default_key: String,
}

#[derive(Clone, Debug)]
struct Include {
    path: PathBuf,
    text: Option<String>,
}

impl Include {
    fn load(parent: &Path, node: &KdlNode) -> Result<Self> {
        let value = node
            .get(0)
            .and_then(|e| e.as_string())
            .context("Invalid Niri include path")?;
        let path = if let Some(relative) = value.strip_prefix("~/") {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .context("Home directory is unavailable")?
                .join(relative)
        } else {
            parent.join(value)
        };
        let optional = node.get("optional").and_then(|e| e.as_bool()) == Some(true);
        Ok(Self {
            text: Self::read(&path, optional)?,
            path,
        })
    }

    fn read(path: &Path, optional: bool) -> Result<Option<String>> {
        if optional
            && matches!(std::fs::metadata(path), Err(err) if err.kind() == std::io::ErrorKind::NotFound)
        {
            return Ok(None);
        }
        let bytes = canopa_file::read_canonical_bounded(path, crate::storage::MAX_BYTES)
            .with_context(|| format!("Could not read Niri include {}", path.display()))?;
        Ok(Some(String::from_utf8(bytes)?))
    }

    fn unchanged(&self) -> Result<()> {
        ensure!(
            Self::read(&self.path, self.text.is_none())? == self.text,
            "Niri includes changed outside this editor. Reload before saving."
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Editor {
    source: Snapshot,
    document: KdlDocument,
    managed_index: usize,
    includes: Vec<Include>,
    external_keys: HashSet<String>,
    pub rows: Vec<Shortcut>,
}

impl Editor {
    pub fn load(paths: &Paths) -> Result<Self> {
        let source = Snapshot::load(paths.niri())?;
        ensure!(
            !source.text.is_empty(),
            "Log into the Canopa session to edit keyboard shortcuts."
        );
        Self::parse(source)
    }

    fn parse(source: Snapshot) -> Result<Self> {
        let defaults = KdlDocument::parse_v1(DEFAULTS)?;
        let mut document = KdlDocument::parse_v1(&source.text)?;
        let mut includes = Vec::new();
        let mut external_keys = HashSet::new();
        let parent = source
            .path
            .parent()
            .context("Niri configuration needs a directory")?;
        let root_binds = document
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, node)| node.name().value() == "binds")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        ensure!(
            root_binds.len() <= 1,
            "Multiple top-level binding blocks are read-only."
        );
        let mut inherited = None;
        let mut include_budget = crate::storage::MAX_BYTES * 2;
        for (index, node) in document.nodes().iter().enumerate() {
            if node.name().value() == "include" {
                let include = Include::load(parent, node)?;
                if include.text.as_deref() == Some(DEFAULTS) {
                    ensure!(
                        root_binds.is_empty() && inherited.is_none(),
                        "Ambiguous Canopa binding source. Edit this configuration in Niri's KDL files."
                    );
                    inherited = Some(index);
                } else {
                    collect_included_keys(
                        &include,
                        &mut includes,
                        &mut external_keys,
                        &mut include_budget,
                    )?;
                }
                includes.push(include);
                ensure!(
                    includes.len() <= MAX_INCLUDES,
                    "Too many Niri includes for the shortcut editor."
                );
            }
        }
        let managed_index = if let Some(index) = inherited {
            // Niri merges includes by key and cannot remove an inherited key.
            // Materialize the shipped block in its original position on first
            // save so changing a key really releases the old binding.
            let mut nodes = defaults.nodes().to_vec();
            let first = nodes.first_mut().context("Missing Canopa defaults")?;
            if let (Some(origin), Some(format)) =
                (document.nodes()[index].format(), first.format_mut())
            {
                format.leading.insert_str(0, &origin.leading);
            }
            let offset = nodes
                .iter()
                .position(|node| node.name().value() == "binds")
                .context("Missing Canopa bindings")?;
            document.nodes_mut().splice(index..=index, nodes);
            index + offset
        } else {
            *root_binds.first().context(
                "No editable Canopa binding block. Custom Niri bindings are edited in KDL.",
            )?
        };
        let shipped = bindings(defaults.get("binds").context("Missing Canopa bindings")?)?;
        let current = bindings(&document.nodes()[managed_index])?;
        ensure!(
            shipped.len() == current.len(),
            "Custom top-level binding blocks are read-only."
        );
        let mut rows = Vec::with_capacity(current.len());
        for (entry, default) in current.iter().zip(shipped) {
            ensure!(
                same_action(entry, default),
                "Custom top-level binding blocks are read-only."
            );
            rows.push(Shortcut {
                label: label(default)?,
                key: entry.name().value().to_owned(),
                saved_key: entry.name().value().to_owned(),
                default_key: default.name().value().to_owned(),
            });
        }
        Ok(Self {
            source,
            document,
            managed_index,
            includes,
            external_keys,
            rows,
        })
    }

    fn candidate(&self) -> Result<String> {
        let mut document = self.document.clone();
        let mut keys = HashSet::new();
        let nodes = document.nodes_mut()[self.managed_index]
            .children_mut()
            .as_mut()
            .context("Missing bindings")?
            .nodes_mut();
        ensure!(
            nodes.len() == self.rows.len(),
            "Shortcut list changed while editing."
        );
        for (node, shortcut) in nodes.iter_mut().zip(&self.rows) {
            let key = shortcut.key.trim();
            let normalized = normalize(key)
                .with_context(|| format!("{} needs a valid shortcut", shortcut.label))?;
            ensure!(
                keys.insert(normalized.clone()),
                "{key} is assigned to more than one shortcut."
            );
            if node.name().value() != key {
                ensure!(
                    normalize(node.name().value()).as_ref() == Some(&normalized)
                        || !self.external_keys.contains(&normalized),
                    "{key} is already assigned in a Niri include."
                );
                node.set_name(key);
            }
        }
        document.ensure_v1();
        Ok(document.to_string())
    }

    pub fn save(&self, paths: &Paths) -> Result<Self> {
        ensure!(
            paths.active() && self.source.path == paths.niri(),
            "Log into the Canopa session to edit keyboard shortcuts."
        );
        let text = self.candidate()?;
        self.source.unchanged()?;
        if !self.dirty() {
            return Ok(self.clone());
        }
        let saved = niri::save_main(&self.source, &text, || {
            for include in &self.includes {
                include.unchanged()?;
            }
            Ok(())
        })?;
        Self::parse(saved)
    }

    pub fn dirty(&self) -> bool {
        self.rows.iter().any(|row| row.key.trim() != row.saved_key)
    }
}

fn bindings(node: &KdlNode) -> Result<&[KdlNode]> {
    ensure!(
        node.entries().is_empty() && node.ty().is_none(),
        "Unsupported bindings block"
    );
    Ok(node.children().context("Missing bindings")?.nodes())
}

fn same_action(node: &KdlNode, default: &KdlNode) -> bool {
    let signature = |node: &KdlNode| {
        let mut node = node.clone();
        node.set_name("action");
        node.clear_format_recursive();
        node.to_string()
    };
    signature(node) == signature(default)
}

fn label(node: &KdlNode) -> Result<String> {
    let action = node
        .children()
        .and_then(|children| children.nodes().first())
        .context("Missing shortcut action")?;
    let label = match node.name().value() {
        "Mod+Return" => "Open terminal",
        "Mod+Space" => "Open Applications",
        "Mod+E" => "Open Files",
        "Mod+Comma" => "Open Settings",
        "Mod+L" => "Lock screen",
        "Mod+Q" => "Close window",
        "Mod+F" => "Maximize column",
        "Mod+Shift+F" => "Toggle fullscreen",
        "Mod+V" => "Toggle floating window",
        "Mod+O" => "Toggle overview",
        "Mod+Shift+E" => "Log out",
        "Print" => "Select screenshot region",
        "Ctrl+Print" => "Capture screen",
        "Alt+Print" => "Capture window",
        "XF86AudioRaiseVolume" => "Raise volume",
        "XF86AudioLowerVolume" => "Lower volume",
        "XF86AudioMute" => "Mute speakers",
        "XF86AudioMicMute" => "Mute microphone",
        _ => {
            let mut label = action.name().value().replace('-', " ");
            if let Some(initial) = label.get_mut(..1) {
                initial.make_ascii_uppercase();
            }
            return Ok(label);
        }
    };
    Ok(label.into())
}

fn normalize(key: &str) -> Option<String> {
    if key.is_empty() || key.len() > 100 {
        return None;
    }
    let (trigger, modifiers) = {
        let mut parts = key.split('+').map(str::trim);
        (parts.next_back()?, parts)
    };
    let trigger =
        xkbcommon::xkb::keysym_from_name(trigger, xkbcommon::xkb::KEYSYM_CASE_INSENSITIVE);
    if trigger.raw() == 0 {
        return None;
    }
    let mut set = Vec::new();
    for modifier in modifiers {
        // The supported physical Canopa session uses Super as Niri's Mod key.
        let modifier = match modifier.to_ascii_lowercase().as_str() {
            "mod" | "super" | "win" => "super",
            "ctrl" | "control" => "ctrl",
            "alt" => "alt",
            "shift" => "shift",
            "mod5" | "iso_level3_shift" => "mod5",
            "mod3" | "iso_level5_shift" => "mod3",
            _ => return None,
        };
        if set.contains(&modifier) {
            return None;
        }
        set.push(modifier);
    }
    set.sort_unstable();
    Some(format!("{}/{}", set.join("+"), trigger.raw()))
}

fn collect_included_keys(
    include: &Include,
    collected: &mut Vec<Include>,
    keys: &mut HashSet<String>,
    budget: &mut usize,
) -> Result<()> {
    let Some(text) = &include.text else {
        return Ok(());
    };
    *budget = budget
        .checked_sub(text.len())
        .context("Niri includes exceed the editor's size limit")?;
    let document = KdlDocument::parse_v1(text)?;
    let parent = include
        .path
        .parent()
        .context("Niri include needs a parent directory")?;
    for node in document.nodes() {
        match node.name().value() {
            "binds" => {
                keys.extend(
                    bindings(node)?
                        .iter()
                        .filter_map(|node| normalize(node.name().value())),
                );
            }
            "include" => {
                ensure!(
                    collected.len() < MAX_INCLUDES,
                    "Too many or recursive Niri includes."
                );
                let nested = Include::load(parent, node)?;
                collected.push(nested.clone());
                collect_included_keys(&nested, collected, keys, budget)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_releases_inherited_key_and_preserves_custom_sources() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: root.path().to_owned(),
        };
        paths.initialize(None).unwrap();
        // Use the checked-in defaults even if this test host has installed a different version.
        let bundled = root.path().join("default-fixture.kdl");
        std::fs::write(&bundled, DEFAULTS).unwrap();
        let before = "include \"default-fixture.kdl\"\n// retain this include\ninclude \"niri-custom.kdl\"\n";
        std::fs::write(paths.niri(), before).unwrap();
        std::fs::write(
            root.path().join("niri-custom.kdl"),
            "binds { Super+X { close-window; }; }\n",
        )
        .unwrap();
        let mut editor = Editor::load(&paths).unwrap();
        let index = editor
            .rows
            .iter()
            .position(|row| row.default_key == "Mod+Space")
            .unwrap();
        editor.rows[index].key = "Mod+X".into();
        assert!(
            editor
                .candidate()
                .unwrap_err()
                .to_string()
                .contains("already assigned")
        );
        editor.rows[index].key = "Mod+E".into();
        assert!(
            editor
                .candidate()
                .unwrap_err()
                .to_string()
                .contains("more than one")
        );
        editor.rows[index].key = "Mod+Shift+Space".into();
        let text = editor.candidate().unwrap();
        assert!(text.contains("Mod+Shift+Space"));
        assert!(!text.contains("Mod+Space"));
        assert!(text.contains("// retain this include\ninclude \"niri-custom.kdl\""));
        assert_eq!(std::fs::read_to_string(paths.niri()).unwrap(), before);
        std::fs::write(paths.niri(), text).unwrap();
        let mut reloaded = Editor::load(&paths).unwrap();
        assert_eq!(reloaded.rows[index].key, "Mod+Shift+Space");
        reloaded.rows[index].key = reloaded.rows[index].default_key.clone();
        assert!(reloaded.candidate().unwrap().contains("Mod+Space"));
    }
}
