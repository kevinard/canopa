//! The ordered Notify adapter owns a private pointer source, never an EI FD.
use super::*;
use crate::remote_input::{PointerCommand as Command, PointerInput as Transport, PointerStatus};

pub(super) struct LegacyPointer {
    input: Transport,
    monitor: Monitor,
    ended: watch::Sender<bool>,
    axes: [bool; 2],
}
impl LegacyPointer {
    pub async fn open(
        connector: &Connector,
        connection: &Connection,
        endpoint: &InputEndpoint,
        caller: OwnedUniqueName,
        binding: Option<&crate::screencast_capture::Bindings>,
        cancelled: watch::Receiver<bool>,
        ended: watch::Sender<bool>,
    ) -> anyhow::Result<Self> {
        let input = connector
            .start_pointer(connection, endpoint, caller, binding, cancelled)
            .await?;
        let monitor = Monitor(watch_source(input.closed(), ended.clone()));
        Ok(Self {
            input,
            monitor,
            ended,
            axes: [false; 2],
        })
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.input.closed()
    }
    pub async fn stop(self) -> anyhow::Result<()> {
        let Self { input, monitor, .. } = self;
        drop(monitor); // A deliberate EI handoff does not revoke consent.
        input.stop().await
    }
    pub async fn send(&mut self, input: PointerInput) -> fdo::Result<()> {
        for command in commands(input, self.axes) {
            let result = self.input.send(command).await;
            match result {
                Ok(PointerStatus::Accepted) => {
                    if let Command::Scroll { x, y, .. } = command {
                        for (axis, value) in [x, y].into_iter().enumerate() {
                            if let Some(value) = value {
                                self.axes[axis] = value != 0.;
                            }
                        }
                    }
                }
                Ok(PointerStatus::Invalid) => {
                    return Err(fdo::Error::InvalidArgs(
                        "Invalid pointer transition or position".into(),
                    ));
                }
                Ok(PointerStatus::Limit) => {
                    return Err(fdo::Error::LimitsExceeded(
                        "Too many held pointer buttons".into(),
                    ));
                }
                Ok(PointerStatus::Unavailable) => {
                    self.axes = [false; 2];
                    return Err(fdo::Error::Failed(
                        "Pointer source changed; input was not replayed".into(),
                    ));
                }
                Err(_) => {
                    self.ended.send_replace(true);
                    return Err(fdo::Error::Failed("Legacy pointer transport ended".into()));
                }
            }
        }
        Ok(())
    }
}
pub(super) fn zero(input: PointerInput) -> bool {
    matches!(
        input,
        PointerInput::Discrete { steps: 0, .. } | PointerInput::Axis { x: 0., y: 0., .. }
    )
}
fn commands(input: PointerInput, axes: [bool; 2]) -> Vec<Command> {
    match input {
        PointerInput::Relative { x, y } => vec![Command::Relative { x, y }],
        PointerInput::Absolute { stream, x, y } => vec![Command::Absolute { node: stream, x, y }],
        PointerInput::Button { code, pressed } => vec![Command::Button { code, pressed }],
        PointerInput::Discrete { axis, steps } => {
            if steps == 0 {
                vec![]
            } else {
                let delta = steps * 120;
                vec![Command::Wheel {
                    x: (axis == 1).then_some(delta),
                    y: (axis == 0).then_some(delta),
                }]
            }
        }
        PointerInput::Axis { x, y, finish } => {
            let mut result = Vec::with_capacity(2);
            if x != 0. || y != 0. {
                result.push(Command::Scroll {
                    x: (x != 0.).then_some(x),
                    y: (y != 0.).then_some(y),
                    finger: true,
                });
            }
            if finish && (axes[0] || axes[1] || x != 0. || y != 0.) {
                result.push(Command::Scroll {
                    x: (axes[0] || x != 0.).then_some(0.),
                    y: (axes[1] || y != 0.).then_some(0.),
                    finger: true,
                });
            }
            result
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smooth_finish_delivers_its_final_delta_before_stopping_all_active_axes() {
        let result = commands(
            PointerInput::Axis {
                x: 0.,
                y: 2.5,
                finish: true,
            },
            [true, false],
        );
        assert_eq!(
            result,
            vec![
                Command::Scroll {
                    x: None,
                    y: Some(2.5),
                    finger: true
                },
                Command::Scroll {
                    x: Some(0.),
                    y: Some(0.),
                    finger: true
                }
            ]
        );
        assert_eq!(
            commands(
                PointerInput::Axis {
                    x: 0.,
                    y: 0.,
                    finish: true
                },
                [false, true]
            ),
            vec![Command::Scroll {
                x: None,
                y: Some(0.),
                finger: true
            }]
        );
        assert!(
            commands(
                PointerInput::Axis {
                    x: 0.,
                    y: 0.,
                    finish: false
                },
                [true, true]
            )
            .is_empty()
        );
        assert!(
            commands(
                PointerInput::Axis {
                    x: 0.,
                    y: 0.,
                    finish: true
                },
                [false, false]
            )
            .is_empty()
        );
    }
    #[test]
    fn discrete_axes_use_v120_and_zero_steps_do_not_end_scrolling() {
        assert!(commands(PointerInput::Discrete { axis: 0, steps: 0 }, [true, true]).is_empty());
        assert_eq!(
            commands(
                PointerInput::Discrete { axis: 0, steps: -2 },
                [false, false]
            ),
            vec![Command::Wheel {
                x: None,
                y: Some(-240)
            }]
        );
        assert_eq!(
            commands(
                PointerInput::Discrete { axis: 1, steps: 32 },
                [false, false]
            ),
            vec![Command::Wheel {
                x: Some(3840),
                y: None
            }]
        );
    }
}
