//! The Sound page: outputs and inputs with default, volume, mute and balance,
//! application streams, card profiles, a microphone level meter while the
//! page is shown, and the notification sound switch.
use super::{Message, State, adjust, kit, live_page::LivePage, pick, settings_group, toggle};
use crate::{
    live::Event,
    sound::{SoundAction, SoundService},
};
use canopa_services::audio::{AudioDevice, AudioSnapshot, DeviceKind, StreamKind};
use canopa_ui::accessibility::{Node, semantic};
use canopa_ui::direction;
use iced::{
    Element, Length, Subscription,
    widget::{column, text},
};

pub(super) type SoundPage = LivePage<SoundService>;

pub(super) fn message(event: Event<SoundService>) -> Message {
    Message::Sound(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(13).style(canopa_ui::muted).into()
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
        // One row each: the device, its volume, mute, balance and level.
        let mut rows: Vec<Element<'a, Message>> = vec![
            direction::row([
                text(device.description.clone())
                    .font(kit::strong_font(iced::font::Weight::Medium))
                    .width(Length::Fill)
                    .align_x(direction::text_start())
                    .into(),
                if device.is_default {
                    kit::badge(fl!("settings-sound-in-use"))
                } else {
                    self.action(
                        "settings-button-use-this-device",
                        fl!("settings-sound-use-device"),
                        Some(message(Event::Apply(SoundAction::Default(id.clone())))),
                        false,
                    )
                },
            ])
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into(),
            column![
                kit::slider_row(
                    fl!("settings-sound-volume-label"),
                    canopa_i18n::format::percent(f64::from(volume)),
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
                ),
                page.control_status(&key("volume"), message),
            ]
            .into(),
            column![
                toggle(
                    format!("sound-{}", key("mute")),
                    fl!("settings-sound-mute"),
                    device.muted,
                    message(Event::Apply(SoundAction::Mute(id.clone(), !device.muted))),
                ),
                page.control_status(&key("mute"), message),
            ]
            .into(),
        ];
        if let Some(balance) = device.balance.filter(|_| id.kind == DeviceKind::Output) {
            let balance = match page.live.pending(&key("balance")) {
                Some(SoundAction::Balance(_, value)) => *value,
                _ => balance,
            };
            rows.push(
                column![
                    kit::slider_row(
                        fl!("settings-sound-balance-label"),
                        match balance {
                            0 => fl!("settings-sound-balance-centered"),
                            value if value < 0 => fl!(
                                "settings-sound-balance-left-value",
                                value = canopa_i18n::format::percent(-f64::from(value))
                            ),
                            value => fl!(
                                "settings-sound-balance-right-value",
                                value = canopa_i18n::format::percent(f64::from(value))
                            ),
                        },
                        super::centered_adjust(
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
                        ),
                    ),
                    page.control_status(&key("balance"), message),
                ]
                .into(),
            );
        }
        if id.kind == DeviceKind::Input && device.is_default {
            let level = self.input_level.unwrap_or(0.0);
            rows.push(kit::slider_row(
                fl!("settings-sound-input-level"),
                canopa_i18n::format::percent(f64::from(level) * 100.),
                semantic(
                    Node::progress(fl!("settings-sound-input-level"), Some(f64::from(level))),
                    meter_bars(level),
                ),
            ));
        }
        column![
            kit::rows(rows),
            page.control_status(&format!("default-{:?}", id.kind).to_lowercase(), message)
        ]
        .into()
    }

    fn sound_sections<'a>(&'a self, snapshot: &'a AudioSnapshot) -> Element<'a, Message> {
        let page = &self.sound;
        // A card per device under one heading, each divided into rows.
        let devices = |kind: DeviceKind, label: String, empty: String| -> Element<'a, Message> {
            let mut cards = snapshot
                .devices
                .iter()
                .filter(|device| device.id.kind == kind)
                .map(|device| self.device_card(device));
            let Some(first) = cards.next() else {
                return settings_group(label, kit::empty(empty));
            };
            cards
                .fold(column![settings_group(label, first)], |section, card| {
                    section.push(
                        iced::widget::container(card)
                            .padding(kit::ROW_PADDING)
                            .width(Length::Fill)
                            .style(kit::card),
                    )
                })
                .spacing(10)
                .into()
        };
        let mut streams: Vec<Element<'_, Message>> = Vec::new();
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
                text(stream.application.clone()),
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
                    .push(kit::slider_row(
                        fl!("settings-sound-volume-label"),
                        canopa_i18n::format::percent(f64::from(volume)),
                        adjust(
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
                        ),
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
            streams.push(card.spacing(8).into());
        }
        let streams: Element<'_, Message> = if snapshot.streams.is_empty() {
            super::kit::empty(fl!("settings-sound-no-streams"))
        } else {
            kit::rows(streams)
        };
        let mut cards: Vec<Element<'_, Message>> = Vec::new();
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
            cards.push(
                column![
                    kit::row(
                        card.description.clone(),
                        None,
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
                    ),
                    page.control_status(&control, message),
                ]
                .spacing(4)
                .into(),
            );
        }
        let cards: Element<'_, Message> = if snapshot.cards.is_empty() {
            super::kit::empty(fl!("settings-sound-no-cards"))
        } else {
            kit::rows(cards)
        };
        column![
            devices(
                DeviceKind::Output,
                fl!("settings-sound-output"),
                fl!("settings-sound-no-outputs")
            ),
            devices(
                DeviceKind::Input,
                fl!("settings-sound-input"),
                fl!("settings-sound-no-inputs")
            ),
            settings_group(fl!("settings-sound-applications"), streams),
            super::noted_group(
                fl!("settings-sound-profiles"),
                cards,
                [fl!("settings-sound-profiles-hint")],
            ),
        ]
        .spacing(28)
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
            super::noted_group(
                fl!("settings-sound-alerts"),
                alerts,
                [fl!("settings-sound-alerts-hint")],
            ),
        ]
        .spacing(28)
        .into()
    }
}

/// The microphone level as a row of lit segments, which reads as a meter
/// rather than a control.
fn meter_bars<'a>(level: f32) -> Element<'a, Message> {
    const SEGMENTS: usize = 24;
    let lit = (level.clamp(0., 1.) * SEGMENTS as f32).round() as usize;
    direction::row((0..SEGMENTS).map(|index| {
        iced::widget::container(iced::widget::space())
            .width(Length::Fill)
            .height(Length::Fixed(10.))
            .style(move |theme: &iced::Theme| {
                let p = canopa_ui::colors(theme);
                iced::widget::container::Style {
                    background: Some(
                        if index < lit {
                            p.accent
                        } else {
                            kit::blend(p.surface, p.text, 0.12)
                        }
                        .into(),
                    ),
                    border: iced::border::rounded(2),
                    ..Default::default()
                }
            })
            .into()
    }))
    .spacing(3)
    .into()
}
