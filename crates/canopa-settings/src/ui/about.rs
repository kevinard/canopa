//! The About page: the device name (changed through hostnamed and polkit),
//! the hardware and the software.
use super::{Message, State, field, live_page::LivePage, settings_group};
use crate::{
    about::{AboutAction, AboutService, AboutState, binary_size, decimal_size},
    live::Event,
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, row, text},
};

pub(super) type AboutPage = LivePage<AboutService>;

pub(super) fn message(event: Event<AboutService>) -> Message {
    Message::About(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

fn fact<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    row![
        text(label)
            .width(Length::Fixed(160.))
            .style(canopa_ui::muted),
        text(value).width(Length::Fill),
    ]
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
            None => muted(
                "systemd-hostnamed is not running, so the computer's name cannot be shown or changed here.",
            ),
            Some(host) => {
                let current = host.pretty_or_static();
                let typed = self.about_name.as_deref().unwrap_or(current);
                let changed = self
                    .about_name
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty() && name.trim() != current);
                column![
                    row![
                        field("about-name", "Device name", typed, |value| {
                            Message::AboutForm(Edit::Name(value))
                        }),
                        self.action(
                            "settings-button-rename",
                            "Rename",
                            changed.then_some(Message::AboutForm(Edit::Rename)),
                            changed,
                        ),
                    ]
                    .spacing(12)
                    .align_y(Alignment::End),
                    muted(format!(
                        "Other devices see this computer as {} on the network.",
                        if host.static_hostname.is_empty() {
                            &host.hostname
                        } else {
                            &host.static_hostname
                        }
                    )),
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
        let mut hardware = column![].spacing(8);
        let computer = [vendor, model]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !computer.is_empty() {
            hardware = hardware.push(fact("Model", computer));
        }
        if !info.cpu.is_empty() {
            hardware = hardware.push(fact(
                "Processor",
                if info.cores > 1 {
                    format!("{} × {}", info.cpu, info.cores)
                } else {
                    info.cpu.clone()
                },
            ));
        }
        if info.memory_bytes > 0 {
            hardware = hardware.push(fact("Memory", binary_size(info.memory_bytes)));
        }
        for (index, graphics) in info.graphics.iter().enumerate() {
            hardware = hardware.push(fact(
                if index == 0 { "Graphics" } else { "" },
                graphics.clone(),
            ));
        }
        if !info.disks.is_empty() {
            let total: u64 = info.disks.iter().map(|(_, size)| size).sum();
            let detail = info
                .disks
                .iter()
                .map(|(name, size)| format!("{name} ({})", decimal_size(*size)))
                .collect::<Vec<_>>()
                .join(", ");
            hardware = hardware.push(fact(
                "Disk capacity",
                format!("{} — {detail}", decimal_size(total)),
            ));
        }
        let firmware = pick(host.map(|h| &h.firmware), &info.firmware);
        if !firmware.is_empty() {
            hardware = hardware.push(fact("Firmware", firmware));
        }
        let os = [info.os_name.as_str(), info.os_version.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let mut software = column![fact("Operating system", os)].spacing(8);
        if !info.os_build.is_empty() {
            software = software.push(fact("Build", info.os_build.clone()));
        }
        let kernel = pick(host.map(|h| &h.kernel), &info.kernel);
        if !kernel.is_empty() {
            software = software.push(fact("Kernel", kernel));
        }
        software = software.push(fact("Canopa", info.canopa_version.clone()));
        column![
            settings_group("Name", name),
            settings_group("Hardware", hardware),
            settings_group("Software", software),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn about_view(&self) -> Element<'_, Message> {
        self.about.view(message, |state| self.about_sections(state))
    }
}
