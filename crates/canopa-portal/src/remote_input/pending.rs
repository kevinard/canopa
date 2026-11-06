//! Bound raw input before Reis buffers it until a device frame.
use reis::{Interface, Object, eis, request::Device};
use std::{collections::HashMap, io};

const LIMIT: usize = 512;
const DEVICES: usize = 3;

#[derive(Default)]
pub(crate) struct Pending {
    owners: HashMap<Object, Object>,
    counts: HashMap<Object, usize>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Pending {
    pub(crate) fn attach(&mut self, device: &Device) -> io::Result<()> {
        let owner = device.device().as_object().clone();
        if self.counts.len() >= DEVICES || self.counts.contains_key(&owner) {
            return Err(invalid("Too many or repeated EIS devices"));
        }
        let mut objects = vec![owner.clone()];
        macro_rules! interface {
            ($kind:ty) => {
                if let Some(proxy) = device.interface::<$kind>() {
                    objects.push(proxy.as_object().clone());
                }
            };
        }
        interface!(eis::Keyboard);
        interface!(eis::Pointer);
        interface!(eis::PointerAbsolute);
        interface!(eis::Button);
        interface!(eis::Scroll);
        interface!(eis::Touchscreen);
        if objects.len() == 1
            || objects
                .iter()
                .any(|object| self.owners.contains_key(object))
        {
            return Err(invalid("Unsupported or repeated EIS device interfaces"));
        }
        for object in objects {
            self.owners.insert(object, owner.clone());
        }
        self.counts.insert(owner, 0);
        Ok(())
    }

    pub(crate) fn detach(&mut self, device: &Device) {
        let owner = device.device().as_object();
        self.counts.remove(owner);
        self.owners.retain(|_, value| value != owner);
    }

    /// Call before conversion, then drain the converter after each raw request.
    /// A failure retires the connection; never drop an event and continue input.
    pub(crate) fn observe(&mut self, request: &eis::Request) -> io::Result<()> {
        let (object, input, flush) = match request {
            eis::Request::Keyboard(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::keyboard::Request::Release),
                false,
            ),
            eis::Request::Pointer(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::pointer::Request::Release),
                false,
            ),
            eis::Request::PointerAbsolute(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::pointer_absolute::Request::Release),
                false,
            ),
            eis::Request::Button(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::button::Request::Release),
                false,
            ),
            eis::Request::Scroll(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::scroll::Request::Release),
                false,
            ),
            eis::Request::Device(proxy, event) => (
                proxy.as_object(),
                false,
                matches!(
                    event,
                    eis::device::Request::Frame { .. }
                        | eis::device::Request::StopEmulating { .. }
                        | eis::device::Request::Release
                ),
            ),
            eis::Request::Touchscreen(proxy, event) => (
                proxy.as_object(),
                !matches!(event, eis::touchscreen::Request::Release),
                false,
            ),
            eis::Request::Text(_, _) => {
                return Err(invalid("Unsupported EIS input class"));
            }
            _ => return Ok(()),
        };
        let owner = self
            .owners
            .get(object)
            .ok_or_else(|| invalid("Unknown EIS input device"))?;
        let count = self.counts.get_mut(owner).expect("registered EIS owner");
        if flush {
            // reis commits this device's pending input on Frame/Stop/Release.
            *count = 0;
        } else if input {
            if *count >= LIMIT {
                return Err(invalid("Too much unframed EIS input"));
            }
            *count += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reis::{
        handshake::EisHandshakeResp,
        request::{DeviceCapability, EisRequestConverter, Seat},
    };
    use std::os::unix::net::UnixStream;

    struct Fixture {
        _context: eis::Context,
        _client: UnixStream,
        _converter: EisRequestConverter,
        seat: Seat,
    }

    impl Fixture {
        fn new() -> Self {
            let (server, client) = UnixStream::pair().unwrap();
            let context = eis::Context::new(server).unwrap();
            let response = EisHandshakeResp {
                connection: context.handshake().connection(1, 1),
                name: Some("pending budget test".into()),
                context_type: eis::handshake::ContextType::Sender,
                negotiated_interfaces: HashMap::from([
                    (eis::Device::NAME.into(), 1),
                    (eis::Seat::NAME.into(), 1),
                    (eis::Keyboard::NAME.into(), 1),
                    (eis::Pointer::NAME.into(), 1),
                    (eis::PointerAbsolute::NAME.into(), 1),
                    (eis::Button::NAME.into(), 1),
                    (eis::Scroll::NAME.into(), 1),
                    (eis::Touchscreen::NAME.into(), 1),
                ]),
            };
            let converter = EisRequestConverter::new(&context, response, 1);
            let seat = converter.handle().add_seat(
                Some("test"),
                DeviceCapability::Keyboard
                    | DeviceCapability::Pointer
                    | DeviceCapability::PointerAbsolute
                    | DeviceCapability::Button
                    | DeviceCapability::Scroll
                    | DeviceCapability::Touch,
            );
            Self {
                _context: context,
                _client: client,
                _converter: converter,
                seat,
            }
        }

        fn pointer(&self) -> Device {
            self.seat.add_device(
                Some("pointer"),
                eis::device::DeviceType::Virtual,
                DeviceCapability::Pointer
                    | DeviceCapability::PointerAbsolute
                    | DeviceCapability::Button
                    | DeviceCapability::Scroll,
                |device| device.device().region(0, 0, 1280, 800, 1.0),
            )
        }

        fn keyboard(&self) -> Device {
            self.seat.add_device(
                Some("keyboard"),
                eis::device::DeviceType::Virtual,
                DeviceCapability::Keyboard.into(),
                |_| {},
            )
        }
    }

    fn motion(device: &Device) -> eis::Request {
        eis::Request::Pointer(
            device.interface().unwrap(),
            eis::pointer::Request::MotionRelative { x: 1.0, y: 2.0 },
        )
    }

    fn frame(device: &Device) -> eis::Request {
        eis::Request::Device(
            device.device().clone(),
            eis::device::Request::Frame {
                last_serial: 1,
                timestamp: 1,
            },
        )
    }

    #[test]
    fn another_devices_frames_do_not_free_a_flooding_source() {
        let fixture = Fixture::new();
        let first = fixture.pointer();
        let other = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&first).unwrap();
        budget.attach(&other).unwrap();
        for _ in 0..LIMIT {
            budget.observe(&motion(&first)).unwrap();
            budget.observe(&frame(&other)).unwrap();
        }
        assert_eq!(
            budget.observe(&motion(&first)).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn a_devices_own_frame_and_stop_commit_its_pending_input() {
        let fixture = Fixture::new();
        let device = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&device).unwrap();
        for _ in 0..LIMIT {
            budget.observe(&motion(&device)).unwrap();
        }
        budget.observe(&frame(&device)).unwrap();
        for _ in 0..LIMIT {
            budget.observe(&motion(&device)).unwrap();
        }
        budget
            .observe(&eis::Request::Device(
                device.device().clone(),
                eis::device::Request::StopEmulating { last_serial: 1 },
            ))
            .unwrap();
        budget.observe(&motion(&device)).unwrap();
    }

    #[test]
    fn pointer_interfaces_share_one_budget() {
        let fixture = Fixture::new();
        let device = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&device).unwrap();
        for index in 0..LIMIT {
            let event = match index % 4 {
                0 => motion(&device),
                1 => eis::Request::PointerAbsolute(
                    device.interface().unwrap(),
                    eis::pointer_absolute::Request::MotionAbsolute { x: 10.0, y: 20.0 },
                ),
                2 => eis::Request::Button(
                    device.interface().unwrap(),
                    eis::button::Request::Button {
                        button: 272,
                        state: eis::button::ButtonState::Press,
                    },
                ),
                _ => eis::Request::Scroll(
                    device.interface().unwrap(),
                    eis::scroll::Request::Scroll { x: 1.0, y: 2.0 },
                ),
            };
            budget.observe(&event).unwrap();
        }
        assert!(budget.observe(&motion(&device)).is_err());
    }

    #[test]
    fn keyboard_budget_is_not_reset_by_pointer_frames() {
        let fixture = Fixture::new();
        let keyboard = fixture.keyboard();
        let pointer = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&keyboard).unwrap();
        budget.attach(&pointer).unwrap();
        for _ in 0..LIMIT {
            budget
                .observe(&eis::Request::Keyboard(
                    keyboard.interface().unwrap(),
                    eis::keyboard::Request::Key {
                        key: 30,
                        state: eis::keyboard::KeyState::Press,
                    },
                ))
                .unwrap();
            budget.observe(&frame(&pointer)).unwrap();
        }
        assert!(
            budget
                .observe(&eis::Request::Keyboard(
                    keyboard.interface().unwrap(),
                    eis::keyboard::Request::Key {
                        key: 30,
                        state: eis::keyboard::KeyState::Press
                    }
                ))
                .is_err()
        );
    }

    #[test]
    fn retired_identity_cannot_control_a_replacement() {
        let fixture = Fixture::new();
        let old = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&old).unwrap();
        budget.detach(&old);
        let new = fixture.pointer();
        budget.attach(&new).unwrap();
        budget.observe(&motion(&new)).unwrap();
        assert!(budget.observe(&motion(&old)).is_err());
    }

    #[test]
    fn attachment_count_is_bounded_without_partial_publication() {
        let fixture = Fixture::new();
        let first = fixture.pointer();
        let second = fixture.keyboard();
        let third = fixture.seat.add_device(
            Some("touch"),
            eis::device::DeviceType::Virtual,
            DeviceCapability::Touch.into(),
            |_| {},
        );
        let fourth = fixture.pointer();
        let mut budget = Pending::default();
        budget.attach(&first).unwrap();
        budget.attach(&second).unwrap();
        budget.attach(&third).unwrap();
        budget
            .observe(&eis::Request::Touchscreen(
                third.interface().unwrap(),
                eis::touchscreen::Request::Down {
                    touchid: 0,
                    x: 1.,
                    y: 2.,
                },
            ))
            .unwrap();
        for _ in 1..LIMIT {
            budget
                .observe(&eis::Request::Touchscreen(
                    third.interface().unwrap(),
                    eis::touchscreen::Request::Motion {
                        touchid: 0,
                        x: 2.,
                        y: 3.,
                    },
                ))
                .unwrap();
            budget.observe(&frame(&second)).unwrap();
        }
        assert!(
            budget
                .observe(&eis::Request::Touchscreen(
                    third.interface().unwrap(),
                    eis::touchscreen::Request::Motion {
                        touchid: 0,
                        x: 2.,
                        y: 3.
                    }
                ))
                .is_err()
        );
        assert!(budget.attach(&fourth).is_err());
        assert!(budget.observe(&motion(&fourth)).is_err());
    }
}
