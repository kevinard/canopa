//! The on-screen display (OSD) for device keys and outside level changes.
//!
//! A small overlay-layer surface on the focused output shows one indicator
//! (Shell ADR 0025). It never takes keyboard focus, passes pointer input
//! through, follows the theme and fades out only when motion is allowed.
//! [`canopa_shell::osd`] owns its lifetime; this module owns the surface, the
//! key commands, the service observers and the screen-reader announcement.
use std::{path::Path, sync::Arc, time::Instant};

use canopa_services::{
    ServiceState,
    audio::Defaults,
    backlight::{self, Backlight, KeyboardBacklight},
};
use canopa_shell::{
    device_keys::{self, Channel, Indicator, Keys, Outcome, Request},
    osd::{Next, Osd, Timing},
};
use iced::{
    Alignment, Element, Length, Task,
    futures::{StreamExt, stream},
    widget::{Space, column, container, progress_bar},
    window::Id,
};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

#[cfg(not(test))]
use super::announcer::Announcer;
use super::{App, Message, messages::IpcResponder};

const WIDTH: u32 = 340;
const HEIGHT: u32 = 76;
/// Above the bottom edge, clear of a bottom bar's own reservation.
const BOTTOM_MARGIN: i32 = 72;
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

#[derive(Debug, Clone)]
pub enum Event {
    Key {
        request: Request,
        responder: Option<IpcResponder>,
    },
    Finished {
        outcome: Outcome,
        responder: Option<IpcResponder>,
    },
    Audio(ServiceState<Defaults>),
    Observed(Channel, Option<Indicator>),
    Flush,
    Expire(u64),
    Frame,
}

struct Window {
    id: Id,
    output: Option<String>,
}

pub(super) struct OsdRuntime {
    state: Osd,
    keys: Arc<Keys>,
    window: Option<Window>,
    #[cfg(not(test))]
    announcer: Option<Announcer>,
}

impl Default for OsdRuntime {
    fn default() -> Self {
        Self {
            state: Osd::default(),
            keys: Arc::new(Keys::default()),
            window: None,
            #[cfg(not(test))]
            announcer: None,
        }
    }
}

fn settings(output: Option<&str>) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((WIDTH, HEIGHT)),
        layer: Layer::Overlay,
        anchor: Anchor::Bottom,
        // Respect bars' reservations, so it never covers a bottom bar.
        exclusive_zone: Some(0),
        margin: Some((0, 0, BOTTOM_MARGIN, 0)),
        keyboard_interactivity: KeyboardInteractivity::None,
        events_transparent: true,
        output_option: output.map_or(OutputOption::Active, |output| {
            OutputOption::OutputName(output.into())
        }),
        namespace: Some("canopa-shell-osd".into()),
    }
}

fn after(at: Instant, message: Message) -> Task<Message> {
    // The timer is created when the task runs, on the Shell's runtime.
    let at = tokio::time::Instant::from_std(at);
    Task::perform(
        async move { tokio::time::sleep_until(at).await },
        move |()| message,
    )
}

impl OsdRuntime {
    pub(super) fn contains(&self, window: Id) -> bool {
        self.window.as_ref().is_some_and(|w| w.id == window)
    }

    /// The surface closed underneath us, for example with its output.
    pub(super) fn closed(&mut self, window: Id) -> bool {
        if !self.contains(window) {
            return false;
        }
        self.window = None;
        self.state.hidden();
        true
    }

    pub(super) fn view(&self, window: Id) -> Option<Element<'_, Message>> {
        if !self.contains(window) {
            return None;
        }
        let Some(visible) = self.state.visible() else {
            return Some(Space::new().width(Length::Fill).height(Length::Fill).into());
        };
        Some(indicator_view(
            &visible.indicator,
            self.state.opacity(Instant::now()),
        ))
    }

    fn timing(config: &crate::config::OsdConfig, reduced_motion: bool) -> Timing {
        Timing {
            timeout: std::time::Duration::from_millis(config.timeout_ms),
            fade: (!reduced_motion)
                .then_some(Timing::default().fade)
                .flatten(),
            ..Timing::default()
        }
    }

    /// Readings from the services, while the display is enabled: the default
    /// audio devices (event-driven) and the kernel's brightness notifications.
    pub(super) fn subscription(enabled: bool) -> iced::Subscription<Message> {
        if !enabled {
            return iced::Subscription::none();
        }
        iced::Subscription::batch([
            iced::Subscription::run(audio_readings),
            iced::Subscription::run(light_readings),
        ])
    }
}

