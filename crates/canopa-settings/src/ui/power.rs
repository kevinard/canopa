//! The Power page: live battery, power mode and brightness sections, then the
//! saved lid choice and idle timeouts, and the power button (applied at once).
use super::{Message, State, adjust, choice, live_page::LivePage, settings_group};
use crate::{
    live::Event,
    power::{DURATIONS, PowerAction, PowerService, PowerState, duration_label, lid_label},
    preferences::IdleDraft,
};
use canopa_services::{power_switches::LidAction, upower::PowerState as Charge};
use iced::{
    Alignment, Element,
    widget::{column, row, text},
};

pub(super) type PowerPage = LivePage<PowerService>;

pub(super) fn message(event: Event<PowerService>) -> Message {
    Message::Power(event)
}

/// The power-button choices: Niri suspends by default; the system setting
/// leaves the button to logind.
const POWER_KEY: [(&str, bool); 2] = [("Suspend", false), ("Use the system setting", true)];

fn charge(state: Charge) -> &'static str {
    match state {
        Charge::Charging => "Charging",
        Charge::Discharging => "On battery",
        Charge::Empty => "Empty",
        Charge::FullyCharged => "Fully charged",
        Charge::PendingCharge => "Plugged in, not charging",
        Charge::PendingDischarge => "Waiting to discharge",
        Charge::Unknown => "Charge state unknown",
    }
}

