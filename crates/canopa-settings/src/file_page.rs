//! The file-backed page pattern: load a draft from a document, edit it, Save.
//!
//! A [`FileModel`] says how a page reads, falls back and writes its draft.
//! [`FileDraft`] keeps the draft with the version it was read from, so a save
//! detects outside edits ([`Snapshot::save`]) and "Reload this page" replaces
//! only this page's draft.
//!
//! **Shared-document rule.** Pages can edit the same document when each
//! writes its own [`FileModel::section`]. When one page saves, every other page
//! that edits the document (see [`crate::registry::Kind::File`]) keeps its
//! unsaved draft and continues from the new version, but only if it had read
//! the version that save replaced and the save left its section unchanged.
//! Otherwise its own next save fails as "changed outside this editor", and it
//! has to be reloaded. Changes made outside Settings are therefore never
//! adopted silently.
use crate::{registry::Document, session::Paths, storage::Snapshot};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// How a file-backed page reads, falls back and writes its draft.
pub trait FileModel: Clone + Send + Sized + 'static {
    /// The document this page saves.
    const DOCUMENT: Document;
    /// The save error when the document can't be edited (missing, not an
    /// owned regular file, or unreadable).
    const READ_ONLY: &'static str = "configuration is read-only";

    fn path(paths: &Paths) -> PathBuf;
    /// Read the draft from a saved version.
    fn read(source: &Snapshot) -> Result<Self>;
    /// The values shown when the document is missing or can't be read.
    fn fallback() -> Self;
    /// Validate and publish the draft over `source`, then ask the document's
    /// owner to reload. Runs on a worker thread.
    fn write(&self, paths: &Paths, source: &Snapshot) -> Result<Snapshot>;
    /// The part of the document this page writes, without formatting. The
    /// default, the whole text, only adopts saves that changed nothing.
    fn section(text: &str) -> Result<String> {
        Ok(text.to_owned())
    }
}

/// A page's draft and the saved version it was read from.
#[derive(Clone, Debug)]
pub struct FileDraft<M> {
    pub value: M,
    source: Option<Snapshot>,
}

impl<M: FileModel> FileDraft<M> {
    /// Read the saved document. A read error is returned for the page to
    /// report; the draft then shows [`FileModel::fallback`].
    pub fn load(paths: &Paths) -> (Self, Option<String>) {
        let source = Snapshot::load(M::path(paths)).ok();
        let (value, problem) = match source.as_ref().map(M::read).transpose() {
            Ok(value) => (value.unwrap_or_else(M::fallback), None),
            Err(error) => (M::fallback(), Some(error.to_string())),
        };
        (Self { value, source }, problem)
    }

    /// Discard the draft and read the saved document again.
    pub fn reload(&mut self, paths: &Paths) -> Option<String> {
        let (draft, problem) = Self::load(paths);
        *self = draft;
        problem
    }

    /// The version the draft was read from; `None` when it can't be edited.
    pub fn source(&self) -> Option<&Snapshot> {
        self.source.as_ref()
    }

