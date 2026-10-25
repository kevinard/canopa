//! Cancellable filename-only search. Directory links are results, never traversal edges.
use std::{
    collections::{HashSet, VecDeque},
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{
    Cancellation,
    browse::{Directory, Entry, EntryKind, ReadFailure},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
    CurrentFolder,
    Recursive,
}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub root: PathBuf,
    pub query: String,
    pub scope: SearchScope,
    pub show_hidden: bool,
    /// The UI can explicitly increase this when results are truncated.
    pub maximum_results: usize,
}

#[derive(Clone, Debug)]
pub struct SearchResults {
    pub root: PathBuf,
    pub query: String,
    pub scope: SearchScope,
    pub entries: Vec<Entry>,
    pub failures: Vec<ReadFailure>,
    pub cancelled: bool,
    pub truncated: bool,
    pub scanned: usize,
}

impl SearchRequest {
    pub fn new(root: impl AsRef<Path>, query: impl Into<String>, scope: SearchScope) -> Self {
        Self {
            root: root.as_ref().to_owned(),
            query: query.into(),
            scope,
            show_hidden: false,
            maximum_results: 50_000,
        }
    }

    pub fn run(&self, cancellation: &Cancellation) -> SearchResults {
        self.run_with_progress(cancellation, |_| {})
    }

    pub fn run_with_progress(
        &self,
        cancellation: &Cancellation,
        mut progress: impl FnMut(&SearchResults),
    ) -> SearchResults {
        let mut results = SearchResults {
            root: self.root.clone(),
            query: self.query.clone(),
            scope: self.scope,
            entries: Vec::new(),
            failures: Vec::new(),
            cancelled: false,
            truncated: false,
            scanned: 0,
        };
        match std::path::absolute(&self.root) {
            Ok(root) => results.root = root,
            Err(error) => {
                results.failures.push(ReadFailure {
                    path: self.root.clone(),
                    message: error.to_string(),
                });
                return results;
            }
        }
        if self.query.is_empty() {
            return results;
        }
        let query = self.query.to_lowercase();
        let mut pending = VecDeque::from([results.root.clone()]);
        let mut visited = HashSet::new();
        let mut last_report: Option<Instant> = None;
        'folders: while let Some(path) = pending.pop_front() {
            if cancellation.is_cancelled() {
                results.cancelled = true;
                break;
            }
            match fs::metadata(&path) {
                Ok(metadata) => {
                    if !visited.insert((metadata.dev(), metadata.ino())) {
                        continue;
                    }
                }
                Err(error) => {
                    results.failures.push(ReadFailure {
                        path,
                        message: error.to_string(),
                    });
                    continue;
                }
            }
            let directory = match Directory::read_cancellable(&path, cancellation) {
                Ok(directory) => directory,
                Err(_) if cancellation.is_cancelled() => {
                    results.cancelled = true;
                    break;
                }
                Err(error) => {
                    results.failures.push(ReadFailure {
                        path,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            results.failures.extend(directory.failures);
            for entry in directory.entries {
                if cancellation.is_cancelled() {
                    results.cancelled = true;
                    break 'folders;
                }
                results.scanned += 1;
                if !self.show_hidden && entry.is_hidden() {
                    continue;
                }
                if entry.name().to_lowercase().contains(&query) {
                    if results.entries.len() >= self.maximum_results {
                        results.truncated = true;
                        break 'folders;
                    }
                    results.entries.push(entry.clone());
                }
                if self.scope == SearchScope::Recursive && entry.kind == EntryKind::Directory {
                    pending.push_back(entry.path);
                }
                if last_report.is_none_or(|last| last.elapsed() >= Duration::from_millis(100)) {
                    progress(&results);
                    last_report = Some(Instant::now());
                }
            }
        }
        results.entries.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        results
    }
}
