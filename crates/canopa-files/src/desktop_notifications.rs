//! One bounded worker sends standard desktop notifications. Notification policy
//! and presentation remain owned by the session notification service.
use futures_channel::oneshot;
use std::{io, sync::mpsc, time::Duration};

struct Notice {
    title: String,
    body: String,
    reply: oneshot::Sender<Result<(), String>>,
}
#[derive(Clone)]
pub struct Worker {
    requests: mpsc::SyncSender<Notice>,
}

impl Worker {
    pub fn new() -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Notice>(16);
        std::thread::Builder::new()
            .name("files-notifications".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                while let Ok(notice) = receiver.recv() {
                    let result = match &runtime {
                        Err(_) => Err("Desktop notifications are unavailable.".into()),
                        Ok(runtime) => runtime.block_on(async {
                            tokio::time::timeout(Duration::from_secs(2), async {
                                let connection = zbus::Connection::session().await?;
                                let hints = std::collections::HashMap::from([(
                                    "desktop-entry",
                                    zbus::zvariant::Value::from("org.canopa.Files"),
                                )]);
                                let reply = connection
                                    .call_method(
                                        Some("org.freedesktop.Notifications"),
                                        "/org/freedesktop/Notifications",
                                        Some("org.freedesktop.Notifications"),
                                        "Notify",
                                        &(
                                            "Canopa Files",
                                            0_u32,
                                            "system-run-symbolic",
                                            notice.title.as_str(),
                                            notice.body.as_str(),
                                            Vec::<&str>::new(),
                                            hints,
                                            -1_i32,
                                        ),
                                    )
                                    .await?;
                                reply.body().deserialize::<u32>()?;
                                Ok::<(), zbus::Error>(())
                            })
                            .await
                            .map_err(|_| "Desktop notification timed out.".to_owned())?
                            .map_err(|_| "Desktop notification could not be sent.".to_owned())
                        }),
                    };
                    let _ = notice.reply.send(result);
                }
            })?;
        Ok(Self { requests })
    }

    pub fn send(
        &self,
        title: String,
        body: String,
    ) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.requests
            .try_send(Notice { title, body, reply })
            .map_err(|_| io::Error::other("Desktop notifications are busy or unavailable."))?;
        Ok(receiver)
    }
}
