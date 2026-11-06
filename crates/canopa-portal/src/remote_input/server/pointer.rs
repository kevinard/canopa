use super::super::pointer::{
    Command, Status,
    compositor::{EiRegion, Pointer as Actor},
};
use super::*;

#[derive(Clone, Debug, PartialEq)]
struct Region {
    descriptor: EiRegion,
    x: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Layout(Vec<Region>);
impl Layout {
    pub(super) fn new(regions: Vec<EiRegion>) -> anyhow::Result<Self> {
        let mut x = 0u32;
        let mut layout = Vec::new();
        for descriptor in regions {
            anyhow::ensure!(
                descriptor.scale.is_finite() && descriptor.scale > 0.,
                "Invalid EI region scale"
            );
            let next = x
                .checked_add(u32::try_from(descriptor.size.0)?)
                .ok_or_else(|| anyhow::anyhow!("EI layout overflow"))?;
            layout.push(Region { descriptor, x });
            x = next;
        }
        Ok(Self(layout))
    }
    pub(super) fn position(&self, x: f64, y: f64) -> anyhow::Result<(u32, f64, f64)> {
        anyhow::ensure!(
            x.is_finite() && y.is_finite() && x >= 0. && y >= 0.,
            "Invalid EI absolute coordinates"
        );
        let region = self
            .0
            .iter()
            .find(|r| {
                x >= f64::from(r.x)
                    && x < f64::from(r.x) + f64::from(r.descriptor.size.0)
                    && y < f64::from(r.descriptor.size.1)
            })
            .ok_or_else(|| anyhow::anyhow!("EI absolute position outside approved regions"))?;
        Ok((region.descriptor.node, x - f64::from(region.x), y))
    }
    fn absolute(&self, x: f64, y: f64) -> anyhow::Result<Command> {
        let (node, x, y) = self.position(x, y)?;
        Ok(Command::Absolute { node, x, y })
    }
    pub(super) fn announce(&self, device: &Device) {
        for region in &self.0 {
            let r = &region.descriptor;
            device.device().region_mapping_id(&r.mapping_id);
            device
                .device()
                .region(region.x, 0, r.size.0 as u32, r.size.1 as u32, r.scale);
        }
    }
}

pub(super) struct Pointer {
    actor: Actor,
    layout: Layout,
    frame: Vec<Command>,
    scroll: bool,
    scroll_stop: bool,
    pub state: Option<State>,
}
impl Pointer {
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
            scroll: false,
            scroll_stop: false,
            state: None,
        })
    }
    pub fn capabilities(&self) -> Capabilities {
        let mut caps =
            DeviceCapability::Pointer | DeviceCapability::Button | DeviceCapability::Scroll;
        if !self.layout.0.is_empty() {
            caps |= DeviceCapability::PointerAbsolute;
        }
        caps
    }
    pub fn add(
        &mut self,
        seat: &reis::request::Seat,
        caps: Capabilities,
        converter: &EisRequestConverter,
        budget: &mut Pending,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.state.is_none() && !caps.is_empty() && self.capabilities().contains(caps),
            "Unapproved EI pointer capabilities"
        );
        if caps.contains(DeviceCapability::PointerAbsolute) {
            anyhow::ensure!(
                converter
                    .handle()
                    .interface_version(eis::Device::NAME)
                    .is_some_and(|version| version >= 2),
                "EI region pairing requires device version 2"
            );
        }
        let added = seat.add_device(
            Some("Canopa pointer"),
            eis::device::DeviceType::Virtual,
            caps,
            |device| {
                // Regions belong to the absolute interface and precede done. The
                // private layout keeps window streams distinct without exposing any
                // capture-session or compositor-stream handles to the client.
                if caps.contains(DeviceCapability::PointerAbsolute) {
                    self.layout.announce(device);
                }
            },
        );
        budget.attach(&added)?;
        self.state = Some(State::new(added, converter));
        Ok(())
    }
    pub fn input(&mut self, request: EisRequest) -> anyhow::Result<()> {
        let command = match request {
            EisRequest::PointerMotion(e) => Some(Command::Relative {
                x: e.dx.into(),
                y: e.dy.into(),
            }),
            EisRequest::PointerMotionAbsolute(e) => Some(
                self.layout
                    .absolute(e.dx_absolute.into(), e.dy_absolute.into())?,
            ),
            EisRequest::Button(e) => Some(Command::Button {
                code: e.button,
                pressed: e.state == eis::button::ButtonState::Press,
            }),
            EisRequest::ScrollDelta(e) => {
                self.begin_scroll()?;
                anyhow::ensure!(
                    e.dx.is_finite() && e.dy.is_finite(),
                    "Invalid EI scroll delta"
                );
                (e.dx != 0. || e.dy != 0.).then_some(Command::Scroll {
                    x: (e.dx != 0.).then_some(e.dx.into()),
                    y: (e.dy != 0.).then_some(e.dy.into()),
                    finger: true,
                })
            }
            EisRequest::ScrollDiscrete(e) => {
                self.begin_scroll()?;
                (e.discrete_dx != 0 || e.discrete_dy != 0).then_some(Command::Wheel {
                    x: (e.discrete_dx != 0).then_some(e.discrete_dx),
                    y: (e.discrete_dy != 0).then_some(e.discrete_dy),
                })
            }
            EisRequest::ScrollStop(e) => self.end_scroll(e.x, e.y)?,
            // The compositor does not synthesize kinetic scroll events; a
            // cancel still ends the selected axes immediately, like stop.
            EisRequest::ScrollCancel(e) => self.end_scroll(e.x, e.y)?,
            _ => anyhow::bail!("Unsupported EI pointer request"),
        };
        if let Some(command) = command {
            anyhow::ensure!(
                command.valid() && self.frame.len() < 512,
                "Invalid or excessive EI pointer input"
            );
            self.frame.push(command);
        }
        Ok(())
    }
    fn begin_scroll(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.scroll, "Repeated EI scroll event in one frame");
        self.scroll = true;
        Ok(())
    }
    fn end_scroll(&mut self, x: bool, y: bool) -> anyhow::Result<Option<Command>> {
        anyhow::ensure!(!self.scroll_stop, "Repeated EI scroll stop in one frame");
        self.scroll_stop = true;
        Ok((x || y).then_some(Command::Scroll {
            x: x.then_some(0.),
            y: y.then_some(0.),
            finger: true,
        }))
    }
    /// False is an acknowledged compositor retirement. The caller discards the
    /// entire old EI device; an interrupted command is never sent a second time.
    pub async fn frame(&mut self) -> anyhow::Result<bool> {
        if self.frame.is_empty() {
            self.frame.push(Command::Barrier);
        }
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            for command in self.frame.drain(..) {
                match self.actor.send(command).await? {
                    Some(Status::Accepted) => {}
                    None => return Ok(false),
                    _ => anyhow::bail!("Compositor rejected EI pointer input"),
                }
            }
            Ok::<_, anyhow::Error>(true)
        })
        .await?;
        self.clear();
        result
    }
    fn clear(&mut self) {
        self.frame.clear();
        self.scroll = false;
        self.scroll_stop = false;
    }
    pub async fn ended(&mut self) -> anyhow::Result<()> {
        self.actor.ended().await
    }
    pub async fn renew(
        &mut self,
        connection: &zbus::Connection,
        input: &super::super::Input,
    ) -> anyhow::Result<bool> {
        self.clear();
        if !self.actor.renew(connection, input).await? {
            return Ok(false);
        }
        self.layout = Layout::new(self.actor.ei_regions()?)?;
        Ok(true)
    }
    pub async fn reset(
        &mut self,
        connection: &zbus::Connection,
        input: &super::super::Input,
        stop: &watch::Receiver<bool>,
    ) -> anyhow::Result<bool> {
        self.close().await?;
        anyhow::ensure!(!*stop.borrow(), "Input cancelled");
        let mut next = Self::open(connection, input).await?;
        if *stop.borrow() {
            next.close().await?;
            anyhow::bail!("Input cancelled");
        }
        let changed = self.layout != next.layout;
        next.state = self.state.take();
        if let Some(state) = &mut next.state {
            state.active = false;
        }
        *self = next;
        Ok(changed)
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        self.clear();
        self.actor.close().await
    }
    pub fn replace_device(
        &mut self,
        converter: &EisRequestConverter,
        budget: &mut Pending,
        ids: &mut std::collections::VecDeque<u64>,
    ) -> anyhow::Result<()> {
        if let Some(old) = self.state.take() {
            let seat = old.device.seat().clone();
            let caps: Capabilities = self
                .capabilities()
                .iter()
                .filter(|cap| old.device.has_capability(*cap))
                .collect();
            old.retire(budget, ids);
            self.add(&seat, caps, converter, budget)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn region(node: u32, size: (i32, i32), scale: f32) -> EiRegion {
        EiRegion {
            node,
            mapping_id: format!("stream-{node}"),
            size,
            scale,
        }
    }
    #[test]
    fn distinct_window_regions_keep_their_mapping_and_local_coordinates() {
        let layout = Layout::new(vec![
            region(10, (400, 300), 1.25),
            region(20, (640, 480), 1.5),
        ])
        .unwrap();
        assert_eq!(layout.0[1].x, 400);
        assert_eq!(
            layout.absolute(450., 40.).unwrap(),
            Command::Absolute {
                node: 20,
                x: 50.,
                y: 40.
            }
        );
        assert_eq!(
            layout.absolute(399.5, 299.5).unwrap(),
            Command::Absolute {
                node: 10,
                x: 399.5,
                y: 299.5
            }
        );
        assert_eq!(layout.0[1].descriptor.mapping_id, "stream-20");
        assert_eq!(layout.0[1].descriptor.scale, 1.5);
        for (x, y) in [
            (1040., 0.),
            (399., 300.),
            (-1., 0.),
            (0., -1.),
            (f64::NAN, 0.),
            (0., f64::INFINITY),
        ] {
            assert!(layout.absolute(x, y).is_err());
        }
    }
}
