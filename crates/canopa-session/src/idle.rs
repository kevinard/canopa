//! Stasis remains an external policy process; Canopa owns session/lock coordination.
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::OsString,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

/// The compositor whose `msg` actions the Stasis rules run: the explicit
/// `CANOPA_NIRI_BIN`, else the bundled Canopa compositor, else `niri` from `PATH`,
/// the same choice Canopa Settings makes.
pub fn compositor() -> PathBuf {
    std::env::var_os("CANOPA_NIRI_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let bundled = PathBuf::from("/usr/lib/canopa/niri");
            if bundled.is_file() {
                bundled
            } else {
                PathBuf::from("niri")
            }
        })
}
pub fn config_path() -> Result<PathBuf> {
    Ok(canopa_config::config_home()?.join("canopa/stasis.rune"))
}
pub fn initialize(path: &Path) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .context("configuration needs a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(include_bytes!("../../../packaging/stasis/stasis.rune"))?;
    file.sync_all()?;
    Ok(())
}
pub fn executable(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() || path.components().count() > 1 {
        std::path::absolute(path)?
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .filter(|p| p.is_absolute())
            .map(|p| p.join(path))
            .find(|p| {
                p.metadata()
                    .is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0)
            })
            .with_context(|| fl!("idle-not-installed", program = path.display().to_string()))?
    };
    let candidate = candidate.canonicalize()?;
    let metadata = candidate.metadata()?;
    ensure!(
        metadata.is_file() && metadata.mode() & 0o111 != 0,
        "backend must be an executable regular file"
    );
    Ok(candidate)
}
fn stasis_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command
        .env_remove("NOTIFY_SOCKET")
        .env_remove("GREETD_SOCK");
    // A mostly sleeping backend/IPC client does not need one reactor worker per
    // CPU. Preserve an explicitly supplied upstream setting.
    if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        command.env("TOKIO_WORKER_THREADS", "1");
    }
    command
}
pub fn forward(program: &Path, args: Vec<OsString>) -> Result<()> {
    Err(stasis_command(executable(program)?)
        .args(args)
        .exec()
        .into())
}
fn supervisor_guard() -> Result<std::fs::File> {
    use std::os::fd::AsRawFd;
    let path = crate::ipc::scoped_socket_path("idle-supervisor")?.with_extension("lock");
    let guard = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    ensure!(
        unsafe { libc::flock(guard.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "{}",
        fl!("idle-already-running")
    );
    Ok(guard)
}
fn watch_line(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= 16384, "Stasis watch frame exceeds budget");
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    ensure!(
        value
            .get("state")
            .and_then(|v| v.as_str())
            .is_some_and(|s| ["waiting", "active", "inhibited", "locked", "manual"].contains(&s))
            && value.get("paused").and_then(|v| v.as_bool()).is_some(),
        "invalid Stasis watch state"
    );
    Ok(())
}
async fn next_watch(reader: &mut BufReader<tokio::process::ChildStdout>) -> Result<()> {
    let mut bytes = Vec::new();
    // read_until alone would allocate without bound for a broken/malicious peer.
    use tokio::io::AsyncReadExt;
    let count = reader.take(16385).read_until(b'\n', &mut bytes).await?;
    ensure!(
        count > 0 && bytes.last() == Some(&b'\n'),
        "Stasis watch disconnected"
    );
    watch_line(&bytes)
}
async fn stop(child: &mut Child, group: Option<u32>) {
    if let Some(pid) = group {
        // Group identity comes only from our process_group(0) child. This also
        // retires command helpers if the backend leader has already exited.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }
        if tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_err()
        {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            let _ = child.wait().await;
        }
    }
}

fn backend(program: &Path, session: &str) -> Result<Command> {
    // Stasis executes this path later, after an upgrade may have replaced it.
    let here = canopa_file::executable::installed()?;
    let directory = here.parent().context("missing executable directory")?;
    let locker = directory.join("canopa-lock");
    ensure!(locker.is_file(), "{}", fl!("idle-locker-missing"));
    let mut command = stasis_command(program);
    command
        .env("XDG_SESSION_ID", session)
        .env("CANOPA_LOCK", locker)
        .env("CANOPA_IDLE", here)
        .env("CANOPA_NIRI", executable(&compositor())?)
        .stdin(Stdio::null())
        .process_group(0);
    Ok(Command::from(command))
}
pub async fn suspend(selected: Option<&str>) -> Result<()> {
    let target = crate::logind::selected(selected).await?;
    let status = crate::control::status().await?;
    ensure!(
        status.session == target.id && status.sleep_coordinator,
        "{}",
        fl!("idle-coordinator-unavailable")
    );
    crate::control::ready(Duration::from_secs(5)).await?;
    // The listener already owns a delay inhibitor and will handle PrepareForSleep.
    tokio::time::timeout(
        Duration::from_secs(5),
        crate::logind::manager(&target)
            .await?
            .call::<_, _, ()>("Suspend", &(true,)),
    )
    .await
    .context(fl!("idle-suspend-timeout"))??;
    Ok(())
}
pub async fn run(program: &Path, config: &Path, selected: Option<&str>) -> Result<()> {
    let _guard = supervisor_guard()?;
    let target = crate::logind::selected(selected).await?;
    let (status, mut coordinator) = crate::control::watch().await?;
    ensure!(
        status.session == target.id && status.sleep_coordinator,
        "{}",
        fl!("idle-coordinator-unavailable")
    );
    let config = config.canonicalize().context(fl!("idle-config-missing"))?;
    let metadata = config.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 1024 * 1024,
        "{}",
        fl!("idle-config-invalid")
    );
    let program = executable(program)?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut child = backend(&program, &target.id)?
        .arg("--config")
        .arg(&config)
        .kill_on_drop(true)
        .spawn()?;
    let group = child.id();
    let result=async {
        use tokio::io::AsyncReadExt;
        let startup=async {
            let deadline=tokio::time::Instant::now()+Duration::from_secs(8);
            loop {
                if let Some(status)=child.try_wait()? {bail!("{}", fl!("idle-stasis-exited", status = status.to_string()));}
                let mut process=backend(&program,&target.id)?.arg("watch").stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()?;
                let mut reader=BufReader::new(process.stdout.take().context("missing Stasis watch output")?);
                if tokio::time::timeout(Duration::from_millis(500),next_watch(&mut reader)).await.is_ok_and(|r|r.is_ok()) {
                    break Ok::<_,anyhow::Error>((process,reader));
                }
                let group=process.id();stop(&mut process,group).await;
                ensure!(tokio::time::Instant::now()<deadline,"Stasis did not expose a valid watch stream");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        let (mut watch,mut reader)=tokio::select! {
            result=startup=>result?,
            _=terminate.recv()=>return Ok(()),
            _=interrupt.recv()=>return Ok(()),
            _=coordinator.read_u8()=>bail!("{}", fl!("idle-coordinator-stopped")),
        };
        let watch_group=watch.id();
        let result=async {
            crate::logind::notify_ready()?;
            loop {
                tokio::select! {
                    _=terminate.recv()=>break Ok(()),
                    _=interrupt.recv()=>break Ok(()),
                    _=coordinator.read_u8()=>break Err(anyhow::anyhow!(fl!("idle-coordinator-stopped"))),
                    status=child.wait()=>break Err(anyhow::anyhow!(fl!("idle-stasis-exited", status = status?.to_string()))),
                    event=next_watch(&mut reader)=>if let Err(error)=event {break Err(error);},
                }
            }
        }.await;
        stop(&mut watch,watch_group).await;result
    }.await;
    stop(&mut child, group).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watch_is_bounded_and_rejects_non_state_messages() {
        assert!(watch_line(br#"{"state":"active","paused":false,"future":1}"#).is_ok());
        for value in [
            br#"{"state":"unknown","paused":false}"#.as_slice(),
            br#"{"state":"active"}"#,
            b"oops",
            &vec![b' '; 16385],
        ] {
            assert!(watch_line(value).is_err());
        }
    }
    #[test]
    fn init_is_non_clobbering_and_does_not_execute_configuration() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config/stasis.rune");
        initialize(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(initialize(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert!(String::from_utf8(bytes).unwrap().contains("CANOPA_LOCK"));
    }
}
