//! The Power page: live battery, power mode and brightness sections, then the
//! saved lid choice and idle timeouts, and the power button (applied at once).
use super::{Message, State, adjust, kit, live_page::LivePage, pick, settings_group};
use crate::{
    live::Event,
    power::{DURATIONS, PowerAction, PowerService, PowerState, duration_label, lid_label},
    preferences::IdleDraft,
};
use canopa_services::{power_switches::LidAction, upower::PowerState as Charge};
use canopa_ui::direction;
use iced::{
    Alignment, Element,
    widget::{column, container, text},
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
    text(label)
        .size(kit::size::CAPTION)
        .style(canopa_ui::muted)
        .width(iced::Length::Fill)
        .align_x(direction::text_start())
        .into()
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

    /// What the power supply says about the computer: `None` reads "no battery".
    fn hardware(&self) -> Option<&crate::power::PowerState> {
        match self.power.live.status() {
            crate::live::Status::Ready(state) => Some(state),
            _ => None,
        }
    }
    fn has_battery(&self) -> bool {
        self.hardware()
            .and_then(|state| state.supply.as_ref())
            .is_none_or(|supply| !supply.batteries.is_empty())
    }

    /// The battery row; `None` on a computer without one.
    fn battery_section(state: &PowerState) -> Option<Element<'_, Message>> {
        let Some(supply) = &state.supply else {
            return Some(kit::row(
                fl!("settings-battery"),
                Some(fl!("settings-battery-no-upower")),
                iced::widget::space(),
            ));
        };
        if supply.batteries.is_empty() {
            return None;
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
        Some(
            column![
                super::heading(fl!("settings-battery"), kit::size::BODY),
                body
            ]
            .spacing(8)
            .into(),
        )
    }

    fn live_power_sections<'a>(&'a self, state: &'a PowerState) -> Element<'a, Message> {
        let page = &self.power;
        let profiles: Element<'_, Message> = match &state.profiles {
            None => kit::status(
                kit::Notice::Warning,
                fl!("settings-power-modes-unavailable-title"),
                Some(fl!("settings-power-modes-unavailable")),
                None,
            ),
            Some(profiles) => {
                let active = match page.live.pending("power-profile") {
                    Some(PowerAction::Profile(profile)) => profile.as_str(),
                    _ => profiles.active.as_str(),
                };
                let buttons = kit::segmented(profiles.supported.iter().map(|name| {
                    self.segment(
                        format!("power-profile-{name}"),
                        profile_label(name),
                        (active != *name)
                            .then(|| message(Event::Apply(PowerAction::Profile((*name).into())))),
                        active == *name,
                    )
                }));
                column![
                    kit::row(
                        fl!("settings-power-mode"),
                        Some(fl!(
                            "settings-power-mode-current",
                            mode = profile_label(active)
                        )),
                        buttons,
                    ),
                    page.control_status("power-profile", message),
                ]
                .spacing(6)
                .into()
            }
        };
        // Desktops have no backlight to set; the row is left out.
        let brightness: Option<Element<'_, Message>> = state.backlight.as_ref().map(|light| {
            let value = match page.live.pending("brightness") {
                Some(PowerAction::Brightness(value)) => *value,
                _ => light.perceptual().max(1),
            };
            column![
                kit::slider_row(
                    fl!("settings-brightness"),
                    canopa_i18n::format::percent(f64::from(value)),
                    adjust("power-brightness", 1.0..=100., f64::from(value), 1., |v| {
                        message(Event::Apply(PowerAction::Brightness(v.round() as u16)))
                    }),
                ),
                page.control_status("brightness", message),
            ]
            .spacing(6)
            .into()
        });
        let battery = Self::battery_section(state);
        if battery.is_none() && brightness.is_none() && state.profiles.is_none() {
            return column![
                kit::section_heading(fl!("settings-power-energy")),
                kit::status_card(
                    kit::Notice::Warning,
                    fl!("settings-power-modes-unavailable-title"),
                    Some(fl!("settings-power-modes-unavailable")),
                ),
            ]
            .spacing(10)
            .into();
        }
        let title = if battery.is_some() {
            fl!("settings-power-status")
        } else {
            fl!("settings-power-energy")
        };
        settings_group(
            title,
            kit::rows(battery.into_iter().chain([profiles]).chain(brightness)),
        )
    }

    fn lid_section(&self) -> Element<'_, Message> {
        let lid = self.power_choices.value.lid_close;
        let options: Vec<(LidAction, String)> = LidAction::ALL
            .into_iter()
            .map(|action| (action, lid_label(action)))
            .collect();
        let power_key: Element<'_, Message> = if self.paths.active() {
            kit::row(
                fl!("settings-power-button-pressed"),
                Some(fl!("settings-power-button-hint")),
                pick(
                    "power-button",
                    power_key_options(),
                    Some(self.input.value.power_key_to_logind),
                    Message::PowerKey,
                ),
            )
        } else {
            kit::row(
                fl!("settings-power-button-pressed"),
                Some(fl!("settings-power-button-needs-session")),
                iced::widget::space(),
            )
        };
        // Only a computer with a lid gets the lid choice.
        let lid_present = self
            .hardware()
            .and_then(|state| state.supply.as_ref())
            .is_none_or(|supply| supply.lid_present);
        if !lid_present {
            return settings_group(fl!("settings-power-button-title"), power_key);
        }
        settings_group(
            fl!("settings-power-lid-and-button"),
            kit::rows([
                kit::row(
                    fl!("settings-power-lid-closed"),
                    Some(fl!("settings-power-lid-hint")),
                    pick("lid-close", options, Some(lid), Message::LidClose),
                ),
                power_key,
            ]),
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
            pick(
                format!("idle-{}-{index}", if battery { "battery" } else { "ac" }),
                options,
                current,
                move |value| Message::IdleTime(battery, index, value.to_string()),
            )
        };
        let has_battery = self.has_battery();
        // One row per stage; on a laptop, a column for each power source.
        let mut timeouts: Vec<Element<'_, Message>> = Vec::new();
        if has_battery {
            timeouts.push(
                direction::row([
                    iced::widget::space().width(iced::Length::Fill).into(),
                    container(muted(fl!("settings-idle-ac")))
                        .width(kit::PICK_WIDTH)
                        .into(),
                    container(muted(fl!("settings-idle-battery")))
                        .width(kit::PICK_WIDTH)
                        .into(),
                ])
                .spacing(12)
                .into(),
            );
        }
        for (index, stage) in stages.iter().enumerate() {
            let mut controls = vec![picker(false, index)];
            if has_battery {
                controls.push(picker(true, index));
            }
            timeouts.push(kit::row(
                stage.clone(),
                match index {
                    1 => Some(fl!("settings-idle-displays-after-hint")),
                    2 => Some(fl!("settings-idle-suspend-after-hint")),
                    _ => None,
                },
                direction::row(controls).spacing(12),
            ));
        }
        // The stages add up; the total answers "when does it suspend?".
        let total: u32 = self
            .idle
            .value
            .ac
            .iter()
            .filter_map(|value| value.parse::<u32>().ok())
            .sum();
        if total > 0 {
            // Said as what will happen while idle actions are not running.
            let time = crate::power::duration_label(total);
            timeouts.push(muted(if self.idle_available {
                fl!("settings-idle-total", time = time)
            } else {
                fl!("settings-idle-total-pending", time = time)
            }));
        }
        let daemon: Element<'_, Message> = if self.idle_available {
            column![
                direction::row([
                    text(fl!(
                        "settings-idle-running",
                        status = self.idle_status.as_str()
                    ))
                    .width(iced::Length::Fill)
                    .into(),
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
                .spacing(8)
                .align_y(Alignment::Center),
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
            // A stopped service is said above the card, as on every page.
            return column![
                kit::section_heading(fl!("settings-idle")),
                kit::status_card(
                    kit::Notice::Warning,
                    fl!("settings-idle-not-running"),
                    Some(fl!("settings-idle-optional")),
                ),
                container(kit::rows(timeouts))
                    .padding(kit::ROW_PADDING)
                    .width(iced::Length::Fill)
                    .style(kit::card),
                kit::footnote(fl!("settings-idle-custom-hint")),
            ]
            .spacing(10)
            .into();
        };
        column![
            settings_group(
                fl!("settings-idle"),
                kit::rows(std::iter::once(daemon).chain(timeouts)),
            ),
            kit::footnote(fl!("settings-idle-custom-hint")),
        ]
        .spacing(10)
        .into()
    }

    pub(super) fn power_view(&self) -> Element<'_, Message> {
        let live = self
            .power
            .view(message, |state| self.live_power_sections(state));
        column![live, self.lid_section(), self.idle_section()]
            .spacing(28)
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
