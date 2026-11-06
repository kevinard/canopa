//! Passive bounded command widgets. No input, credentials, actions or UI types.
use anyhow::{Result, ensure};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncReadExt;
#[derive(Clone, Debug)]
pub struct Script {
    pub argv: Vec<String>,
    pub interval: u64,
    pub timeout: u64,
    pub format: String,
    pub size: u32,
    pub color: String,
    pub preview: String,
    pub directory: Option<PathBuf>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            interval: 30,
            timeout: 5,
            format: "{output}".into(),
            size: 15,
            color: "muted".into(),
            preview: String::new(),
            directory: None,
        }
    }
}
struct Group(u32);
impl Drop for Group {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0 as libc::pid_t), libc::SIGKILL);
        }
    }
}
pub async fn capture(script: &Script) -> Result<String> {
    use std::{os::unix::process::CommandExt, process::Stdio};
    ensure!(!script.argv.is_empty(), "empty widget command");
    let mut command = std::process::Command::new(&script.argv[0]);
    command
        .args(&script.argv[1..])
        .env_remove("GREETD_SOCK")
        .env_remove("NOTIFY_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    if let Some(path) = &script.directory {
        command.current_dir(path);
    }
    let parent_pid = std::process::id() as libc::pid_t;
    // SAFETY: scalar async-signal-safe syscalls only. A killed consumer must not
    // leave the command leader running with its stdout pipe held open.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent_pid {
                return Err(std::io::Error::other("widget consumer exited"));
            }
            Ok(())
        });
    }
    let mut child = tokio::process::Command::from(command)
        .kill_on_drop(true)
        .spawn()?;
    let _group = Group(
        child
            .id()
            .ok_or_else(|| anyhow::anyhow!("widget process unavailable"))?,
    );
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("widget output unavailable"))?;
    tokio::time::timeout(Duration::from_secs(script.timeout), async {
        let mut bytes = Vec::new();
        stdout.take(16385).read_to_end(&mut bytes).await?;
        ensure!(bytes.len() <= 16384, "widget output exceeds 16 KiB");
        let status = child.wait().await?;
        ensure!(status.success(), "widget command failed");
        let text = String::from_utf8(bytes)?;
        Ok::<_, anyhow::Error>(
            text.trim()
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .take(1024)
                .collect(),
        )
    })
    .await?
}
pub async fn observe(script: Script, sender: tokio::sync::watch::Sender<Option<String>>) {
    loop {
        let value = capture(&script).await.ok();
        sender.send_if_modified(|old| {
            if *old == value {
                false
            } else {
                *old = value;
                true
            }
        });
        tokio::select! {_=sender.closed()=>return,_=tokio::time::sleep(Duration::from_secs(script.interval))=>{}}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn captures_utf8_but_rejects_failures_timeouts_and_oversized_output() {
        let mut script = Script {
            argv: vec!["printf".into(), "hello".into()],
            ..Default::default()
        };
        assert_eq!(capture(&script).await.unwrap(), "hello");
        script.argv = vec!["sh".into(), "-c".into(), "exit 1".into()];
        assert!(capture(&script).await.is_err());
        script.argv = vec!["sh".into(), "-c".into(), "head -c 20000 /dev/zero".into()];
        assert!(capture(&script).await.is_err());
        script.timeout = 1;
        script.argv = vec!["sleep".into(), "30".into()];
        assert!(capture(&script).await.is_err());
    }
}
