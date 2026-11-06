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
    fn title(&self) -> &'static str {
        if self.capture_input {
            return "Input capture";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "Clipboard sharing"
            } else {
                "Desktop sharing"
            };
        }
        if self.devices == 0 {
            "Screen sharing"
        } else {
            "Remote control"
        }
    }
    fn heading(&self) -> &'static str {
        if self.capture_input {
            return "Input capture is allowed";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "Clipboard sharing is active"
            } else {
                "Desktop sharing is active"
            };
        }
        if self.devices == 0 {
            "Screen sharing is active"
        } else if self.sources.is_empty() {
            match self.devices {
                1 => "Keyboard control is active",
                2 => "Pointer control is active",
                _ => "Keyboard and pointer control are active",
            }
        } else {
            "Sharing and control are active"
        }
    }
    fn stop_label(&self) -> &'static str {
        if self.capture_input {
            if self.clipboard {
                return "Stop input and clipboard sharing";
            }
            return "Stop input capture";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "Stop sharing"
            } else {
                "Stop all sharing"
            };
        }
        if self.devices == 0 {
            "Stop sharing"
        } else if self.sources.is_empty() {
            "Stop control"
        } else {
            "Stop sharing and control"
        }
    }
    fn hint(&self) -> &'static str {
        if self.capture_input {
            if self.clipboard {
                return "Ctrl+Alt+Esc pauses input and clipboard sharing. Stop or close ends both.";
            }
            return "Ctrl+Alt+Esc pauses capture. Stop or close ends this session.";
        }
        if self.clipboard {
            return if self.devices == 0 && self.sources.is_empty() {
                "Esc or closing this window stops clipboard sharing"
            } else {
                "Esc or closing this window stops all sharing"
            };
        }
        if self.devices == 0 {
            "Esc or closing this window stops sharing"
        } else if self.sources.is_empty() {
            "Esc or closing this window stops control"
        } else {
            "Esc or closing this window stops sharing and control"
        }
    }
    fn control_label(&self) -> &'static str {
        if self.capture_input {
            return match self.devices {
                1 => "Keyboard input · Screen edges",
                2 => "Pointer input · Screen edges",
                _ => "Keyboard and pointer input · Screen edges",
            };
        }
        match self.devices {
            1 => "Keyboard control · Desktop",
            2 => "Pointer control · Desktop",
            4 => "Touch control · Shared screen or window",
            5 => "Keyboard and touch control",
            6 => "Pointer and touch control",
            7 => "Keyboard, pointer and touch control",
            _ => "Keyboard and pointer control · Desktop",
        }
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
