//! Canopa-session observer. Wayland head batches wake it even for disabled outputs.
use crate::{
    display::{Monitor, TrialHandle},
    niri::Client,
    profiles::{Book, Switching},
    session::Paths,
};
use anyhow::{Context, Result, ensure};
use canopa_file::watch::{Target, Watcher};
use futures_util::StreamExt;
use niri_ipc::Output;
use std::{collections::HashMap, time::Duration};
use tokio::{
    io::unix::AsyncFd,
    sync::{oneshot, watch},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_head_v1 as head, zwlr_output_manager_v1 as manager, zwlr_output_mode_v1 as mode,
};
use zbus::zvariant::Value;

struct Heads(watch::Sender<()>);
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Heads {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<manager::ZwlrOutputManagerV1, ()> for Heads {
    fn event(
        state: &mut Self,
        _: &manager::ZwlrOutputManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, manager::Event::Done { .. }) {
            state.0.send_replace(());
        }
    }
    wayland_client::event_created_child!(Heads, manager::ZwlrOutputManagerV1, [
        manager::EVT_HEAD_OPCODE => (head::ZwlrOutputHeadV1, ())
    ]);
}
impl Dispatch<head::ZwlrOutputHeadV1, ()> for Heads {
    fn event(
        _: &mut Self,
        head: &head::ZwlrOutputHeadV1,
        event: head::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, head::Event::Finished) && head.version() >= 3 {
            head.release();
        }
    }
    wayland_client::event_created_child!(Heads, head::ZwlrOutputHeadV1, [
        head::EVT_MODE_OPCODE => (mode::ZwlrOutputModeV1, ())
    ]);
}
impl Dispatch<mode::ZwlrOutputModeV1, ()> for Heads {
    fn event(
        _: &mut Self,
        mode: &mode::ZwlrOutputModeV1,
        event: mode::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, mode::Event::Finished) && mode.version() >= 3 {
            mode.release();
        }
    }
}

fn watch_heads() -> Result<watch::Receiver<()>> {
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Heads>(&connection)?;
    let manager: manager::ZwlrOutputManagerV1 = globals.bind(&queue.handle(), 1..=4, ())?;
    let (send, receive) = watch::channel(());
    std::thread::Builder::new()
        .name("display-heads".into())
        .spawn(move || {
            let mut state = Heads(send);
            while manager.is_alive() && queue.blocking_dispatch(&mut state).is_ok() {}
        })?;
    Ok(receive)
}

