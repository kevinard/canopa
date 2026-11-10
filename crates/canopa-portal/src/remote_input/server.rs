//! An anonymous EI connection with independent keyboard and pointer lifetimes.
use super::pending::Pending;
use reis::{
    Interface, PendingRequestResult, eis,
    handshake::EisHandshaker,
    request::{Device, DeviceCapability, EisRequest, EisRequestConverter},
};
use std::os::unix::net::UnixStream;
use tokio::sync::watch;
mod device;
mod keyboard;
mod pointer;
mod touch;
use device::{Capabilities, State};
use keyboard::Keyboard;
use pointer::Pointer;
use touch::Touch;

fn serial_valid(serial: u32, first: u32, last: u32) -> bool {
    serial.wrapping_sub(first) < 1 << 31 && last.wrapping_sub(serial) < 1 << 31
}
fn peer_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::UnexpectedEof
    )
}
fn flush(context: &eis::Context) -> std::io::Result<bool> {
    match context.flush().map_err(std::io::Error::from) {
        Ok(()) => Ok(true),
        Err(error) if peer_closed(&error) => Ok(false),
        Err(error) => Err(error),
    }
}
pub(super) async fn run(
    connection: &zbus::Connection,
    proxy: &zbus::Proxy<'_>,
    input: &super::Input,
    stop: &mut watch::Receiver<bool>,
    ready: impl FnOnce(UnixStream) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut session = Session {
        connection,
        proxy: proxy.clone(),
        input,
        stop,
        keyboard: None,
        pointer: None,
        touch: None,
    };
    let result = async {
        if input.grants & 1 != 0 {
            session.keyboard = Some(Keyboard::open(proxy).await?);
        }
        if input.grants & 2 != 0 {
            session.pointer = Some(Pointer::open(connection, input).await?);
        }
        if input.grants & 4 != 0 {
            session.touch = Some(Touch::open(connection, input).await?);
        }
        anyhow::ensure!(!*session.stop.borrow(), "Input cancelled");
        let (server, client) = UnixStream::pair()?;
        ready(client)?;
        session.serve(server).await
    }
    .await;
    // Attempt both acknowledgements even if one source fails to retire.
    let keyboard = match &mut session.keyboard {
        Some(source) => source.close().await,
        None => Ok(()),
    };
    let pointer = match &mut session.pointer {
        Some(source) => source.close().await,
        None => Ok(()),
    };
    let touch = match &mut session.touch {
        Some(source) => source.close().await,
        None => Ok(()),
    };
    keyboard?;
    pointer?;
    touch?;
    result
}
struct Session<'a> {
    connection: &'a zbus::Connection,
    proxy: zbus::Proxy<'a>,
    input: &'a super::Input,
    stop: &'a mut watch::Receiver<bool>,
    keyboard: Option<Keyboard>,
    pointer: Option<Pointer>,
    touch: Option<Touch>,
}
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Keyboard,
    Pointer,
    Touch,
}
impl Session<'_> {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::empty();
        if self.keyboard.is_some() {
            caps |= DeviceCapability::Keyboard;
        }
        if let Some(pointer) = &self.pointer {
            caps |= pointer.capabilities();
        }
        if self.touch.is_some() {
            caps |= DeviceCapability::Touch;
        }
        caps
    }
    fn state(&self, device: &eis::Device) -> anyhow::Result<&State> {
        self.keyboard
            .as_ref()
            .and_then(|k| k.state.as_ref())
            .into_iter()
            .chain(self.pointer.as_ref().and_then(|p| p.state.as_ref()))
            .chain(self.touch.as_ref().and_then(|t| t.state.as_ref()))
            .find(|s| s.device.device() == device)
            .ok_or_else(|| anyhow::anyhow!("Unknown EI device"))
    }
    async fn renew_pointer(
        &mut self,
        converter: Option<&EisRequestConverter>,
        budget: &mut Pending,
        ids: &mut std::collections::VecDeque<u64>,
    ) -> anyhow::Result<bool> {
        let pointer = self.pointer.as_mut().expect("pointer source");
        if !pointer.renew(self.connection, self.input).await? {
            return Ok(false);
        }
        anyhow::ensure!(!*self.stop.borrow(), "Input cancelled");
        if let Some(converter) = converter {
            pointer.replace_device(converter, budget, ids)?;
        }
        Ok(true)
    }
    async fn renew_touch(
        &mut self,
        converter: Option<&EisRequestConverter>,
        budget: &mut Pending,
        ids: &mut std::collections::VecDeque<u64>,
    ) -> anyhow::Result<bool> {
        let touch = self.touch.as_mut().expect("touch source");
        if !touch.renew(self.connection, self.input).await? {
            return Ok(false);
        }
        anyhow::ensure!(!*self.stop.borrow(), "Input cancelled");
        if let Some(converter) = converter {
            touch.replace_device(converter, budget, ids)?;
        }
        Ok(true)
    }
    async fn serve(&mut self, socket: UnixStream) -> anyhow::Result<()> {
        let context = eis::Context::new(socket)?;
        // SAFETY: every Context clone shares one Arc-owned socket, kept open by
        // the clone this AsyncFd holds.
        let readiness = unsafe { tokio::io::unix::AsyncFd::register(context.clone()) }
            .map_err(std::io::Error::from)?;
        let mut handshaker = EisHandshaker::new(&context, 1);
        let mut handshake_count = 0;
        let mut sender = false;
        let mut converter: Option<EisRequestConverter> = None;
        let mut budget = Pending::default();
        let mut retired_ids = std::collections::VecDeque::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = async {
            loop {
                if *self.stop.borrow() { return Ok(()); }
                tokio::select! {
                    biased;
                    _ = self.stop.changed() => return Ok(()),
                    _ = tokio::time::sleep_until(deadline), if converter.is_none() => anyhow::bail!("EI handshake timed out"),
                    read = async { match &mut self.keyboard { Some(k) => k.read().await, None => std::future::pending().await } } => {
                        if !read? { return Ok(()); }
                        if let (Some(converter), Some(keyboard)) = (&converter, &self.keyboard) { keyboard.modifiers(converter); if !flush(&context)? { return Ok(()); } }
                    }
                    ended = async { match &mut self.pointer { Some(p) => p.ended().await, None => std::future::pending().await } } => {
                        ended?;
                        if !self.renew_pointer(converter.as_ref(), &mut budget, &mut retired_ids).await? { return Ok(()); }
                        if !flush(&context)? { return Ok(()); }
                    }
                    ended=async {match &mut self.touch {Some(t)=>t.ended().await,None=>std::future::pending().await}}=>{
                        ended?;if !self.renew_touch(converter.as_ref(),&mut budget,&mut retired_ids).await? {return Ok(());}
                        if !flush(&context)? {return Ok(());}
                    }
                    ready = readiness.readable() => {
                        let mut ready = ready?;
                        match context.read_chunk() {
                            Ok(0) => { ready.clear_ready(); continue; }
                            Ok(_) => {},
                            Err(e) if peer_closed(&e) => return Ok(()),
                            Err(e) => return Err(e.into()),
                        }
                        // Keep readiness until a bounded read reaches WouldBlock.
                        while let Some(raw) = context.pending_request() {
                            if *self.stop.borrow() { return Ok(()); }
                            let mut raw = match raw {
                                PendingRequestResult::Request(raw) => raw,
                                PendingRequestResult::InvalidObject(id) if retired_ids.contains(&id) => continue,
                                _ => anyhow::bail!("Invalid EI request"),
                            };
                            if let Some(converter) = converter.as_mut() {
                                if matches!(&raw, eis::Request::Seat(_, eis::seat::Request::Release)) { return Ok(()); }
                                if let Some(keyboard) = self.keyboard.as_mut().filter(|k| k.state.as_ref().is_some_and(|s| s.closing(&raw))) {
                                    keyboard.reset(&self.proxy, self.stop).await?;
                                    keyboard.state.take().unwrap().retire(&mut budget, &mut retired_ids);
                                    if !flush(&context)? { return Ok(()); } continue;
                                }
                                if let Some(pointer) = self.pointer.as_mut().filter(|p| p.state.as_ref().is_some_and(|s| s.closing(&raw))) {
                                    pointer.reset(self.connection, self.input, self.stop).await?;
                                    pointer.state.take().unwrap().retire(&mut budget, &mut retired_ids);
                                    if !flush(&context)? { return Ok(()); } continue;
                                }
                                if let Some(touch)=self.touch.as_mut().filter(|t|t.state.as_ref().is_some_and(|s|s.closing(&raw))) {
                                    touch.reset(self.connection,self.input,self.stop).await?;
                                    touch.state.take().unwrap().retire(&mut budget,&mut retired_ids);
                                    if !flush(&context)? {return Ok(());}continue;
                                }
                                if let eis::Request::Device(proxy, eis::device::Request::Frame { last_serial, .. } | eis::device::Request::StopEmulating { last_serial }) = &raw {
                                    self.state(proxy)?.validate_frame(*last_serial, converter.handle().last_serial())?;
                                }
                                budget.observe(&raw)?;
                                converter.handle_request(raw)?;
                                while let Some(request) = converter.next_request() {
                                    if *self.stop.borrow() { return Ok(()); }
                                    // StopEmulating can commit an implicit frame.
                                    // If that frame retired the old pointer, its
                                    // queued stop cannot affect the replacement.
                                    if request.device().is_some_and(|d| retired_ids.contains(&d.device().as_object().id())) { continue; }
                                    if !self.handle(request, converter, &mut budget, &mut retired_ids).await? { return Ok(()); }
                                }
                            } else {
                                handshake_count += 1;
                                anyhow::ensure!(handshake_count <= 64, "Too many EI handshake requests");
                                if let eis::Request::Handshake(_, request) = &raw {
                                    match request {
                                        eis::handshake::Request::ContextType { context_type } => {
                                            anyhow::ensure!(*context_type == eis::handshake::ContextType::Sender, "EI receiver role denied"); sender = true;
                                        }
                                        eis::handshake::Request::Name { name } | eis::handshake::Request::InterfaceVersion { name, .. } => anyhow::ensure!(name.len() <= 256, "EI handshake label too large"),
                                        eis::handshake::Request::Finish => anyhow::ensure!(sender, "EI sender role required"),
                                        _ => {},
                                    }
                                }
                                if let eis::Request::Handshake(_,eis::handshake::Request::InterfaceVersion { name,version })=&mut raw
                                    && name==eis::Touchscreen::NAME { *version=(*version).min(1); }
                                if let Some(response) = handshaker.handle_request(raw)? {
                                    anyhow::ensure!(response.context_type == eis::handshake::ContextType::Sender, "EI sender role required");
                                    let next = EisRequestConverter::new(&context, response, 1);
                                    let _ = next.handle().add_seat(Some("Canopa approved input"), self.capabilities());
                                    converter = Some(next);
                                }
                            }
                            if !flush(&context)? { return Ok(()); }
                        }
                        tokio::task::yield_now().await;
                    }
                }
            }
        }.await;
        if let Some(state) = self.keyboard.as_mut().and_then(|k| k.state.take()) {
            state.retire(&mut budget, &mut retired_ids);
        }
        if let Some(state) = self.pointer.as_mut().and_then(|p| p.state.take()) {
            state.retire(&mut budget, &mut retired_ids);
        }
        if let Some(state) = self.touch.as_mut().and_then(|t| t.state.take()) {
            state.retire(&mut budget, &mut retired_ids);
        }
        let _ = context.flush();
        result
    }
    async fn handle(
        &mut self,
        request: EisRequest,
        converter: &EisRequestConverter,
        budget: &mut Pending,
        ids: &mut std::collections::VecDeque<u64>,
    ) -> anyhow::Result<bool> {
        let request = match request {
            EisRequest::RequestDevice(r) => EisRequest::Bind(reis::request::Bind {
                seat: r.seat,
                capabilities: r.capabilities,
            }),
            other => other,
        };
        if let EisRequest::Bind(bind) = request {
            anyhow::ensure!(
                !bind.capabilities.is_empty() && self.capabilities().contains(bind.capabilities),
                "Unapproved EI capabilities"
            );
            if bind.capabilities.contains(DeviceCapability::Keyboard) {
                self.keyboard
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Keyboard not granted"))?
                    .add(&bind.seat, converter, budget)?;
            }
            if bind.capabilities.contains(DeviceCapability::Touch) {
                self.touch
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Touch not granted"))?
                    .add(&bind.seat, converter, budget)?;
            }
            let caps = bind.capabilities & !(DeviceCapability::Keyboard | DeviceCapability::Touch);
            if !caps.is_empty() {
                self.pointer
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("Pointer not granted"))?
                    .add(&bind.seat, caps, converter, budget)?;
            }
            return Ok(true);
        }
        if matches!(request, EisRequest::Disconnect) {
            return Ok(false);
        }
        let device = request
            .device()
            .ok_or_else(|| anyhow::anyhow!("Unsupported EI request"))?;
        self.state(device.device())?;
        let kind = if self
            .keyboard
            .as_ref()
            .is_some_and(|k| k.state.as_ref().is_some_and(|s| &s.device == device))
        {
            Kind::Keyboard
        } else if self
            .pointer
            .as_ref()
            .is_some_and(|p| p.state.as_ref().is_some_and(|s| &s.device == device))
        {
            Kind::Pointer
        } else {
            Kind::Touch
        };
        let state = self.state(device.device())?;
        if !matches!(
            request,
            EisRequest::Ready(_) | EisRequest::DeviceStartEmulating(_)
        ) {
            anyhow::ensure!(state.active, "EI device not emulating");
        }
        match request {
            EisRequest::Ready(_) => {}
            EisRequest::DeviceStartEmulating(event) => {
                let state = match kind {
                    Kind::Keyboard => self.keyboard.as_mut().unwrap().state.as_mut().unwrap(),
                    Kind::Pointer => self.pointer.as_mut().unwrap().state.as_mut().unwrap(),
                    Kind::Touch => self.touch.as_mut().unwrap().state.as_mut().unwrap(),
                };
                state.start(
                    event.last_serial,
                    event.sequence,
                    converter.handle().last_serial(),
                )?;
            }
            EisRequest::KeyboardKey(event) => {
                anyhow::ensure!(kind == Kind::Keyboard, "Wrong EI input class");
                self.keyboard
                    .as_mut()
                    .unwrap()
                    .key(event.key, event.state)?;
            }
            EisRequest::Frame(event) => {
                state.validate_frame(event.last_serial, converter.handle().last_serial())?;
                match kind {
                    Kind::Keyboard => self.keyboard.as_mut().unwrap().frame().await?,
                    Kind::Pointer => {
                        if !self.pointer.as_mut().unwrap().frame().await?
                            && !self.renew_pointer(Some(converter), budget, ids).await?
                        {
                            return Ok(false);
                        }
                    }
                    Kind::Touch => {
                        if !self.touch.as_mut().unwrap().frame().await?
                            && !self.renew_touch(Some(converter), budget, ids).await?
                        {
                            return Ok(false);
                        }
                    }
                }
            }
            EisRequest::DeviceStopEmulating(_) => match kind {
                Kind::Keyboard => {
                    let k = self.keyboard.as_mut().unwrap();
                    k.reset(&self.proxy, self.stop).await?;
                    k.modifiers(converter);
                }
                Kind::Pointer => {
                    let p = self.pointer.as_mut().unwrap();
                    if p.reset(self.connection, self.input, self.stop).await? {
                        p.replace_device(converter, budget, ids)?;
                    }
                }
                Kind::Touch => {
                    let t = self.touch.as_mut().unwrap();
                    t.reset(self.connection, self.input, self.stop).await?;
                    // A fresh device also discards Reis's old held contact IDs.
                    t.replace_device(converter, budget, ids)?;
                }
            },
            EisRequest::DeviceClosed(_) => anyhow::bail!("Unexpected EI device release"),
            other => match kind {
                Kind::Keyboard => anyhow::bail!("Wrong EI input class"),
                Kind::Pointer => self.pointer.as_mut().unwrap().input(other)?,
                Kind::Touch => self.touch.as_mut().unwrap().input(other)?,
            },
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledged_serial_must_be_after_resume_and_not_in_the_future() {
        assert!(serial_valid(10, 10, 12));
        assert!(serial_valid(12, 10, 12));
        assert!(!serial_valid(9, 10, 12));
        assert!(!serial_valid(13, 10, 12));
        assert!(serial_valid(0, u32::MAX, 1));
        assert!(!serial_valid(u32::MAX - 1, u32::MAX, 1));
    }
}
