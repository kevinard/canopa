//! The Default applications page: the web browser, mail, calendar, music,
//! video, photos, text editor, file manager and terminal, and any other file
//! type found by name or extension. Each choice is saved at once.
use super::{Message, State, choice, field_id, live_page::LivePage, settings_group};
use crate::{
    default_apps::{CATEGORIES, Choice, DefaultAction, DefaultApps, DefaultAppsService},
    live::{Event, LiveAction},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, row, text},
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

/// Menu labels for a choice's applications; equal names get their IDs.
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
            (label, option.id.clone())
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
        let labels = labels(current);
        let selected = selected_id.and_then(|chosen| {
            labels
                .iter()
                .find(|(_, option)| *option == chosen)
                .map(|(label, _)| label.clone())
        });
        let control: Element<'_, Message> = if labels.is_empty() {
            muted("No installed application opens these.")
        } else {
            let options: Vec<String> = labels.iter().map(|(label, _)| label.clone()).collect();
            choice(id, options, selected, move |label| {
                let chosen = labels
                    .iter()
                    .find(|(name, _)| *name == label)
                    .map(|(_, id)| id.clone())
                    .unwrap_or_default();
                message(Event::Apply(action(chosen)))
            })
        };
        column![
            row![text(title).width(Length::Fixed(180.)), control]
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
                category.title.into(),
                current,
                move |id| DefaultAction::Category(key, id),
            ));
        }
        defaults = defaults
            .push(self.app_choice(
                "default-apps-terminal".into(),
                "Terminal".into(),
                &state.terminal,
                DefaultAction::Terminal,
            ))
            .push(muted(
                "Programs that run in a terminal open in this one, through xdg-terminal-exec. Without a choice, the first installed terminal is used.",
            ));
        let mut others = column![field_id(
            "default-apps-search",
            "File type or extension",
            &self.default_apps_query,
            Message::DefaultAppsSearch,
        )]
        .spacing(10);
        let query = self.default_apps_query.trim();
        if !query.is_empty() {
            let found = state.catalog.search(query, MATCHES);
            if found.is_empty() {
                others = others.push(muted("No installed application opens a matching type."));
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
            others = others.push(muted(
                "Type a file type such as application/pdf, or an extension such as pdf.",
            ));
        }
        column![
            settings_group("Default applications", defaults),
            settings_group("Other file types", others),
            muted("Choices are saved in ~/.config/mimeapps.list, which other desktops and applications read too."),
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
            .map(|(label, _)| label)
            .collect();
        assert_eq!(
            labels,
            ["Editor (a.desktop)", "Editor (b.desktop)", "Other"]
        );
    }
}
