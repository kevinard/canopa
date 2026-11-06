//! Receiver-role EI output. No client-supplied input request reaches a converter.
use super::wire::CaptureEvent;
use reis::{
    Interface, PendingRequestResult, eis,
    handshake::EisHandshaker,
    request::{Device, DeviceCapability, EisRequest, EisRequestConverter},
};
use std::{
    collections::{BTreeSet, VecDeque},
    os::{fd::AsFd, unix::net::UnixStream},
    sync::Arc,
};
use tokio::io::unix::AsyncFd;

mod keymap;
use keymap::Keymap;

type Capabilities = <DeviceCapability as std::ops::BitOr>::Output;
fn capabilities(grants: u32) -> Capabilities {
    let mut caps = Capabilities::empty();
    if grants & 2 != 0 {
        caps |= DeviceCapability::Pointer | DeviceCapability::Button | DeviceCapability::Scroll;
    }
    if grants & 1 != 0 {
        caps |= DeviceCapability::Keyboard;
    }
    caps
}
pub(super) struct ReadOutcome {
    pub ready_changed: bool,
    pub disable: bool,
}
pub(super) struct Receiver {
    context: eis::Context,
    readable: Arc<AsyncFd<eis::Context>>,
    handshaker: EisHandshaker,
    handshake_count: usize,
    converter: Option<EisRequestConverter>,
    device: Option<Device>,
    bound: Capabilities,
    grants: u32,
    keymap: Option<Keymap>,
    seat: Option<reis::request::Seat>,
    retired: VecDeque<u64>,
    ready: bool,
    active: Option<u32>,
    buttons: BTreeSet<u32>,
    keys: BTreeSet<u32>,
    wheel: [f64; 2],
    scrolling: [bool; 2],
    last_time: u64,
}
impl Receiver {
    pub fn new(socket: UnixStream, grants: u32, text: Option<&str>) -> anyhow::Result<Self> {
        anyhow::ensure!(grants != 0 && grants & !3 == 0, "Invalid capture grants");
        anyhow::ensure!(
            (grants & 1 != 0) == text.is_some(),
            "Capture keymap does not match approval"
        );
        let keymap = text.map(Keymap::new).transpose()?;
        let context = eis::Context::new(socket)?;
        Ok(Self {
            readable: Arc::new(AsyncFd::new(context.clone())?),
            handshaker: EisHandshaker::new(&context, 1),
            context,
            handshake_count: 0,
            converter: None,
            device: None,
            bound: Capabilities::empty(),
            grants,
            keymap,
            seat: None,
            retired: VecDeque::new(),
            ready: false,
            active: None,
            buttons: BTreeSet::new(),
            keys: BTreeSet::new(),
            wheel: [0.; 2],
            scrolling: [false; 2],
            last_time: 0,
        })
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
    pub fn handshake_done(&self) -> bool {
        self.converter.is_some()
    }
    pub fn active(&self) -> Option<u32> {
        self.active
    }
    pub fn deactivate(&mut self) -> anyhow::Result<()> {
        self.end();
        self.flush()
    }
    fn flush(&self) -> anyhow::Result<()> {
        // WouldBlock is terminal: a slow consumer cannot retain capture while
        // an unbounded protocol output queue accumulates in this process.
        self.context.flush().map_err(std::io::Error::from)?;
        Ok(())
    }
    fn remove(&mut self) {
        self.end();
        self.ready = false;
        self.bound = Capabilities::empty();
        if let Some(device) = self.device.take() {
            self.retired.push_back(device.device().as_object().id());
            macro_rules! retired {
                ($kind:ty) => {
                    if let Some(interface) = device.interface::<$kind>() {
                        self.retired.push_back(interface.as_object().id());
                    }
                };
            }
            retired!(eis::Pointer);
            retired!(eis::Button);
            retired!(eis::Scroll);
            retired!(eis::Keyboard);
            while self.retired.len() > 64 {
                self.retired.pop_front();
            }
            device.remove();
        }
    }
    /// Returns true when an old receiving device was replaced or unbound.
    fn bind(&mut self, seat: reis::request::Seat, caps: Capabilities) -> anyhow::Result<bool> {
        anyhow::ensure!(
            capabilities(self.grants).contains(caps),
            "Unapproved EI receiver capability"
        );
        if self.device.is_some() && caps == self.bound {
            return Ok(false);
        }
        let replaced = self.device.is_some();
        self.seat = Some(seat.clone());
        self.remove();
        if !caps.is_empty() {
            let device = seat.add_device(
                Some("Canopa captured input"),
                eis::device::DeviceType::Virtual,
                caps,
                |device| {
                    if let Some(keyboard) = device.interface::<eis::Keyboard>() {
                        let map = self.keymap.as_ref().expect("approved keyboard keymap");
                        keyboard.keymap(eis::keyboard::KeymapType::Xkb, map.size, map.file.as_fd());
                    }
                },
            );
            device.resumed();
            self.device = Some(device);
            self.bound = caps;
            // ei_device.ready is sender-only. Configuration and resumed are
            // ordered before captured input on this receiver's EI stream.
            self.ready = true;
        }
        Ok(replaced)
    }
    pub async fn read(&mut self) -> anyhow::Result<Option<ReadOutcome>> {
        let before = self.ready;
        let readable = self.readable.clone();
        let mut guard = readable.readable().await?;
        match self.context.read_chunk() {
            Ok(0) => {
                guard.clear_ready();
                return Ok(Some(ReadOutcome {
                    ready_changed: false,
                    disable: false,
                }));
            }
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        }
        let mut disable = false;
        while let Some(request) = self.context.pending_request() {
            let mut raw = match request {
                PendingRequestResult::Request(raw) => raw,
                PendingRequestResult::InvalidObject(id) if self.retired.contains(&id) => continue,
                _ => anyhow::bail!("Invalid EI receiver request"),
            };
            if self.converter.is_none() {
                self.handshake_count += 1;
                anyhow::ensure!(self.handshake_count <= 64, "Too many EI handshake requests");
                if let eis::Request::Handshake(_, request) = &mut raw {
                    match request {
                        eis::handshake::Request::ContextType { context_type } => anyhow::ensure!(
                            *context_type == eis::handshake::ContextType::Receiver,
                            "EI sender role denied"
                        ),
                        eis::handshake::Request::Name { name } => {
                            anyhow::ensure!(name.len() <= 256, "EI handshake label too large")
                        }
                        eis::handshake::Request::InterfaceVersion { name, version } => {
                            anyhow::ensure!(name.len() <= 256, "EI handshake label too large");
                            // Device v3 adds sender-only ready. libei 1.6 sends
                            // it on receiver connections too; negotiate v2,
                            // which supports every receiving event used here.
                            if name == "ei_device" {
                                *version = (*version).min(2);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(response) = self.handshaker.handle_request(raw)? {
                    anyhow::ensure!(
                        response.context_type == eis::handshake::ContextType::Receiver,
                        "EI receiver role required"
                    );
                    let converter = EisRequestConverter::new(&self.context, response, 1);
                    self.seat = Some(
                        converter
                            .handle()
                            .add_seat(Some("Canopa approved capture"), capabilities(self.grants)),
                    );
                    self.converter = Some(converter);
                }
            } else {
                // Reject event injection before Reis can retain timestamped
                // requests waiting for a frame. Receiver clients send control only.
                let allowed = matches!(
                    &raw,
                    eis::Request::Connection(_, _)
                        | eis::Request::Pingpong(_, _)
                        | eis::Request::Seat(_, _)
                        | eis::Request::Device(_, eis::device::Request::Release)
                        | eis::Request::Pointer(_, eis::pointer::Request::Release)
                        | eis::Request::Button(_, eis::button::Request::Release)
                        | eis::Request::Scroll(_, eis::scroll::Request::Release)
                        | eis::Request::Keyboard(_, eis::keyboard::Request::Release)
                );
                anyhow::ensure!(allowed, "Input request on an EI receiver connection");
                if matches!(&raw, eis::Request::Seat(_, eis::seat::Request::Release)) {
                    self.remove();
                    self.converter.as_mut().unwrap().handle_request(raw)?;
                    return Ok(None);
                }
                // Dropping a receiving interface ends its device. Niri is
                // disabled before this connection may enable a replacement.
                if matches!(
                    &raw,
                    eis::Request::Pointer(_, _)
                        | eis::Request::Button(_, _)
                        | eis::Request::Scroll(_, _)
                        | eis::Request::Keyboard(_, _)
                ) {
                    self.remove();
                    disable = true;
                    continue;
                }
                let converter = self.converter.as_mut().expect("EI converter");
                converter.handle_request(raw)?;
                while let Some(request) = self.converter.as_mut().unwrap().next_request() {
                    match request {
                        EisRequest::Bind(request) => {
                            disable |= self.bind(request.seat, request.capabilities)?;
                        }
                        EisRequest::RequestDevice(request) => {
                            if !self.bound.contains(request.capabilities) {
                                disable |=
                                    self.bind(request.seat, self.bound | request.capabilities)?;
                            }
                        }
                        EisRequest::DeviceClosed(_) => {
                            self.remove();
                            disable = true;
                        }
                        EisRequest::Disconnect => return Ok(None),
                        _ => anyhow::bail!("Unexpected EI receiver control"),
                    }
                }
            }
            self.flush()?;
        }
        Ok(Some(ReadOutcome {
            ready_changed: before != self.ready,
            disable,
        }))
    }
    pub fn begin(&mut self, activation: u32) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.ready && self.active.is_none(),
            "EI receiver is not ready for capture"
        );
        self.device
            .as_ref()
            .expect("ready device")
            .start_emulating(activation);
        self.active = Some(activation);
        self.flush()
    }
    pub fn end(&mut self) {
        if self.active.take().is_none() {
            return;
        }
        if let Some(device) = &self.device {
            let mut framed = false;
            if let Some(button) = device.interface::<eis::Button>() {
                for code in std::mem::take(&mut self.buttons) {
                    button.button(code, eis::button::ButtonState::Released);
                    framed = true;
                }
            }
            if let Some(scroll) = device
                .interface::<eis::Scroll>()
                .filter(|_| self.scrolling.iter().any(|value| *value))
            {
                scroll.scroll_stop(
                    u32::from(self.scrolling[0]),
                    u32::from(self.scrolling[1]),
                    1,
                );
                framed = true;
            }
            if let Some(keyboard) = device.interface::<eis::Keyboard>() {
                for code in std::mem::take(&mut self.keys) {
                    keyboard.key(code, eis::keyboard::KeyState::Released);
                    framed = true;
                }
            }
            if framed {
                device.frame(self.last_time);
            }
            if let Some(keyboard) = device.interface::<eis::Keyboard>()
                && let Some(converter) = &self.converter
            {
                converter
                    .handle()
                    .with_next_serial(|serial| keyboard.modifiers(serial, 0, 0, 0, 0));
            }
            device.stop_emulating();
        }
        self.buttons.clear();
        self.keys.clear();
        self.wheel = [0.; 2];
        self.scrolling = [false; 2];
    }
    pub fn replace_keymap(&mut self, text: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.grants & 1 != 0 && self.active.is_none(),
            "Keymap changed during active capture"
        );
        let next = Keymap::new(text)?;
        let bound = self.bound;
        let seat = self.seat.clone();
        self.remove();
        self.keymap = Some(next);
        if let Some(seat) = seat {
            self.bind(seat, bound)?;
        }
        self.flush()
    }
    pub fn finish(&mut self) {
        self.remove();
        if let Some(converter) = &self.converter {
            converter
                .handle()
                .disconnected(reis::ei::connection::DisconnectReason::Disconnected, None);
        }
        let _ = self.flush();
    }
    pub fn input(&mut self, event: &CaptureEvent) -> anyhow::Result<()> {
        let Some(device) = self
            .device
            .as_ref()
            .filter(|_| self.active.is_some() && self.ready)
        else {
            return Ok(());
        };
        if let CaptureEvent::Modifiers {
            depressed,
            latched,
            locked,
            group,
        } = event
        {
            anyhow::ensure!(self.grants & 1 != 0, "Unapproved keyboard modifiers");
            if let (Some(keyboard), Some(converter)) =
                (device.interface::<eis::Keyboard>(), &self.converter)
            {
                converter.handle().with_next_serial(|serial| {
                    keyboard.modifiers(serial, *depressed, *locked, *latched, *group)
                });
            }
            return self.flush();
        }
        let scalar = |value: f64| -> anyhow::Result<f32> {
            anyhow::ensure!(
                value.is_finite() && (value as f32).is_finite(),
                "Invalid captured pointer amount"
            );
            Ok(value as f32)
        };
        let time = match event {
            CaptureEvent::Key {
                time,
                code,
                pressed,
            } => {
                anyhow::ensure!(
                    self.grants & 1 != 0 && (1..=0x2ff).contains(code),
                    "Unapproved captured key"
                );
                if let Some(keyboard) = device.interface::<eis::Keyboard>() {
                    let valid = if *pressed {
                        self.keys.insert(*code)
                    } else {
                        self.keys.remove(code)
                    };
                    anyhow::ensure!(valid, "Unpaired captured key transition");
                    keyboard.key(
                        *code,
                        if *pressed {
                            eis::keyboard::KeyState::Press
                        } else {
                            eis::keyboard::KeyState::Released
                        },
                    );
                }
                *time
            }
            CaptureEvent::Motion { time, delta } => {
                if let Some(pointer) = device.interface::<eis::Pointer>() {
                    pointer.motion_relative(scalar(delta.0)?, scalar(delta.1)?);
                }
                *time
            }
            CaptureEvent::Button {
                time,
                button,
                pressed,
            } => {
                anyhow::ensure!((1..=65535).contains(button), "Invalid captured button");
                if let Some(interface) = device.interface::<eis::Button>() {
                    if *pressed {
                        anyhow::ensure!(
                            self.buttons.len() < 64 && self.buttons.insert(*button),
                            "Invalid captured button press"
                        );
                    } else {
                        anyhow::ensure!(
                            self.buttons.remove(button),
                            "Unpaired captured button release"
                        );
                    }
                    interface.button(
                        *button,
                        if *pressed {
                            eis::button::ButtonState::Press
                        } else {
                            eis::button::ButtonState::Released
                        },
                    );
                }
                *time
            }
            CaptureEvent::Axis {
                time,
                source,
                amount,
                v120,
            } => {
                anyhow::ensure!((1..=4).contains(source), "Invalid captured axis source");
                if let Some(scroll) = device.interface::<eis::Scroll>() {
                    let amounts = [amount.0, amount.1];
                    let wheels = [v120.0, v120.1];
                    let mut discrete = [0i32; 2];
                    for i in 0..2 {
                        if let Some(value) = wheels[i] {
                            scalar(value)?;
                            let total = self.wheel[i] + value;
                            anyhow::ensure!(
                                total.abs() <= f64::from(i32::MAX),
                                "Captured wheel amount too large"
                            );
                            discrete[i] = total.trunc() as i32;
                            self.wheel[i] = total - f64::from(discrete[i]);
                        }
                    }
                    if wheels.iter().any(Option::is_some) {
                        if discrete != [0, 0] {
                            scroll.scroll_discrete(discrete[0], discrete[1]);
                        }
                    } else {
                        scroll.scroll(
                            scalar(amounts[0].unwrap_or(0.))?,
                            scalar(amounts[1].unwrap_or(0.))?,
                        );
                    }
                    if *source == 2 {
                        for (i, amount) in amounts.iter().enumerate() {
                            if let Some(value) = amount {
                                self.scrolling[i] = *value != 0.;
                            }
                        }
                    }
                    if *source == 2 && amounts.contains(&Some(0.)) {
                        scroll.scroll_stop(
                            u32::from(amounts[0] == Some(0.)),
                            u32::from(amounts[1] == Some(0.)),
                            0,
                        );
                    }
                }
                *time
            }
            _ => anyhow::bail!("Expected captured input"),
        };
        anyhow::ensure!(time >= self.last_time, "Captured timestamp moved backwards");
        self.last_time = time;
        device.frame(time);
        self.flush()
    }
}