fn approximately(seconds: u64) -> String {
    // Estimates move constantly; minutes are as precise as they get.
    let minutes = (seconds + 30) / 60;
    duration_label((minutes.max(1) * 60).min(u64::from(u32::MAX)) as u32)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

impl State {
    /// Whether the idle draft differs from the saved timeouts.
    pub(super) fn idle_dirty(&self) -> bool {
        use crate::file_page::FileModel;
        self.idle
            .source()
            .and_then(|source| IdleDraft::read(source).ok())
            .is_none_or(|saved| saved != self.idle.value)
    }

    fn battery_section(state: &PowerState) -> Element<'_, Message> {
        let Some(supply) = &state.supply else {
            return settings_group(
                "Battery",
                muted("Battery information is not available because UPower is not running."),
            );
        };
        if supply.batteries.is_empty() {
            return settings_group("Battery", muted("This computer has no battery."));
        }
        let mut body = column![].spacing(10);
        for battery in &supply.batteries {
            let mut status = format!("{:.0}% · {}", battery.percentage, charge(battery.state));
            if let Some(seconds) = battery.time_to_empty.filter(|_| supply.on_battery) {
                status.push_str(&format!(" · about {} left", approximately(seconds)));
            } else if let Some(seconds) = battery.time_to_full {
                status.push_str(&format!(" · full in about {}", approximately(seconds)));
            }
            let name = [battery.vendor.as_str(), battery.model.as_str()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let mut card = column![text(status)].spacing(4);
            if !name.is_empty() {
                card = card.push(muted(name));
            }
            let health = match (
                battery.capacity,
                battery.energy_full,
                battery.energy_full_design,
            ) {
                (Some(capacity), Some(full), Some(design)) => Some(format!(
                    "Health: {capacity:.0}% of its original capacity ({full:.1} of {design:.1} Wh)"
                )),
                (Some(capacity), ..) => {
                    Some(format!("Health: {capacity:.0}% of its original capacity"))
                }
                _ => None,
            };
            card =
                card.push(muted(health.unwrap_or_else(|| {
                    "The battery does not report its health.".into()
                })));
            if let Some(cycles) = battery.cycles {
                card = card.push(muted(format!("Charge cycles: {cycles}")));
            }
            body = body.push(card);
        }
        settings_group("Battery", body)
    }

    fn live_power_sections<'a>(&'a self, state: &'a PowerState) -> Element<'a, Message> {
        let page = &self.power;
        let profiles: Element<'_, Message> = match &state.profiles {
            None => {
                muted("Power modes are not available because power-profiles-daemon is not running.")
            }
            Some(profiles) => {
                let active = match page.live.pending("power-profile") {
                    Some(PowerAction::Profile(profile)) => profile.as_str(),
                    _ => profiles.active.as_str(),
                };
                let mut buttons = row![].spacing(10);
                for name in &profiles.supported {
                    buttons =
                        buttons.push(self.action(
                            profile_label(name),
                            (active != *name).then(|| {
                                message(Event::Apply(PowerAction::Profile((*name).into())))
                            }),
                            active == *name,
                        ));
                }
                column![
                    text(format!("Current mode: {}", profile_label(active))),
                    buttons,
                    page.control_status("power-profile", message),
                ]
                .spacing(10)
                .into()
            }
        };
        let brightness: Element<'_, Message> = match &state.backlight {
            None => muted("This computer has no built-in display backlight."),
            Some(light) => {
                let value = match page.live.pending("brightness") {
                    Some(PowerAction::Brightness(value)) => *value,
                    _ => light.perceptual().max(1),
                };
                column![
                    text(format!("Built-in display: {value}%")),
                    adjust("power-brightness", 1.0..=100., f64::from(value), 1., |v| {
                        message(Event::Apply(PowerAction::Brightness(v.round() as u16)))
                    }),
                    page.control_status("brightness", message),
                ]
                .spacing(10)
                .into()
            }
        };
        column![
            Self::battery_section(state),
            settings_group("Power mode", profiles),
            settings_group("Brightness", brightness),
        ]
        .spacing(14)
        .into()
    }

    fn lid_section(&self) -> Element<'_, Message> {
        let lid = self.power_choices.value.lid_close;
        let options: Vec<String> = LidAction::ALL
            .into_iter()
            .map(|action| lid_label(action).to_owned())
            .collect();
        let power_key: Element<'_, Message> = if self.paths.active() {
            let current = POWER_KEY
                .iter()
                .find(|(_, logind)| *logind == self.input.value.power_key_to_logind)
                .map(|(label, _)| label.to_string());
            column![
                text("When the power button is pressed"),
                choice(
                    "power-button",
                    POWER_KEY
                        .iter()
                        .map(|(label, _)| label.to_string())
                        .collect(),
                    current,
                    |label| Message::PowerKey(
                        POWER_KEY
                            .iter()
                            .any(|(name, logind)| *name == label && *logind)
                    ),
                ),
                muted("Applies at once. The system setting is usually Power off."),
            ]
            .spacing(6)
            .into()
        } else {
            muted("The power button can be changed in a Canopa session.")
        };
        settings_group(
            "Lid and power button",
            column![
                column![
                    text("When the lid is closed"),
                    choice("lid-close", options, Some(lid_label(lid).to_owned()), |label| {
                        Message::LidClose(
                            LidAction::ALL
                                .into_iter()
                                .find(|action| lid_label(*action) == label)
                                .unwrap_or_default(),
                        )
                    }),
                    muted(
                        "Applies after Save, in the Canopa session. While an external display is connected, closing the lid does nothing.",
                    ),
                ]
                .spacing(6),
                power_key,
            ]
            .spacing(14),
        )
    }

    fn idle_section(&self) -> Element<'_, Message> {
        let stages = [
            "Lock the screen after",
            "Then turn off the displays after",
            "Then suspend after",
        ];
        let picker = |battery: bool, index: usize| {
            let values = if battery {
                &self.idle.value.battery
            } else {
                &self.idle.value.ac
            };
            let current = values[index].parse::<u32>().ok();
            let mut seconds: Vec<u32> = DURATIONS.to_vec();
            if let Some(current) = current
                && !seconds.contains(&current)
            {
                seconds.push(current);
                seconds.sort_unstable();
            }
            let labels: Vec<String> = seconds.iter().map(|value| duration_label(*value)).collect();
            column![
                text(stages[index]),
                choice(
                    format!("idle-{}-{index}", if battery { "battery" } else { "ac" }),
                    labels,
                    current.map(duration_label),
                    move |label| {
                        let value = seconds
                            .iter()
                            .find(|value| duration_label(**value) == label)
                            .copied()
                            .unwrap_or(300);
                        Message::IdleTime(battery, index, value.to_string())
                    },
                ),
            ]
            .spacing(6)
        };
        let mut ac = column![].spacing(10);
        let mut battery = column![].spacing(10);
        for index in 0..3 {
            ac = ac.push(picker(false, index));
            battery = battery.push(picker(true, index));
        }
        let daemon: Element<'_, Message> = if self.idle_available {
            column![
                text(format!("Idle daemon: running. {}", self.idle_status)),
                row![
                    self.action(
                        "Pause idle actions",
                        (!self.busy).then_some(Message::IdleAction("pause")),
                        false
                    ),
                    self.action(
                        "Resume",
                        (!self.busy).then_some(Message::IdleAction("resume")),
                        false
                    ),
                ]
                .spacing(12),
                row![
                    super::field("Stasis profile", &self.profile, Message::Profile),
                    self.action(
                        "Use profile",
                        (!self.busy && !self.profile.trim().is_empty())
                            .then_some(Message::UseProfile),
                        false
                    )
                ]
                .spacing(12)
                .align_y(Alignment::End),
            ]
            .spacing(12)
            .into()
        } else {
            column![
                text("Idle daemon: not running."),
                muted(
                    "Stasis is optional. You can still save timeouts; they take effect when Stasis starts.",
                ),
            ]
            .spacing(6)
            .into()
        };
        column![
            settings_group("Idle", daemon),
            muted("Each stage waits after the previous one: lock, displays off, then suspend."),
            row![
                settings_group("On AC power", ac),
                settings_group("On battery", battery)
            ]
            .spacing(12),
            muted(
                "Custom commands, profiles and inhibition rules stay in your existing idle policy.",
            ),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn power_view(&self) -> Element<'_, Message> {
        let live = self
            .power
            .view(message, |state| self.live_power_sections(state));
        column![live, self.lid_section(), self.idle_section()]
            .spacing(14)
            .into()
    }
}

fn profile_label(profile: &str) -> &str {
    match profile {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}
