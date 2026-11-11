use super::{Action, Batch, Cancellation, Config, NamedAction, RESULT_LIMIT, ResultItem};
use crate::app::panel_search::{SearchField, SearchMatch, SearchText, best_match, rank_matches};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    paths: Vec<(PathBuf, bool)>,
    pub status: String,
}

/// A session-local name cache, refreshed independently of typing. Never follows
/// directory symlinks, crosses a filesystem boundary, or indexes hidden entries.
pub(crate) fn build_index(config: &Config, cancel: &Cancellation) -> FileIndex {
    use std::os::unix::fs::MetadataExt;
    let started = Instant::now();
    let mut index = FileIndex::default();
    let mut visited = HashSet::new();
    let mut skipped = false;
    let mut limited = false;
    for root in &config.roots {
        let root = if root == std::path::Path::new("~") {
            let Some(home) = std::env::var_os("HOME") else {
                skipped = true;
                continue;
            };
            PathBuf::from(home)
        } else {
            root.clone()
        };
        let Ok(root) = root.canonicalize() else {
            skipped = true;
            continue;
        };
        let Ok(metadata) = root.metadata() else {
            skipped = true;
            continue;
        };
        let device = metadata.dev();
        let mut pending = vec![(root, 0)];
        while let Some((directory, depth)) = pending.pop() {
            if cancel.cancelled() {
                return index;
            }
            if index.paths.len() >= config.max_paths || started.elapsed() > Duration::from_secs(10)
            {
                limited = true;
                break;
            }
            if !visited.insert(directory.clone()) {
                continue;
            }
            let Ok(entries) = directory.read_dir() else {
                skipped = true;
                continue;
            };
            for entry in entries {
                if cancel.cancelled() {
                    return index;
                }
                if index.paths.len() >= config.max_paths
                    || started.elapsed() > Duration::from_secs(10)
                {
                    limited = true;
                    break;
                }
                let Ok(entry) = entry else {
                    skipped = true;
                    continue;
                };
                let name = entry.file_name();
                use std::os::unix::ffi::OsStrExt;
                if name.as_bytes().starts_with(b".")
                    || config
                        .excludes
                        .iter()
                        .any(|excluded| name == excluded.as_str())
                {
                    continue;
                }
                let Ok(kind) = entry.file_type() else {
                    skipped = true;
                    continue;
                };
                if kind.is_symlink() {
                    continue;
                }
                let is_dir = kind.is_dir();
                if !is_dir && !kind.is_file() {
                    continue;
                }
                let path = entry.path();
                if is_dir {
                    let Ok(metadata) = entry.metadata() else {
                        skipped = true;
                        continue;
                    };
                    if metadata.dev() != device {
                        continue;
                    }
                    if depth < 32 {
                        pending.push((path.clone(), depth + 1));
                    } else {
                        limited = true;
                    }
                }
                index.paths.push((path, is_dir));
            }
        }
        if limited {
            break;
        }
    }
    index.paths.sort_by(|left, right| left.0.cmp(&right.0));
    index.paths.dedup_by(|left, right| left.0 == right.0);
    index.status = match (limited, skipped) {
        (true, _) => fl!("shell-files-index-partial"),
        (_, true) => fl!("shell-files-locations-unreadable"),
        _ => String::new(),
    };
    index
}

/// The status of a file search that found more matches than it shows.
pub(crate) fn match_limit_status() -> String {
    fl!("shell-files-top-matches")
}