fn audio_readings() -> impl iced::futures::Stream<Item = Message> {
    let receiver = canopa_services::audio::watch_defaults(None);
    stream::unfold((receiver, true), |(mut receiver, first)| async move {
        if !first {
            receiver.changed().await.ok()?;
        }
        let state = receiver.borrow_and_update().clone();
        Some((Message::Osd(Event::Audio(state)), (receiver, false)))
    })
}

fn light_reading(channel: Channel) -> Option<Indicator> {
    match channel {
        Channel::Brightness => Backlight::discover(Path::new(backlight::ROOT))
            .as_ref()
            .map(device_keys::display_indicator),
        _ => KeyboardBacklight::discover_all(Path::new(backlight::LEDS))
            .first()
            .map(device_keys::keyboard_indicator),
    }
}

fn light_readings() -> impl iced::futures::Stream<Item = Message> {
    let (backlights, leds) = (Path::new(backlight::ROOT), Path::new(backlight::LEDS));
    let display = Backlight::discover(backlights);
    let keyboards = KeyboardBacklight::discover_all(leds);
    let mut files = Vec::new();
    files.extend(display.iter().map(|light| light.notifications(backlights)));
    files.extend(keyboards.iter().map(|light| light.notifications(leds)));
    // The current levels are the baselines that later notifications compare to.
    let initial = [
        Message::Osd(Event::Observed(
            Channel::Brightness,
            display.as_ref().map(device_keys::display_indicator),
        )),
        Message::Osd(Event::Observed(
            Channel::KeyboardBacklight,
            keyboards.first().map(device_keys::keyboard_indicator),
        )),
    ];
    // Register the kernel notifications on the runtime that polls the stream.
    let changes = stream::once(async move { backlight::hardware_changes(files) }).flatten();
    stream::iter(initial).chain(changes.map(move |path| {
        let channel = if path.starts_with(backlight::ROOT) {
            Channel::Brightness
        } else {
            Channel::KeyboardBacklight
        };
        Message::Osd(Event::Observed(channel, light_reading(channel)))
    }))
}

fn icon(indicator: &Indicator) -> &'static [u8] {
    let muted = indicator.muted();
    match indicator.channel() {
        Channel::Volume if muted => include_bytes!("../../assets/sound-muted.svg"),
        Channel::Volume => include_bytes!("../../assets/sound.svg"),
        Channel::Microphone if muted => include_bytes!("../../assets/microphone-muted.svg"),
        Channel::Microphone => include_bytes!("../../assets/microphone.svg"),
        Channel::Brightness => include_bytes!("../../assets/brightness.svg"),
        Channel::KeyboardBacklight => include_bytes!("../../assets/keyboard-backlight.svg"),
    }
}

fn faded(mut color: iced::Color, opacity: f32) -> iced::Color {
    color.a *= opacity;
    color
}

fn indicator_view(indicator: &Indicator, opacity: f32) -> Element<'static, Message> {
    let appearance = crate::theme::appearance();
    let ui = appearance.style(canopa_theme::UiKind::Notification);
    let palette = crate::theme::palette();
    let muted = indicator.muted() || matches!(indicator, Indicator::Unavailable(_));
    let foreground = faded(if muted { palette.muted } else { palette.text }, opacity);
    let title = match indicator {
        Indicator::Level {
            device: Some(device),
            ..
        } => device.clone(),
        _ => indicator.channel().title(),
    };
    let bar: Element<'static, Message> = match indicator.fill() {
        Some(fill) => {
            let (track, bar) = (
                faded(palette.raised, opacity),
                faded(if muted { palette.muted } else { palette.accent }, opacity),
            );
            progress_bar(0.0..=1.0, fill)
                .girth(6.0)
                .style(move |_| progress_bar::Style {
                    background: track.into(),
                    bar: bar.into(),
                    border: iced::Border::default().rounded(3),
                })
                .into()
        }
        None => Space::new().height(Length::Fixed(6.0)).into(),
    };
    let label = column![
        crate::theme::text(title)
            .size(crate::theme::label_size())
            .color(foreground)
            .width(Length::Fill)
            .align_x(canopa_ui::direction::text_start())
            .wrapping(iced::widget::text::Wrapping::None),
        bar,
    ]
    .spacing(8)
    .width(Length::Fill);
    // Icon, title and value in reading order (mirrored for RTL).
    let content = canopa_ui::direction::row([
        crate::theme::icon(icon(indicator), 24.0, foreground),
        label.into(),
        crate::theme::strong_text(indicator.value())
            .color(foreground)
            .width(Length::Fixed(88.0))
            .align_x(canopa_ui::direction::text_end())
            .into(),
    ])
    .spacing(14)
    .align_y(Alignment::Center);
    let mut style = canopa_ui::frame(&appearance, ui);
    style.background = style.background.map(|background| match background {
        iced::Background::Color(color) => faded(color, opacity).into(),
        other => other,
    });
    style.border.color = faded(style.border.color, opacity);
    style.text_color = Some(foreground);
    container(content)
        .padding([14, 18])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| style)
        .into()
}

