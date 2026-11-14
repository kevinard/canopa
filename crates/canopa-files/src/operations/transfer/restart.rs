//! Durable facts captured from the original opened selection. Decoding these
//! facts grants no execution ownership; journals, stages and mutation boundaries
//! must be proved separately before a stopped plan can become executable.
use super::*;
use crate::operations::original::Original;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum Evidence<T> {
    Available(T),
    Unavailable(String),
}
impl<T> Evidence<T> {
    pub(super) fn unavailable(reason: impl ToString) -> Self {
        Self::Unavailable(reason.to_string().chars().take(1024).collect())
    }
    fn valid(&self) -> bool {
        match self {
            Self::Available(_) => true,
            Self::Unavailable(reason) => !reason.is_empty() && reason.len() <= 4096,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CapturedEntry {
    directory: bool,
    original: Evidence<Parents>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Parents {
    entry: Original,
    parent: Original,
}

impl CapturedEntry {
    pub(super) fn reopen(&self, path: &Path) -> io::Result<fs::File> {
        let Evidence::Available(originals) = &self.original else {
            return Err(invalid(
                "The original selected object cannot be verified after restart.",
            ));
        };
        use rustix::fs::{Mode, OFlags, open, openat};
        let parent = fs::File::from(open(
            path.parent()
                .ok_or_else(|| invalid("A selected entry needs its original parent."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        originals.parent.require(&parent)?;
        let file = fs::File::from(openat(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid("A selected entry needs its original name."))?,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        originals.entry.require(&file)?;
        if file.metadata()?.is_dir() != self.directory {
            return Err(invalid("The original selected object changed its type."));
        }
        self.require_name(path)?;
        Ok(file)
    }
    /// Live work must retain the admitted parent as well as the opened inode.
    /// A hard link in a new folder is the same object but a different selection.
    pub(super) fn require_name(&self, path: &Path) -> io::Result<()> {
        let Evidence::Available(originals) = &self.original else {
            return Ok(());
        };
        if let Some(observed) = crate::operations::proofs::shared(path) {
            return observed.require_name(&originals.parent, &originals.entry);
        }
        use rustix::fs::{Mode, OFlags, open, openat};
        let parent = fs::File::from(open(
            path.parent()
                .ok_or_else(|| invalid("An observed entry needs its original parent."))?,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        originals.parent.require(&parent)?;
        let entry = fs::File::from(openat(
            &parent,
            path.file_name()
                .ok_or_else(|| invalid("An observed entry needs its original name."))?,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        originals.entry.require(&entry)
    }
    /// The caller opens the entry relative to this same parent descriptor.
    /// Keeping both alive here binds the facts without a pathname recapture.
    pub(super) fn capture_in(file: &fs::File, parent: &fs::File) -> Self {
        let directory = file.metadata().is_ok_and(|m| m.is_dir());
        let capture = || -> io::Result<Parents> {
            Ok(Parents {
                entry: Original::capture(file)?,
                parent: Original::capture(parent)?,
            })
        };
        Self {
            directory,
            original: match capture() {
                Ok(value) => Evidence::Available(value),
                Err(error) => Evidence::unavailable(error),
            },
        }
    }
    pub(super) fn is_directory(&self) -> bool {
        self.directory
    }
    pub(super) fn capture(file: &fs::File, path: &Path) -> Self {
        let directory = file.metadata().is_ok_and(|m| m.is_dir());
        let capture = || -> io::Result<Parents> {
            use rustix::fs::{Mode, OFlags, open, openat};
            let entry = Original::capture(file)?;
            let parent = path
                .parent()
                .ok_or_else(|| invalid("A selected entry needs its original parent."))?;
            let name = path
                .file_name()
                .ok_or_else(|| invalid("A selected entry needs its original name."))?;
            let opened = fs::File::from(open(
                parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            let named = fs::File::from(openat(
                &opened,
                name,
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            entry.require(&named)?;
            Ok(Parents {
                entry,
                parent: Original::capture(&opened)?,
            })
        };
        let original = match capture() {
            Ok(value) => Evidence::Available(value),
            Err(error) => Evidence::unavailable(error),
        };
        Self {
            directory,
            original,
        }
    }
    pub(super) fn unavailable(reason: impl ToString) -> Self {
        Self {
            directory: false,
            original: Evidence::unavailable(reason),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RestartObservation {
    version: u8,
    kind: TransferKind,
    #[serde(with = "crate::path_serde")]
    destination: PathBuf,
    root: Evidence<Original>,
    sources: Vec<Source>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    captured: CapturedEntry,
}
impl RestartObservation {
    pub(super) fn with_current_root(mut self, root: Evidence<Original>) -> io::Result<Self> {
        if let Evidence::Available(original) = &self.root
            && !matches!(&root, Evidence::Available(current) if current == original)
        {
            return Err(invalid(
                "The active transfer changed its original destination.",
            ));
        }
        self.root = root;
        Ok(self)
    }

    pub(super) fn required_sources(
        &self,
        plan: &TransferContinuation,
    ) -> io::Result<Vec<(PathBuf, CapturedEntry)>> {
        let request = TransferRequest {
            kind: plan.kind,
            destination: plan.destination.clone(),
            sources: plan.source_paths().cloned().collect(),
        };
        if !self.covers(&request) {
            return Err(invalid(
                "The saved continuation has no original selection proof.",
            ));
        }
        Ok(self
            .sources
            .iter()
            .filter(|source| {
                request.sources.iter().any(|path| {
                    path == &source.path
                        || (source.captured.directory && path.starts_with(&source.path))
                })
            })
            .map(|source| (source.path.clone(), source.captured.clone()))
            .collect())
    }

    pub(super) fn destination_original(&self) -> io::Result<Original> {
        match self.root {
            Evidence::Available(original) => Ok(original),
            Evidence::Unavailable(_) => Err(invalid(
                "The original destination cannot be verified after restart.",
            )),
        }
    }
    pub(super) fn capture(
        request: &TransferRequest,
        root: Evidence<Original>,
        sources: Vec<(PathBuf, CapturedEntry)>,
    ) -> Self {
        let sources: BTreeMap<_, _> = sources.into_iter().collect();
        Self {
            version: 1,
            kind: request.kind,
            destination: request.destination.clone(),
            root,
            sources: sources
                .into_iter()
                .map(|(path, captured)| Source { path, captured })
                .collect(),
        }
    }
    pub(super) fn initial_sources(&self, request: &TransferRequest) -> Option<BTreeSet<PathBuf>> {
        let mut selected: BTreeSet<_> = request.sources.iter().cloned().collect();
        if !self.covers(request)
            || selected
                != self
                    .sources
                    .iter()
                    .map(|source| source.path.clone())
                    .collect()
        {
            return None;
        }
        let parents: Vec<_> = self
            .sources
            .iter()
            .filter(|source| source.captured.directory)
            .map(|source| &source.path)
            .collect();
        selected.retain(|path| {
            !parents
                .iter()
                .any(|parent| path != *parent && path.starts_with(parent))
        });
        Some(selected)
    }
    pub(crate) fn covers(&self, request: &TransferRequest) -> bool {
        self.version == 1
            && self.kind == request.kind
            && self.destination == request.destination
            && self.destination.is_absolute()
            && self.root.valid()
            && !self.sources.is_empty()
            && self.sources.len() <= 8192
            && self
                .sources
                .iter()
                .all(|source| source.path.is_absolute() && source.captured.original.valid())
            && request.sources.iter().all(|path| {
                self.sources.iter().any(|source| {
                    path == &source.path
                        || (source.captured.directory
                            && path.strip_prefix(&source.path).is_ok_and(|tail| {
                                tail.components().all(|component| {
                                    matches!(
                                        component,
                                        std::path::Component::Normal(_)
                                            | std::path::Component::CurDir
                                    )
                                })
                            }))
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_selection_records_originals_without_reobserving_replacement_paths() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"Original selected bytes").unwrap();
        fs::create_dir(&destination).unwrap();
        let request = TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: destination.clone(),
        };
        let observed = request.observe_sources().unwrap();
        let open = |path: &Path| {
            fs::File::from(
                rustix::fs::open(
                    path,
                    rustix::fs::OFlags::PATH
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .unwrap(),
            )
        };
        let original_file = open(&source);
        let snapshot = observed.restart_observation();
        if Original::capture(&original_file).is_ok() {
            let Evidence::Available(pair) = &snapshot.sources[0].captured.original else {
                panic!("original source/parent facts were lost");
            };
            pair.entry.require(&original_file).unwrap();
            pair.parent.require(&open(root.path())).unwrap();
            let Evidence::Available(destination_original) = snapshot.root else {
                panic!("original destination facts were lost");
            };
            destination_original.require(&open(&destination)).unwrap();
        }
        let before = serde_json::to_vec(&observed.restart_observation()).unwrap();
        fs::rename(&source, root.path().join("parked-source")).unwrap();
        fs::write(&source, b"Replacement selected bytes").unwrap();
        fs::rename(&destination, root.path().join("parked-destination")).unwrap();
        fs::create_dir(&destination).unwrap();
        let after = serde_json::to_vec(&observed.restart_observation()).unwrap();
        assert_eq!(before, after);
        let restored: RestartObservation = serde_json::from_slice(&before).unwrap();
        assert!(restored.covers(&request));
        let mut other = request.clone();
        other.kind = TransferKind::Move;
        assert!(!restored.covers(&other));
        other = request.clone();
        other.sources = vec![root.path().join("unselected")];
        assert!(!restored.covers(&other));
        // Serialization retains facts, and still creates no process execution owner.
        let saved = TransferContinuation::unstarted(&observed).into_saved_plan();
        let decoded: TransferContinuation =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(decoded.observed.is_none() && decoded.destination_observation.is_none());
        assert!(decoded.restart.as_ref().unwrap().covers(&request));
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy["version"] = serde_json::json!(1);
        assert!(
            serde_json::from_value::<TransferContinuation>(legacy.clone()).is_err(),
            "v1 cannot smuggle later original facts"
        );
        legacy.as_object_mut().unwrap().remove("restart");
        let legacy: TransferContinuation = serde_json::from_value(legacy).unwrap();
        assert!(legacy.restart.is_none() && legacy.observed.is_none());
    }
    #[test]
    fn parent_facts_stay_with_the_opened_parent_when_the_original_entry_moves() {
        use rustix::fs::{Mode, OFlags, open, openat};
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        let parked = root.path().join("parked");
        fs::create_dir(&parent).unwrap();
        fs::write(parent.join("entry"), b"Original entry").unwrap();
        let old_parent = fs::File::from(
            open(
                &parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        let file = fs::File::from(
            openat(
                &old_parent,
                "entry",
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        if Original::capture(&file).is_err() {
            return;
        }
        fs::rename(&parent, &parked).unwrap();
        fs::create_dir(&parent).unwrap();
        fs::rename(parked.join("entry"), parent.join("entry")).unwrap();
        let captured = CapturedEntry::capture_in(&file, &old_parent);
        let Evidence::Available(pair) = captured.original else {
            panic!("original descriptor pair lost");
        };
        let replacement_parent = fs::File::from(
            open(
                &parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        let same_entry = fs::File::from(
            openat(
                &replacement_parent,
                "entry",
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        pair.entry.require(&same_entry).unwrap();
        assert_eq!(
            pair.parent.require(&replacement_parent).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        pair.parent.require(&old_parent).unwrap();
    }

    #[test]
    fn unavailable_sources_cannot_acquire_a_replacement_observation_later() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("missing");
        let request = TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: root.path().to_owned(),
        };
        let observed = request.observe_sources().unwrap();
        let before = serde_json::to_vec(&observed.restart_observation()).unwrap();
        fs::write(source, b"Replacement that arrived later").unwrap();
        assert_eq!(
            before,
            serde_json::to_vec(&observed.restart_observation()).unwrap()
        );
    }
}
