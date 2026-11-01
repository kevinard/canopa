use crate::settings::Settings;
use std::time::Duration;
pub enum Update {
    Battery(canopa_widgets::battery::BatteryState),
    Media(Option<String>),
    Custom { name: String, text: Option<String> },
}
#[derive(Clone)]
struct Sink(std::sync::Arc<dyn Fn(Update) -> bool + Send + Sync>);
impl Sink {
    fn send(&self, update: Update) -> Result<(), ()> {
        if (self.0)(update) { Ok(()) } else { Err(()) }
    }
}
pub struct Runtime {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn start(
    settings: &Settings,
    notify: impl Fn(Update) -> bool + Send + Sync + 'static,
) -> Option<Runtime> {
    let updates = Sink(std::sync::Arc::new(notify));
    let battery = settings.widgets.iter().any(|w| w == "battery");
    let media = settings.widgets.iter().any(|w| w == "media");
    let scripts: Vec<_> = settings
        .custom
        .iter()
        .filter(|(name, _)| settings.widgets.contains(name))
        .map(|(n, w)| (n.clone(), w.clone()))
        .collect();
    if !battery && !media && scripts.is_empty() {
        return None;
    }
    let (shutdown, stop) = tokio::sync::oneshot::channel();
    let thread = std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        runtime.block_on(async move {
            let mut tasks = tokio::task::JoinSet::new();
            if battery {
                let updates = updates.clone();
                tasks.spawn(async move {
                    loop {
                        if let Ok(conn) = zbus::Connection::system().await {
                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                            let worker =
                                tokio::spawn(canopa_widgets::battery::run_upower_task(conn, tx));
                            while let Some(event) = rx.recv().await {
                                let value = event
                                    .map(|(percentage, power_state, present)| {
                                        canopa_widgets::battery::BatteryState {
                                            percentage,
                                            power_state,
                                            present,
                                        }
                                    })
                                    .unwrap_or_default();
                                if updates.send(Update::Battery(value)).is_err() {
                                    worker.abort();
                                    return;
                                }
                            }
                            worker.abort();
                        }
                        if updates.send(Update::Battery(Default::default())).is_err() {
                            return;
                        }
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                });
            }
            if media {
                let updates=updates.clone();
                tasks.spawn(async move {
                    let (sender, mut receiver) = tokio::sync::watch::channel(Default::default());
                    let worker = tokio::spawn(canopa_widgets::media::observe(sender));
                    while receiver.changed().await.is_ok() {
                        let snapshot = receiver.borrow_and_update().clone();
                        let label = snapshot.current().map(|p| p.label());
                        if updates.send(Update::Media(label)).is_err() {
                            break;
                        }
                    }
                    worker.abort();
                });
            }
            for (name,script) in scripts {
                let updates=updates.clone();tasks.spawn(async move{
                    let (sender,mut receiver)=tokio::sync::watch::channel(None);
                    // This task owns the observation future, so aborting it also cancels
                    // the command and its process-group guard rather than detaching it.
                    let observe=canopa_widgets::script::observe(script,sender);
                    tokio::pin!(observe);
                    loop {tokio::select!{
                        _=&mut observe=>break,
                        changed=receiver.changed()=>{if changed.is_err(){break;}let text=receiver.borrow_and_update().clone();if updates.send(Update::Custom{name:name.clone(),text}).is_err(){break;}}
                    }}
                });
            }
            tokio::select!{_=async{while tasks.join_next().await.is_some() {}}=>{},_=stop=>{}}
            tasks.abort_all();while tasks.join_next().await.is_some() {}
        });
    });
    Some(Runtime {
        shutdown: Some(shutdown),
        thread: Some(thread),
    })
}
