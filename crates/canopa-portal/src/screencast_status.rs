//! Session-owned sharing controls. The short-lived Start request owns no UI lease.
use crate::process::DialogProcess;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{oneshot, watch},
};

#[cfg(test)]
mod tests;
mod ui;

const FRAME_BYTES: usize = 16 * 1024;
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub requester: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub devices: u32,
    #[serde(default)]
    pub clipboard: bool,
    #[serde(default)]
    pub capture_input: bool,
}
impl Input {
    pub(crate) fn new(requester: &str, sources: Vec<String>) -> anyhow::Result<Self> {
        Self::for_remote(requester, sources, 0)
    }
    pub(crate) fn for_remote(
        requester: &str,
        sources: Vec<String>,
        devices: u32,
    ) -> anyhow::Result<Self> {
        Self::for_remote_clipboard(requester, sources, devices, false)
    }
    pub(crate) fn for_remote_clipboard(
        requester: &str,
        sources: Vec<String>,
        devices: u32,
        clipboard: bool,
    ) -> anyhow::Result<Self> {
        let clean = |value: &str| {
            value
                .chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect()
        };
        let input = Self {
            requester: clean(requester),
            sources: sources.iter().map(|source| clean(source)).collect(),
            devices,
            clipboard,
            capture_input: false,
        };
        input.validate()?;
        Ok(input)
    }
    pub(crate) fn for_capture(
        requester: &str,
        devices: u32,
        clipboard: bool,
    ) -> anyhow::Result<Self> {
        let mut input = Self::for_remote_clipboard(requester, Vec::new(), devices, clipboard)?;
        input.capture_input = true;
        input.validate()?;
        Ok(input)
    }
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.requester.len() <= 512
                && (!self.capture_input || self.devices != 0 && self.sources.is_empty())
                && !self.requester.chars().any(char::is_control)
                && self.devices & !(if self.capture_input { 3 } else { 7 }) == 0
                && (self.devices & 4 == 0 || !self.sources.is_empty())
                && (!self.sources.is_empty() || self.devices != 0 || self.clipboard)
                && self.sources.len() <= 16
                && self.sources.iter().all(|source| {
                    !source.trim().is_empty()
                        && source.len() <= 512
                        && !source.chars().any(char::is_control)
                }),
            "Invalid sharing controls"
        );
        Ok(())
    }
    fn title_kind(&self) -> &'static str {
        if self.capture_input {
            return "input-capture";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "clipboard-sharing"
            } else {
                "desktop-sharing"
            };
        }
        if self.devices == 0 {
            "screen-sharing"
        } else {
            "remote-control"
        }
    }
    fn heading_kind(&self) -> &'static str {
        if self.capture_input {
            return "input-capture-is-allowed";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "clipboard-sharing-is-active"
            } else {
                "desktop-sharing-is-active"
            };
        }
        if self.devices == 0 {
            "screen-sharing-is-active"
        } else if self.sources.is_empty() {
            match self.devices {
                1 => "keyboard-control-is-active",
                2 => "pointer-control-is-active",
                _ => "keyboard-and-pointer-control-are-active",
            }
        } else {
            "sharing-and-control-are-active"
        }
    }
    fn stop_label_kind(&self) -> &'static str {
        if self.capture_input {
            if self.clipboard {
                return "stop-input-and-clipboard-sharing";
            }
            return "stop-input-capture";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "stop-sharing"
            } else {
                "stop-all-sharing"
            };
        }
        if self.devices == 0 {
            "stop-sharing"
        } else if self.sources.is_empty() {
            "stop-control"
        } else {
            "stop-sharing-and-control"
        }
    }
    fn hint_kind(&self) -> &'static str {
        if self.capture_input {
            if self.clipboard {
                return "capture-clipboard";
            }
            return "capture";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "clipboard"
            } else {
                "all"
            };
        }
        if self.devices == 0 {
            "sharing"
        } else if self.sources.is_empty() {
            "control"
        } else {
            "sharing-control"
        }
    }
    fn control_label_kind(&self) -> &'static str {
        if self.capture_input {
            return match self.devices {
                1 => "keyboard-input-screen-edges",
                2 => "pointer-input-screen-edges",
                _ => "keyboard-and-pointer-input-screen-edges",
            };
        }
        match self.devices {
            1 => "keyboard-control-desktop",
            2 => "pointer-control-desktop",
            4 => "touch-control-shared-screen-or-window",
            5 => "keyboard-and-touch-control",
            6 => "pointer-and-touch-control",
            7 => "keyboard-pointer-and-touch-control",
            _ => "keyboard-and-pointer-control-desktop",
        }
    }
    fn title(&self) -> String {
        fl!("portal-sharing-title", kind = self.title_kind())
    }
    fn heading(&self) -> String {
        fl!("portal-sharing-heading", kind = self.heading_kind())
    }
    fn stop_label(&self) -> String {
        fl!("portal-sharing-stop", kind = self.stop_label_kind())
    }
    fn hint(&self) -> String {
        fl!("portal-sharing-hint", kind = self.hint_kind())
    }
    fn control_label(&self) -> String {
        fl!("portal-sharing-control", kind = self.control_label_kind())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    ready: bool,
}

