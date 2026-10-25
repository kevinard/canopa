//! Pending member destinations are slots inside an owned Merge container.
//! Moving another occupant of that slot does not move the member's intent.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Movement {
    source: bool,
    destination: bool,
}

pub(in crate::operations::store) struct Movements(BTreeMap<(u64, u64), Movement>);

fn adjusted(path: &Path, relocation: &undo::Relocation) -> PathBuf {
    let mut result = path.to_owned();
    relocation.apply(&mut result);
    result
}

fn moved_directory(path: &Path, identity: Identity, relocation: &undo::Relocation) -> bool {
    let target = adjusted(path, relocation);
    target != path
        && fs::symlink_metadata(target).is_ok_and(|metadata| {
            metadata.is_dir() && Identity::from_metadata(&metadata) == identity
        })
}

impl Movements {
    pub(in crate::operations::store) fn planned_destination(
        &mut self,
        operation: u64,
        ordinal: u64,
    ) {
        self.0.entry((operation, ordinal)).or_default().destination = true;
    }
    pub(in crate::operations::store) fn saved_slot_moves(
        &self,
        connection: &Connection,
        operation: u64,
        ordinal: u64,
        change: &Change,
        relocation: &undo::Relocation,
    ) -> io::Result<bool> {
        let path = change.forward_path();
        let target = adjusted(path, relocation);
        if target == path {
            return Ok(false);
        }
        if self.roles(connection, operation, ordinal)?.source {
            return Ok(true);
        }
        let Some(retained) = change.retained() else {
            return Ok(false);
        };
        let stage = &retained.stage;
        let moved_stage = adjusted(&stage.directory, relocation);
        Ok(path.parent() != target.parent()
            && target.parent().is_some_and(|parent| {
                stage.parent_identity.map_or_else(
                    || stage.identity.matches(&moved_stage),
                    |expected| expected.matches(parent),
                )
            }))
    }
    // Capture before rewriting any rows: parent bodies must not depend on page order.
    pub(in crate::operations::store) fn read(
        connection: &Connection,
        before: u64,
        relocation: &undo::Relocation,
    ) -> io::Result<Self> {
        let mut statement = connection.prepare("SELECT r.operation,r.ordinal,c.body FROM directory_returns r JOIN changes c ON c.operation=r.operation AND c.ordinal=r.ordinal WHERE r.operation<=?1").map_err(db_error)?;
        let rows = statement
            .query_map([before], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(db_error)?;
        let mut movements = BTreeMap::new();
        for row in rows {
            let (operation, ordinal, body) = row.map_err(db_error)?;
            let change: Change = decode(&body)?;
            let (source, destination, incoming, existing) = directory_return::roots(&change)?;
            let mut source_moved = moved_directory(source, incoming, relocation);
            if source_moved
                && let Change::DirectoryMerge { change, .. } = &change
                && let Change::Trashed { info, .. } = change.as_ref()
            {
                let mut info = info.clone();
                info.visit_path(|path| relocation.apply(path));
                source_moved = info.validate(&adjusted(source, relocation)).is_ok();
            }
            movements.insert(
                (operation, ordinal),
                Movement {
                    source: source_moved,
                    destination: moved_directory(destination, existing, relocation),
                },
            );
        }
        Ok(Self(movements))
    }

    fn roles(&self, connection: &Connection, operation: u64, ordinal: u64) -> io::Result<Movement> {
        let mut roots = parents(connection, operation, ordinal)?;
        roots.push(ordinal);
        let mut moved = Movement::default();
        for root in roots {
            if let Some(role) = self.0.get(&(operation, root)) {
                moved.source |= role.source;
                moved.destination |= role.destination;
            }
        }
        Ok(moved)
    }

    pub(in crate::operations::store) fn relocate_member(
        &self,
        connection: &Connection,
        operation: u64,
        ordinal: u64,
        change: &mut Change,
        relocation: &undo::Relocation,
    ) -> io::Result<()> {
        let saved = saved::is_reserved(connection, operation, ordinal)?;
        let saved_moves =
            saved && self.saved_slot_moves(connection, operation, ordinal, change, relocation)?;
        if saved && let Change::RetainedOnly { path, retained, .. } = change {
            let roles = self.roles(connection, operation, ordinal)?;
            let target = adjusted(path, relocation);
            let stage = &mut retained.stage;
            let moved_stage = adjusted(&stage.directory, relocation);
            if saved_moves {
                *path = target;
            }
            if roles.source
                || stage.identity.matches(&moved_stage)
                || stage.parent_identity.is_some_and(|expected| {
                    moved_stage
                        .parent()
                        .is_some_and(|parent| expected.matches(parent))
                })
            {
                stage.directory = moved_stage;
            }
            return Ok(());
        }
        let member: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM directory_members WHERE operation=?1 AND ordinal=?2)",
                params![operation, ordinal],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !member && !matches!(change, Change::DirectoryMerge { .. }) {
            change.relocate(relocation);
            return Ok(());
        }
        let roles = self.roles(connection, operation, ordinal)?;
        let marker = matches!(change, Change::DirectoryMerge { .. });
        let inner = match change {
            Change::DirectoryMerge { change, .. } => change.as_mut(),
            change => change,
        };
        match inner {
            Change::RemovedDirectory { path, moved, .. } => {
                if roles.destination {
                    relocation.apply(path);
                }
                if roles.source
                    && let Some(moved) = moved
                {
                    relocation.apply(&mut moved.current);
                }
            }
            Change::SourceRemoved {
                original,
                current,
                copy_identity,
                quarantine,
                ..
            } => {
                if roles.destination {
                    relocation.apply(original);
                    relocation.apply(&mut quarantine.directory);
                }
                if roles.source
                    || (current == &relocation.from
                        && references::resolve(
                            connection,
                            operation,
                            ordinal,
                            current,
                            *copy_identity,
                        )?
                        .matches(&adjusted(current, relocation)))
                {
                    relocation.apply(current);
                }
            }
            Change::Copied {
                path,
                identity,
                retained,
                ..
            } => {
                if roles.source
                    || (path == &relocation.from
                        && references::resolve(connection, operation, ordinal, path, *identity)?
                            .matches(&adjusted(path, relocation)))
                {
                    relocation.apply(path);
                }
                if let Some(retained) = retained {
                    let stage = &mut retained.stage;
                    let target = adjusted(&stage.directory, relocation);
                    if stage.identity.matches(&target)
                        || stage.parent_identity.is_some_and(|identity| {
                            target
                                .parent()
                                .is_some_and(|parent| identity.matches(parent))
                        })
                    {
                        stage.directory = target;
                    }
                }
            }
            Change::Rename {
                original,
                current,
                identity,
                retained,
                saved,
                ..
            } => {
                if roles.destination {
                    relocation.apply(original);
                    if let Some(context) = saved.as_mut().and_then(|saved| saved.context.as_mut()) {
                        relocation.apply(&mut context.from);
                    }
                }
                if roles.source
                    || (!marker
                        && current == &relocation.from
                        && references::resolve(connection, operation, ordinal, current, *identity)?
                            .matches(&adjusted(current, relocation)))
                {
                    relocation.apply(current);
                    if let Some(context) = saved.as_mut().and_then(|saved| saved.context.as_mut()) {
                        relocation.apply(&mut context.to);
                    }
                }
                if let Some(retained) = retained {
                    let stage = &mut retained.stage;
                    let target = adjusted(&stage.directory, relocation);
                    if target != stage.directory
                        && (stage.identity.matches(&target)
                            || stage.parent_identity.is_some_and(|parent| {
                                target.parent().is_some_and(|path| parent.matches(path))
                            }))
                    {
                        stage.directory = target;
                    }
                }
            }
            Change::Trashed {
                original,
                current,
                info,
                ..
            } => {
                if roles.destination {
                    relocation.apply(original);
                }
                if roles.source {
                    let target = adjusted(current, relocation);
                    let mut moved = info.clone();
                    moved.visit_path(|path| relocation.apply(path));
                    if moved.validate(&target).is_ok() {
                        *current = target;
                        *info = moved;
                    }
                }
            }
            _ => return Err(invalid("invalid directory member relocation")),
        }
        Ok(())
    }

    pub(in crate::operations::store) fn owns_archive_destination(
        &self,
        connection: &Connection,
        entry: u64,
    ) -> io::Result<bool> {
        let mut statement = connection
            .prepare("SELECT operation,parent FROM directory_archives WHERE entry=?1")
            .map_err(db_error)?;
        let rows = statement
            .query_map([entry], |row| {
                Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?))
            })
            .map_err(db_error)?;
        for row in rows {
            let (operation, parent) = row.map_err(db_error)?;
            if self.roles(connection, operation, parent)?.destination {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