async fn file_changed(watcher: &mut AsyncFd<Watcher>) -> Result<()> {
    loop {
        let mut ready = watcher.readable_mut().await?;
        let changed = ready.get_inner_mut().changed()?;
        if ready.get_inner().drained() {
            ready.clear_ready();
        }
        if changed {
            return Ok(());
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct DisplayIdentity {
    connector: String,
    make: String,
    model: String,
    serial: Option<String>,
}
fn topology(outputs: &HashMap<String, Output>) -> Vec<DisplayIdentity> {
    let mut result: Vec<_> = outputs
        .values()
        .map(|o| DisplayIdentity {
            connector: o.name.clone(),
            make: o.make.clone(),
            model: o.model.clone(),
            serial: o.serial.clone(),
        })
        .collect();
    result.sort_unstable();
    result
}

pub fn run() -> Result<()> {
    let paths = Paths::discover()?;
    ensure!(
        paths.active(),
        "display profile watching requires the Canopa session"
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(watch_profiles(paths))
}

async fn watch_profiles(paths: Paths) -> Result<()> {
    let client = Client::discover()?;
    let mut heads = watch_heads()?;
    let mut file = AsyncFd::new(Watcher::new(vec![Target::File(paths.profiles())])?)?;
    let mut settle = Book::load(&paths).map_or(Duration::from_millis(600), |book| book.settle);
    let mut observed = None;
    let mut source_text = None;
    let mut pending_offer = None;
    let mut has_profiles = false;
    // The first Wayland done event contains the initial complete head batch.
    heads
        .changed()
        .await
        .context("Wayland output watcher ended")?;
    loop {
        loop {
            tokio::select! {
                result = heads.changed() => result.context("Wayland output watcher ended")?,
                result = file_changed(&mut file) => result?,
                _ = tokio::time::sleep(settle) => break,
            }
        }

        let mut outputs = client.outputs()?;
        let connected = topology(&outputs);
        let startup = observed.is_none();
        let changed = observed.as_ref() != Some(&connected);
        observed = Some(connected);
        let loaded = Book::load(&paths);
        if changed
            && !startup
            && (has_profiles || loaded.as_ref().is_ok_and(|book| !book.profiles.is_empty()))
            && !outputs.values().any(|output| output.current_mode.is_some())
        {
            match crate::display::recover_after_hotplug(&client) {
                Ok(()) => outputs = client.outputs()?,
                Err(error) => eprintln!("canopa-settings: display recovery: {error:#}"),
            }
        }
        match loaded {
            Err(error) => {
                drop(pending_offer.take());
                source_text = None;
                eprintln!("canopa-settings: {error:#}");
            }
            Ok(book) => {
                has_profiles = !book.profiles.is_empty();
                settle = book.settle;
                if changed || source_text.as_ref() != Some(&book.source.text) {
                    drop(pending_offer.take());
                    source_text = Some(book.source.text.clone());
                }
                if changed && (!startup || book.startup) && book.switching != Switching::Manual {
                    match book.matching(&outputs) {
                        Err(error) => eprintln!("canopa-settings: {error:#}"),
                        Ok(Some((profile, layout)))
                            if !layout.iter().all(|m| m.matches_live(&outputs[&m.name])) =>
                        {
                            let name = profile.name.clone();
                            let paths = paths.clone();
                            if book.switching == Switching::Automatic {
                                let result = tokio::task::spawn_blocking(move || {
                                    TrialHandle::start(&paths, layout, Some(&name), true)?
                                        .finish(true)
                                })
                                .await?;
                                match result {
                                    Ok(status) => eprintln!("canopa-settings: {status}"),
                                    Err(error) => eprintln!(
                                        "canopa-settings: automatic display switch: {error:#}"
                                    ),
                                }
                            } else {
                                let (cancel, cancelled) = oneshot::channel();
                                pending_offer = Some(cancel);
                                let client = client.clone();
                                tokio::spawn(async move {
                                    if let Err(error) =
                                        offer(paths, client, name, layout, book.offer, cancelled)
                                            .await
                                    {
                                        eprintln!(
                                            "canopa-settings: display profile offer: {error:#}"
                                        );
                                    }
                                });
                            }
                        }
                        Ok(_) => {}
                    }
                }
            }
        }

        tokio::select! {
            result = heads.changed() => result.context("Wayland output watcher ended")?,
            result = file_changed(&mut file) => result?,
        }
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app: &str,
        replace: u32,
        icon: &str,
        title: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        timeout: i32,
    ) -> zbus::Result<u32>;
    fn close_notification(&self, id: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

async fn offer(
    paths: Paths,
    client: Client,
    name: String,
    layout: Vec<Monitor>,
    lifetime: Duration,
    mut cancelled: oneshot::Receiver<()>,
) -> Result<()> {
    let timeout = Duration::from_secs(3);
    let Some((proxy, mut actions, mut closed, id)) = tokio::time::timeout(timeout, async {
        let bus = zbus::Connection::session().await?;
        let dbus = zbus::fdo::DBusProxy::new(&bus).await?;
        let service = "org.freedesktop.Notifications";
        let mut owners = dbus.receive_name_owner_changed_with_args(&[(0, service)]).await?;
        let owner = loop {
            match dbus.get_name_owner(service.try_into()?).await {
                Ok(owner) => break owner,
                Err(zbus::fdo::Error::NameHasNoOwner(_)) => {
                    tokio::select! {
                        event = owners.next() => { event.context("notification service watcher ended")?; }
                        _ = &mut cancelled => return Ok(None),
                    }
                }
                Err(error) => return Err(error.into()),
            }
        };
        // IDs are scoped to this specific notification service owner.
        let proxy = NotificationsProxy::builder(&bus)
            .destination(owner)?
            .build()
            .await?;
        let actions = proxy.receive_action_invoked().await?;
        let closed = proxy.receive_notification_closed().await?;
        let id = proxy
            .notify(
                "Canopa Settings",
                0,
                "video-display",
                &format!("Display profile available · {name}"),
                "The connected displays match a saved layout.",
                &["review", "Review layout"],
                HashMap::from([("transient", Value::Bool(true))]),
                lifetime.as_millis() as i32,
            )
            .await?;
        Ok::<_, anyhow::Error>(Some((proxy, actions, closed, id)))
    })
    .await?? else {
        return Ok(());
    };
    let elapsed = tokio::time::sleep(lifetime);
    tokio::pin!(elapsed);
    let review = loop {
        tokio::select! {
            biased;
            _ = &mut cancelled => break false,
            _ = &mut elapsed => break false,
            event = actions.next() => match event {
                Some(event) if *event.args()?.id() == id && *event.args()?.action() == "review" => break true,
                None => break false,
                _ => {}
            },
            event = closed.next() => match event {
                Some(event) if *event.args()?.id() == id => break false,
                None => break false,
                _ => {}
            },
        }
    };
    let _ = tokio::time::timeout(timeout, proxy.close_notification(id)).await;
    if review {
        // An old notification cannot open a different layout after hotplug or edits.
        let book = Book::load(&paths)?;
        let outputs = client.outputs()?;
        if book.switching == Switching::Suggest
            && book
                .matching(&outputs)?
                .is_some_and(|(profile, current)| profile.name == name && current == layout)
        {
            let mut child = tokio::process::Command::new(std::env::current_exe()?)
                .arg(format!("--display-profile={name}"))
                .spawn()?;
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
    }
    Ok(())
}
