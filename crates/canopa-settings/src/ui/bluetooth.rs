//! The Bluetooth page: each adapter's switch and visibility, paired devices
//! (connect, disconnect, trust, forget, battery) and nearby devices to pair.
//! Pairing questions (confirm a code, enter a PIN or passkey, type a code on
//! the device) appear at the top. Changes apply at once.
use super::{Message, State, field, focus, live_page::LivePage, settings_group, toggle};
use crate::{
    bluetooth::{BluetoothAction, BluetoothService, BluetoothState, Pairing, kind_label},
    live::{Event, LiveAction},
};
use canopa_services::bluetooth::{Device, Question};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, text},
};

pub(super) type BluetoothPage = LivePage<BluetoothService>;

pub(super) fn message(event: Event<BluetoothService>) -> Message {
    Message::Bluetooth(event)
}

fn apply(action: BluetoothAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

/// A button with a focus id of its own, for rows with repeated labels.
fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label.clone())
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            canopa_ui::action(text(label), message, canopa_ui::Action::Normal, false)
                .padding([6, 12]),
        ),
    )
}

/// A stable, short focus id part for an object path.
fn key(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_lowercase()
}

/// The PIN or passkey being typed for the open question.
#[derive(Clone, Debug)]
pub(super) enum Edit {
    Entry(String),
    Submit,
}

impl State {
    pub(super) fn bluetooth_edit(&mut self, edit: Edit) -> Task<Message> {
        match edit {
            Edit::Entry(value) => {
                self.bluetooth_entry = value.chars().filter(|c| !c.is_control()).take(16).collect();
                Task::none()
            }
            Edit::Submit => {
                let value = std::mem::take(&mut self.bluetooth_entry);
                self.bluetooth
                    .update(Event::Apply(BluetoothAction::Answer(value)), message)
            }
        }
    }

