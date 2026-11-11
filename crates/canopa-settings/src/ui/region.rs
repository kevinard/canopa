//! The Region & language page: the person's language and formats (with a
//! preview), the login screen's language, and the system keyboard layout
//! next to the person's layouts from the Keyboard & pointer page.
use super::{Message, State, choice, live_page::LivePage, settings_group};
use crate::{
    Page,
    file_page::FileModel,
    input::Input,
    live::Event,
    region::{RegionAction, RegionService, RegionState, same_locale},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, row, text},
};

pub(super) type RegionPage = LivePage<RegionService>;

pub(super) fn message(event: Event<RegionService>) -> Message {
    Message::Region(event)
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

impl State {
    /// A choice among the installed locales, applying `action` for the name.
    fn locale_choice<'a>(
        &'a self,
        id: &'static str,
        title: &'a str,
        state: &'a RegionState,
        current: String,
        action: fn(String) -> RegionAction,
    ) -> Element<'a, Message> {
        let page = &self.region;
        let control = action(String::new());
        let control = crate::live::LiveAction::control(&control);
        let selected = match page.live.pending(&control) {
            Some(
                RegionAction::UserLanguage(name)
                | RegionAction::UserFormats(name)
                | RegionAction::SystemLanguage(name),
            ) => name.clone(),
            _ => current,
        };
        let labels: Vec<String> = state
            .installed
            .iter()
            .map(|locale| locale.label.clone())
            .collect();
        let selected = state
            .installed
            .iter()
            .find(|locale| same_locale(&locale.name, &selected))
            .map(|locale| locale.label.clone());
        let installed = state.installed.clone();
        column![
            row![
                text(title).width(Length::Fixed(220.)),
                choice(id, labels, selected, move |label| {
                    let name = installed
                        .iter()
                        .find(|locale| locale.label == label)
                        .map(|locale| locale.name.clone())
                        .unwrap_or_default();
                    message(Event::Apply(action(name)))
                }),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            page.control_status(&control, message),
        ]
        .spacing(4)
        .into()
    }

    fn region_sections<'a>(&'a self, state: &'a RegionState) -> Element<'a, Message> {
        let mut personal = column![].spacing(10);
        if state.installed.len() <= 1 {
            personal = personal.push(muted(
                "Only the C locale is installed. An administrator adds languages by enabling them in /etc/locale.gen and running locale-gen.",
            ));
        }
        personal = personal
            .push(self.locale_choice(
                "region-language",
                "Language",
                state,
                state.language(),
                RegionAction::UserLanguage,
            ))
            .push(self.locale_choice(
                "region-formats",
                "Formats (dates, numbers, money)",
                state,
                state.formats(),
                RegionAction::UserFormats,
            ));
        if let Some(preview) = &state.preview {
            personal = personal.push(
                column![
                    fact("Date", preview.date.clone()),
                    fact("Time", preview.time.clone()),
                    fact("Number", preview.number.clone()),
                    fact("Money", preview.currency.clone()),
                    fact(
                        "Measurement",
                        if preview.metric {
                            "Metric"
                        } else {
                            "US customary"
                        }
                        .into()
                    ),
                ]
                .spacing(6),
            );
        }
        personal = personal.push(muted(
            "Applies the next time you log in. Saved in ~/.config/locale.conf.",
        ));

        let layouts = self
            .input
            .source()
            .and_then(|source| Input::read(source).ok())
            .unwrap_or_else(|| self.input.value.clone());
        let system: Element<'_, Message> = match &state.system {
            None => muted(
                "systemd-localed is not running, so the system language and keyboard cannot be shown or changed here.",
            ),
            Some(system) => {
                let keyboard = if system.x11_layout.is_empty() {
                    "Not set (us)".to_owned()
                } else if system.x11_variant.is_empty() {
                    system.x11_layout.clone()
                } else {
                    format!("{} ({})", system.x11_layout, system.x11_variant)
                };
                let same = system.x11_layout == layouts.layouts
                    && system.x11_variant == layouts.variants
                    && system.x11_options == layouts.options;
                column![
                    self.locale_choice(
                        "region-system-language",
                        "Login screen and new users",
                        state,
                        system.get("LANG").unwrap_or("C.UTF-8").to_owned(),
                        RegionAction::SystemLanguage,
                    ),
                    fact("System keyboard", keyboard),
                    fact("Your layouts", layouts.layouts.clone()),
                    row![
                        self.action(
                            "Use my layouts for the login screen and console",
                            (!same && !layouts.layouts.trim().is_empty()).then(|| {
                                message(Event::Apply(RegionAction::SystemKeyboard {
                                    layouts: layouts.layouts.clone(),
                                    variants: layouts.variants.clone(),
                                    options: layouts.options.clone(),
                                }))
                            }),
                            false,
                        ),
                        self.action(
                            "Change my layouts…",
                            Some(Message::Page(Page::Input)),
                            false
                        ),
                    ]
                    .spacing(10),
                    self.region.control_status("system-keyboard", message),
                    muted("System changes ask for an administrator's permission."),
                ]
                .spacing(10)
                .into()
            }
        };
        column![
            settings_group("Your language and formats", personal),
            settings_group("System", system),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn region_view(&self) -> Element<'_, Message> {
        self.region
            .view(message, |state| self.region_sections(state))
    }
}
