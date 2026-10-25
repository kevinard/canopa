//! Prepared Trash contents. Filesystem discovery and policy stay outside views.
use super::{LocationDiscovery, TrashItem, TrashLocation};
use crate::Cancellation;
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

#[cfg(feature = "local-runtime")]
mod reader;
#[cfg(feature = "local-runtime")]
pub use reader::{Published, Reader};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    All,
    Main,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub items: Vec<TrashItem>,
    pub roots: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

impl Catalog {
    /// Read on a filesystem worker. Discovery never creates Trash storage.
    pub fn read(data_home: &Path, scope: Scope, cancellation: &Cancellation) -> io::Result<Self> {
        check_cancel(cancellation)?;
        Self::read_locations(locations(data_home, scope)?.locations, cancellation)
    }

    fn read_locations(
        locations: Vec<TrashLocation>,
        cancellation: &Cancellation,
    ) -> io::Result<Self> {
        let mut catalog = Self::default();
        let mut seen = BTreeSet::new();
        for location in locations {
            check_cancel(cancellation)?;
            if !seen.insert(location.directory().to_owned()) {
                continue;
            }
            catalog.roots.push(location.directory().to_owned());
            match location.list(cancellation) {
                Ok(mut items) => catalog.items.append(&mut items),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => return Err(error),
                Err(error) => catalog
                    .warnings
                    .push(format!("{}: {error}", location.directory().display())),
            }
        }
        catalog.items.sort_by(|left, right| {
            left.name()
                .cmp(right.name())
                .then_with(|| left.path.cmp(&right.path))
        });
        check_cancel(cancellation)?;
        Ok(catalog)
    }
}

fn locations(data_home: &Path, scope: Scope) -> io::Result<LocationDiscovery> {
    match scope {
        Scope::All => TrashLocation::discover(data_home),
        Scope::Main => Ok(LocationDiscovery {
            locations: vec![TrashLocation::home(data_home)?],
            #[cfg(feature = "local-runtime")]
            creation_parents: Vec::new(),
        }),
    }
}

fn check_cancel(cancellation: &Cancellation) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Trash read cancelled",
        ))
    } else {
        Ok(())
    }
}
