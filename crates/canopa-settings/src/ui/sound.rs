//! The Sound page: outputs and inputs with default, volume, mute and balance,
//! application streams, card profiles, a microphone level meter while the
//! page is shown, and the notification sound switch.
use super::{Message, State, adjust, live_page::LivePage, pick, settings_group, toggle};
use crate::{
    live::Event,
    sound::{SoundAction, SoundService},
};
use canopa_services::audio::{AudioDevice, AudioSnapshot, DeviceKind, StreamKind};
use canopa_ui::direction;
use iced::{
    Element, Length, Subscription,
    widget::{column, progress_bar, text},
};

pub(super) type SoundPage = LivePage<SoundService>;

pub(super) fn message(event: Event<SoundService>) -> Message {
    Message::Sound(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

/// Identifies the level meter subscription by server and input device.
#[derive(Clone, Hash)]
struct Meter(Option<String>, String);

fn meter(meter: &Meter) -> futures_util::stream::BoxStream<'static, Message> {
    use futures_util::StreamExt;
    let levels = canopa_services::audio::level_meter(meter.0.clone(), Some(meter.1.clone()));
    tokio_stream_from(levels).map(Message::Level).boxed()
}

fn tokio_stream_from(
    receiver: tokio::sync::watch::Receiver<Option<f32>>,
) -> impl futures_util::Stream<Item = Option<f32>> {
    futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.changed().await.ok()?;
        let value = *receiver.borrow_and_update();
        Some((value, receiver))
    })
}

impl State {
    /// The level meter runs only while the page is shown and has an input.
    pub(super) fn sound_subscription(&self) -> Subscription<Message> {
        let input = self.sound.live.state().and_then(|snapshot| {
            snapshot
                .devices
                .iter()
                .find(|device| device.id.kind == DeviceKind::Input && device.is_default)
        });
        let mut subscriptions = vec![self.sound.subscription(Message::Sound)];
        if let Some(input) = input {
            subscriptions.push(Subscription::run_with(
                Meter(self.sound.service.server(), input.id.name.clone()),
                meter,
            ));
        }
        Subscription::batch(subscriptions)
    }

