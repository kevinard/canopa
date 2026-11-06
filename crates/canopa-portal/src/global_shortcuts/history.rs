//! Saved application/action choices never create live shortcut authority.
use super::{Action, Actions, Binding, MAX_ACTIONS};
use anyhow::{Result, ensure};
use rusqlite::{Connection, OpenFlags, config::DbConfig, params};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

const APPLICATION_ID: i64 = 0x434e5053; // CNPS
const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAX_APPS: usize = 128;
#[derive(Clone, Default)]
pub struct Saved {
    pub actions: Vec<Action>,
    pub bindings: Vec<Binding>,
}
impl Saved {
    pub fn response(&self) -> Result<crate::requests::Options> {
        let actions = Actions(self.actions.clone());
        let bindings = actions.definitions(&self.bindings)?;
        Ok(actions.response(&bindings.into_iter().collect())?)
    }
    fn validate(&self) -> Result<()> {
        let values = self
            .actions
            .iter()
            .map(|a| {
                let mut properties = crate::requests::Options::new();
                properties.insert(
                    "description".into(),
                    zbus::zvariant::Value::from(a.description.clone())
                        .try_into()
                        .expect("owned text"),
                );
                (a.id.clone(), properties)
            })
            .collect();
        let actions = Actions::parse(values)?;
        actions.definitions(&self.bindings)?;
        ensure!(
            self.actions.iter().all(|a| a.preferred_trigger.is_none()),
            "Invalid saved shortcut metadata"
        );
        Ok(())
    }
}
enum Source {
    Memory(HashMap<String, Saved>),
    File(PathBuf),
}
#[derive(Clone)]
pub struct History(Arc<Mutex<Source>>);
impl History {
    pub fn memory() -> Self {
        Self(Arc::new(Mutex::new(Source::Memory(HashMap::new()))))
    }
    pub fn file(path: PathBuf) -> Result<Self> {
        ensure!(path.is_absolute(), "Shortcut history path must be absolute");
        Ok(Self(Arc::new(Mutex::new(Source::File(path)))))
    }
    pub fn discover() -> Result<Self> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".local/state")))
            .ok_or_else(|| anyhow::anyhow!("Shortcut history directory unavailable"))?;
        Self::file(state.join("canopa/portal-shortcuts.db"))
    }
    pub async fn load(&self, app: String) -> Result<Saved> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.read(&app)).await?
    }
    pub async fn save(
        &self,
        app: String,
        actions: Actions,
        definitions: BTreeMap<u32, String>,
    ) -> Result<()> {
        let bindings = actions.selected(&definitions)?;
        let saved = Saved {
            actions: actions
                .0
                .into_iter()
                .map(|mut a| {
                    a.preferred_trigger = None;
                    a
                })
                .collect(),
            bindings,
        };
        saved.validate()?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.write(&app, saved)).await?
    }
    fn read(&self, app: &str) -> Result<Saved> {
        super::portal::app_id(app)?;
        let source = self.0.lock().expect("shortcut history");
        let path = match &*source {
            Source::Memory(values) => return Ok(values.get(app).cloned().unwrap_or_default()),
            Source::File(path) => path,
        };
        let Some(identity) = metadata(path)? else {
            return Ok(Saved::default());
        };
        let db = connect(path, true)?;
        if uninitialized(&db)? {
            ensure!(
                metadata(path)? == Some(identity),
                "Shortcut history changed while reading"
            );
            return Ok(Saved::default());
        }
        schema(&db)?;
        let mut statement = db.prepare("SELECT action,description,trigger FROM choices WHERE app=?1 ORDER BY position LIMIT 65")?;
        let rows = statement.query_map([app], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut saved = Saved::default();
        for row in rows {
            let (id, description, trigger) = row?;
            saved.actions.push(Action {
                id: id.clone(),
                description,
                preferred_trigger: None,
            });
            if let Some(trigger) = trigger {
                saved.bindings.push(Binding { id, trigger });
            }
        }
        saved.validate()?;
        ensure!(
            metadata(path)? == Some(identity),
            "Shortcut history changed while reading"
        );
        Ok(saved)
    }
    fn write(&self, app: &str, saved: Saved) -> Result<()> {
        super::portal::app_id(app)?;
        let mut source = self.0.lock().expect("shortcut history");
        let path = match &mut *source {
            Source::Memory(values) => {
                if !values.contains_key(app) && values.len() >= MAX_APPS {
                    let key = values.keys().min().cloned().expect("nonempty history");
                    values.remove(&key);
                }
                values.insert(app.into(), saved);
                return Ok(());
            }
            Source::File(path) => path,
        };
        if metadata(path)?.is_none() {
            let parent = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid history directory"))?;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
            let directory = fs::symlink_metadata(parent)?;
            ensure!(
                directory.is_dir()
                    && directory.uid() == rustix::process::geteuid().as_raw()
                    && directory.mode() & 0o022 == 0,
                "Unsafe shortcut history directory"
            );
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                .open(&*path)
            {
                Ok(file) => file.sync_all()?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let identity =
            metadata(path)?.ok_or_else(|| anyhow::anyhow!("Shortcut history disappeared"))?;
        let mut db = connect(path, false)?;
        let transaction = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if uninitialized(&transaction)? {
            transaction.execute_batch("PRAGMA application_id=1129205843; PRAGMA user_version=1;
                CREATE TABLE apps(app TEXT PRIMARY KEY, updated INTEGER NOT NULL) STRICT;
                CREATE TABLE choices(app TEXT NOT NULL REFERENCES apps(app) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position>=0 AND position<64), action TEXT NOT NULL, description TEXT NOT NULL, trigger TEXT, PRIMARY KEY(app,action), UNIQUE(app,position)) STRICT;")?;
        }
        schema(&transaction)?;
        let revision: i64 =
            transaction.query_row("SELECT coalesce(max(updated),0) FROM apps", [], |r| {
                r.get(0)
            })?;
        let revision = revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Shortcut history revision exhausted"))?;
        transaction.execute("INSERT INTO apps VALUES(?1,?2) ON CONFLICT(app) DO UPDATE SET updated=excluded.updated",params![app,revision])?;
        transaction.execute("DELETE FROM choices WHERE app=?1", [app])?;
        for (position, action) in saved.actions.iter().enumerate() {
            let trigger = saved
                .bindings
                .iter()
                .find(|b| b.id == action.id)
                .map(|b| b.trigger.as_str());
            transaction.execute(
                "INSERT INTO choices VALUES(?1,?2,?3,?4,?5)",
                params![app, position as i64, action.id, action.description, trigger],
            )?;
        }
        transaction.execute("DELETE FROM apps WHERE app NOT IN (SELECT app FROM apps ORDER BY updated DESC LIMIT 128)",[])?;
        ensure!(
            metadata(path)? == Some(identity),
            "Shortcut history changed before commit"
        );
        transaction.commit()?;
        ensure!(
            metadata(path)? == Some(identity),
            "Shortcut history changed after commit"
        );
        Ok(())
    }
}
fn metadata(path: &Path) -> Result<Option<(u64, u64)>> {
    let value = match fs::symlink_metadata(path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        value.is_file()
            && value.len() <= MAX_FILE
            && value.nlink() == 1
            && value.uid() == rustix::process::geteuid().as_raw()
            && value.mode() & 0o077 == 0,
        "Unsafe shortcut history file"
    );
    Ok(Some((value.dev(), value.ino())))
}
fn connect(path: &Path, readonly: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let db = Connection::open_with_flags(
        path,
        flags | OpenFlags::SQLITE_OPEN_NOFOLLOW | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_millis(250))?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
    db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON;")?;
    if !readonly {
        db.execute_batch("PRAGMA synchronous=FULL;")?;
    }
    Ok(db)
}
fn uninitialized(db: &Connection) -> Result<bool> {
    let application: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let count: i64 = db.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
    Ok(application == 0 && version == 0 && count == 0)
}
fn schema(db: &Connection) -> Result<()> {
    let application: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let journal: String = db.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    ensure!(
        application == APPLICATION_ID && version == 1 && journal == "delete",
        "Unrecognized shortcut history schema"
    );
    let objects: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    let tables: i64 = db.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN ('apps','choices')",
        [],
        |r| r.get(0),
    )?;
    ensure!(
        objects == 2 && tables == 2,
        "Unrecognized shortcut history tables"
    );
    let apps: i64 = db.query_row("SELECT count(*) FROM apps", [], |r| r.get(0))?;
    let rows: i64 = db.query_row("SELECT count(*) FROM choices", [], |r| r.get(0))?;
    ensure!(
        apps <= MAX_APPS as i64 && rows <= (MAX_APPS * MAX_ACTIONS) as i64,
        "Shortcut history exceeds limits"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actions() -> Actions {
        Actions(vec![
            Action {
                id: "record".into(),
                description: "Record something".into(),
                preferred_trigger: Some("CTRL+r".into()),
            },
            Action {
                id: "stop".into(),
                description: "Stop recording".into(),
                preferred_trigger: None,
            },
        ])
    }
    #[tokio::test]
    async fn an_interrupted_first_initialization_recovers_without_deleting_the_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shortcuts.db");
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        let history = History::file(path.clone()).unwrap();
        assert!(
            history
                .load("example.App".into())
                .await
                .unwrap()
                .actions
                .is_empty()
        );
        history
            .save("example.App".into(), actions(), [(1, "F9".into())].into())
            .await
            .unwrap();
        assert_eq!(
            history.load("example.App".into()).await.unwrap().bindings[0].trigger,
            "F9"
        );
    }
    #[tokio::test]
    async fn missing_history_is_read_only_and_committed_choices_survive_reopening_with_disabled_rows()
     {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private/shortcuts.db");
        let history = History::file(path.clone()).unwrap();
        assert!(
            history
                .load("example.App".into())
                .await
                .unwrap()
                .actions
                .is_empty()
        );
        assert!(!path.exists());
        history
            .save("example.App".into(), actions(), [(1, "F9".into())].into())
            .await
            .unwrap();
        let saved = History::file(path.clone())
            .unwrap()
            .load("example.App".into())
            .await
            .unwrap();
        assert_eq!(saved.actions.len(), 2);
        assert!(saved.actions[0].preferred_trigger.is_none());
        assert_eq!(
            saved.bindings,
            vec![Binding {
                id: "record".into(),
                trigger: "F9".into()
            }]
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        history
            .save("example.App".into(), actions(), BTreeMap::new())
            .await
            .unwrap();
        assert!(
            history
                .load("example.App".into())
                .await
                .unwrap()
                .bindings
                .is_empty()
        );
        assert!(
            history
                .load("another.App".into())
                .await
                .unwrap()
                .actions
                .is_empty()
        );
    }
    #[tokio::test]
    async fn foreign_database_symlinks_hardlinks_and_publicly_readable_choices_are_refused() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shortcuts.db");
        let history = History::file(path.clone()).unwrap();
        history
            .save("example.App".into(), actions(), [(1, "F9".into())].into())
            .await
            .unwrap();
        let alias = directory.path().join("alias");
        symlink(&path, &alias).unwrap();
        assert!(
            History::file(alias)
                .unwrap()
                .load("example.App".into())
                .await
                .is_err()
        );
        let link = directory.path().join("link");
        fs::hard_link(&path, &link).unwrap();
        assert!(history.load("example.App".into()).await.is_err());
        fs::remove_file(link).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(history.load("example.App".into()).await.is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA application_id=7;").unwrap();
        assert!(history.load("example.App".into()).await.is_err());
        assert!(
            history
                .save("example.App".into(), actions(), BTreeMap::new())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn concurrent_applications_keep_independent_choices_and_invalid_ids_cannot_be_saved() {
        let history = History::memory();
        let a = history.clone();
        let b = history.clone();
        let (a, b) = tokio::join!(
            a.save("one.App".into(), actions(), [(1, "F9".into())].into()),
            b.save("two.App".into(), actions(), [(2, "F10".into())].into())
        );
        a.unwrap();
        b.unwrap();
        assert_eq!(
            history.load("one.App".into()).await.unwrap().bindings[0].id,
            "record"
        );
        assert_eq!(
            history.load("two.App".into()).await.unwrap().bindings[0].id,
            "stop"
        );
        assert!(
            history
                .save("one.App".into(), actions(), [(3, "F11".into())].into())
                .await
                .is_err()
        );
        assert!(history.load("".into()).await.is_err());
    }
}