impl App {
    fn osd_steps(&self) -> device_keys::Steps {
        self.config.osd.steps()
    }

    pub(super) fn handle_osd_event(&mut self, event: Event) -> Task<Message> {
        self.osd.state.timing =
            OsdRuntime::timing(&self.config.osd, self.config.appearance.reduced_motion());
        let now = Instant::now();
        let next = match event {
            Event::Key { request, responder } => {
                let (keys, steps) = (self.osd.keys.clone(), self.osd_steps());
                return Task::perform(
                    async move { keys.run(request, steps).await },
                    move |outcome| Message::Osd(Event::Finished { outcome, responder }),
                );
            }
            Event::Finished { outcome, responder } => {
                if let Some(responder) = responder {
                    super::app_effects::send_ipc_response(
                        responder,
                        if outcome.succeeded() {
                            crate::ipc::protocol::IpcResponse::Accepted
                        } else {
                            crate::ipc::protocol::IpcResponse::Rejected {
                                reason: "unavailable".into(),
                            }
                        },
                    );
                }
                match outcome.indicator() {
                    Some(indicator) if self.config.osd.enabled => {
                        self.osd.state.pressed(indicator, now)
                    }
                    _ => Next::Nothing,
                }
            }
            Event::Audio(state) => {
                let defaults = state.available();
                // No output is like no server: the next device is a baseline.
                let volume = defaults.and_then(|d| {
                    d.output
                        .as_ref()
                        .map(|device| device_keys::audio_indicator(Channel::Volume, device))
                });
                let microphone = defaults.and_then(|d| {
                    d.input
                        .as_ref()
                        .map(|device| device_keys::audio_indicator(Channel::Microphone, device))
                });
                let first = self.osd.state.observed(Channel::Volume, volume, now);
                let second = self
                    .osd
                    .state
                    .observed(Channel::Microphone, microphone, now);
                return Task::batch([self.apply_osd(first), self.apply_osd(second)]);
            }
            Event::Observed(channel, reading) => self.osd.state.observed(channel, reading, now),
            Event::Flush => self.osd.state.flush(now),
            Event::Expire(generation) => self.osd.state.expire(generation, now),
            Event::Frame => self.osd.state.fade(now),
        };
        self.apply_osd(next)
    }

    fn apply_osd(&mut self, next: Next) -> Task<Message> {
        match next {
            Next::Nothing => Task::none(),
            Next::Flush(at) => after(at, Message::Osd(Event::Flush)),
            Next::Fade => after(Instant::now() + FRAME, Message::Osd(Event::Frame)),
            Next::Hide => self.osd.window.take().map_or_else(Task::none, |window| {
                Task::done(Message::RemoveWindow(window.id))
            }),
            Next::Show {
                generation,
                hide_at,
            } => {
                #[cfg(not(test))]
                if let Some(visible) = self.osd.state.visible() {
                    self.osd
                        .announcer
                        .get_or_insert_with(|| Announcer::new(fl!("shell-osd-name")))
                        .announce(&visible.indicator.announcement());
                }
                let output = self
                    .niri
                    .state()
                    .connected
                    .then(|| self.niri.state().focused_output().map(str::to_owned))
                    .flatten();
                let mut tasks = vec![after(hide_at, Message::Osd(Event::Expire(generation)))];
                let current = self.osd.window.as_ref().map(|window| &window.output);
                if current != Some(&output) {
                    if let Some(old) = self.osd.window.take() {
                        tasks.push(Task::done(Message::RemoveWindow(old.id)));
                    }
                    let (id, open) = Message::layershell_open(settings(output.as_deref()));
                    self.osd.window = Some(Window { id, output });
                    tasks.push(open);
                }
                Task::batch(tasks)
            }
        }
    }

