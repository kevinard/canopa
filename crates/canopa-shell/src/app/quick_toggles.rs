//! Quick settings radios: Wi-Fi and Bluetooth switches beside the rows that
//! open their providers. NetworkManager and BlueZ own the state and policy;
//! the Shell asks them to change it and shows what they report.
use canopa_services::{ServiceState, backlight, bluetooth};
use iced::Task;

use super::{App, Message};

#[derive(Debug, Clone)]
pub enum QuickToggle {
    Wireless(bool),
    Bluetooth(bool),
}

#[derive(Debug, Clone)]
pub enum Event {
    Toggle(QuickToggle),
    Bluetooth(Box<ServiceState<bluetooth::Snapshot>>),
    /// The default output, named on the Sound card.
    AudioDefaults(Box<ServiceState<canopa_services::audio::Defaults>>),
    /// The built-in display, polled while Power & display is linked.
    Backlight(Box<ServiceState<backlight::Backlight>>),
    /// The Quick settings brightness slider, in perceptual percent.
    SetBrightness(u8),
}

fn set_brightness(percent: u8) -> Message {
    Message::QuickToggles(Event::SetBrightness(percent))
}

/// The last BlueZ snapshot, kept to address its adapters.
#[derive(Default)]
pub(super) struct QuickToggles {
    bluetooth: Option<bluetooth::Snapshot>,
    backlight: Option<backlight::Backlight>,
}

struct BluetoothWatch(zbus::Connection);

impl std::hash::Hash for BluetoothWatch {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "quick-settings-bluetooth".hash(h);
        self.0.unique_name().hash(h);
        self.0.server_guid().hash(h);
    }
}

#[derive(Hash)]
struct AudioDefaultsWatch;

#[derive(Hash)]
struct BacklightWatch;

impl App {
    /// The default output's name, while a volume module can show it.
    fn audio_defaults_subscription(&self) -> iced::Subscription<Message> {
        if !self.has_module_kind(crate::modules::ModuleKind::Volume) {
            return iced::Subscription::none();
        }
        iced::Subscription::run_with(AudioDefaultsWatch, |_| {
            let receiver = canopa_services::audio::watch_defaults(None);
            iced::futures::stream::unfold(Some(receiver), |receiver| async move {
                let mut receiver = receiver?;
                receiver.changed().await.ok()?;
                let state = receiver.borrow_and_update().clone();
                Some((
                    Message::QuickToggles(Event::AudioDefaults(Box::new(state))),
                    Some(receiver),
                ))
            })
        })
    }

    /// The backlight is polled only while some module links to Power & display.
    fn backlight_subscription(&self) -> iced::Subscription<Message> {
        use iced::futures::StreamExt;
        if !self.modules.opens_provider("power") {
            return iced::Subscription::none();
        }
        iced::Subscription::run_with(BacklightWatch, |_| {
            backlight::watch(std::time::Duration::from_secs(2))
                .map(|state| Message::QuickToggles(Event::Backlight(Box::new(state))))
        })
    }

    /// BlueZ is watched only while some module opens the Bluetooth provider.
    pub(super) fn quick_toggle_subscription(&self) -> iced::Subscription<Message> {
        use iced::futures::StreamExt;
        let audio = iced::Subscription::batch([
            self.audio_defaults_subscription(),
            self.backlight_subscription(),
        ]);
        let Some(connection) = self.shared.dbus.system.clone() else {
            return audio;
        };
        if !self.modules.opens_provider("bluetooth") {
            return audio;
        }
        iced::Subscription::batch([
            audio,
            iced::Subscription::run_with(BluetoothWatch(connection), |watch| {
                bluetooth::watch(watch.0.clone())
                    .map(|state| Message::QuickToggles(Event::Bluetooth(Box::new(state))))
            }),
        ])
    }

    fn publish_backlight(&mut self) {
        let status = self.quick_toggles.backlight.as_ref().map(|light| {
            super::module_runtime::ProviderStatus {
                summary: fl!("shell-provider-power-description"),
                switch: None,
                level: Some((light.perceptual().clamp(1, 100) as u8, set_brightness)),
            }
        });
        self.modules.set_provider_status("power", status);
    }

