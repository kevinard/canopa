//! Each capture activation owns a fresh, independently revocable byte transport.
use super::*;
use crate::{
    clipboard::{self as portal, Control as _},
    clipboard_transport::{Revoker, Serials},
    input_capture_transport::Snapshot,
};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

struct State {
    epoch: u64,
    activation: Option<u32>,
    revoker: Option<Arc<Revoker>>,
}
struct Gate {
    capture: watch::Receiver<Snapshot>,
    state: Mutex<State>,
    phase: watch::Sender<Option<u64>>,
    ended: watch::Sender<bool>,
    closed: AtomicBool,
}
impl Gate {
    fn current(&self) -> Option<(u64, u32)> {
        let mut state = self.state.lock().expect("clipboard phase");
        let activation = if self.closed.load(Ordering::Acquire) || *self.ended.borrow() {
            None
        } else {
            self.capture.borrow().activation
        };
        if state.activation != activation {
            if let Some(revoker) = state.revoker.take() {
                revoker.revoke();
            }
            state.activation = activation;
            if activation.is_some() {
                let Some(next) = state.epoch.checked_add(1) else {
                    self.closed.store(true, Ordering::Release);
                    self.ended.send_replace(true);
                    state.activation = None;
                    self.phase.send_replace(None);
                    return None;
                };
                state.epoch = next;
            }
            self.phase.send_replace(activation.map(|_| state.epoch));
        }
        state.activation.map(|activation| (state.epoch, activation))
    }
    fn install(&self, epoch: u64, revoker: Arc<Revoker>) -> bool {
        if self.current().map(|v| v.0) != Some(epoch) {
            revoker.revoke();
            return false;
        }
        let mut state = self.state.lock().expect("clipboard phase");
        // Recheck the producer while holding the publication lock.
        if self.closed.load(Ordering::Acquire)
            || *self.ended.borrow()
            || self.capture.borrow().activation != state.activation
            || state.epoch != epoch
        {
            revoker.revoke();
            return false;
        }
        state.revoker = Some(revoker);
        true
    }
    fn revoke(&self) {
        self.closed.store(true, Ordering::Release);
        let mut state = self.state.lock().expect("clipboard phase");
        state.activation = None;
        if let Some(revoker) = state.revoker.take() {
            revoker.revoke();
        }
        self.phase.send_replace(None);
    }
}
struct Monitor(tokio::task::JoinHandle<()>);
impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct Live {
    epoch: u64,
    transport: portal::production::Production,
    _ended: watch::Sender<bool>,
    _forwarder: Monitor,
    _monitor: Monitor,
    _phase: Monitor,
}
pub(super) struct Production {
    pid: u32,
    gate: Arc<Gate>,
    serials: Serials,
    live: Option<Live>,
    events: Option<BoxStream<'static, ClipboardEvent>>,
    output: mpsc::Sender<ClipboardEvent>,
    issued: Arc<Mutex<BTreeMap<u32, u64>>>,
    _monitor: Monitor,
}
impl Production {
    pub fn refresh(&self) {
        self.gate.current();
    }
    pub fn new(
        pid: u32,
        mut capture: watch::Receiver<Snapshot>,
        ended: watch::Sender<bool>,
    ) -> Self {
        let (phase, _) = watch::channel(None);
        let gate = Arc::new(Gate {
            capture: capture.clone(),
            state: Mutex::new(State {
                epoch: 0,
                activation: None,
                revoker: None,
            }),
            phase,
            ended: ended.clone(),
            closed: AtomicBool::new(false),
        });
        let source = gate.clone();
        let mut closing = ended.subscribe();
        let monitor = Monitor(tokio::spawn(async move {
            loop {
                tokio::select! {biased;
                    _=stopped(&mut closing)=>break,
                    changed=capture.changed()=>{if changed.is_err() {break;}source.current();},
                }
            }
            source.revoke();
        }));
        let (output, events) = mpsc::channel(32);
        Self {
            pid,
            gate,
            serials: Serials::default(),
            live: None,
            output,
            events: Some(
                futures_util::stream::unfold(events, |mut receiver| async move {
                    receiver.recv().await.map(|event| (event, receiver))
                })
                .boxed(),
            ),
            issued: Arc::new(Mutex::new(BTreeMap::new())),
            _monitor: monitor,
        }
    }
    pub fn revoke(&self) {
        self.gate.revoke();
    }
    async fn retire(&mut self) -> anyhow::Result<()> {
        if let Some(live) = self.live.take() {
            let Live {
                transport,
                _ended,
                _forwarder,
                _monitor,
                _phase,
                ..
            } = live;
            drop(_forwarder);
            drop(_monitor);
            drop(_phase);
            transport.stop().await?;
        }
        Ok(())
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        self.revoke();
        self.retire().await
    }
}
async fn changed(phase: &mut watch::Receiver<Option<u64>>, epoch: u64) {
    loop {
        if *phase.borrow() != Some(epoch) {
            return;
        }
        if phase.changed().await.is_err() {
            return;
        }
    }
}
impl Clipboard for Production {
    fn phase(&self) -> watch::Receiver<Option<u64>> {
        self.gate.current();
        self.gate.phase.subscribe()
    }
    fn events(&mut self) -> Option<BoxStream<'static, ClipboardEvent>> {
        self.events.take()
    }
    fn synchronize<'a>(
        &'a mut self,
        activation: Option<u32>,
    ) -> BoxFuture<'a, anyhow::Result<Option<u64>>> {
        Box::pin(async move {
            self.gate.current();
            let target = activation.and_then(|activation| {
                self.gate
                    .current()
                    .filter(|(_, current)| *current == activation)
            });
            if let Some((epoch, _)) = target
                && self.live.as_ref().is_some_and(|live| live.epoch == epoch)
            {
                return Ok(Some(epoch));
            }
            self.retire().await?;
            let Some((epoch, _)) = target else {
                return Ok(None);
            };
            if self.gate.current() != target {
                return Ok(None);
            }
            let (ended, mut ending) = watch::channel(false);
            let mut phase = self.gate.phase.subscribe();
            let cancellation = ended.clone();
            let phase_monitor = Monitor(tokio::spawn(async move {
                changed(&mut phase, epoch).await;
                cancellation.send_replace(true);
            }));
            let mut transport = match portal::production::Production::start_with_serials(
                self.pid,
                ended.clone(),
                self.serials.clone(),
            )
            .await
            {
                Ok(transport) => transport,
                Err(_) if self.gate.current() != target => return Ok(None),
                Err(error) => return Err(error),
            };
            let revoker = Arc::new(transport.revoker()?);
            if !self.gate.install(epoch, revoker) {
                transport.stop().await?;
                return Ok(None);
            }
            let mut events = transport
                .events()
                .ok_or_else(|| anyhow::anyhow!("Clipboard events unavailable"))?;
            let gate = self.gate.clone();
            let issued = self.issued.clone();
            let output = self.output.clone();
            let mut phase = gate.phase.subscribe();
            let forwarder = Monitor(tokio::spawn(async move {
                loop {
                    let event = tokio::select! {biased;_=changed(&mut phase,epoch)=>return,event=events.next()=>event};
                    let Some(event) = event else {
                        if gate.current().map(|v| v.0) == Some(epoch) {
                            gate.ended.send_replace(true);
                            gate.revoke();
                        }
                        return;
                    };
                    if gate.current().map(|v| v.0) != Some(epoch) {
                        return;
                    }
                    if let portal::Event::Transfer { serial, .. } = &event {
                        let mut issued = issued.lock().expect("clipboard serials");
                        while issued.len() >= 128 {
                            issued.pop_first();
                        }
                        if issued.insert(*serial, epoch).is_some() {
                            gate.ended.send_replace(true);
                            gate.revoke();
                            return;
                        }
                    }
                    if output.try_send(ClipboardEvent { epoch, event }).is_err() {
                        gate.ended.send_replace(true);
                        gate.revoke();
                        return;
                    }
                }
            }));
            let gate = self.gate.clone();
            let monitor = Monitor(tokio::spawn(async move {
                stopped(&mut ending).await;
                if gate.current().map(|v| v.0) == Some(epoch) {
                    gate.ended.send_replace(true);
                    gate.revoke();
                }
            }));
            self.live = Some(Live {
                epoch,
                transport,
                _ended: ended,
                _forwarder: forwarder,
                _monitor: monitor,
                _phase: phase_monitor,
            });
            Ok(Some(epoch))
        })
    }
    fn perform<'a>(
        &'a mut self,
        command: portal::Command,
        admitted: Option<u64>,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, fdo::Result<portal::Response>> {
        Box::pin(async move {
            let current = self.gate.current().map(|v| v.0);
            let serial = match &command {
                portal::Command::Write(serial) | portal::Command::Done(serial, _) => Some(*serial),
                _ => None,
            };
            let issued = serial.and_then(|serial| {
                self.issued
                    .lock()
                    .expect("clipboard serials")
                    .get(&serial)
                    .copied()
            });
            if matches!(command, portal::Command::Done(..)) && issued.is_some() && issued != current
            {
                self.issued
                    .lock()
                    .expect("clipboard serials")
                    .remove(&serial.unwrap());
                return Ok(portal::Response::Unit); // Completion only; never touches a newer transport.
            }
            let epoch = current
                .filter(|epoch| Some(*epoch) == admitted)
                .ok_or_else(|| {
                    fdo::Error::AccessDenied("Clipboard capture activation ended".into())
                })?;
            if serial.is_some() && issued != Some(epoch) {
                return Err(fdo::Error::AccessDenied(
                    "Clipboard serial belongs to another activation".into(),
                ));
            }
            let live = self
                .live
                .as_mut()
                .filter(|live| live.epoch == epoch)
                .ok_or_else(|| fdo::Error::AccessDenied("Clipboard capture is not ready".into()))?;
            let done = matches!(command, portal::Command::Done(..));
            let mut phase = self.gate.phase.subscribe();
            let result = tokio::select! {biased;
                _=changed(&mut phase,epoch)=>Err(fdo::Error::AccessDenied("Clipboard capture activation ended".into())),
                result=live.transport.perform(command,cancelled)=>result,
            };
            if done {
                self.issued
                    .lock()
                    .expect("clipboard serials")
                    .remove(&serial.unwrap());
            }
            result
        })
    }
}
impl Drop for Production {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(activation: Option<u32>) -> Snapshot {
        Snapshot {
            zone_set: 1,
            zones: vec![(100, 80, 0, 0)],
            connected: true,
            receiver_ready: true,
            enabled: true,
            activation,
            zones_valid: true,
        }
    }
    #[test]
    fn producer_changes_invalidate_old_epochs_and_revocation_cannot_be_reopened() {
        let (source, capture) = watch::channel(snapshot(None));
        let (ended, _) = watch::channel(false);
        let (phase, _) = watch::channel(None);
        let gate = Gate {
            capture,
            state: Mutex::new(State {
                epoch: 0,
                activation: None,
                revoker: None,
            }),
            phase,
            ended,
            closed: AtomicBool::new(false),
        };
        assert_eq!(gate.current(), None);
        source.send_replace(snapshot(Some(7)));
        assert_eq!(gate.current(), Some((1, 7)));
        assert_eq!(gate.current(), Some((1, 7)));
        // A coalesced producer watch still invalidates the prior authority.
        source.send_replace(snapshot(Some(8)));
        assert_eq!(gate.current(), Some((2, 8)));
        source.send_replace(snapshot(None));
        assert_eq!(gate.current(), None);
        source.send_replace(snapshot(Some(9)));
        assert_eq!(gate.current(), Some((3, 9)));
        gate.revoke();
        source.send_replace(snapshot(Some(10)));
        assert_eq!(gate.current(), None);
    }
    #[tokio::test]
    async fn retired_completion_does_no_io_and_cannot_complete_a_new_activations_serial() {
        let (source, capture) = watch::channel(snapshot(None));
        let (ended, _) = watch::channel(false);
        let (_cancel, cancelled) = watch::channel(false);
        let mut control = Production::new(1, capture, ended);
        control.issued.lock().unwrap().insert(41, 0);
        assert!(matches!(
            control
                .perform(portal::Command::Done(41, true), None, cancelled.clone())
                .await,
            Ok(portal::Response::Unit)
        ));
        assert!(
            control
                .perform(portal::Command::Done(41, true), None, cancelled.clone())
                .await
                .is_err()
        );
        source.send_replace(snapshot(Some(3)));
        let epoch = control.gate.current().unwrap().0;
        control.issued.lock().unwrap().insert(42, epoch);
        assert!(
            control
                .perform(portal::Command::Done(42, true), None, cancelled.clone())
                .await
                .is_err()
        );
        assert_eq!(control.issued.lock().unwrap().get(&42), Some(&epoch));
        assert!(
            control
                .perform(portal::Command::Write(42), Some(epoch - 1), cancelled)
                .await
                .is_err()
        );
        control.stop().await.unwrap();
    }
}
