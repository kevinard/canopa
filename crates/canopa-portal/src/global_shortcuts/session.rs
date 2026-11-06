//! One actor owns the live lease and serializes consent with compositor events.
use super::{
    Actions,
    editor::{self, Decision, Feedback},
    portal::*,
    transport::{self, Notice},
};
use anyhow::{Result, anyhow, bail, ensure};
use futures_util::{StreamExt, stream::BoxStream};
use std::{
    future::{Future, pending},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, mpsc, oneshot, watch};
use zbus::{Connection, fdo, message::Header, zvariant::OwnedObjectPath};

pub(super) enum Command {
    Bind {
        actions: Actions,
        parent_window: String,
        cancelled: watch::Receiver<bool>,
        reply: oneshot::Sender<fdo::Result<Outcome>>,
    },
    Configure {
        parent_window: String,
        activation_token: Option<String>,
    },
    List(oneshot::Sender<fdo::Result<Listing>>),
}
pub(super) struct Endpoint(pub Handle);
#[zbus::interface(name = "org.freedesktop.impl.portal.Session")]
impl Endpoint {
    fn close(&self, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        no_fds(&header)?;
        if sender(&header)? != self.0.owner {
            return Err(fdo::Error::AccessDenied(
                "Shortcut session belongs to another caller".into(),
            ));
        }
        self.0.notify.store(false, Ordering::SeqCst);
        self.0.close.send_replace(true);
        Ok(())
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
    #[zbus(signal)]
    async fn closed(emitter: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;
}
struct Active {
    lease: transport::Lease,
    control: transport::Control,
    events: mpsc::Receiver<Notice>,
    closed: watch::Receiver<bool>,
    pending: Vec<Notice>,
}
struct Actor {
    host: Arc<Host>,
    connection: Connection,
    key: OwnedObjectPath,
    handle: Handle,
    inbox: mpsc::Receiver<Command>,
    owners: BoxStream<'static, ()>,
    actions: Actions,
    active: Option<Active>,
    editor: Option<Box<dyn editor::Editor>>,
    applying: bool,
}
impl Actor {
    fn listing(&self) -> fdo::Result<Listing> {
        if *self.handle.close.borrow() {
            return Err(failed());
        }
        if !self.handle.attempted.load(Ordering::SeqCst) {
            return Ok(Listing::History);
        }
        let definitions = if self.handle.bound.load(Ordering::SeqCst) {
            let active = self.active.as_ref().ok_or_else(failed)?;
            if *active.closed.borrow() {
                return Err(failed());
            }
            active.control.snapshot().bindings
        } else {
            Default::default()
        };
        Ok(Listing::Live(self.actions.response(&definitions)?))
    }
    fn list(&self, reply: oneshot::Sender<fdo::Result<Listing>>) {
        let _ = reply.send(self.listing());
    }
    async fn emit(&mut self, notice: Notice) -> Result<()> {
        let sending = async {
            match notice {
                Notice::Changed(snapshot) => {
                    let mut options = self.actions.response(&snapshot.bindings)?;
                    let rows: Vec<(String, crate::requests::Options)> = options
                        .remove("shortcuts")
                        .expect("shortcut result")
                        .try_into()?;
                    self.connection
                        .emit_signal(
                            Some(&self.handle.owner),
                            crate::OBJECT_PATH,
                            super::INTERFACE,
                            "ShortcutsChanged",
                            &(self.key.clone(), rows),
                        )
                        .await?;
                }
                Notice::Activated {
                    binding,
                    timestamp_ms,
                    ..
                }
                | Notice::Deactivated {
                    binding,
                    timestamp_ms,
                    ..
                } => {
                    let action = binding
                        .checked_sub(1)
                        .and_then(|i| self.actions.0.get(i as usize))
                        .ok_or_else(|| anyhow!("Unknown shortcut event"))?;
                    let member = if matches!(notice, Notice::Activated { .. }) {
                        "Activated"
                    } else {
                        "Deactivated"
                    };
                    self.connection
                        .emit_signal(
                            Some(&self.handle.owner),
                            crate::OBJECT_PATH,
                            super::INTERFACE,
                            member,
                            &(
                                self.key.clone(),
                                action.id.clone(),
                                timestamp_ms,
                                crate::requests::Options::new(),
                            ),
                        )
                        .await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        };
        tokio::time::timeout(Duration::from_secs(5), sending).await??;
        Ok(())
    }
    async fn notice(&mut self, notice: Notice) -> Result<()> {
        if let Notice::Changed(ref snapshot) = notice {
            self.actions.selected(&snapshot.bindings)?;
            if let Some(editor) = &self.editor {
                let mut feedback = Feedback::snapshot(&self.actions, snapshot)?;
                feedback.busy = self.applying;
                editor.update(feedback)?;
            }
        }
        if self.handle.bound.load(Ordering::SeqCst) {
            self.emit(notice).await?;
        } else {
            let active = self
                .active
                .as_mut()
                .ok_or_else(|| anyhow!("Shortcut source stopped"))?;
            ensure!(
                active.pending.len() < 64,
                "Unconsumed provisional shortcut events"
            );
            active.pending.push(notice);
        }
        Ok(())
    }
    /// Observe revocation and source events even while storage/UI/RPC is pending.
    /// Request cancellation applies only until Bind completes, never to an
    /// already accepted session after the Request object has been removed.
    async fn guarded<T>(
        &mut self,
        future: impl Future<Output = Result<T>>,
        cancelled: &mut watch::Receiver<bool>,
    ) -> Result<Option<T>> {
        tokio::pin!(future);
        let mut close = self.handle.close.subscribe();
        loop {
            tokio::select! {biased;
                _=stopped(&mut close)=>bail!("Shortcut session closed"),
                _=self.owners.next()=>{self.handle.close.send_replace(true);bail!("Shortcut caller disconnected");}
                _=source_closed(&self.active)=>{self.handle.close.send_replace(true);bail!("Shortcut compositor stopped");}
                _=stopped(cancelled)=>return Ok(None),
                notice=next_notice(&mut self.active)=>{self.notice(notice.ok_or_else(||anyhow!("Shortcut compositor stopped"))?).await?;}
                command=self.inbox.recv()=>match command {
                    Some(Command::List(reply))=>self.list(reply),
                    _=>bail!("Unexpected concurrent shortcut edit"),
                },
                result=&mut future=>return result.map(Some),
            }
        }
    }
    async fn edit(
        &mut self,
        parent_window: String,
        activation_token: Option<String>,
        cancelled: &mut watch::Receiver<bool>,
    ) -> Result<Option<Accepted>> {
        let result = self
            .review(parent_window, activation_token, cancelled)
            .await;
        self.applying = false;
        if let Some(editor) = self.editor.take() {
            // Authority is already owned by this actor; helper termination never
            // changes a confirmed decision. Drop still kills a stalled child.
            if let Err(error) = editor.stop().await {
                eprintln!("level=warn component=global_shortcuts event=editor_stop error={error}");
            }
        }
        if !self.handle.bound.load(Ordering::SeqCst)
            && let Some(active) = self.active.take()
        {
            active.lease.stop().await;
        }
        self.handle.editing.store(false, Ordering::SeqCst);
        result
    }
    async fn review(
        &mut self,
        parent_window: String,
        activation_token: Option<String>,
        cancelled: &mut watch::Receiver<bool>,
    ) -> Result<Option<Accepted>> {
        let configuring = self.handle.bound.load(Ordering::SeqCst);
        let history = self.host.history.clone();
        let app = self.handle.app.clone();
        let Some(saved) = self
            .guarded(async move { history.load(app).await }, cancelled)
            .await?
        else {
            return Ok(None);
        };
        if self.active.is_none() {
            let connector = self.host.connector.clone();
            let connection = self.connection.clone();
            let Some(mut lease) = self
                .guarded(async move { connector.open(&connection).await }, cancelled)
                .await?
            else {
                return Ok(None);
            };
            let control = lease.control();
            let events = lease
                .events()
                .ok_or_else(|| anyhow!("Shortcut events unavailable"))?;
            let closed = control.closed();
            self.active = Some(Active {
                lease,
                control,
                events,
                closed,
                pending: Vec::new(),
            });
        }
        let active = self.active.as_ref().expect("open shortcut source");
        let snapshot = active.control.snapshot();
        let current = self.actions.selected(&snapshot.bindings)?;
        let defaults = self
            .actions
            .0
            .iter()
            .filter_map(|action| {
                let trigger = saved
                    .bindings
                    .iter()
                    .find(|binding| binding.id == action.id)
                    .map(|b| b.trigger.clone())
                    .or_else(|| action.preferred_trigger.clone())?;
                Some(super::Binding {
                    id: action.id.clone(),
                    trigger,
                })
            })
            .collect();
        let request = editor::Request {
            app_id: self.handle.app.clone(),
            parent_window,
            activation_token,
            configuring,
            actions: self.actions.clone(),
            defaults,
            current,
            generation: snapshot.generation,
        };
        let presenter = self.host.presenter.clone();
        let Some(editor) = self
            .guarded(async move { presenter.open(request).await }, cancelled)
            .await?
        else {
            return Ok(None);
        };
        self.editor = Some(editor);
        let mut decisions = self
            .editor
            .as_mut()
            .expect("open editor")
            .events()
            .ok_or_else(|| anyhow!("Shortcut editor events unavailable"))?;
        self.editor
            .as_ref()
            .expect("open editor")
            .update(Feedback::snapshot(
                &self.actions,
                &self.active.as_ref().expect("source").control.snapshot(),
            )?)?;
        loop {
            let Some(decision) = self
                .guarded(async { Ok(decisions.recv().await) }, cancelled)
                .await?
            else {
                return Ok(None);
            };
            let Some(Decision::Apply {
                generation,
                bindings,
            }) = decision
            else {
                return Ok(None);
            };
            let definitions = match self.actions.definitions(&bindings) {
                Ok(definitions) => definitions,
                Err(error) => {
                    self.feedback(error.to_string(), Vec::new())?;
                    continue;
                }
            };
            self.applying = true;
            let control = self.active.as_ref().expect("source").control.clone();
            let commit = self
                .guarded(
                    async move { control.replace(generation, definitions).await },
                    cancelled,
                )
                .await;
            self.applying = false;
            let commit = match commit {
                Ok(Some(commit)) => commit,
                Ok(None) => {
                    self.handle.close.send_replace(true);
                    return Ok(None);
                }
                Err(error) => {
                    self.handle.close.send_replace(true);
                    return Err(error);
                }
            };
            if let Some(error) = commit.error {
                let failed = commit
                    .failed
                    .iter()
                    .filter_map(|id| {
                        id.checked_sub(1)
                            .and_then(|i| self.actions.0.get(i as usize))
                            .map(|a| a.id.clone())
                    })
                    .collect();
                self.feedback(error, failed)?;
                continue;
            }
            // Only an explicit, successful Apply is saved. If storage or its
            // recipient disappears after the mutation, revoke the entire lease.
            let history = self.host.history.clone();
            let app = self.handle.app.clone();
            let actions = self.actions.clone();
            let definitions = commit.snapshot.bindings.clone();
            self.applying = true;
            let saved = self
                .guarded(
                    async move { history.save(app, actions, definitions).await },
                    cancelled,
                )
                .await;
            self.applying = false;
            match saved {
                Ok(Some(())) => {}
                other => {
                    self.handle.close.send_replace(true);
                    return match other {
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                        _ => unreachable!(),
                    };
                }
            }
            self.handle.bound.store(true, Ordering::SeqCst);
            let pending = std::mem::take(&mut self.active.as_mut().expect("source").pending);
            for notice in pending {
                self.emit(notice).await?;
            }
            let options = self.actions.response(
                &self
                    .active
                    .as_ref()
                    .expect("source")
                    .control
                    .snapshot()
                    .bindings,
            )?;
            return Ok(Some(Accepted {
                options,
                close: self.handle.close.clone(),
                armed: !configuring,
            }));
        }
    }
    fn feedback(&self, error: String, failed: Vec<String>) -> Result<()> {
        let mut feedback = Feedback::snapshot(
            &self.actions,
            &self.active.as_ref().expect("source").control.snapshot(),
        )?;
        feedback.error = Some(error);
        feedback.failed = failed;
        self.editor.as_ref().expect("editor").update(feedback)
    }
    async fn body(&mut self) -> Result<()> {
        let mut close = self.handle.close.subscribe();
        loop {
            tokio::select! {biased;
                _=stopped(&mut close)=>return Ok(()),
                _=self.owners.next()=>return Ok(()),
                _=source_closed(&self.active)=>bail!("Shortcut compositor stopped"),
                notice=next_notice(&mut self.active)=>self.notice(notice.ok_or_else(||anyhow!("Shortcut compositor stopped"))?).await?,
                command=self.inbox.recv()=>match command {
                    None=>return Ok(()),
                    Some(Command::List(reply))=>self.list(reply),
                    Some(Command::Bind {actions,parent_window,mut cancelled,reply})=>{
                        self.actions=actions;
                        let result=self.edit(parent_window,None,&mut cancelled).await.map(|accepted|accepted.map(Outcome::Accepted).unwrap_or(Outcome::Cancelled)).map_err(|e|fdo::Error::Failed(e.to_string()));
                        let _=reply.send(result);
                    }
                    Some(Command::Configure {parent_window,activation_token})=>{
                        let (_keep,mut cancelled)=watch::channel(false);
                        if let Err(error)=self.edit(parent_window,activation_token,&mut cancelled).await {eprintln!("level=warn component=global_shortcuts event=configure_failed error={error}");}
                    }
                }
            }
        }
    }
}
async fn next_notice(active: &mut Option<Active>) -> Option<Notice> {
    match active {
        Some(active) => active.events.recv().await,
        None => pending().await,
    }
}
fn source_closed(active: &Option<Active>) -> futures_util::future::BoxFuture<'static, ()> {
    let closed = active.as_ref().map(|active| active.closed.clone());
    Box::pin(async move {
        match closed {
            Some(mut closed) => stopped(&mut closed).await,
            None => pending().await,
        }
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    host: Arc<Host>,
    connection: Connection,
    key: OwnedObjectPath,
    handle: Handle,
    inbox: mpsc::Receiver<Command>,
    owners: BoxStream<'static, ()>,
    done: watch::Sender<bool>,
    _slot: OwnedSemaphorePermit,
) {
    let mut actor = Actor {
        host,
        connection,
        key,
        handle,
        inbox,
        owners,
        actions: Actions(Vec::new()),
        active: None,
        editor: None,
        applying: false,
    };
    if let Err(error) = actor.body().await {
        eprintln!("level=warn component=global_shortcuts event=session_stopped error={error}");
    }
    actor.handle.close.send_replace(true);
    actor.handle.bound.store(false, Ordering::SeqCst);
    if let Some(active) = actor.active.take() {
        active.lease.stop().await;
    }
    if let Some(editor) = actor.editor.take() {
        let _ = editor.stop().await;
    }
    if actor.handle.notify.load(Ordering::SeqCst) {
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            actor.connection.emit_signal(
                Some(&actor.handle.owner),
                &actor.key,
                "org.freedesktop.impl.portal.Session",
                "Closed",
                &(),
            ),
        )
        .await;
    }
    let _ = actor
        .connection
        .object_server()
        .remove::<Endpoint, _>(actor.key.clone())
        .await;
    actor
        .host
        .entries
        .lock()
        .expect("shortcut sessions")
        .remove(&actor.key);
    done.send_replace(true);
}
