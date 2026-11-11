//! The Power page: live battery, power mode and brightness sections, then the
//! saved lid choice and idle timeouts, and the power button (applied at once).
use super::{Message, State, adjust, live_page::LivePage, pick, settings_group};
use crate::{
    live::Event,
    power::{DURATIONS, PowerAction, PowerService, PowerState, duration_label, lid_label},
    preferences::IdleDraft,
};
use canopa_services::{power_switches::LidAction, upower::PowerState as Charge};
use canopa_ui::direction;
use iced::{
    Alignment, Element,
    widget::{column, text},
};

pub(super) type PowerPage = LivePage<PowerService>;

pub(super) fn message(event: Event<PowerService>) -> Message {
    Message::Power(event)
}

/// The power-button choices, by whether the button is left to logind: Niri
/// suspends by default.
fn power_key_options() -> Vec<(bool, String)> {
    vec![
        (false, fl!("settings-power-suspend")),
        (true, fl!("settings-power-system-setting")),
    ]
}

fn charge(state: Charge) -> String {
    match state {
        Charge::Charging => fl!("settings-battery-charging"),
        Charge::Discharging => fl!("settings-battery-discharging"),
        Charge::Empty => fl!("settings-battery-empty"),
        Charge::FullyCharged => fl!("settings-battery-full"),
        Charge::PendingCharge => fl!("settings-battery-pending-charge"),
        Charge::PendingDischarge => fl!("settings-battery-pending-discharge"),
        Charge::Unknown => fl!("settings-battery-unknown"),
    }
}