    fn pairing_section<'a>(&'a self, pairing: &'a Pairing) -> Element<'a, Message> {
        let page = &self.bluetooth;
        let name = pairing.label.as_str();
        let cancel = || {
            row_button(
                "bluetooth-pairing-cancel".into(),
                fl!("settings-cancel"),
                Some(apply(BluetoothAction::CancelPairing)),
            )
        };
        let answer = |label: String| {
            row_button(
                "bluetooth-pairing-answer".into(),
                label,
                Some(apply(BluetoothAction::Answer(String::new()))),
            )
        };
        let body: Element<'_, Message> = match &pairing.question {
            None => column![
                text(fl!("settings-bluetooth-pairing-with", name = name)),
                cancel()
            ]
            .spacing(10)
            .into(),
            Some(Question::Confirm(code)) => column![
                text(fl!("settings-bluetooth-confirm-code", name = name)),
                text(format!("{code:06}")).size(28),
                direction::row([answer(fl!("settings-bluetooth-pair")), cancel()]).spacing(10),
            ]
            .spacing(10)
            .into(),
            Some(Question::Authorize) => column![
                text(fl!("settings-bluetooth-authorize", name = name)),
                direction::row([answer(fl!("settings-bluetooth-allow")), cancel()]).spacing(10),
            ]
            .spacing(10)
            .into(),
            Some(Question::Service) => column![
                text(fl!("settings-bluetooth-service", name = name)),
                direction::row([answer(fl!("settings-bluetooth-allow")), cancel()]).spacing(10),
            ]
            .spacing(10)
            .into(),
            Some(question @ (Question::Pin | Question::Passkey)) => {
                let pin = *question == Question::Pin;
                let (label, ask, hint) = if pin {
                    (
                        fl!("settings-bluetooth-pin"),
                        fl!("settings-bluetooth-asks-pin", name = name),
                        fl!("settings-bluetooth-pin-hint"),
                    )
                } else {
                    (
                        fl!("settings-bluetooth-passkey"),
                        fl!("settings-bluetooth-asks-passkey", name = name),
                        fl!("settings-bluetooth-passkey-hint"),
                    )
                };
                let submit = question
                    .accepts(self.bluetooth_entry.trim())
                    .map(|_| Message::BluetoothForm(Edit::Submit));
                column![
                    text(ask),
                    field(
                        "bluetooth-pairing-entry",
                        label,
                        &self.bluetooth_entry,
                        |value| { Message::BluetoothForm(Edit::Entry(value)) }
                    ),
                    muted(hint),
                    direction::row([
                        row_button(
                            "bluetooth-pairing-answer".into(),
                            fl!("settings-bluetooth-pair"),
                            submit,
                        ),
                        cancel(),
                    ])
                    .spacing(10),
                ]
                .spacing(10)
                .into()
            }
            Some(Question::DisplayPinCode(pin)) => column![
                text(fl!("settings-bluetooth-type-pin", name = name)),
                text(pin.clone()).size(28),
                cancel(),
            ]
            .spacing(10)
            .into(),
            Some(Question::DisplayPasskey { passkey, entered }) => column![
                text(fl!("settings-bluetooth-type-code", name = name)),
                text(format!("{passkey:06}")).size(28),
                muted({
                    let (entered, total) = (u64::from(*entered), 6u64);
                    fl!(
                        "settings-bluetooth-digits-typed",
                        entered = entered,
                        total = total
                    )
                }),
                cancel(),
            ]
            .spacing(10)
            .into(),
        };
        settings_group(
            fl!("settings-bluetooth-pairing"),
            column![body, page.control_status("pairing", message)].spacing(8),
        )
    }

    fn device_status(device: &Device) -> String {
        // A list of short facts joined by " · ", each a message of its own.
        let mut parts = vec![kind_label(device.kind)];
        if device.paired {
            parts.push(if device.connected {
                fl!("settings-bluetooth-connected")
            } else {
                fl!("settings-bluetooth-not-connected")
            });
        }
        if let Some(battery) = device.battery {
            parts.push(fl!(
                "settings-bluetooth-battery",
                level = canopa_i18n::format::percent(f64::from(battery))
            ));
        }
        if device.blocked {
            parts.push(fl!("settings-bluetooth-blocked"));
        }
        parts.join(" · ")
    }

    fn paired_device<'a>(&'a self, device: &'a Device) -> Element<'a, Message> {
        let page = &self.bluetooth;
        let path = device.path.as_str();
        let id = key(path);
        let control = BluetoothAction::Connect(path.into()).control();
        let trust = BluetoothAction::Trusted(path.into(), true).control();
        let trusted = match page.live.pending(&trust) {
            Some(BluetoothAction::Trusted(_, value)) => *value,
            _ => device.trusted,
        };
        let connect = if device.connected {
            (
                fl!("settings-bluetooth-disconnect"),
                BluetoothAction::Disconnect(path.into()),
            )
        } else {
            (
                fl!("settings-bluetooth-connect"),
                BluetoothAction::Connect(path.into()),
            )
        };
        column![
            direction::row([
                column![text(&device.label), muted(Self::device_status(device))]
                    .spacing(2)
                    .align_x(direction::start())
                    .width(Length::Fill)
                    .into(),
                row_button(
                    format!("bluetooth-{id}-connect"),
                    connect.0,
                    Some(apply(connect.1)),
                ),
                row_button(
                    format!("bluetooth-{id}-forget"),
                    fl!("settings-bluetooth-forget"),
                    Some(apply(BluetoothAction::Forget(path.into()))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            toggle(
                format!("bluetooth-{id}-trusted"),
                fl!("settings-bluetooth-trust"),
                trusted,
                apply(BluetoothAction::Trusted(path.into(), !trusted)),
            ),
            page.control_status(&control, message),
            page.control_status(&trust, message),
        ]
        .spacing(6)
        .into()
    }

    fn nearby_device<'a>(&'a self, device: &'a Device, pairing: bool) -> Element<'a, Message> {
        let path = device.path.as_str();
        let control = BluetoothAction::Pair(path.into()).control();
        column![
            direction::row([
                column![text(&device.label), muted(Self::device_status(device))]
                    .spacing(2)
                    .align_x(direction::start())
                    .width(Length::Fill)
                    .into(),
                row_button(
                    format!("bluetooth-{}-pair", key(path)),
                    fl!("settings-bluetooth-pair"),
                    (!pairing).then(|| apply(BluetoothAction::Pair(path.into()))),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            self.bluetooth.control_status(&control, message),
        ]
        .spacing(4)
        .into()
    }

    fn bluetooth_sections<'a>(&'a self, state: &'a BluetoothState) -> Element<'a, Message> {
        let page = &self.bluetooth;
        let snapshot = &state.snapshot;
        let mut body = column![].spacing(14);
        if let Some(pairing) = &state.pairing {
            body = body.push(self.pairing_section(pairing));
        }
        if snapshot.adapters.is_empty() {
            return body
                .push(settings_group(
                    "Bluetooth",
                    muted(fl!("settings-bluetooth-no-adapter")),
                ))
                .into();
        }
        let several = snapshot.adapters.len() > 1;
        for adapter in &snapshot.adapters {
            let path = adapter.path.as_str();
            let power = BluetoothAction::Powered(path.into(), true).control();
            let visible = BluetoothAction::Discoverable(path.into(), true).control();
            let powered = match page.live.pending(&power) {
                Some(BluetoothAction::Powered(_, value)) => *value,
                _ => adapter.powered,
            };
            let discoverable = match page.live.pending(&visible) {
                Some(BluetoothAction::Discoverable(_, value)) => *value,
                _ => adapter.discoverable,
            };
            let id = key(path);
            let mut group = column![
                toggle(
                    format!("bluetooth-{id}-powered"),
                    "Bluetooth",
                    powered,
                    apply(BluetoothAction::Powered(path.into(), !powered)),
                ),
                page.control_status(&power, message),
            ]
            .spacing(8);
            if adapter.powered {
                group = group
                    .push(toggle(
                        format!("bluetooth-{id}-visible"),
                        fl!("settings-bluetooth-visible", name = adapter.label.as_str()),
                        discoverable,
                        apply(BluetoothAction::Discoverable(path.into(), !discoverable)),
                    ))
                    .push(page.control_status(&visible, message));
                if adapter.discovering {
                    group = group.push(muted(fl!("settings-bluetooth-searching")));
                }
            } else {
                group = group.push(muted(fl!("settings-bluetooth-off-hint")));
            }
            if !adapter.address.is_empty() {
                group = group.push(muted(fl!(
                    "settings-bluetooth-address",
                    address = adapter.address.as_str()
                )));
            }
            body = body.push(settings_group(
                if several {
                    adapter.label.as_str()
                } else {
                    "Bluetooth"
                }
                .to_owned(),
                group,
            ));
        }
        if !snapshot.adapters.iter().any(|adapter| adapter.powered) {
            return body.into();
        }
        let pairing = state.pairing.is_some();
        let mut paired = column![].spacing(12);
        let mut nearby = column![].spacing(12);
        let (mut any_paired, mut any_nearby) = (false, false);
        for device in &snapshot.devices {
            if device.paired {
                any_paired = true;
                paired = paired.push(self.paired_device(device));
            } else {
                any_nearby = true;
                nearby = nearby.push(self.nearby_device(device, pairing));
            }
        }
        body.push(settings_group(
            fl!("settings-bluetooth-mine"),
            if any_paired {
                paired.into()
            } else {
                muted(fl!("settings-bluetooth-none-paired"))
            },
        ))
        .push(settings_group(
            fl!("settings-bluetooth-nearby"),
            column![
                if any_nearby {
                    nearby.into()
                } else {
                    muted(fl!("settings-bluetooth-none-nearby"))
                },
                muted(fl!("settings-bluetooth-profiles-hint")),
            ]
            .spacing(10),
        ))
        .into()
    }

    pub(super) fn bluetooth_view(&self) -> Element<'_, Message> {
        self.bluetooth
            .view(message, |state| self.bluetooth_sections(state))
    }
}