    fn device_card<'a>(&'a self, device: &'a AudioDevice) -> Element<'a, Message> {
        let page = &self.sound;
        let id = &device.id;
        let volume = match page
            .live
            .pending(&SoundAction::device_control("volume", id))
        {
            Some(SoundAction::Volume(_, value)) => *value,
            _ => device.volume.round().min(100.) as u8,
        };
        let key = |kind: &str| SoundAction::device_control(kind, id);
        let mut card = column![
            direction::row([
                text(&device.description)
                    .width(Length::Fill)
                    .align_x(direction::text_start())
                    .into(),
                if device.is_default {
                    muted(fl!("settings-sound-in-use"))
                } else {
                    self.action(
                        "settings-button-use-this-device",
                        fl!("settings-sound-use-device"),
                        Some(message(Event::Apply(SoundAction::Default(id.clone())))),
                        false,
                    )
                },
            ])
            .spacing(12),
            text(fl!(
                "settings-sound-volume",
                volume = canopa_i18n::format::percent(f64::from(volume))
            )),
            adjust(
                format!("sound-{}", key("volume")),
                0.0..=100.,
                f64::from(volume),
                5.,
                {
                    let id = id.clone();
                    move |value| {
                        message(Event::Apply(SoundAction::Volume(
                            id.clone(),
                            value.round() as u8,
                        )))
                    }
                },
            ),
            page.control_status(&key("volume"), message),
            toggle(
                format!("sound-{}", key("mute")),
                fl!("settings-sound-mute"),
                device.muted,
                message(Event::Apply(SoundAction::Mute(id.clone(), !device.muted))),
            ),
            page.control_status(&key("mute"), message),
        ]
        .spacing(8);
        if let Some(balance) = device.balance.filter(|_| id.kind == DeviceKind::Output) {
            let balance = match page.live.pending(&key("balance")) {
                Some(SoundAction::Balance(_, value)) => *value,
                _ => balance,
            };
            card = card
                .push(text(match balance {
                    0 => fl!("settings-sound-balance-center"),
                    value if value < 0 => fl!(
                        "settings-sound-balance-left",
                        value = canopa_i18n::format::percent(-f64::from(value))
                    ),
                    value => fl!(
                        "settings-sound-balance-right",
                        value = canopa_i18n::format::percent(f64::from(value))
                    ),
                }))
                .push(adjust(
                    format!("sound-{}", key("balance")),
                    -100.0..=100.,
                    f64::from(balance),
                    10.,
                    {
                        let id = id.clone();
                        move |value| {
                            message(Event::Apply(SoundAction::Balance(
                                id.clone(),
                                value.round() as i8,
                            )))
                        }
                    },
                ))
                .push(page.control_status(&key("balance"), message));
        }
        if id.kind == DeviceKind::Input && device.is_default {
            card = card
                .push(text(fl!("settings-sound-input-level")))
                .push(progress_bar(0.0..=1.0, self.input_level.unwrap_or(0.0)).girth(8));
        }
        card = card
            .push(page.control_status(&format!("default-{:?}", id.kind).to_lowercase(), message));
        iced::widget::container(card)
            .padding(12)
            .width(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }

    fn sound_sections<'a>(&'a self, snapshot: &'a AudioSnapshot) -> Element<'a, Message> {
        let page = &self.sound;
        let devices = |kind: DeviceKind, empty: String| -> Element<'a, Message> {
            let mut list = column![].spacing(10);
            let mut any = false;
            for device in snapshot
                .devices
                .iter()
                .filter(|device| device.id.kind == kind)
            {
                any = true;
                list = list.push(self.device_card(device));
            }
            if any { list.into() } else { muted(empty) }
        };
        let mut streams = column![].spacing(10);
        for stream in &snapshot.streams {
            let id = &stream.id;
            let key = |kind: &str| SoundAction::stream_control(kind, id);
            let volume = match page.live.pending(&key("volume")) {
                Some(SoundAction::StreamVolume(_, value)) => *value,
                _ => stream.volume.round().min(100.) as u8,
            };
            let recording = match id.kind {
                StreamKind::Playback => "no",
                StreamKind::Recording => "yes",
            };
            let mut card = column![
                text(&stream.application),
                muted(if stream.title.is_empty() {
                    fl!("settings-sound-stream", recording = recording)
                } else {
                    fl!(
                        "settings-sound-stream-titled",
                        recording = recording,
                        title = stream.title.as_str()
                    )
                }),
            ]
            .spacing(6);
            if stream.volume_writable {
                card = card
                    .push(text(fl!(
                        "settings-sound-volume",
                        volume = canopa_i18n::format::percent(f64::from(volume))
                    )))
                    .push(adjust(
                        format!("sound-{}", key("volume")),
                        0.0..=100.,
                        f64::from(volume),
                        5.,
                        {
                            let id = id.clone();
                            move |value| {
                                message(Event::Apply(SoundAction::StreamVolume(
                                    id.clone(),
                                    value.round() as u8,
                                )))
                            }
                        },
                    ))
                    .push(page.control_status(&key("volume"), message));
            }
            card = card
                .push(toggle(
                    format!("sound-{}", key("mute")),
                    fl!("settings-sound-mute"),
                    stream.muted,
                    message(Event::Apply(SoundAction::StreamMute(
                        id.clone(),
                        !stream.muted,
                    ))),
                ))
                .push(page.control_status(&key("mute"), message));
            streams = streams.push(
                iced::widget::container(card)
                    .padding(12)
                    .width(Length::Fill)
                    .style(canopa_ui::surface),
            );
        }
        let streams: Element<'_, Message> = if snapshot.streams.is_empty() {
            muted(fl!("settings-sound-no-streams"))
        } else {
            streams.into()
        };
        let mut cards = column![].spacing(12);
        for card in &snapshot.cards {
            let available: Vec<_> = card
                .profiles
                .iter()
                .filter(|profile| profile.available || card.active.as_ref() == Some(&profile.name))
                .collect();
            let control = format!("profile-{}", card.name);
            let selected = match page.live.pending(&control) {
                Some(SoundAction::Profile(_, _, profile)) => Some(profile.clone()),
                _ => card.active.clone(),
            };
            let (index, name) = (card.index, card.name.clone());
            // The audio server's own profile names and descriptions.
            let profiles: Vec<(String, String)> = available
                .iter()
                .map(|p| (p.name.clone(), p.description.clone()))
                .collect();
            cards = cards.push(
                column![
                    text(&card.description),
                    pick(
                        format!("sound-{control}"),
                        profiles,
                        selected,
                        move |profile| {
                            message(Event::Apply(SoundAction::Profile(
                                index,
                                name.clone(),
                                profile,
                            )))
                        }
                    ),
                    page.control_status(&control, message),
                ]
                .spacing(6),
            );
        }
        let cards: Element<'_, Message> = if snapshot.cards.is_empty() {
            muted(fl!("settings-sound-no-cards"))
        } else {
            column![muted(fl!("settings-sound-profiles-hint")), cards,]
                .spacing(10)
                .into()
        };
        column![
            settings_group(
                fl!("settings-sound-output"),
                devices(DeviceKind::Output, fl!("settings-sound-no-outputs"))
            ),
            settings_group(
                fl!("settings-sound-input"),
                devices(DeviceKind::Input, fl!("settings-sound-no-inputs"))
            ),
            settings_group(fl!("settings-sound-applications"), streams),
            settings_group(fl!("settings-sound-profiles"), cards),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn sound_view(&self) -> Element<'_, Message> {
        let alerts: Element<'_, Message> = match self.alert_sounds {
            Some(enabled) => toggle(
                "sound-alerts",
                fl!("settings-sound-alerts-toggle"),
                enabled,
                Message::AlertSounds(!enabled),
            ),
            None => muted(fl!("settings-sound-alerts-unreadable")),
        };
        column![
            self.sound
                .view(message, |snapshot| self.sound_sections(snapshot)),
            settings_group(
                fl!("settings-sound-alerts"),
                column![alerts, muted(fl!("settings-sound-alerts-hint")),].spacing(8),
            ),
        ]
        .spacing(14)
        .into()
    }
}