pub(crate) fn search_files(index: &FileIndex, query: &str, cancel: &Cancellation) -> Batch {
    let mut matches = Vec::new();
    if query.is_empty() {
        return Batch::default();
    }
    let query = query.to_lowercase();
    for (source_index, (path, _is_dir)) in index.paths.iter().enumerate() {
        if cancel.cancelled() {
            return Batch::default();
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let parent = path
            .parent()
            .unwrap_or(path)
            .to_string_lossy()
            .to_lowercase();
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: &name,
            },
            SearchText {
                field: SearchField::Secondary,
                text: &parent,
            },
        ];
        if let Some(score) = best_match(&fields, &query) {
            matches.push(SearchMatch {
                item: source_index,
                score,
                source_index,
            });
        }
    }
    let more = matches.len() > RESULT_LIMIT;
    let results = rank_matches(matches)
        .into_iter()
        .take(RESULT_LIMIT)
        .map(|position| {
            let (path, is_dir) = &index.paths[position];
            let actions = actions(path, *is_dir);
            ResultItem {
                id: crate::app::launcher::file_uri(path),
                label: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                description: {
                    let parent = path.parent().unwrap_or(path).display().to_string();
                    if *is_dir {
                        fl!("shell-files-folder-in", parent = parent)
                    } else {
                        fl!("shell-files-file-in", parent = parent)
                    }
                },
                icon: Some(if *is_dir { "folder" } else { "text-x-generic" }.into()),
                actions,
            }
        })
        .collect();
    Batch {
        results,
        status: if more {
            match_limit_status()
        } else {
            index.status.clone()
        },
    }
}

pub(crate) fn actions(path: &std::path::Path, is_dir: bool) -> Vec<NamedAction> {
    let mut actions = vec![NamedAction {
        label: fl!("shell-launcher-open"),
        action: Action::Open {
            path: path.to_path_buf(),
        },
    }];
    if let Some(text) = path.to_str() {
        actions.push(NamedAction {
            label: fl!("shell-files-copy-path"),
            action: Action::Copy { text: text.into() },
        });
    }
    actions.push(NamedAction {
        label: fl!("shell-files-open-folder"),
        action: Action::Reveal {
            path: path.to_path_buf(),
        },
    });
    if !is_dir {
        actions.push(NamedAction {
            label: fl!("shell-files-open-with"),
            action: Action::ChooseApplication {
                path: path.to_path_buf(),
            },
        });
        actions.push(NamedAction {
            label: fl!("shell-files-quick-look"),
            action: Action::Preview {
                path: path.to_path_buf(),
            },
        });
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_exclusions_unicode_and_refresh_follow_real_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("folder")).unwrap();
        std::fs::create_dir(temp.path().join("target")).unwrap();
        for name in [
            "Été.txt",
            "folder/a quote'\n.txt",
            ".secret",
            "target/hidden",
        ] {
            std::fs::write(temp.path().join(name), b"fixture").unwrap();
        }
        std::os::unix::fs::symlink(temp.path(), temp.path().join("loop")).unwrap();
        let config = Config {
            roots: vec![temp.path().into(), temp.path().join("folder")],
            ..Config::default()
        };
        let cancel = Cancellation::default();
        let index = build_index(&config, &cancel);
        assert_eq!(index.paths.len(), 3);
        assert_eq!(
            search_files(&index, "ÉTÉ", &cancel).results[0].label,
            "Été.txt"
        );
        let found = search_files(&index, "quote", &cancel).results;
        assert!(
            matches!(&found[0].actions[0].action, Action::Open { path } if path == &temp.path().join("folder/a quote'\n.txt"))
        );
        std::fs::rename(temp.path().join("Été.txt"), temp.path().join("renamed.txt")).unwrap();
        let index = build_index(&config, &cancel);
        assert!(search_files(&index, "été", &cancel).results.is_empty());
        assert_eq!(search_files(&index, "renamed", &cancel).results.len(), 1);
    }

    #[test]
    fn cap_cancellation_missing_roots_and_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join(std::ffi::OsString::from_vec(b"odd-\xff.txt".to_vec()));
        std::fs::write(&path, b"fixture").unwrap();
        let config = Config {
            roots: vec![temp.path().into()],
            max_paths: 1,
            ..Config::default()
        };
        let index = build_index(&config, &Cancellation::default());
        let batch = search_files(&index, "odd", &Cancellation::default());
        assert_eq!(batch.results[0].actions.len(), 4);
        assert_eq!(batch.results[0].actions[0].action, Action::Open { path });
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(build_index(&config, &cancel).paths.is_empty());
        let missing = Config {
            roots: vec![temp.path().join("absent")],
            ..config
        };
        assert!(
            !build_index(&missing, &Cancellation::default())
                .status
                .is_empty()
        );
    }
}
