//! A published leaf retains the actual opened source it copied. These facts
//! authorize neither a fresh source selection nor replacement of its output.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Source {
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
    parent: original::Original,
    entry: original::Original,
    pub identity: Identity,
    pub fingerprint: Fingerprint,
    pub bytes: u64,
}
impl Source {
    pub(super) fn capture(
        source: &fingerprint::source::Opened,
        fingerprint: &Fingerprint,
    ) -> Option<Box<Self>> {
        let metadata = source.entry().metadata().ok()?;
        Some(Box::new(Self {
            path: source.path().to_owned(),
            parent: original::Original::capture(source.parent()).ok()?,
            entry: original::Original::capture(source.entry()).ok()?,
            identity: Identity::from_metadata(&metadata),
            fingerprint: fingerprint.clone(),
            bytes: if metadata.is_symlink() {
                0
            } else {
                metadata.len()
            },
        }))
    }
    fn opened(&self) -> io::Result<fingerprint::source::Opened> {
        let source = fingerprint::source::Opened::open(&self.path)?;
        self.parent.require(source.parent())?;
        self.entry.require(source.entry())?;
        if Identity::from_metadata(&source.entry().metadata()?) != self.identity {
            return Err(changed());
        }
        source.require_current()?;
        Ok(source)
    }
    fn verified(&self, cancellation: &Cancellation) -> io::Result<fingerprint::source::Opened> {
        let source = self.opened()?;
        if Fingerprint::read_opened(source.entry(), cancellation)? != self.fingerprint {
            return Err(changed());
        }
        source.require_current()?;
        Ok(source)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cleanup {
    pub source: Source,
    #[serde(with = "crate::path_serde")]
    pub destination: PathBuf,
    pub publication: staging::Published,
    pub identity: Identity,
    pub fingerprint: Fingerprint,
}
impl Cleanup {
    pub(super) fn from_change(change: &Change) -> io::Result<Self> {
        let Change::Copied {
            source: Some(source),
            publication: Some(publication),
            path,
            identity,
            fingerprint,
            ..
        } = change
        else {
            return Err(invalid(
                "This copied step has no original source/publication proof.",
            ));
        };
        Ok(Self {
            source: (**source).clone(),
            destination: path.clone(),
            publication: (**publication).clone(),
            identity: *identity,
            fingerprint: fingerprint.clone(),
        })
    }
    pub(super) fn valid_for(&self, source: &Path, destination: &Path) -> bool {
        self.source.path == source
            && self.destination == destination
            && source.is_absolute()
            && destination.is_absolute()
    }
    pub(super) fn require_originals(&self) -> io::Result<()> {
        self.source.opened()?;
        let copy = self.publication.require(&self.destination)?;
        if Identity::from_metadata(&copy.metadata()?) != self.identity {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn execute(
        &self,
        state: &mut Store,
        id: u64,
        cancellation: &Cancellation,
        mut checkpoint: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        use staging::VerifiedCopy;
        checkpoint()?;
        let source = self.source.verified(cancellation)?;
        let copy = self.publication.reopen(&self.destination)?;
        if Identity::from_metadata(&copy.metadata()?) != self.identity
            || copy.fingerprint(cancellation)? != self.fingerprint
        {
            return Err(changed());
        }
        checkpoint()?;
        let removal = source_removal::Prepared::open(source, &self.destination, &copy)?;
        let change = Change::SourceRemoved {
            originals: removal.originals(),
            original: self.source.path.clone(),
            current: self.destination.clone(),
            identity: self.source.identity,
            copy_identity: self.identity,
            fingerprint: self.fingerprint.clone(),
            source_fingerprint: self.source.fingerprint.clone(),
            quarantine: removal.quarantine(),
            reviewed: None,
        };
        removal.commit(state, id, &change, &copy, cancellation, checkpoint)
    }
    pub(super) fn matches_removal(&self, change: &Change) -> bool {
        matches!(change,Change::SourceRemoved { original,current,identity,copy_identity,fingerprint,source_fingerprint,originals:Some(originals),.. }
            if original==&self.source.path && current==&self.destination && *identity==self.source.identity && *copy_identity==self.identity && fingerprint==&self.fingerprint && source_fingerprint==&self.source.fingerprint && originals.matches_copied(self.source.parent,self.source.entry,&self.publication))
    }
}
fn changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "The original copied source or its committed output changed. Both remain preserved.",
    )
}
