//! Standard local presentation requests on the owning runtime's session bus.
use crate::{
    activation::Instance,
    presentation::{FileManagerRequest, Presentation},
};
use std::{
    io,
    sync::{Mutex, mpsc},
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::oneshot;

pub const NAME: &str = "org.freedesktop.FileManager1";
pub const PATH: &str = "/org/freedesktop/FileManager1";

struct Interface {
    instance: Instance,
}
impl Interface {
    fn present(
        &self,
        kind: FileManagerRequest,
        uris: &[String],
        startup_id: &str,
    ) -> zbus::fdo::Result<()> {
        if let Some(request) = Presentation::from_file_manager(kind, uris, startup_id)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?
        {
            self.instance
                .present(request)
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl Interface {
    fn show_folders(&self, uris: Vec<String>, startup_id: &str) -> zbus::fdo::Result<()> {
        self.present(FileManagerRequest::Folders, &uris, startup_id)
    }
    fn show_items(&self, uris: Vec<String>, startup_id: &str) -> zbus::fdo::Result<()> {
        self.present(FileManagerRequest::Items, &uris, startup_id)
    }
    fn show_item_properties(&self, uris: Vec<String>, startup_id: &str) -> zbus::fdo::Result<()> {
        self.present(FileManagerRequest::Properties, &uris, startup_id)
    }
}

struct Running {
    stop: oneshot::Sender<()>,
    thread: JoinHandle<()>,
}
pub struct Host(Mutex<Option<Running>>);

impl Host {
    /// Never replaces or queues behind another file manager. None means its name
    /// was already owned. Configuration/history still have their original owner.
    pub fn start(instance: Instance) -> io::Result<Option<Self>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (ready, result) = mpsc::sync_channel(1);
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("files-file-manager".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let connect = async {
                        let connection = zbus::connection::Builder::session()?
                            .serve_at(PATH, Interface { instance })?
                            .build()
                            .await?;
                        connection
                            .request_name_with_flags(
                                NAME,
                                zbus::fdo::RequestNameFlags::DoNotQueue.into(),
                            )
                            .await?;
                        Ok::<_, zbus::Error>(connection)
                    };
                    let connection =
                        match tokio::time::timeout(Duration::from_secs(2), connect).await {
                            Ok(Ok(connection)) => connection,
                            Ok(Err(zbus::Error::NameTaken)) => {
                                let _ = ready.send(Ok(false));
                                return;
                            }
                            Ok(Err(error)) => {
                                let _ = ready.send(Err(error.to_string()));
                                return;
                            }
                            Err(_) => {
                                let _ = ready
                                    .send(Err("FileManager1 registration timed out.".to_owned()));
                                return;
                            }
                        };
                    if ready.send(Ok(true)).is_err() {
                        return;
                    }
                    let _ = stopped.await;
                    // Let an already dispatched reply finish, but never let a broken
                    // bus prevent the owning runtime's orderly teardown.
                    let _ = tokio::time::timeout(
                        Duration::from_secs(2),
                        connection.graceful_shutdown(),
                    )
                    .await;
                });
            })?;
        match result.recv() {
            Ok(Ok(true)) => Ok(Some(Self(Mutex::new(Some(Running { stop, thread }))))),
            result => {
                let _ = stop.send(());
                thread
                    .join()
                    .map_err(|_| io::Error::other("FileManager1 worker stopped unexpectedly."))?;
                match result {
                    Ok(Ok(false)) => Ok(None),
                    Ok(Err(message)) => Err(io::Error::other(message)),
                    _ => Err(io::Error::other(
                        "FileManager1 worker stopped before registration.",
                    )),
                }
            }
        }
    }

    pub fn shutdown(&self) -> io::Result<()> {
        let mut running = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(Running { stop, thread }) = running.take() {
            let _ = stop.send(());
            thread
                .join()
                .map_err(|_| io::Error::other("FileManager1 worker stopped unexpectedly."))?;
        }
        Ok(())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
