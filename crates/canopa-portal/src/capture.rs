//! Request-owned screenshot pixels. Capture never selects clipboard contents or
//! publishes a file; consent and publication belong to the portal workflow.
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

#[cfg(test)]
mod tests;
mod wayland;
pub(crate) mod window;

pub(crate) const MAX_OUTPUTS: usize = 16;
pub(crate) const MAX_SIDE: u32 = 16_384;
pub(crate) const MAX_PIXELS: usize = 64 * 1024 * 1024;
pub(crate) const MAX_BYTES: usize = MAX_PIXELS * 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LogicalRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct OutputFrame {
    pub name: String,
    pub logical: LogicalRect,
    /// Upright physical pixels, independent of subsequent output/layout changes.
    pub pixels: image::RgbaImage,
}

pub struct Captured {
    outputs: Vec<OutputFrame>,
    // Admission covers full pixels through review, not merely the Wayland read.
    _permit: OwnedSemaphorePermit,
}

impl Captured {
    pub fn outputs(&self) -> &[OutputFrame] {
        &self.outputs
    }
    pub(crate) fn into_parts(self) -> (Vec<OutputFrame>, OwnedSemaphorePermit) {
        (self.outputs, self._permit)
    }
}

#[derive(Clone)]
pub struct Captures {
    socket: Option<PathBuf>,
    slots: Arc<Semaphore>,
    deadline: Duration,
}

impl Captures {
    /// Resolve a service's display without consuming WAYLAND_SOCKET or mutating
    /// the environment shared with Iced helpers. Each request needs a new socket.
    pub fn from_environment() -> Self {
        let socket = display_socket(
            std::env::var_os("WAYLAND_DISPLAY").map(PathBuf::from),
            std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
            std::env::var_os("WAYLAND_SOCKET").is_some(),
        );
        Self::new(socket)
    }

    pub fn new(socket: Option<PathBuf>) -> Self {
        Self {
            socket,
            slots: Arc::new(Semaphore::new(2)),
            deadline: Duration::from_secs(10),
        }
    }

    pub async fn capture(&self, mut cancelled: watch::Receiver<bool>) -> anyhow::Result<Captured> {
        anyhow::ensure!(
            !*cancelled.borrow() && cancelled.has_changed().is_ok(),
            "Screenshot cancelled"
        );
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("Screenshot busy"))?;
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Screenshot display unavailable"))?;
        let end = Instant::now() + self.deadline;
        let connect = tokio::net::UnixStream::connect(socket);
        let stream = tokio::select! {
            biased;
            _ = cancelled.changed() => anyhow::bail!("Screenshot cancelled"),
            result = tokio::time::timeout(self.deadline, connect) => result
                .map_err(|_| anyhow::anyhow!("Screenshot timed out"))?
                .map_err(|_| anyhow::anyhow!("Screenshot connection unavailable"))?,
        };
        let stream = stream.into_std()?;
        let stop = Stop(Arc::new(AtomicBool::new(false)));
        let worker_stop = stop.0.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let outputs = wayland::capture(stream, &worker_stop, end)?;
            Ok::<_, anyhow::Error>(Captured {
                outputs,
                _permit: permit,
            })
        });
        let captured = tokio::select! {
            biased;
            _ = cancelled.changed() => anyhow::bail!("Screenshot cancelled"),
            result = worker => result.map_err(|_| anyhow::anyhow!("Screenshot worker stopped"))??,
        };
        anyhow::ensure!(!*cancelled.borrow(), "Screenshot cancelled");
        Ok(captured)
    }
}

fn display_socket(
    display: Option<PathBuf>,
    runtime: Option<PathBuf>,
    inherited: bool,
) -> Option<PathBuf> {
    // An inherited stream cannot be duplicated into independent connections.
    // Guessing another socket could capture a different desktop from Iced.
    if inherited {
        return None;
    }
    let display = display.filter(|path| !path.as_os_str().is_empty())?;
    if display.is_absolute() {
        Some(display)
    } else {
        runtime
            .filter(|path| path.is_absolute())
            .map(|path| path.join(display))
    }
}

struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
