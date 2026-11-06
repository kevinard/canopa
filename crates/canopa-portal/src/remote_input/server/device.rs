use super::*;
use std::collections::VecDeque;

pub(super) type Capabilities = reis::enumflags2::BitFlags<DeviceCapability>;

pub(super) struct State {
    pub device: Device,
    resumed: u32,
    sequence: Option<u32>,
    pub active: bool,
}
impl State {
    pub fn new(device: Device, converter: &EisRequestConverter) -> Self {
        device.resumed();
        Self {
            device,
            resumed: converter.handle().last_serial(),
            sequence: None,
            active: false,
        }
    }
    pub fn validate_frame(&self, serial: u32, last: u32) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.active && serial_valid(serial, self.resumed, last),
            "Invalid EI input frame"
        );
        Ok(())
    }
    pub fn start(&mut self, serial: u32, sequence: u32, last: u32) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.active && serial_valid(serial, self.resumed, last),
            "Invalid EI start"
        );
        anyhow::ensure!(
            self.sequence.is_none_or(|previous| {
                let delta = sequence.wrapping_sub(previous);
                delta != 0 && delta < 1 << 31
            }),
            "Invalid EI emulation sequence"
        );
        self.sequence = Some(sequence);
        self.active = true;
        Ok(())
    }
    pub fn closing(&self, raw: &eis::Request) -> bool {
        match raw {
            eis::Request::Device(proxy, eis::device::Request::Release) => {
                self.device.device() == proxy
            }
            eis::Request::Keyboard(proxy, eis::keyboard::Request::Release) => {
                self.device.interface::<eis::Keyboard>().as_ref() == Some(proxy)
            }
            eis::Request::Pointer(proxy, eis::pointer::Request::Release) => {
                self.device.interface::<eis::Pointer>().as_ref() == Some(proxy)
            }
            eis::Request::PointerAbsolute(proxy, eis::pointer_absolute::Request::Release) => {
                self.device.interface::<eis::PointerAbsolute>().as_ref() == Some(proxy)
            }
            eis::Request::Button(proxy, eis::button::Request::Release) => {
                self.device.interface::<eis::Button>().as_ref() == Some(proxy)
            }
            eis::Request::Scroll(proxy, eis::scroll::Request::Release) => {
                self.device.interface::<eis::Scroll>().as_ref() == Some(proxy)
            }
            eis::Request::Touchscreen(proxy, eis::touchscreen::Request::Release) => {
                self.device.interface::<eis::Touchscreen>().as_ref() == Some(proxy)
            }
            _ => false,
        }
    }
    pub fn retire(self, budget: &mut Pending, ids: &mut VecDeque<u64>) {
        budget.detach(&self.device);
        ids.push_back(self.device.device().as_object().id());
        macro_rules! remember {
            ($kind:ty) => {
                if let Some(proxy) = self.device.interface::<$kind>() {
                    ids.push_back(proxy.as_object().id());
                }
            };
        }
        remember!(eis::Keyboard);
        remember!(eis::Pointer);
        remember!(eis::PointerAbsolute);
        remember!(eis::Button);
        remember!(eis::Scroll);
        remember!(eis::Touchscreen);
        while ids.len() > 128 {
            ids.pop_front();
        }
        self.device.remove();
    }
}
