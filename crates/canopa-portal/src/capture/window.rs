//! Capture one real window through the existing creator-owned ScreenCast lease.
use super::*;
use crate::{
    process::DialogProcess,
    screencast::production::{catalog, inventory, serial::Serials},
    screencast_capture::{Cursor, Owner, Recorder, Source},
};
use zbus::Connection;

mod frame;

#[derive(Clone)]
pub(crate) struct Windows {
    captures: Captures,
    process: Arc<DialogProcess>,
    recorder: Recorder,
    serials: Serials,
    socket: Option<PathBuf>,
}
pub(crate) struct Target {
    id: u64,
    title: String,
    owner: String,
}
struct Desktop {
    connection: Connection,
    owner: Owner,
    pid: u32,
    socket: PathBuf,
}
struct Cancel(tokio::sync::watch::Sender<bool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}

impl Windows {
    pub(crate) fn new(captures: Captures, process: Arc<DialogProcess>) -> Self {
        Self {
            captures,
            recorder: Recorder::new(process.clone()),
            process,
            serials: Serials::default(),
            socket: std::env::var_os("NIRI_SOCKET").map(PathBuf::from),
        }
    }
    async fn desktop(&self, connection: Connection) -> anyhow::Result<Desktop> {
        let owner = Owner::resolve(&connection).await?;
        let pid = owner.process_id(&connection).await?;
        let socket = self
            .socket
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Window capture catalog unavailable"))?;
        // A stale NIRI_SOCKET must never capture a different desktop from Iced.
        catalog::read(&socket, pid, 0).await?;
        let display = self
            .captures
            .socket
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Window screenshot display unavailable"))?;
        let stream = tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::UnixStream::connect(display),
        )
        .await??;
        anyhow::ensure!(
            stream
                .peer_cred()?
                .pid()
                .and_then(|v| u32::try_from(v).ok())
                == Some(pid),
            "Window capture and dialog belong to different compositors"
        );
        Ok(Desktop {
            connection,
            owner,
            pid,
            socket,
        })
    }
    pub(crate) async fn available(&self, connection: &Connection) -> bool {
        tokio::time::timeout(Duration::from_secs(5), self.desktop(connection.clone()))
            .await
            .is_ok_and(|v| v.is_ok())
    }
    async fn target(desktop: &Desktop, id: u64) -> anyhow::Result<Target> {
        let items = inventory(
            &desktop.connection,
            Some(&desktop.owner),
            &desktop.socket,
            desktop.pid,
            2,
        )
        .await?;
        let item = items
            .into_iter()
            .find(|v| v.source == Source::Window(id))
            .ok_or_else(|| anyhow::anyhow!("Selected screenshot window is no longer available"))?;
        Ok(Target {
            id,
            title: item.title,
            owner: desktop.owner.unique_name().to_owned(),
        })
    }
    pub(crate) async fn focused(&self) -> anyhow::Result<Target> {
        let desktop = self.desktop(Connection::session().await?).await?;
        let focused = catalog::request(&desktop.socket, desktop.pid, "FocusedWindow").await?;
        let id = focused
            .pointer("/Ok/FocusedWindow/id")
            .and_then(serde_json::Value::as_u64)
            .filter(|id| *id != 0)
            .ok_or_else(|| anyhow::anyhow!("No active screenshot window"))?;
        Self::target(&desktop, id).await
    }
    pub(crate) async fn choose(
        &self,
        request: &crate::screenshot::ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Target>> {
        let desktop = self.desktop(Connection::session().await?).await?;
        let items = inventory(
            &desktop.connection,
            Some(&desktop.owner),
            &desktop.socket,
            desktop.pid,
            2,
        )
        .await?;
        anyhow::ensure!(!items.is_empty(), "No screenshot windows available");
        let input = crate::screencast::DialogInput {
            app_id: request.app_id.clone(),
            parent_window: request.parent_window.clone(),
            modal: request.modal,
            multiple: false,
            items,
            socket: desktop.socket.clone(),
            compositor_pid: desktop.pid,
            compositor_owner: Some(desktop.owner.unique_name().to_owned()),
            types: 2,
            activation_token: None,
        };
        let selected = self
            .process
            .exchange::<_, Vec<Source>>("--screenshot-window-picker", input, cancelled.clone())
            .await?;
        anyhow::ensure!(!*cancelled.borrow(), "Window screenshot cancelled");
        let Some(selected) = selected else {
            return Ok(None);
        };
        let [Source::Window(id)] = selected.as_slice() else {
            anyhow::bail!("Invalid screenshot window choice");
        };
        Ok(Some(Self::target(&desktop, *id).await?))
    }
    pub(crate) async fn capture(
        &self,
        target: Target,
        mut cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Captured> {
        anyhow::ensure!(
            !*cancelled.borrow() && cancelled.has_changed().is_ok(),
            "Window screenshot cancelled"
        );
        let permit = self
            .captures
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("Screenshot busy"))?;
        let (sender, stop) = watch::channel(false);
        let guard = Cancel(sender.clone());
        let this = self.clone();
        // Dropping the request signals cancellation. This retained task owns
        // admission until both the reader and compositor producer have retired.
        let task = tokio::spawn(async move {
            let desktop = this.desktop(Connection::session().await?).await?;
            anyhow::ensure!(
                desktop.owner.unique_name() == target.owner && !*stop.borrow(),
                "Screenshot compositor changed or request cancelled"
            );
            Self::target(&desktop, target.id).await?;
            let caller = desktop
                .connection
                .unique_name()
                .ok_or_else(|| anyhow::anyhow!("Screenshot bus unavailable"))?
                .to_owned();
            let recording = this
                .recorder
                .start(
                    &desktop.connection,
                    &desktop.owner,
                    caller,
                    vec![Source::Window(target.id)],
                    Cursor::Hidden,
                    stop.clone(),
                )
                .await?;
            let outcome = async {
                let [stream] = recording.streams() else { anyhow::bail!("Invalid window recording"); };
                let serials = this.serials.resolve(vec![stream.node], stop.clone()).await?;
                let serial = *serials.get(&stream.node).ok_or_else(|| anyhow::anyhow!("Window capture identity missing"))?;
                let mut closed = recording.closed();
                anyhow::ensure!(!*closed.borrow() && !*stop.borrow(), "Window source ended before capture");
                let reading = frame::capture(serial, desktop.pid, stop.clone()); tokio::pin!(reading);
                tokio::select! {
                    biased;
                    _ = closed.changed() => { sender.send_replace(true); let _ = reading.await; anyhow::bail!("Window source ended during capture") }
                    pixels = &mut reading => pixels,
                }
            }.await;
            recording.stop().await?;
            let pixels = outcome?;
            anyhow::ensure!(!*stop.borrow(), "Window screenshot cancelled");
            let logical = LogicalRect {
                x: 0,
                y: 0,
                width: pixels.width(),
                height: pixels.height(),
            };
            Ok::<_, anyhow::Error>(Captured {
                outputs: vec![OutputFrame {
                    name: target.title,
                    logical,
                    pixels,
                }],
                _permit: permit,
            })
        });
        let result = tokio::select! {
            biased;
            _ = cancelled.changed() => Err(anyhow::anyhow!("Window screenshot cancelled")),
            result = task => result?,
        };
        drop(guard);
        result
    }
}