    /// A worker job that saves the current draft.
    pub fn save_job(&self, paths: &Paths) -> impl FnOnce() -> Result<Snapshot> + Send + 'static {
        let value = self.value.clone();
        let source = self.source.clone();
        let paths = paths.clone();
        move || value.write(&paths, &source.context(M::READ_ONLY)?)
    }

    /// Adopt this page's own save and show the values as read back. Returns
    /// the replaced version, for the shared-document rule.
    pub fn saved(&mut self, saved: Snapshot) -> Option<Snapshot> {
        if let Ok(value) = M::read(&saved) {
            self.value = value;
        }
        self.source.replace(saved)
    }

    /// The shared-document rule: another page saved `saved` over `previous`.
    /// Returns whether this draft now continues from `saved`.
    pub fn rebase(&mut self, previous: &Snapshot, saved: &Snapshot) -> bool {
        let adopt = self
            .source
            .as_ref()
            .is_some_and(|source| source.same(previous))
            && matches!(
                (M::section(&previous.text), M::section(&saved.text)),
                (Ok(before), Ok(after)) if before == after
            );
        if adopt {
            self.source = Some(saved.clone());
        }
        adopt
    }

    /// Another page changed this page's section in a way it can repeat:
    /// apply `change` to the draft and continue from `saved`. Only a draft
    /// read from `previous` follows; others keep failing safely on save.
    pub fn follow(
        &mut self,
        previous: &Snapshot,
        saved: &Snapshot,
        change: impl FnOnce(&mut M),
    ) -> bool {
        let follows = self
            .source
            .as_ref()
            .is_some_and(|source| source.same(previous));
        if follows {
            change(&mut self.value);
            self.source = Some(saved.clone());
        }
        follows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document of `key value` lines; this page writes the `mine` line.
    #[derive(Clone, Debug, PartialEq)]
    struct Mine(String);

    impl FileModel for Mine {
        const DOCUMENT: Document = Document::Common;
        fn path(paths: &Paths) -> PathBuf {
            paths.directory.join("lines")
        }
        fn read(source: &Snapshot) -> Result<Self> {
            anyhow::ensure!(!source.text.contains("broken"), "unreadable");
            Ok(Self(Self::section(&source.text)?))
        }
        fn fallback() -> Self {
            Self("default".into())
        }
        fn write(&self, _: &Paths, source: &Snapshot) -> Result<Snapshot> {
            let mut lines: Vec<_> = source
                .text
                .lines()
                .filter(|line| !line.starts_with("mine "))
                .map(str::to_owned)
                .collect();
            lines.push(format!("mine {}", self.0));
            source.save(&(lines.join("\n") + "\n"), |_| Ok(()))
        }
        fn section(text: &str) -> Result<String> {
            Ok(text
                .lines()
                .find_map(|line| line.strip_prefix("mine "))
                .unwrap_or_default()
                .to_owned())
        }
    }

    fn setup(text: &str) -> (tempfile::TempDir, Paths) {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: root.path().to_owned(),
        };
        std::fs::write(Mine::path(&paths), text).unwrap();
        (root, paths)
    }

    #[test]
    fn drafts_load_fall_back_save_and_reload() {
        let (_root, paths) = setup("other 1\nmine a\n");
        let (mut draft, problem) = FileDraft::<Mine>::load(&paths);
        assert_eq!((draft.value.clone(), problem), (Mine("a".into()), None));
        draft.value.0 = "b".into();
        let saved = draft.save_job(&paths)().unwrap();
        let previous = draft.saved(saved).unwrap();
        assert!(previous.text.contains("mine a"));
        assert_eq!(draft.value, Mine("b".into()));
        draft.value.0 = "c".into();
        assert_eq!(draft.reload(&paths), None);
        assert_eq!(draft.value, Mine("b".into()));

        std::fs::write(Mine::path(&paths), "broken\n").unwrap();
        assert_eq!(draft.reload(&paths).as_deref(), Some("unreadable"));
        assert_eq!(draft.value, Mine::fallback());

        // Symlinked configuration is read-only.
        std::fs::remove_file(Mine::path(&paths)).unwrap();
        std::fs::write(paths.directory.join("target"), "mine a\n").unwrap();
        std::os::unix::fs::symlink("target", Mine::path(&paths)).unwrap();
        let (draft, problem) = FileDraft::<Mine>::load(&paths);
        assert_eq!((draft.value.clone(), problem), (Mine::fallback(), None));
        assert!(draft.source().is_none());
        let error = draft.save_job(&paths)().unwrap_err();
        assert_eq!(error.to_string(), Mine::READ_ONLY);
    }

    #[test]
    fn outside_edits_fail_the_save() {
        let (_root, paths) = setup("mine a\n");
        let (mut draft, _) = FileDraft::<Mine>::load(&paths);
        std::fs::write(Mine::path(&paths), "mine z\n").unwrap();
        draft.value.0 = "b".into();
        let error = draft.save_job(&paths)().unwrap_err();
        assert!(error.to_string().contains("outside this editor"), "{error}");
    }

    #[test]
    fn shared_documents_rebase_only_saves_that_left_this_section_alone() {
        let (_root, paths) = setup("other 1\nmine a\n");
        let (mut draft, _) = FileDraft::<Mine>::load(&paths);
        draft.value.0 = "pending".into();
        let previous = draft.source().unwrap().clone();
        // Another page changes its own line.
        let other = previous.save("other 2\nmine a\n", |_| Ok(())).unwrap();
        assert!(draft.rebase(&previous, &other));
        assert_eq!(draft.value, Mine("pending".into()));
        let saved = draft.save_job(&paths)().unwrap();
        assert!(saved.text.contains("other 2") && saved.text.contains("mine pending"));

        // A save that changed this page's section is not adopted.
        let previous = saved.clone();
        let other = previous.save("mine theirs\n", |_| Ok(())).unwrap();
        let (mut stale, _) = FileDraft::<Mine>::load(&paths);
        stale.source = Some(previous.clone());
        assert!(!stale.rebase(&previous, &other));
        // Nor one made over a version this draft never read.
        let (mut draft, _) = FileDraft::<Mine>::load(&paths);
        assert!(!draft.rebase(&previous, &other));
    }
}
