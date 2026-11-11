//! The Region & language page: the person's language and formats (with a
//! preview), the login screen's language, and the system keyboard layout
//! next to the person's layouts from the Keyboard & pointer page.
//!
//! A new language or formats choice also applies to Settings at once
//! (`canopa_i18n::select_locale`); the window shows it at its next redraw,
//! and a notice names the applications that need a restart.
use super::{Message, State, live_page::LivePage, pick, settings_group};
use crate::{
    Page,
    file_page::FileModel,
    input::Input,
    live::Event,
    region::{RegionAction, RegionService, RegionState, same_locale},
};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, text},
};

pub(super) type RegionPage = LivePage<RegionService>;

pub(super) fn message(event: Event<RegionService>) -> Message {
    Message::Region(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
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
            .align_x(direction::text_start())
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
            direction::row([
                text(title)
                    .width(Length::Fixed(220.))
                    .align_x(direction::text_start())
                    .into(),
                pick(id, options, selected, move |name| {
                    message(Event::Apply(action(name)))
                }),
            ])
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
            personal = personal.push(muted(fl!("settings-region-only-c")));
        }
        personal = personal
            .push(self.locale_choice(
                "region-language",
                fl!("settings-region-language"),
                state,
                state.language(),
                RegionAction::UserLanguage,
            ))
            .push(self.locale_choice(
                "region-formats",
                fl!("settings-region-formats"),
                state,
                state.formats(),
                RegionAction::UserFormats,
            ));
        if let Some(preview) = &state.preview {
            personal = personal.push(
                column![
                    fact(fl!("settings-region-preview-date"), preview.date.clone()),
                    fact(fl!("settings-region-preview-time"), preview.time.clone()),
                    fact(
                        fl!("settings-region-preview-number"),
                        preview.number.clone()
                    ),
                    fact(
                        fl!("settings-region-preview-money"),
                        preview.currency.clone()
                    ),
                    fact(
                        fl!("settings-region-preview-measurement"),
                        if preview.metric {
                            fl!("settings-region-metric")
                        } else {
                            fl!("settings-region-us-customary")
                        }
                    ),
                ]
                .spacing(6),
            );
        }
        personal = personal.push(muted(fl!(
            "settings-region-applies",
            file = "~/.config/locale.conf"
        )));

        let layouts = self
            .input
            .source()
            .and_then(|source| Input::read(source).ok())
            .unwrap_or_else(|| self.input.value.clone());
        let system: Element<'_, Message> = match &state.system {
            None => muted(fl!("settings-region-no-localed")),
            Some(system) => {
                let keyboard = if system.x11_layout.is_empty() {
                    fl!("settings-region-keyboard-unset", layout = "us")
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
                        fl!("settings-region-system-language"),
                        state,
                        system.get("LANG").unwrap_or("C.UTF-8").to_owned(),
                        RegionAction::SystemLanguage,
                    ),
                    fact(fl!("settings-region-system-keyboard"), keyboard),
                    fact(fl!("settings-region-your-layouts"), layouts.layouts.clone()),
                    direction::row([
                        self.action(
                            "settings-button-use-my-layouts-for-the-login-screen-and-console",
                            fl!("settings-region-use-my-layouts"),
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
                            "settings-button-change-my-layouts",
                            fl!("settings-region-change-layouts"),
                            Some(Message::Page(Page::Input)),
                            false,
                        ),
                    ])
                    .spacing(10),
                    self.region.control_status("system-keyboard", message),
                    muted(fl!("settings-region-system-permission")),
                ]
                .spacing(10)
                .into()
            }
        };
        column![
            settings_group(fl!("settings-region-yours"), personal),
            settings_group(fl!("settings-region-system"), system),
        ]
        .spacing(14)
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
