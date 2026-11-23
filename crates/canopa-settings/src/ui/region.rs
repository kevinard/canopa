//! The Region & language page: the person's language and formats (with a
//! preview), the login screen's language, and the system keyboard layout
//! next to the person's layouts from the Keyboard & pointer page.
//!
//! A new language or formats choice also applies to Settings at once
//! (`canopa_i18n::select_locale`); the window shows it at its next redraw,
//! and a notice names the applications that need a restart.
use super::{Message, State, kit, live_page::LivePage, pick, settings_group};
use crate::{
    file_page::FileModel,
    input::Input,
    live::Event,
    region::{RegionAction, RegionService, RegionState, same_locale},
};
use canopa_ui::direction;
use iced::{
    Element, Length, Task,
    widget::{column, text},
};

pub(super) type RegionPage = LivePage<RegionService>;

pub(super) fn message(event: Event<RegionService>) -> Message {
    Message::Region(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(13).style(canopa_ui::muted).into()
}

fn fact<'a>(label: String, value: String) -> Element<'a, Message> {
    direction::row([
        text(label)
            .width(Length::Fixed(160.))
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

/// The name of a shipped language, in the selected language.
fn language_name(language: &canopa_i18n::LanguageIdentifier) -> String {
    fl!("settings-language-name", code = language.language.as_str())
}

impl State {
    /// Handle a page event; a language or formats change that succeeded
    /// also applies to Settings itself.
    pub(super) fn region_event(&mut self, event: Event<RegionService>) -> Task<Message> {
        if let Event::Applied(action, Ok(state)) = &event {
            let state = state.as_ref().or(self.region.live.state()).cloned();
            match (action, state) {
                (RegionAction::UserLanguage(name), Some(state)) => {
                    self.switch_language(name, state.user.formats.as_deref(), &state);
                }
                (RegionAction::UserFormats(name), Some(state)) => {
                    let language = state.language();
                    canopa_i18n::select_locale(&language, Some(name));
                    self.notice = fl!(
                        "settings-region-formats-applied",
                        formats = state.label(name)
                    );
                }
                _ => {}
            }
        }
        self.region.update(event, message)
    }

    /// Show Settings in the language of `name` and say what else follows.
    fn switch_language(&mut self, name: &str, formats: Option<&str>, state: &RegionState) {
        canopa_i18n::select_locale(name, formats);
        // The shortcut editor keeps its rows' names: read them again in the
        // new language unless they hold edits.
        if !matches!(&self.shortcuts, Some(Ok(editor)) if editor.dirty()) {
            self.shortcuts = None;
        }
        let selected = canopa_i18n::selection();
        let shown = selected.language().clone();
        let translated =
            canopa_i18n::language_of(name).is_some_and(|wanted| wanted.language == shown.language);
        self.notice = if translated {
            fl!(
                "settings-region-language-applied",
                language = language_name(&shown)
            )
        } else {
            fl!(
                "settings-region-language-untranslated",
                locale = state.label(name)
            )
        };
    }

    /// A choice among the installed locales, applying `action` for the name.
    fn locale_choice<'a>(
        &'a self,
        id: &'static str,
        title: String,
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
        let options: Vec<(String, String)> = state
            .installed
            .iter()
            .map(|locale| (locale.name.clone(), locale.display()))
            .collect();
        let selected = state
            .installed
            .iter()
            .find(|locale| same_locale(&locale.name, &selected))
            .map(|locale| locale.name.clone());
        column![
            // With one language installed there is nothing to choose: the
            // value is shown as text, not as a list that looks open.
            if options.len() <= 1 {
                kit::row(
                    title,
                    None,
                    iced::widget::text(
                        selected
                            .as_ref()
                            .and_then(|name| options.iter().find(|(value, _)| value == name))
                            .or(options.first())
                            .map(|(_, label)| label.clone())
                            .unwrap_or_else(|| fl!("settings-choice-placeholder")),
                    )
                    .style(canopa_ui::muted),
                )
            } else {
                kit::row(
                    title,
                    None,
                    pick(id, options, selected, move |name| {
                        message(Event::Apply(action(name)))
                    }),
                )
            },
            page.control_status(&control, message),
        ]
        .spacing(4)
        .into()
    }

    fn region_sections<'a>(&'a self, state: &'a RegionState) -> Element<'a, Message> {
        let personal: Vec<Element<'_, Message>> = vec![
            self.locale_choice(
                "region-language",
                fl!("settings-region-language"),
                state,
                state.language(),
                RegionAction::UserLanguage,
            ),
            self.locale_choice(
                "region-formats",
                fl!("settings-region-formats"),
                state,
                state.formats(),
                RegionAction::UserFormats,
            ),
        ];
        // Examples of the chosen formats, as rows like every other list.
        let examples = state.preview.as_ref().map(|preview| {
            kit::rows([
                fact(fl!("settings-region-preview-date"), preview.date.clone()),
                fact(fl!("settings-region-preview-time"), preview.time.clone()),
                fact(
                    fl!("settings-region-preview-number"),
                    preview.number.clone(),
                ),
                fact(
                    fl!("settings-region-preview-money"),
                    preview.currency.clone(),
                ),
                fact(
                    fl!("settings-region-preview-measurement"),
                    if preview.metric {
                        fl!("settings-region-metric")
                    } else {
                        fl!("settings-region-us-customary")
                    },
                ),
            ])
        });
        let personal = kit::rows(personal);

        let layouts = self
            .input
            .source()
            .and_then(|source| Input::read(source).ok())
            .unwrap_or_else(|| self.input.value.clone());
        let system: Element<'_, Message> = match &state.system {
            None => muted(fl!("settings-region-no-localed")),
            Some(system) => {
                let keyboard = if system.x11_layout.is_empty() {
                    fl!(
                        "settings-region-keyboard-unset",
                        layout = crate::keyboard_layouts::name("us")
                    )
                } else if system.x11_variant.is_empty() {
                    crate::keyboard_layouts::codes(&system.x11_layout)
                        .into_iter()
                        .map(crate::keyboard_layouts::name)
                        .collect::<Vec<_>>()
                        .join(", ")
                } else {
                    format!("{} ({})", system.x11_layout, system.x11_variant)
                };
                let same = system.x11_layout == layouts.layouts
                    && system.x11_variant == layouts.variants
                    && system.x11_options == layouts.options;
                kit::rows([
                    self.locale_choice(
                        "region-system-language",
                        fl!("settings-region-system-language"),
                        state,
                        system.get("LANG").unwrap_or("C.UTF-8").to_owned(),
                        RegionAction::SystemLanguage,
                    ),
                    column![
                        kit::row(
                            fl!("settings-region-login-keyboard"),
                            Some(fl!(
                                "settings-region-keyboard-detail",
                                system = keyboard,
                                mine = crate::keyboard_layouts::codes(&layouts.layouts)
                                    .into_iter()
                                    .map(crate::keyboard_layouts::name)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )),
                            self.action(
                                "settings-button-use-my-layouts-for-the-login-screen-and-console",
                                fl!("settings-region-use-my-layouts-short"),
                                (!same && !layouts.layouts.trim().is_empty()).then(|| {
                                    message(Event::Apply(RegionAction::SystemKeyboard {
                                        layouts: layouts.layouts.clone(),
                                        variants: layouts.variants.clone(),
                                        options: layouts.options.clone(),
                                    }))
                                }),
                                false,
                            ),
                        ),
                        self.region.control_status("system-keyboard", message),
                    ]
                    .spacing(10)
                    .into(),
                ])
            }
        };
        let mut yours = column![kit::section_heading(fl!("settings-region-yours"))].spacing(10);
        if state.installed.len() <= 1 {
            yours = yours.push(kit::status_card(
                kit::Notice::Info,
                fl!("settings-region-only-c-title"),
                Some(fl!("settings-region-only-c")),
            ));
        }
        let yours = yours
            .push(
                iced::widget::container(personal)
                    .padding(kit::ROW_PADDING)
                    .width(Length::Fill)
                    .style(kit::card),
            )
            .push(kit::footnote(fl!("settings-region-applies")));
        let mut sections = column![yours].spacing(28);
        if let Some(examples) = examples {
            sections = sections.push(settings_group(fl!("settings-region-examples"), examples));
        }
        sections
            .push(super::noted_group(
                fl!("settings-region-system"),
                system,
                [fl!("settings-region-system-permission")],
            ))
            .into()
    }

    pub(super) fn region_view(&self) -> Element<'_, Message> {
        self.region
            .view(message, |state| self.region_sections(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Page;
    use crate::region::{Installed, UserLocale};
    use std::sync::Arc;

    fn region(language: &str) -> RegionState {
        let locale = |name: &str, label: &str| Installed {
            name: name.into(),
            label: label.into(),
            unspecified: false,
        };
        RegionState {
            system: None,
            user: UserLocale {
                language: Some(language.into()),
                formats: None,
            },
            session: "en_US.UTF-8".into(),
            installed: Arc::new(vec![
                locale("en_US.UTF-8", "American English (United States)"),
                locale("fr_FR.UTF-8", "French (France)"),
                locale("de_DE.UTF-8", "German (Germany)"),
            ]),
            preview: None,
        }
    }

    fn applied(action: RegionAction, language: &str) -> Message {
        message(Event::Applied(action, Ok(Some(region(language)))))
    }

    #[test]
    fn choosing_a_language_switches_settings_and_names_the_restarts() {
        crate::i18n::isolated(
            concat!(
                module_path!(),
                "::choosing_a_language_switches_settings_and_names_the_restarts"
            ),
            || {
                let _language = crate::i18n::Language::english();
                let (_root, mut state) = crate::ui::tests::fresh();
                let _ = state.update(Message::Page(Page::Region));
                let _ = state.update(applied(
                    RegionAction::UserLanguage("fr_FR.UTF-8".into()),
                    "fr_FR.UTF-8",
                ));
                assert_eq!(canopa_i18n::selection().language().language.as_str(), "fr");
                assert_eq!(canopa_i18n::selection().time.to_string(), "fr-FR");
                // The window's text is French from the next redraw.
                assert_eq!(Page::Region.title(), "Région et langue");
                assert_eq!(fl!("settings-window-title"), "Paramètres Canopa");
                assert!(
                    state
                        .notice
                        .starts_with("Les Paramètres sont maintenant en français."),
                    "{}",
                    state.notice
                );
                assert!(
                    state
                        .notice
                        .contains("Redémarrez Fichiers, Notify et le Shell")
                );
                drop(state.view());

                // Formats apply to Settings' own values, keeping the language.
                let _ = state.update(applied(
                    RegionAction::UserFormats("en_US.UTF-8".into()),
                    "fr_FR.UTF-8",
                ));
                let selection = canopa_i18n::selection();
                assert_eq!(selection.language().language.as_str(), "fr");
                assert_eq!(selection.numbers.to_string(), "en-US");
                assert_eq!(canopa_i18n::format::decimal(1234.5, 1, true), "1,234.5");
                assert!(state.notice.contains("American English (United States)"));
                // The preview follows the chosen formats, not the interface.
                let (_, time, _) = crate::region::sample(&"en-US".parse().unwrap());
                assert_eq!(time, "2:30\u{202f}PM");

                // A language without a translation keeps Settings in English.
                let _ = state.update(applied(
                    RegionAction::UserLanguage("de_DE.UTF-8".into()),
                    "de_DE.UTF-8",
                ));
                assert_eq!(canopa_i18n::selection().language().language.as_str(), "en");
                assert!(
                    state.notice.contains("German (Germany)")
                        && state.notice.contains("stays in English"),
                    "{}",
                    state.notice
                );
                let _ = state.update(applied(
                    RegionAction::UserLanguage("en_US.UTF-8".into()),
                    "en_US.UTF-8",
                ));
                assert!(state.notice.starts_with("Settings now uses English."));
            },
        );
    }
}