    pub(super) fn handle_quick_toggle_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Backlight(state) => {
                self.quick_toggles.backlight = match *state {
                    ServiceState::Available(light) => Some(light),
                    ServiceState::Unavailable => None,
                };
                self.publish_backlight();
                Task::none()
            }
            Event::SetBrightness(percent) => {
                let (Some(light), Some(connection)) = (
                    self.quick_toggles.backlight.clone(),
                    self.shared.dbus.system.clone(),
                ) else {
                    return Task::none();
                };
                let percent = u16::from(percent.clamp(1, 100));
                // Show the new level now; the next poll confirms it.
                if let Some(shown) = &mut self.quick_toggles.backlight {
                    shown.value = shown.perceptual_level(percent);
                }
                self.publish_backlight();
                tokio::spawn(async move {
                    if let Err(error) = light.set_perceptual(&connection, percent).await {
                        tracing::warn!(target: "canopa_shell::modules", %error, "brightness change refused");
                    }
                });
                Task::none()
            }
            Event::AudioDefaults(state) => {
                let output = match *state {
                    ServiceState::Available(defaults) => defaults.output,
                    ServiceState::Unavailable => None,
                };
                self.modules.set_provider_status(
                    "audio",
                    output
                        .filter(|output| !output.description.trim().is_empty())
                        .map(|output| super::module_runtime::ProviderStatus {
                            summary: output.description,
                            switch: None,
                            level: None,
                        }),
                );
                Task::none()
            }
            Event::Bluetooth(state) => {
                let snapshot = match *state {
                    ServiceState::Available(snapshot) if snapshot.owner.is_some() => Some(snapshot),
                    _ => None,
                };
                self.modules
                    .set_provider_status("bluetooth", snapshot.as_ref().and_then(bluetooth_status));
                self.quick_toggles.bluetooth = snapshot;
                Task::none()
            }
            Event::Toggle(toggle) => {
                let Some(connection) = self.shared.dbus.system.clone() else {
                    return Task::none();
                };
                match toggle {
                    QuickToggle::Wireless(on) => {
                        tokio::spawn(async move {
                            if let Err(error) =
                                canopa_services::network::manager::set_wireless_enabled(
                                    &connection,
                                    on,
                                )
                                .await
                            {
                                tracing::warn!(target: "canopa_shell::modules", %error, "Wi-Fi switch refused");
                            }
                        });
                    }
                    QuickToggle::Bluetooth(on) => {
                        let Some(snapshot) = self.quick_toggles.bluetooth.clone() else {
                            return Task::none();
                        };
                        let Some(owner) = snapshot.owner.clone() else {
                            return Task::none();
                        };
                        tokio::spawn(async move {
                            for adapter in &snapshot.adapters {
                                if let Err(error) = bluetooth::set_powered(
                                    &connection,
                                    &owner,
                                    adapter.path.as_str(),
                                    on,
                                )
                                .await
                                {
                                    tracing::warn!(target: "canopa_shell::modules", %error, "Bluetooth switch refused");
                                }
                            }
                        });
                    }
                }
                Task::none()
            }
        }
    }
}

/// "Off", "On", or the connected device: "Headphones" or "2 devices".
fn bluetooth_status(
    snapshot: &bluetooth::Snapshot,
) -> Option<super::module_runtime::ProviderStatus> {
    if snapshot.adapters.is_empty() {
        return None;
    }
    let on = snapshot.adapters.iter().any(|adapter| adapter.powered);
    let connected: Vec<_> = snapshot
        .devices
        .iter()
        .filter(|device| device.connected)
        .collect();
    let summary = match (on, connected.as_slice()) {
        (false, _) => fl!("shell-switch-off"),
        (true, []) => fl!("shell-bluetooth-on"),
        (true, [device]) => device.label.clone(),
        (true, devices) => fl!("shell-bluetooth-connected-count", count = devices.len()),
    };
    Some(super::module_runtime::ProviderStatus {
        summary,
        switch: Some((
            on,
            Message::QuickToggles(Event::Toggle(QuickToggle::Bluetooth(!on))),
        )),
        level: None,
    })
}
