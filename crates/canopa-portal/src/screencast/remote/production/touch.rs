//! Ordered Notify touch owns a private worker, and explicitly frames every call.
use super::*;
use crate::remote_input::{TouchCommand as Command, TouchInput as Transport, TouchStatus};
pub(super) struct LegacyTouch {
    input: Transport,
    monitor: Monitor,
    ended: watch::Sender<bool>,
}
impl LegacyTouch {
    pub async fn open(
        connector: &Connector,
        connection: &Connection,
        endpoint: &InputEndpoint,
        caller: OwnedUniqueName,
        binding: &crate::screencast_capture::Bindings,
        cancelled: watch::Receiver<bool>,
        ended: watch::Sender<bool>,
    ) -> anyhow::Result<Self> {
        let input = connector
            .start_touch(connection, endpoint, caller, binding, cancelled)
            .await?;
        let monitor = Monitor(watch_source(input.closed(), ended.clone()));
        Ok(Self {
            input,
            monitor,
            ended,
        })
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.input.closed()
    }
    pub async fn stop(self) -> anyhow::Result<()> {
        let Self { input, monitor, .. } = self;
        drop(monitor);
        input.stop().await
    }
    pub async fn send(&mut self, input: TouchInput) -> fdo::Result<()> {
        let command = match input {
            TouchInput::Down { stream, slot, x, y } => Command::Down {
                contact: slot,
                node: stream,
                x,
                y,
            },
            TouchInput::Motion { stream, slot, x, y } => Command::Motion {
                contact: slot,
                node: stream,
                x,
                y,
            },
            TouchInput::Up { slot } => Command::Up { contact: slot },
        };
        for command in [command, Command::Frame] {
            match self.input.send(command).await {
                Ok(TouchStatus::Accepted) => {}
                Ok(TouchStatus::Invalid) => {
                    return Err(fdo::Error::InvalidArgs(
                        "Invalid touch transition or position".into(),
                    ));
                }
                Ok(TouchStatus::Limit) => {
                    return Err(fdo::Error::LimitsExceeded("Too many touch contacts".into()));
                }
                Ok(TouchStatus::Unavailable) => {
                    return Err(fdo::Error::Failed(
                        "Touch source changed; input was not replayed".into(),
                    ));
                }
                Err(_) => {
                    self.ended.send_replace(true);
                    return Err(fdo::Error::Failed("Legacy touch transport ended".into()));
                }
            }
        }
        Ok(())
    }
}
