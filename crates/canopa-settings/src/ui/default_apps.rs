//! The Default applications page: the web browser, mail, calendar, music,
//! video, photos, text editor, file manager and terminal, and any other file
//! type found by name or extension. Each choice is saved at once.
use super::{Message, State, field, live_page::LivePage, pick, settings_group};
use crate::{
    default_apps::{CATEGORIES, Choice, DefaultAction, DefaultApps, DefaultAppsService},
    live::{Event, LiveAction},
};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length,
    widget::{column, text},
};

pub(super) type DefaultAppsPage = LivePage<DefaultAppsService>;

pub(super) fn message(event: Event<DefaultAppsService>) -> Message {
    Message::DefaultApps(event)
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

/// Other file types shown for a search, at most.
const MATCHES: usize = 10;

/// A choice's applications by ID, with their menu labels; equal names get
/// their IDs.
fn labels(choice: &Choice) -> Vec<(String, String)> {
    choice
        .options
        .iter()
        .map(|option| {
            let twins = choice
                .options
                .iter()
                .filter(|other| other.name == option.name)
                .count();
            let label = if twins > 1 {
                format!("{} ({})", option.name, option.id)
            } else {
                option.name.clone()
            };
            (option.id.clone(), label)
        })
        .collect()
}

impl State {
    /// One labelled choice of application, applying `action` for the chosen ID.
    fn app_choice<'a>(
        &'a self,
        id: String,
        title: String,
        current: &Choice,
        action: impl Fn(String) -> DefaultAction + 'a,
    ) -> Element<'a, Message> {
        let page = &self.default_apps;
        let probe = action(String::new()).control();
        let selected_id = match page.live.pending(&probe) {
            Some(
                DefaultAction::Category(_, chosen)
                | DefaultAction::Type(_, chosen)
                | DefaultAction::Terminal(chosen),
            ) => Some(chosen.clone()),
            _ => current.current.clone(),
        };
        let options = labels(current);
        let control: Element<'_, Message> = if options.is_empty() {
            muted(fl!("settings-default-apps-none"))
        } else {
            pick(id, options, selected_id, move |chosen| {
                message(Event::Apply(action(chosen)))
            })
        };
        column![
            direction::row([
                text(title)
                    .width(Length::Fixed(180.))
                    .align_x(direction::text_start())
                    .into(),
                control,
            ])
            .spacing(12)
            .align_y(Alignment::Center),
            page.control_status(&probe, message),
        ]
        .spacing(4)
        .into()
    }

    fn default_apps_sections<'a>(&'a self, state: &'a DefaultApps) -> Element<'a, Message> {
        let mut defaults = column![].spacing(10);
        for (category, current) in CATEGORIES.iter().zip(&state.categories) {
            let key = category.key;
            defaults = defaults.push(self.app_choice(
                format!("default-apps-{key}"),
                category.title(),
                current,
                move |id| DefaultAction::Category(key, id),
            ));
        }
        defaults = defaults
            .push(self.app_choice(
                "default-apps-terminal".into(),
                fl!("settings-default-apps-terminal"),
                &state.terminal,
                DefaultAction::Terminal,
            ))
            .push(muted(fl!("settings-default-apps-terminal-hint")));
        let mut others = column![field(
            "default-apps-search",
            fl!("settings-default-apps-search"),
            &self.default_apps_query,
            Message::DefaultAppsSearch,
        )]
        .spacing(10);
        let query = self.default_apps_query.trim();
        if !query.is_empty() {
            let found = state.catalog.search(query, MATCHES);
            if found.is_empty() {
                others = others.push(muted(fl!("settings-default-apps-no-match")));
            }
            for (mime, current) in found {
                let chosen = mime.clone();
                others = others.push(self.app_choice(
                    format!("default-apps-type-{mime}"),
                    mime,
                    &current,
                    move |id| DefaultAction::Type(chosen.clone(), id),
                ));
            }
        } else {
            others = others.push(muted(fl!("settings-default-apps-search-hint")));
        }
        column![
            settings_group(fl!("settings-default-apps"), defaults),
            settings_group(fl!("settings-default-apps-others"), others),
            muted(fl!(
                "settings-default-apps-saved-in",
                file = "~/.config/mimeapps.list"
            )),
        ]
        .spacing(14)
        .into()
    }

    pub(super) fn default_apps_view(&self) -> Element<'_, Message> {
        self.default_apps
            .view(message, |state| self.default_apps_sections(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_apps::AppOption;

    #[test]
    fn equal_names_are_told_apart() {
        let choice = Choice {
            current: None,
            options: vec![
                AppOption {
                    id: "a.desktop".into(),
                    name: "Editor".into(),
                },
                AppOption {
                    id: "b.desktop".into(),
                    name: "Editor".into(),
                },
                AppOption {
                    id: "c.desktop".into(),
                    name: "Other".into(),
                },
            ],
        };
        let labels: Vec<_> = labels(&choice)
            .into_iter()
            .map(|(_, label)| label)
            .collect();
        assert_eq!(
            labels,
            ["Editor (a.desktop)", "Editor (b.desktop)", "Other"]
        );
    }
}
