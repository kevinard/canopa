//! The About page: the device name (changed through hostnamed and polkit),
//! the hardware and the software.
use super::{Message, State, kit, live_page::LivePage, settings_group};
use crate::{
    about::{AboutAction, AboutService, AboutState, binary_size, decimal_size},
    live::Event,
};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, text},
};

pub(super) type AboutPage = LivePage<AboutService>;

pub(super) fn message(event: Event<AboutService>) -> Message {
    Message::About(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(13).style(canopa_ui::muted).into()
}

fn fact<'a>(label: String, value: String) -> Element<'a, Message> {
    direction::row([
        text(label)
            .width(Length::Fixed(170.))
            .align_x(direction::text_start())
            .style(canopa_ui::muted)
            .into(),
        text(value)
            .width(Length::Fill)
            .align_x(direction::text_end())
            .into(),
    ])
    .spacing(12)
    .into()
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    Name(String),
    Rename,
}

impl State {
    pub(super) fn about_edit(&mut self, change: Edit) -> Task<Message> {
        match change {
            Edit::Name(name) => {
                self.about_name = Some(name.chars().filter(|c| !c.is_control()).take(64).collect());
                Task::none()
            }
            Edit::Rename => match self.about_name.take() {
                Some(name) => self
                    .about
                    .update(Event::Apply(AboutAction::Rename(name)), message),
                None => Task::none(),
            },
        }
    }

    fn about_sections<'a>(&'a self, state: &'a AboutState) -> Element<'a, Message> {
        let info = &state.system;
        let name: Element<'_, Message> = match &state.host {
            None => muted(fl!("settings-about-no-hostnamed-hint")),
            Some(host) => {
                let current = host.pretty_or_static();
                let typed = self.about_name.as_deref().unwrap_or(current);
                let changed = self
                    .about_name
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty() && name.trim() != current);
                column![
                    kit::row(
                        fl!("settings-about-device-name"),
                        Some(fl!(
                            "settings-about-network-name",
                            name = if host.static_hostname.is_empty() {
                                host.hostname.as_str()
                            } else {
                                host.static_hostname.as_str()
                            }
                        )),
                        // Rename appears once the typed name differs.
                        direction::row(
                            [Some(super::bare_field(
                                "about-name",
                                fl!("settings-about-device-name"),
                                "",
                                typed,
                                |value| Message::AboutForm(Edit::Name(value)),
                                Length::Fixed(kit::PICK_WIDTH),
                            ))]
                            .into_iter()
                            .chain([changed.then(|| {
                                self.treated_action(
                                    "settings-button-rename",
                                    fl!("settings-about-rename"),
                                    Some(Message::AboutForm(Edit::Rename)),
                                    super::kit::Treatment::Primary,
                                )
                            })])
                            .flatten(),
                        )
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ),
                    self.about.control_status("name", message),
                ]
                .spacing(8)
                .into()
            }
        };
        let host = state.host.as_ref();
        let pick = |from_host: Option<&String>, local: &String| {
            from_host
                .filter(|value| !value.is_empty())
                .unwrap_or(local)
                .clone()
        };
        let vendor = pick(host.map(|h| &h.vendor), &info.vendor);
        let model = pick(host.map(|h| &h.model), &info.model);
        let mut hardware: Vec<Element<'_, Message>> = Vec::new();
        let computer = [vendor, model]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !computer.is_empty() {
            hardware.push(fact(fl!("settings-about-model"), computer));
        }
        if !info.cpu.is_empty() {
            hardware.push(fact(
                fl!("settings-about-processor"),
                if info.cores > 1 {
                    format!("{} × {}", info.cpu, info.cores)
                } else {
                    info.cpu.clone()
                },
            ));
        }
        if info.memory_bytes > 0 {
            hardware.push(fact(
                fl!("settings-about-memory"),
                binary_size(info.memory_bytes),
            ));
        }
        for (index, graphics) in info.graphics.iter().enumerate() {
            hardware.push(fact(
                if index == 0 {
                    fl!("settings-about-graphics")
                } else {
                    String::new()
                },
                graphics.clone(),
            ));
        }
        if !info.disks.is_empty() {
            let total: u64 = info.disks.iter().map(|(_, size)| size).sum();
            let detail = info
                .disks
                .iter()
                .map(|(name, size)| {
                    fl!(
                        "settings-about-disk",
                        name = name.as_str(),
                        size = decimal_size(*size)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            hardware.push(fact(
                fl!("settings-about-disk-capacity"),
                fl!(
                    "settings-about-disks",
                    total = decimal_size(total),
                    disks = detail
                ),
            ));
        }
        let firmware = pick(host.map(|h| &h.firmware), &info.firmware);
        if !firmware.is_empty() {
            hardware.push(fact(fl!("settings-about-firmware"), firmware));
        }
        let os = [info.os_name.as_str(), info.os_version.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let mut software: Vec<Element<'_, Message>> = vec![fact(fl!("settings-about-os"), os)];
        if !info.os_build.is_empty() {
            software.push(fact(fl!("settings-about-build"), info.os_build.clone()));
        }
        let kernel = pick(host.map(|h| &h.kernel), &info.kernel);
        if !kernel.is_empty() {
            software.push(fact(fl!("settings-about-kernel"), kernel));
        }
        software.push(fact("Canopa".into(), info.canopa_version.clone()));
        column![
            settings_group(fl!("settings-about-name"), name),
            settings_group(fl!("settings-about-hardware"), kit::rows(hardware)),
            settings_group(fl!("settings-about-software"), kit::rows(software)),
        ]
        .spacing(28)
        .into()
    }

    pub(super) fn about_view(&self) -> Element<'_, Message> {
        self.about.view(message, |state| self.about_sections(state))
    }
}
