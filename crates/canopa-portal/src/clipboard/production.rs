use super::*;
use crate::clipboard_transport::{Bridge, Snapshot};
use futures_util::StreamExt;

struct Monitor(tokio::task::JoinHandle<()>);
impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub(crate) struct Production {
    bridge: Bridge,
    events: Option<BoxStream<'static, Event>>,
    monitor: Monitor,
}
impl Production {
    pub async fn start(pid: u32, ended: watch::Sender<bool>) -> anyhow::Result<Self> {
        Self::start_with_serials(pid, ended, crate::clipboard_transport::Serials::default()).await
    }
    pub(crate) async fn start_with_serials(
        pid: u32,
        ended: watch::Sender<bool>,
        serials: crate::clipboard_transport::Serials,
    ) -> anyhow::Result<Self> {
        let mut bridge = Bridge::start_scoped(pid, serials, ended.subscribe()).await?;
        let revoker = bridge.revoker()?;
        let mut changed = bridge.snapshots();
        let mut closing = ended.subscribe();
        let monitor = Monitor(tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = async {while !*closing.borrow() {if closing.changed().await.is_err() {break;}}} => {revoker.revoke(); break;},
                    result=changed.changed()=> {
                        if result.is_err() || !changed.borrow().ready { ended.send_replace(true); break; }
                    }
                }
            }
        }));
        let snapshots = bridge.snapshots();
        let transfers = bridge.take_transfers().expect("clipboard events");
        let events = futures_util::stream::unfold((snapshots, transfers, true), |(mut snapshots, mut transfers, mut initial)| async move {
            loop {
                fn info(snapshot: &Snapshot) -> Option<Event> {
                    snapshot.ready.then(|| Event::OwnerChanged { mime_types: snapshot.mime_types.clone(), session_is_owner: snapshot.session_is_owner })
                }
                if initial {
                    initial = false;
                    let event = info(&snapshots.borrow())?;
                    return Some((event, (snapshots, transfers, initial)));
                }
                tokio::select! {
                    biased;
                    result=snapshots.changed()=> {
                        result.ok()?;
                        let event = info(&snapshots.borrow())?;
                        return Some((event, (snapshots, transfers, initial)));
                    }
                    transfer=transfers.recv()=> {
                        let transfer = transfer?;
                        let current = snapshots.borrow().clone();
                        if !current.ready { return None; }
                        if current.session_is_owner && current.generation == transfer.generation {
                            return Some((Event::Transfer {mime_type: transfer.mime_type, serial: transfer.serial}, (snapshots, transfers, initial)));
                        }
                    }
                }
            }
        }).boxed();
        Ok(Self {
            bridge,
            events: Some(events),
            monitor,
        })
    }
    pub fn is_closed(&self) -> bool {
        !self.bridge.snapshots().borrow().ready
    }
    pub(crate) fn revoker(&self) -> std::io::Result<crate::clipboard_transport::Revoker> {
        self.bridge.revoker()
    }
    pub async fn stop(self) -> anyhow::Result<()> {
        let Self {
            bridge, monitor, ..
        } = self;
        drop(monitor);
        bridge.stop().await
    }
}
impl Control for Production {
    fn events(&mut self) -> Option<BoxStream<'static, Event>> {
        self.events.take()
    }
    fn perform<'a>(
        &'a mut self,
        command: Command,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<Response>> {
        Box::pin(async move {
            if *cancelled.borrow() || self.is_closed() {
                return Err(fdo::Error::AccessDenied("Clipboard access ended".into()));
            }
            let operation = async {
                let response = match command {
                    Command::Request => anyhow::bail!("Clipboard already started"),
                    Command::Publish(types) => {
                        self.bridge.publish(types).await?;
                        Response::Unit
                    }
                    Command::Read(mime) => {
                        let generation = self.bridge.snapshots().borrow().generation;
                        Response::Fd(self.bridge.read(generation, mime).await?)
                    }
                    Command::Write(serial) => Response::Fd(self.bridge.write(serial).await?),
                    Command::Done(serial, success) => {
                        self.bridge.write_done(serial, success).await?;
                        Response::Unit
                    }
                };
                Ok::<_, anyhow::Error>(response)
            };
            tokio::select! {
                biased;
                _=async {while !*cancelled.borrow() {if cancelled.changed().await.is_err() {break;}}}=>Err(fdo::Error::AccessDenied("Clipboard access ended".into())),
                result=operation=>result.map_err(|e| fdo::Error::Failed(e.to_string())),
            }
        })
    }
}