pub(crate) struct Indicator {
    lease: Option<oneshot::Sender<()>>,
    closed: watch::Receiver<bool>,
    done: Option<oneshot::Receiver<anyhow::Result<()>>>,
}
impl Drop for Indicator {
    fn drop(&mut self) {
        drop(self.lease.take());
    }
}
impl Indicator {
    pub(crate) async fn start(
        process: &DialogProcess,
        input: Input,
        mut source_closed: watch::Receiver<bool>,
        mut cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        input.validate()?;
        anyhow::ensure!(
            !*cancelled.borrow() && !*source_closed.borrow(),
            "Capture ended"
        );
        let mut bytes = serde_json::to_vec(&input)?;
        anyhow::ensure!(bytes.len() < FRAME_BYTES, "Sharing controls exceed limit");
        bytes.push(b'\n');
        let mut child = process.spawn("--screencast-status", None)?;
        let mut input_pipe = child.stdin.take().expect("piped sharing input");
        let mut output =
            BufReader::new(child.stdout.take().expect("piped sharing output").take(257));
        let (lease, mut released) = oneshot::channel();
        let (ready, response) = oneshot::channel();
        let (done, finished) = oneshot::channel();
        let (closed, stopped) = watch::channel(false);
        let indicator = Self {
            lease: Some(lease),
            closed: stopped,
            done: Some(finished),
        };
        tokio::spawn(async move {
            let opening = async {
                input_pipe.write_all(&bytes).await?;
                input_pipe.flush().await?;
                let mut line = Vec::new();
                output.read_until(b'\n', &mut line).await?;
                anyhow::ensure!(
                    line.len() <= 256 && line.last() == Some(&b'\n'),
                    "Invalid sharing readiness"
                );
                let reply: Ready = serde_json::from_slice(&line)?;
                anyhow::ensure!(reply.ready, "Sharing controls unavailable");
                Ok::<_, anyhow::Error>(())
            };
            let result = tokio::select! {
                biased;
                _ = &mut released => Err(anyhow::anyhow!("Sharing controls cancelled")),
                _ = stopped_source(&mut source_closed) => Err(anyhow::anyhow!("Capture ended")),
                result = tokio::time::timeout(READY_TIMEOUT, opening) => result.map_err(anyhow::Error::from).and_then(|value| value),
            };
            let available = result.is_ok();
            if ready.send(result).is_ok() && available {
                // Any extra output or EOF means the control surface is no longer reliable.
                let mut extra = [0_u8; 1];
                tokio::select! {
                    _ = &mut released => {},
                    _ = stopped_source(&mut source_closed) => {},
                    _ = output.read(&mut extra) => {},
                    _ = child.wait() => {},
                }
            }
            // Revoke immediately; waiting for a misbehaving UI must not delay capture Stop.
            closed.send_replace(true);
            drop(input_pipe);
            let retired = match tokio::time::timeout(EXIT_TIMEOUT, child.wait()).await {
                Ok(result) => result.map(|_| ()).map_err(anyhow::Error::from),
                Err(_) => child.kill().await.map_err(anyhow::Error::from),
            };
            if retired.is_err() {
                eprintln!("level=warn component=screencast_status event=cleanup_unconfirmed");
            }
            let _ = done.send(retired);
        });
        let opening = tokio::select! {
            biased;
            _ = stopped_source(&mut cancelled) => Err(anyhow::anyhow!("Sharing controls cancelled")),
            result = response => result.map_err(anyhow::Error::from).and_then(|value|value),
        };
        let stopped = *indicator.closed.borrow();
        if opening.is_err() || stopped {
            indicator.stop().await?;
            opening?;
            anyhow::bail!("Sharing controls stopped");
        }
        Ok(indicator)
    }
    pub(crate) fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub(crate) async fn stop(mut self) -> anyhow::Result<()> {
        drop(self.lease.take());
        self.done.take().expect("sharing retirement").await?
    }
}

async fn stopped_source(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

pub fn run(
    input: Input,
    appearance: canopa_theme::Appearance,
    lease: impl std::io::BufRead + Send + 'static,
) -> anyhow::Result<()> {
    input.validate()?;
    ui::run(input, appearance, lease)
}
