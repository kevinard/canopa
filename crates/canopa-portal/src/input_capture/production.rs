use super::*;
use crate::{
    input_capture_transport as transport,
    process::DialogProcess,
    screencast_status::{Indicator, Input as StatusInput},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogInput {
    pub app_id: String,
    pub parent_window: String,
    pub modal: bool,
    pub activation_token: Option<String>,
    pub capabilities: u32,
    #[serde(default)]
    pub clipboard: bool,
}
pub struct Production {
    process: Arc<DialogProcess>,
    connector: transport::Connector,
    socket: Option<PathBuf>,
}
impl Production {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self {
            connector: transport::Connector::new(process.clone()),
            process,
            socket: std::env::var_os("NIRI_SOCKET").map(PathBuf::from),
        }
    }
    fn socket(&self) -> anyhow::Result<PathBuf> {
        self.socket
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Input capture compositor unavailable"))
    }
}
struct Lifetime(watch::Sender<bool>);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
struct Monitor(tokio::task::JoinHandle<()>);
impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct Active {
    grants: u32,
    lifetime: Lifetime,
    bridge: Option<transport::Bridge>,
    indicator: Option<Indicator>,
    events: Option<mpsc::Receiver<Event>>,
    clipboard: Option<super::clipboard::Production>,
    _owner: transport::Watch,
    _monitors: Vec<Monitor>,
}
impl Control for Active {
    fn clipboard_enabled(&self) -> bool {
        self.clipboard.is_some()
    }
    fn clipboard(&mut self) -> Option<&mut dyn Clipboard> {
        self.clipboard.as_mut().map(|v| v as &mut dyn Clipboard)
    }
    fn capabilities(&self) -> u32 {
        self.grants
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.lifetime.0.subscribe()
    }
    fn events(&mut self) -> Option<mpsc::Receiver<Event>> {
        self.events.take()
    }
    fn revoke(&self) {
        if let Some(clipboard) = &self.clipboard {
            clipboard.revoke();
        }
        self.lifetime.0.send_replace(true);
        if let Some(bridge) = &self.bridge {
            bridge.revoker().revoke();
        }
    }
    fn perform<'a>(&'a mut self, operation: Operation) -> BoxFuture<'a, fdo::Result<Output>> {
        Box::pin(async move {
            if *self.closed().borrow() {
                return Err(stopped_error());
            }
            let bridge = self.bridge.as_mut().ok_or_else(stopped_error)?;
            if matches!(operation, Operation::Connect) {
                return bridge
                    .connect()
                    .await
                    .map(Output::Fd)
                    .map_err(|error| fdo::Error::Failed(error.to_string()));
            }
            let command = match operation {
                Operation::GetZones => transport::Command::GetZones,
                Operation::SetBarriers { zone_set, barriers } => {
                    transport::Command::SetBarriers { zone_set, barriers }
                }
                Operation::Enable => transport::Command::Enable,
                Operation::Disable => transport::Command::Disable,
                Operation::Release { activation } => {
                    let Some(activation) = activation.or(bridge.snapshot().borrow().activation)
                    else {
                        return Ok(Output::Unit);
                    };
                    transport::Command::Release { activation }
                }
                Operation::Connect => unreachable!(),
            };
            let reply = bridge
                .command(command)
                .await
                .map_err(|error| fdo::Error::Failed(error.to_string()))?;
            if let Some(clipboard) = &self.clipboard {
                clipboard.refresh();
            }
            match reply {
                transport::Reply::Unit => Ok(Output::Unit),
                transport::Reply::Zones { zone_set, zones } => {
                    Ok(Output::Zones { zone_set, zones })
                }
                transport::Reply::Barriers { failed_barriers } => Ok(Output::Barriers {
                    failed: failed_barriers,
                }),
                transport::Reply::Failed { message } => Err(fdo::Error::Failed(message)),
            }
        })
    }
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            self.revoke();
            let bridge = self.bridge.take();
            let indicator = self.indicator.take();
            let clipboard = self.clipboard.take();
            let (bridge, indicator, clipboard) = tokio::join!(
                async {
                    match bridge {
                        Some(v) => v.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match indicator {
                        Some(v) => v.stop().await,
                        None => Ok(()),
                    }
                },
                async {
                    match clipboard {
                        Some(v) => v.stop().await,
                        None => Ok(()),
                    }
                }
            );
            bridge?;
            indicator?;
            clipboard?;
            Ok(())
        })
    }
}
impl Presenter for Production {
    fn capabilities<'a>(&'a self, connection: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async move {
            let Ok(socket) = self.socket() else {
                return 0;
            };
            transport::Endpoint::resolve(connection, socket)
                .await
                .map(|endpoint| endpoint.capabilities())
                .unwrap_or(0)
        })
    }
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Control>>>> {
        Box::pin(async move {
            if *cancelled.borrow() {
                return Ok(None);
            }
            let endpoint = transport::Endpoint::resolve(&connection, self.socket()?).await?;
            let offered = request.capabilities & endpoint.capabilities();
            anyhow::ensure!(offered != 0, "No supported capture capability requested");
            let (ended, _) = watch::channel(false);
            let lifetime = Lifetime(ended);
            let owner = endpoint
                .watch(&connection, request.caller.clone(), lifetime.0.clone())
                .await?;
            let stop = lifetime.0.clone();
            let mut closing = stop.subscribe();
            let request_watch = Monitor(tokio::spawn(async move {
                tokio::select! {biased;_=stopped(&mut closing)=>{},_=stopped(&mut cancelled)=>{stop.send_replace(true);}}
            }));
            let input = DialogInput {
                app_id: request.app_id.clone(),
                parent_window: request.parent_window,
                modal: request.modal,
                activation_token: request.activation_token.clone(),
                capabilities: offered,
                clipboard: request.clipboard,
            };
            let selection: Option<crate::input_capture_ui::Selection> = self
                .process
                .exchange_activated(
                    "--input-capture-dialog",
                    input,
                    lifetime.0.subscribe(),
                    request.activation_token.as_deref(),
                )
                .await?;
            let Some(selection) = selection else {
                return Ok(None);
            };
            anyhow::ensure!(
                selection.capabilities != 0
                    && selection.capabilities & !offered == 0
                    && (!selection.clipboard || request.clipboard)
                    && !*lifetime.0.borrow(),
                "Invalid input capture approval"
            );
            let mut bridge = self
                .connector
                .start(
                    &connection,
                    &endpoint,
                    request.caller,
                    selection.capabilities,
                    lifetime.0.subscribe(),
                )
                .await?;
            let clipboard = selection.clipboard.then(|| {
                super::clipboard::Production::new(
                    endpoint.process_id(),
                    bridge.snapshot(),
                    lifetime.0.clone(),
                )
            });
            let status = StatusInput::for_capture(
                &request.app_id,
                selection.capabilities,
                selection.clipboard,
            )?;
            let indicator = match Indicator::start(
                &self.process,
                status,
                bridge.closed(),
                lifetime.0.subscribe(),
            )
            .await
            {
                Ok(value) => value,
                Err(error) => {
                    bridge.revoker().revoke();
                    bridge.stop().await?;
                    return Err(error);
                }
            };
            let mut source = bridge.closed();
            let mut controls = indicator.closed();
            let revoked = bridge.revoker();
            let stop = lifetime.0.clone();
            let mut closing = stop.subscribe();
            let resource_watch = Monitor(tokio::spawn(async move {
                tokio::select! {biased;_=stopped(&mut closing)=>{},_=stopped(&mut source)=>{},_=stopped(&mut controls)=>{}}
                revoked.revoke();
                stop.send_replace(true);
            }));
            let mut notices = bridge
                .take_events()
                .ok_or_else(|| anyhow::anyhow!("Capture events unavailable"))?;
            let (output, events) = mpsc::channel(16);
            let stop = lifetime.0.clone();
            let mut closing = stop.subscribe();
            let revoked = bridge.revoker();
            let event_watch = Monitor(tokio::spawn(async move {
                loop {
                    let notice = tokio::select! {biased;_=stopped(&mut closing)=>break,notice=notices.recv()=>notice};
                    let event = match notice {
                        Some(transport::Notice::ReceiverReady { .. }) => continue,
                        Some(transport::Notice::Activated {
                            activation,
                            barrier,
                            position,
                        }) => Event::Activated {
                            activation,
                            barrier,
                            position,
                        },
                        Some(transport::Notice::Deactivated { activation }) => {
                            Event::Deactivated { activation }
                        }
                        Some(transport::Notice::Disabled) => Event::Disabled,
                        Some(transport::Notice::ZonesChanged { zone_set }) => {
                            Event::ZonesChanged { zone_set }
                        }
                        None => break,
                    };
                    if output.try_send(event).is_err() {
                        break;
                    }
                }
                revoked.revoke();
                stop.send_replace(true);
            }));
            let active = Active {
                grants: selection.capabilities,
                lifetime,
                bridge: Some(bridge),
                indicator: Some(indicator),
                events: Some(events),
                clipboard,
                _owner: owner,
                _monitors: vec![request_watch, resource_watch, event_watch],
            };
            if *active.closed().borrow() {
                Box::new(active).stop().await?;
                return Ok(None);
            }
            Ok(Some(Box::new(active) as Box<dyn Control>))
        })
    }
}
