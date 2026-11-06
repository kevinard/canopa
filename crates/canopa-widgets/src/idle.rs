//! Passive Stasis state. Consumers own controls, policy and presentation.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::mpsc,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    pub state: String,
    pub paused: bool,
    pub manually_paused: bool,
    pub profile: String,
}
impl Snapshot {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 16384, "idle snapshot exceeds budget");
        let snapshot: Self = serde_json::from_slice(bytes)?;
        ensure!(
            ["waiting", "active", "inhibited", "locked", "manual"]
                .contains(&snapshot.state.as_str())
                && snapshot.profile.len() <= 128
                && !snapshot.profile.chars().any(char::is_control),
            "invalid idle snapshot"
        );
        Ok(snapshot)
    }
}
pub fn companion() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("canopa-idle")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("canopa-idle"))
}
pub async fn observe(sender: mpsc::Sender<Option<Snapshot>>) {
    let mut retry = 1;
    loop {
        let mut available = false;
        let result = async {
            let mut child = Command::new(companion())
                .arg("watch")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()?;
            let mut reader = BufReader::new(child.stdout.take().expect("piped idle watch"));
            loop {
                let mut bytes = Vec::new();
                let mut limited = (&mut reader).take(16385);
                let count = tokio::select! {
                    _=sender.closed()=>return Ok::<_,anyhow::Error>(()),
                    count=limited.read_until(b'\n',&mut bytes)=>count?,
                };
                ensure!(
                    count > 0 && bytes.last() == Some(&b'\n'),
                    "idle watcher disconnected"
                );
                let value = Snapshot::parse(&bytes)?;
                available = true;
                if sender.send(Some(value)).await.is_err() {
                    return Ok(());
                }
            }
        }
        .await;
        if result.is_ok() || sender.is_closed() {
            return;
        }
        if sender.send(None).await.is_err() {
            return;
        }
        if available {
            retry = 1;
        }
        tokio::select! {_=sender.closed()=>return,_=tokio::time::sleep(Duration::from_secs(retry))=>{}}
        retry = (retry * 2).min(16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_preserves_manual_and_automatic_holds_and_rejects_bad_frames() {
        let value=Snapshot::parse(br#"{"state":"inhibited","paused":true,"manually_paused":false,"profile":"work","new-field":42}"#).unwrap();
        assert!(value.paused && !value.manually_paused);
        for bytes in [
            br#"{"state":"bad","paused":true,"manually_paused":false,"profile":"work"}"#.as_slice(),
            br#"{"state":"active"}"#,
            &vec![0; 16385],
        ] {
            assert!(Snapshot::parse(bytes).is_err());
        }
    }
}