    pub(super) fn handle_device_key(
        &mut self,
        request: Request,
        responder: Option<IpcResponder>,
    ) -> Task<Message> {
        self.handle_osd_event(Event::Key { request, responder })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(percent: u16) -> Indicator {
        Indicator::Level {
            channel: Channel::Volume,
            percent,
            muted: false,
            device: Some("Speakers".into()),
        }
    }

    #[test]
    fn surface_is_a_click_through_overlay_without_keyboard_focus() {
        let on = settings(Some("eDP-1"));
        assert_eq!(on.layer, Layer::Overlay);
        assert_eq!(on.keyboard_interactivity, KeyboardInteractivity::None);
        assert!(on.events_transparent);
        assert_eq!(on.output_option, OutputOption::OutputName("eDP-1".into()));
        assert_eq!(settings(None).output_option, OutputOption::Active);
        assert_eq!(on.exclusive_zone, Some(0));
    }

    #[test]
    fn presses_open_one_surface_replace_its_content_and_close_it() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let finished = |outcome| {
            Message::Osd(Event::Finished {
                outcome,
                responder: None,
            })
        };
        let _ = app.update(finished(Outcome::Changed(volume(40))));
        let first = app.osd.window.as_ref().unwrap().id;
        assert!(app.osd.view(first).is_some());
        let _ = app.update(finished(Outcome::Changed(volume(45))));
        assert_eq!(
            app.osd.window.as_ref().unwrap().id,
            first,
            "replaced, not stacked"
        );
        assert_eq!(app.osd.state.visible().unwrap().indicator, volume(45));
        let _ = app.update(finished(Outcome::Unavailable(Channel::Brightness)));
        assert_eq!(
            app.osd.state.visible().unwrap().indicator,
            Indicator::Unavailable(Channel::Brightness)
        );
        // Media keys show nothing.
        let generation = app.osd.state.visible().unwrap().generation;
        let _ = app.update(finished(Outcome::Played));
        assert_eq!(app.osd.state.visible().unwrap().generation, generation);
        // The surface closing underneath forgets the indicator.
        let _ = app.update(Message::Surface(
            super::super::messages::SurfaceEvent::WindowClosed(first),
        ));
        assert!(app.osd.window.is_none() && app.osd.state.visible().is_none());
    }

    #[test]
    fn a_disabled_display_shows_nothing_but_still_answers_the_key() {
        let mut config = crate::config::Config::default();
        config.osd.enabled = false;
        let (mut app, _) = App::boot(config, vec![]);
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        let _ = app.update(Message::Osd(Event::Finished {
            outcome: Outcome::Changed(volume(40)),
            responder: Some(super::super::messages::ipc_responder(sender)),
        }));
        assert!(app.osd.window.is_none());
        assert_eq!(
            receiver.try_recv().unwrap(),
            crate::ipc::protocol::IpcResponse::Accepted
        );
    }

    #[test]
    fn startup_readings_never_show_and_outside_changes_do() {
        let (mut app, _) = App::boot(crate::config::Config::default(), vec![]);
        let defaults = |percent| {
            ServiceState::Available(Defaults {
                output: Some(canopa_services::audio::DefaultDevice {
                    kind: canopa_services::audio::DeviceKind::Output,
                    name: "speakers".into(),
                    description: "Speakers".into(),
                    percent,
                    muted: false,
                }),
                input: None,
            })
        };
        let _ = app.update(Message::Osd(Event::Audio(defaults(40))));
        let _ = app.update(Message::Osd(Event::Observed(
            Channel::Brightness,
            Some(Indicator::Level {
                channel: Channel::Brightness,
                percent: 50,
                muted: false,
                device: None,
            }),
        )));
        assert!(app.osd.window.is_none(), "nothing at startup");
        let _ = app.update(Message::Osd(Event::Audio(defaults(60))));
        assert!(app.osd.window.is_none(), "settling first");
        std::thread::sleep(app.osd.state.timing.max_settle);
        let _ = app.update(Message::Osd(Event::Flush));
        assert_eq!(app.osd.state.visible().unwrap().indicator, volume(60));
        assert!(app.osd.window.is_some());
    }

    #[test]
    fn reduced_motion_hides_without_fading() {
        let mut config = crate::config::Config::default();
        config.osd.timeout_ms = 500;
        let reduced = OsdRuntime::timing(&config.osd, true);
        assert_eq!(reduced.fade, None);
        assert_eq!(reduced.timeout, std::time::Duration::from_millis(500));
        assert!(OsdRuntime::timing(&config.osd, false).fade.is_some());
    }
}
