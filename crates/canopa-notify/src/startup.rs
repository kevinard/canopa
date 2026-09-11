//! Process ownership and filesystem paths. Acquired before opening mutable state.
use anyhow::{Context, bail};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;

pub(crate) struct DaemonPaths {
    pub socket: PathBuf,
    pub database: PathBuf,
    legacy_database: Option<PathBuf>,
}

impl DaemonPaths {
    pub fn resolve(socket: PathBuf, database: Option<PathBuf>) -> anyhow::Result<Self> {
        let socket = std::path::absolute(socket)?;
        let legacy_database = database.is_none().then(|| socket.with_extension("db"));
        let database = match database {
            Some(path) => std::path::absolute(path)?,
            None => xdg_home("XDG_STATE_HOME", ".local/state")?.join("canopa-notify/history.db"),
        };
        Ok(Self {
            socket,
            database,
            legacy_database,
        })
    }

    pub async fn import_legacy_history(&self) -> anyhow::Result<()> {
        let Some(source) = self.legacy_database.as_ref().filter(|source| {
            **source != self.database && source.exists() && !self.database.exists()
        }) else {
            return Ok(());
        };
        check_owned_regular_file(source)?;
        // The source may belong to a daemon listening on a different socket.
        // Keep its normal database ownership lock until publication completes.
        let _source_ownership = lock_path(source)
            .context("legacy notification history is in use or cannot be locked; stop its daemon before importing")?;
        let parent = self.database.parent().context("database has no parent")?;
        let copy = tempfile::NamedTempFile::new_in(parent)?;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(source)
                    .read_only(true),
            )
            .await?;
        // SQLite copies a consistent snapshot, including committed WAL contents.
        let result = sqlx::query("VACUUM INTO ?1")
            .bind(
                copy.path()
                    .to_str()
                    .context("database import path must be UTF-8")?,
            )
            .execute(&pool)
            .await;
        pool.close().await;
        result.context("failed to import legacy notification history; source preserved")?;
        let check = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(copy.path())
                    .read_only(true),
            )
            .await?;
        let integrity: String = sqlx::query_scalar("PRAGMA quick_check")
            .fetch_one(&check)
            .await?;
        check.close().await;
        anyhow::ensure!(
            integrity == "ok",
            "imported notification history failed integrity verification"
        );
        copy.as_file().sync_all()?;
        copy.persist_noclobber(&self.database)
            .context("could not publish imported history; source preserved")?;
        File::open(parent)?.sync_all()?;
        eprintln!(
            "level=info component=storage event=legacy_history_imported source_preserved=true"
        );
        Ok(())
    }
}

pub(crate) fn xdg_home(variable: &str, fallback: &str) -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .context("an absolute HOME or XDG path is required")?;
    Ok(home.join(fallback))
}

pub(crate) fn configuration_path(
    override_path: Option<PathBuf>,
) -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = override_path {
        return Ok(Some(std::path::absolute(path)?));
    }
    let path = xdg_home("XDG_CONFIG_HOME", ".config")?.join("canopa-notify/config.kdl");
    Ok(path.try_exists()?.then_some(path))
}

pub(crate) struct StartupOwnership {
    pub listener: UnixListener,
    socket: PathBuf,
    identity: (u64, u64),
    _locks: Vec<File>,
}

impl StartupOwnership {
    pub fn acquire(paths: &DaemonPaths) -> anyhow::Result<Self> {
        private_parent(&paths.socket)?;
        let mut locks = vec![lock_path(&paths.socket)?];
        match fs::symlink_metadata(&paths.socket) {
            Ok(metadata) => {
                anyhow::ensure!(
                    metadata.file_type().is_socket()
                        && metadata.uid() == rustix::process::geteuid().as_raw(),
                    "refusing to remove a non-socket or foreign control endpoint"
                );
                match UnixStream::connect(&paths.socket) {
                    Ok(_) => bail!(
                        "Canopa Notify daemon already running at {}",
                        paths.socket.display()
                    ),
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                        ) =>
                    {
                        fs::remove_file(&paths.socket)?;
                    }
                    Err(error) => {
                        return Err(error)
                            .context("could not determine control endpoint ownership");
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        private_parent(&paths.database)?;
        locks.push(lock_path(&paths.database)?);
        if paths.database.exists() {
            check_owned_regular_file(&paths.database)?;
        }
        let listener =
            UnixListener::bind(&paths.socket).context("failed to bind control socket")?;
        fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(&paths.socket)?;
        Ok(Self {
            listener,
            socket: paths.socket.clone(),
            identity: (metadata.dev(), metadata.ino()),
            _locks: locks,
        })
    }
}

impl Drop for StartupOwnership {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.socket).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.socket);
        }
        // Keep lock files: unlinking a locked inode lets a competitor lock a new inode.
    }
}

fn private_parent(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    Ok(())
}

fn check_owned_regular_file(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && metadata.uid() == rustix::process::geteuid().as_raw(),
        "notification state must be an owned regular file"
    );
    Ok(())
}

fn lock_path(path: &Path) -> anyhow::Result<File> {
    let mut lock = path.as_os_str().to_os_string();
    lock.push(".lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(PathBuf::from(lock))?;
    let metadata = file.metadata()?;
    anyhow::ensure!(
        metadata.is_file() && metadata.uid() == rustix::process::geteuid().as_raw(),
        "invalid notification ownership lock"
    );
    file.try_lock().map_err(|error| {
        anyhow::anyhow!("Canopa Notify daemon already running or state locked: {error}")
    })?;
    Ok(file)
}
