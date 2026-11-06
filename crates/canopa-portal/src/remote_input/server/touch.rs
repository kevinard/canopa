use super::super::touch::{Command, Status, compositor::Touch as Actor};
use super::pointer::Layout;
use super::*;

pub(super) struct Touch {
    actor: Actor,
    layout: Layout,
    frame: Vec<Command>,
    pub state: Option<State>,
}
impl Touch {
    pub async fn open(
        connection: &zbus::Connection,
        input: &super::super::Input,
    ) -> anyhow::Result<Self> {
        let mut actor = Actor::open(connection, input).await?;
        let layout = match actor.ei_regions().and_then(Layout::new) {
            Ok(layout) => layout,
            Err(error) => {
                actor.close().await?;
                return Err(error);
            }
        };
        Ok(Self {
            actor,
            layout,
            frame: Vec::new(),
            state: None,
        })
    }
    pub fn add(
        &mut self,
        seat: &reis::request::Seat,
        converter: &EisRequestConverter,
        budget: &mut Pending,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.state.is_none(), "Repeated EI touch bind");
        anyhow::ensure!(
            converter
                .handle()
                .interface_version(eis::Device::NAME)
                .is_some_and(|v| v >= 2)
                && converter.handle().interface_version(eis::Touchscreen::NAME) == Some(1),
            "EI touch region pairing unavailable"
        );
        let device = seat.add_device(
            Some("Canopa touch"),
            eis::device::DeviceType::Virtual,
            DeviceCapability::Touch.into(),
            |device| self.layout.announce(device),
        );
        budget.attach(&device)?;
        self.state = Some(State::new(device, converter));
        Ok(())
    }
    pub fn input(&mut self, request: EisRequest) -> anyhow::Result<()> {
        let command = match request {
            EisRequest::TouchDown(e) => {
                let (node, x, y) = self.layout.position(e.x.into(), e.y.into())?;
                Command::Down {
                    contact: e.touch_id,
                    node,
                    x,
                    y,
                }
            }
            EisRequest::TouchMotion(e) => {
                let (node, x, y) = self.layout.position(e.x.into(), e.y.into())?;
                Command::Motion {
                    contact: e.touch_id,
                    node,
                    x,
                    y,
                }
            }
            EisRequest::TouchUp(e) => Command::Up {
                contact: e.touch_id,
            },
            // Version1 is negotiated explicitly; v2 point cancellation is not
            // translated into a seat-wide cancel that could affect other owners.
            _ => anyhow::bail!("Unsupported EI touch request"),
        };
        anyhow::ensure!(
            command.valid() && self.frame.len() < 512,
            "Invalid or excessive EI touch input"
        );
        self.frame.push(command);
        Ok(())
    }
    pub async fn frame(&mut self) -> anyhow::Result<bool> {
        self.frame.push(Command::Frame);
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            for command in self.frame.drain(..) {
                match self.actor.send(command).await? {
                    Some(Status::Accepted) => {}
                    None => return Ok(false),
                    _ => anyhow::bail!("Compositor rejected EI touch input"),
                }
            }
            Ok::<_, anyhow::Error>(true)
        })
        .await?;
        self.frame.clear();
        result
    }
    pub async fn ended(&mut self) -> anyhow::Result<()> {
        self.actor.ended().await
    }
    pub async fn renew(
        &mut self,
        connection: &zbus::Connection,
        input: &super::super::Input,
    ) -> anyhow::Result<bool> {
        self.frame.clear();
        if !self.actor.renew(connection, input).await? {
            return Ok(false);
        }
        self.layout = Layout::new(self.actor.ei_regions()?)?;
        Ok(true)
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.frame.clear();
        self.actor.close().await
    }
    pub async fn reset(
        &mut self,
        connection: &zbus::Connection,
        input: &super::super::Input,
        stop: &watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        self.close().await?;
        anyhow::ensure!(!*stop.borrow(), "Input cancelled");
        let mut next = Self::open(connection, input).await?;
        if *stop.borrow() {
            next.close().await?;
            anyhow::bail!("Input cancelled");
        }
        next.state = self.state.take();
        if let Some(s) = &mut next.state {
            s.active = false;
        }
        *self = next;
        Ok(())
    }
    pub fn replace_device(
        &mut self,
        converter: &EisRequestConverter,
        budget: &mut Pending,
        ids: &mut std::collections::VecDeque<u64>,
    ) -> anyhow::Result<()> {
        if let Some(old) = self.state.take() {
            let seat = old.device.seat().clone();
            old.retire(budget, ids);
            self.add(&seat, converter, budget)?;
        }
        Ok(())
    }
}
