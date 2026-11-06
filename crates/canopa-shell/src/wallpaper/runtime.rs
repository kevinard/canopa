use std::{
    fs::File,
    sync::{Arc, Mutex},
};

use iced::{Subscription, futures::stream};
use tokio::sync::{mpsc, oneshot};

use super::store::{Prepared, Snapshot, Store, Targets};

const BUS: &str = "org.canopa.Shell.Wallpaper";
const OBJECT: &str = "/org/canopa/Shell/Wallpaper";

#[derive(Debug, Clone)]
pub struct Event {
    pub(crate) snapshot: Snapshot,
    acknowledgement: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl Event {
    pub(crate) fn installed(&self) {
        if let Some(sender) = self
            .acknowledgement
            .lock()
            .expect("wallpaper acknowledgement")
            .take()
        {
            let _ = sender.send(());
        }
    }
}

struct Request {
    image: File,
    targets: Targets,
    response: oneshot::Sender<Result<(), String>>,
}

struct Interface {
    commands: mpsc::Sender<Request>,
}

#[zbus::interface(name = "org.canopa.Shell.Wallpaper1")]
impl Interface {
    /// Destinations use bit 1 for the desktop and bit 2 for Canopa's Swaylock handoff.
    #[zbus(property)]
    fn destinations(&self) -> u32 {
        if super::locker::available() { 3 } else { 1 }
    }

    async fn apply(
        &self,
        image: zbus::zvariant::OwnedFd,
        destinations: u32,
    ) -> zbus::fdo::Result<()> {
        let targets = Targets::new(destinations)
            .map_err(|_| zbus::fdo::Error::InvalidArgs("invalid destinations".into()))?;
        if targets.lockscreen() && !super::locker::available() {
            return Err(zbus::fdo::Error::NotSupported(
                "Swaylock unavailable".into(),
            ));
        }
        let (response, result) = oneshot::channel();
        self.commands
            .try_send(Request {
                image: File::from(std::os::fd::OwnedFd::from(image)),
                targets,
                response,
            })
            .map_err(|_| {
                zbus::fdo::Error::LimitsExceeded("wallpaper owner busy or unavailable".into())
            })?;
        result
            .await
            .map_err(|_| zbus::fdo::Error::Failed("wallpaper owner stopped".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }
}

struct Runtime {
    events: mpsc::Receiver<Event>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn subscription() -> Subscription<crate::app::Message> {
    Subscription::run_with("canopa-wallpaper-owner-v1", |_| {
        stream::unfold(None::<Runtime>, |runtime| async move {
            let mut runtime = runtime.unwrap_or_else(|| {
                let (events, receiver) = mpsc::channel(1);
                Runtime {
                    events: receiver,
                    task: tokio::spawn(run(events)),
                }
            });
            runtime
                .events
                .recv()
                .await
                .map(|event| (crate::app::Message::Wallpaper(event), Some(runtime)))
        })
    })
}

async fn publish(events: &mpsc::Sender<Event>, snapshot: Snapshot) -> Result<(), String> {
    let (sender, receiver) = oneshot::channel();
    events
        .send(Event {
            snapshot,
            acknowledgement: Arc::new(Mutex::new(Some(sender))),
        })
        .await
        .map_err(|_| "wallpaper UI unavailable".to_owned())?;
    receiver
        .await
        .map_err(|_| "wallpaper UI stopped".to_owned())
}

async fn run(events: mpsc::Sender<Event>) {
    let initial = tokio::task::spawn_blocking(|| Store::discover()?.snapshot()).await;
    match initial {
        Ok(Ok(snapshot)) => {
            if publish(&events, snapshot).await.is_err() {
                return;
            }
        }
        _ => tracing::warn!("wallpaper state unavailable; retaining an empty desktop selection"),
    }
    let (commands, mut requests) = mpsc::channel::<Request>(2);
    let connection = async {
        zbus::connection::Builder::session()?
            .name(BUS)?
            .serve_at(OBJECT, Interface { commands })?
            .build()
            .await
    }
    .await;
    let _connection = match connection {
        Ok(connection) => connection,
        Err(_) => {
            tracing::warn!("wallpaper owner D-Bus service unavailable");
            // Keep the subscription alive, retaining startup state without retry churn.
            std::future::pending::<()>().await;
            return;
        }
    };
    while let Some(request) = requests.recv().await {
        let Request {
            mut image,
            targets,
            response,
        } = request;
        let prepared = tokio::task::spawn_blocking(move || {
            let image = Prepared::read(&mut image)?;
            Store::discover()?.install(image, targets)
        })
        .await;
        let result = match prepared {
            Ok(Ok(snapshot)) => publish(&events, snapshot).await,
            _ => Err("wallpaper image or state could not be installed".into()),
        };
        let _ = response.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publication_requires_app_acknowledgement_and_clones_acknowledge_once() {
        let (sender, mut receiver) = mpsc::channel(1);
        let task = tokio::spawn(async move { publish(&sender, Snapshot::default()).await });
        let event = receiver.recv().await.unwrap();
        assert!(!task.is_finished());
        event.clone().installed();
        event.installed();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn private_wire_contract_has_fd_and_destination_mask_and_validates_before_admission() {
        let bus = crate::test_bus::TestBus::new();
        let (commands, mut receiver) = mpsc::channel(2);
        let server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(BUS)
            .unwrap()
            .serve_at(OBJECT, Interface { commands })
            .unwrap()
            .build()
            .await
            .unwrap();
        let client = bus.connect().await;
        let proxy = zbus::Proxy::new(&client, BUS, OBJECT, "org.canopa.Shell.Wallpaper1")
            .await
            .unwrap();
        let xml: String = client
            .call_method(
                Some(BUS),
                OBJECT,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
            .unwrap()
            .body()
            .deserialize()
            .unwrap();
        assert!(xml.contains("type=\"h\""));
        assert!(xml.contains("name=\"destinations\" type=\"u\" direction=\"in\""));
        assert!(!xml.contains("property name=\"Version\""));
        let file = tempfile::tempfile().unwrap();
        let fd = zbus::zvariant::Fd::from(&file);
        let error = proxy
            .call::<_, _, ()>("Apply", &(fd, 0u32))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("InvalidArgs"));
        assert!(receiver.try_recv().is_err());
        let admitted = tokio::spawn(async move {
            let fd = zbus::zvariant::Fd::from(&file);
            proxy.call::<_, _, ()>("Apply", &(fd, 1u32)).await
        });
        let request = receiver.recv().await.unwrap();
        assert!(request.image.metadata().unwrap().is_file());
        assert!(!admitted.is_finished());
        admitted.abort();
        // Admission owns the operation after the initiating future is gone.
        let _ = request.response.send(Ok(()));
        drop(server);
    }
}