fn approximately(seconds: u64) -> String {
    // Estimates move constantly; minutes are as precise as they get.
    let minutes = (seconds + 30) / 60;
    canopa_i18n::format::duration(minutes.max(1) * 60)
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
                fl!("settings-battery"),
                muted(fl!("settings-battery-no-upower")),
            );
        };
        if supply.batteries.is_empty() {
            return settings_group(fl!("settings-battery"), muted(fl!("settings-battery-none")));
        }
        let mut body = column![].spacing(10);
        for battery in &supply.batteries {
            let level = canopa_i18n::format::percent(battery.percentage);
            let state = charge(battery.state);
            let status = if let Some(seconds) = battery.time_to_empty.filter(|_| supply.on_battery)
            {
                fl!(
                    "settings-battery-status-left",
                    level = level,
                    state = state,
                    time = approximately(seconds)
                )
            } else if let Some(seconds) = battery.time_to_full {
                fl!(
                    "settings-battery-status-to-full",
                    level = level,
                    state = state,
                    time = approximately(seconds)
                )
            } else {
                fl!("settings-battery-status", level = level, state = state)
            };
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
                (Some(capacity), Some(full), Some(design)) => Some(fl!(
                    "settings-battery-health-energy",
                    capacity = canopa_i18n::format::percent(capacity),
                    full = canopa_i18n::format::decimal(full, 1, true),
                    design = canopa_i18n::format::decimal(design, 1, true)
                )),
                (Some(capacity), ..) => Some(fl!(
                    "settings-battery-health",
                    capacity = canopa_i18n::format::percent(capacity)
                )),
                _ => None,
            };
            card = card.push(muted(
                health.unwrap_or_else(|| fl!("settings-battery-health-unknown")),
            ));
            if let Some(cycles) = battery.cycles {
                card = card.push(muted(fl!("settings-battery-cycles", cycles = cycles)));
            }
            body = body.push(card);
        }
        settings_group(fl!("settings-battery"), body)
    }

    fn live_power_sections<'a>(&'a self, state: &'a PowerState) -> Element<'a, Message> {
        let page = &self.power;
        let profiles: Element<'_, Message> = match &state.profiles {
            None => muted(fl!("settings-power-modes-unavailable")),
            Some(profiles) => {
                let active = match page.live.pending("power-profile") {
                    Some(PowerAction::Profile(profile)) => profile.as_str(),
                    _ => profiles.active.as_str(),
                };
                let buttons = direction::row(profiles.supported.iter().map(|name| {
                    self.action(
                        format!("power-profile-{name}"),
                        profile_label(name),
                        (active != *name)
                            .then(|| message(Event::Apply(PowerAction::Profile((*name).into())))),
                        active == *name,
                    )
                }))
                .spacing(10);
                column![
                    text(fl!(
                        "settings-power-mode-current",
                        mode = profile_label(active)
                    )),
                    buttons,
                    page.control_status("power-profile", message),
                ]
                .spacing(10)
                .into()
            }
        };
        let brightness: Element<'_, Message> = match &state.backlight {
            None => muted(fl!("settings-brightness-none")),
            Some(light) => {
                let value = match page.live.pending("brightness") {
                    Some(PowerAction::Brightness(value)) => *value,
                    _ => light.perceptual().max(1),
                };
                column![
                    text(fl!(
                        "settings-brightness-built-in",
                        value = canopa_i18n::format::percent(f64::from(value))
                    )),
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
            settings_group(fl!("settings-power-mode"), profiles),
            settings_group(fl!("settings-brightness"), brightness),
        ]
        .spacing(14)
        .into()
    }

    fn lid_section(&self) -> Element<'_, Message> {
        let lid = self.power_choices.value.lid_close;
        let options: Vec<(LidAction, String)> = LidAction::ALL
            .into_iter()
            .map(|action| (action, lid_label(action)))
            .collect();
        let power_key: Element<'_, Message> = if self.paths.active() {
            column![
                text(fl!("settings-power-button-pressed")),
                pick(
                    "power-button",
                    power_key_options(),
                    Some(self.input.value.power_key_to_logind),
                    Message::PowerKey,
                ),
                muted(fl!("settings-power-button-hint")),
            ]
            .spacing(6)
            .into()
        } else {
            muted(fl!("settings-power-button-needs-session"))
        };
        settings_group(
            fl!("settings-power-lid-and-button"),
            column![
                column![
                    text(fl!("settings-power-lid-closed")),
                    pick("lid-close", options, Some(lid), Message::LidClose),
                    muted(fl!("settings-power-lid-hint")),
                ]
                .spacing(6),
                power_key,
            ]
            .spacing(14),
        )
    }

    fn idle_section(&self) -> Element<'_, Message> {
        let stages = [
            fl!("settings-idle-lock-after"),
            fl!("settings-idle-displays-after"),
            fl!("settings-idle-suspend-after"),
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
            let options: Vec<(u32, String)> = seconds
                .iter()
                .map(|value| (*value, duration_label(*value)))
                .collect();
            column![
                text(stages[index].clone()),
                pick(
                    format!("idle-{}-{index}", if battery { "battery" } else { "ac" }),
                    options,
                    current,
                    move |value| Message::IdleTime(battery, index, value.to_string()),
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
                text(fl!(
                    "settings-idle-running",
                    status = self.idle_status.as_str()
                )),
                direction::row([
                    self.action(
                        "settings-button-pause-idle-actions",
                        fl!("settings-idle-pause"),
                        (!self.busy).then_some(Message::IdleAction("pause")),
                        false,
                    ),
                    self.action(
                        "settings-button-resume",
                        fl!("settings-idle-resume"),
                        (!self.busy).then_some(Message::IdleAction("resume")),
                        false,
                    ),
                ])
                .spacing(12),
                direction::row([
                    super::field(
                        "settings-field-stasis-profile",
                        fl!("settings-idle-profile"),
                        &self.profile,
                        Message::Profile,
                    ),
                    self.action(
                        "settings-button-use-profile",
                        fl!("settings-idle-use-profile"),
                        (!self.busy && !self.profile.trim().is_empty())
                            .then_some(Message::UseProfile),
                        false,
                    ),
                ])
                .spacing(12)
                .align_y(Alignment::End),
            ]
            .spacing(12)
            .into()
        } else {
            column![
                text(fl!("settings-idle-not-running")),
                muted(fl!("settings-idle-optional")),
            ]
            .spacing(6)
            .into()
        };
        column![
            settings_group(fl!("settings-idle"), daemon),
            muted(fl!("settings-idle-stages-hint")),
            direction::row([
                settings_group(fl!("settings-idle-ac"), ac),
                settings_group(fl!("settings-idle-battery"), battery),
            ])
            .spacing(12),
            muted(fl!("settings-idle-custom-hint")),
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

fn profile_label(profile: &str) -> String {
    match profile {
        "power-saver" => fl!("settings-power-mode-saver"),
        "balanced" => fl!("settings-power-mode-balanced"),
        "performance" => fl!("settings-power-mode-performance"),
        other => other.to_owned(),
    }
}
