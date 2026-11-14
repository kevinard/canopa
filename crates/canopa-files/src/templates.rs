//! Document templates: the files of the XDG Templates folder, offered by New
//! Document. Reading the folder belongs on a filesystem worker.
use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

/// Folders below the Templates folder that are still read, so templates can be
/// grouped one level deep.
const MAX_DEPTH: usize = 2;
/// Entries read per folder, and templates offered in all.
const MAX_ENTRIES: usize = 500;
const MAX_TEMPLATES: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    /// The regular file copied, with links resolved.
    pub path: PathBuf,
    /// The menu label: its name without extension, after its group's.
    pub label: String,
    /// The name proposed for the new document: the template's own name.
    pub name: OsString,
}

/// The templates in `folder`, sorted by label. Hidden files and backups are
/// left out; a link counts when it leads to a regular file. A Templates folder
/// that is Home itself means "no templates", as `xdg-user-dirs` documents.
pub fn list(folder: &Path, home: &Path) -> Vec<Template> {
    let mut templates = Vec::new();
    if folder == home || folder.parent().is_none() {
        return templates;
    }
    collect(folder, &[], &mut templates, 1);
    templates.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.label.cmp(&b.label))
    });
    templates.truncate(MAX_TEMPLATES);
    templates
}

fn collect(folder: &Path, groups: &[String], templates: &mut Vec<Template>, depth: usize) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten().take(MAX_ENTRIES) {
        if templates.len() >= MAX_TEMPLATES {
            return;
        }
        let name = entry.file_name();
        let bytes = name.as_bytes();
        if bytes.starts_with(b".") || bytes.ends_with(b"~") {
            continue;
        }
        let path = entry.path();
        // Follows a link to what a person would open from the folder.
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            if depth < MAX_DEPTH {
                let mut nested = groups.to_vec();
                nested.push(name.to_string_lossy().into_owned());
                collect(&path, &nested, templates, depth + 1);
            }
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        let Ok(resolved) = fs::canonicalize(&path) else {
            continue;
        };
        let own = stem(&name.to_string_lossy()).to_owned();
        let label = groups
            .iter()
            .cloned()
            .chain(std::iter::once(own))
            .collect::<Vec<_>>()
            .join(" › ");
        templates.push(Template {
            path: resolved,
            label,
            name,
        });
    }
}

/// `name`, or the first of `name (2)`, `name (3)`… not in `taken`, keeping a
/// file's extension after the number.
pub fn free_name(name: &OsStr, taken: &BTreeSet<OsString>) -> OsString {
    if !taken.contains(name) {
        return name.to_owned();
    }
    let bytes = name.as_bytes();
    let dot = bytes
        .iter()
        .rposition(|byte| *byte == b'.')
        .filter(|dot| *dot > 0)
        .unwrap_or(bytes.len());
    let (base, extension) = bytes.split_at(dot);
    (2u32..)
        .map(|number| {
            let mut candidate = base.to_vec();
            candidate.extend_from_slice(format!(" ({number})").as_bytes());
            candidate.extend_from_slice(extension);
            OsString::from_vec(candidate)
        })
        .find(|candidate| !taken.contains(candidate))
        .unwrap()
}

/// `name` without its last extension; names that are only an extension stay.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn templates_are_files_by_label_with_one_level_of_groups() {
        let home = tempfile::tempdir().unwrap();
        let folder = home.path().join("Templates");
        fs::create_dir_all(folder.join("Office/Deeper/Deepest")).unwrap();
        fs::write(folder.join("Script.sh"), b"#!/bin/sh\n").unwrap();
        fs::write(folder.join("blank.odt"), b"").unwrap();
        fs::write(folder.join(".hidden"), b"").unwrap();
        fs::write(folder.join("notes.txt~"), b"").unwrap();
        fs::write(folder.join("Office/Letter.odt"), b"letter").unwrap();
        fs::write(folder.join("Office/Deeper/Too deep.txt"), b"").unwrap();
        symlink(folder.join("Script.sh"), folder.join("Linked.sh")).unwrap();
        symlink(folder.join("missing"), folder.join("Broken")).unwrap();
        let templates = list(&folder, home.path());
        let labels: Vec<_> = templates.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["blank", "Linked", "Office › Letter", "Script"]);
        let linked = &templates[1];
        assert_eq!(linked.name, "Linked.sh");
        assert_eq!(
            linked.path,
            fs::canonicalize(folder.join("Script.sh")).unwrap()
        );
    }

    #[test]
    fn free_names_number_before_the_extension() {
        let taken: BTreeSet<OsString> = ["Letter.odt", "Letter (2).odt", "Notes"]
            .into_iter()
            .map(OsString::from)
            .collect();
        assert_eq!(
            free_name(OsStr::new("Letter.odt"), &taken),
            "Letter (3).odt"
        );
        assert_eq!(free_name(OsStr::new("Notes"), &taken), "Notes (2)");
        assert_eq!(free_name(OsStr::new("New.txt"), &taken), "New.txt");
    }

    #[test]
    fn home_or_a_missing_folder_has_no_templates() {
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("file.txt"), b"").unwrap();
        assert!(list(home.path(), home.path()).is_empty());
        assert!(list(&home.path().join("Templates"), home.path()).is_empty());
    }
}
