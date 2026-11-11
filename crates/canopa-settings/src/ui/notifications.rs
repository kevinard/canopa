//! The Notifications page: Do Not Disturb (on, for a while, on a schedule,
//! what breaks through), automatic banner suppression, and each
//! application's delivery, sound and lock-screen rule. Canopa Notify owns all
//! of it; changes apply at once.
use super::{Message, State, choice, field, live_page::LivePage, settings_group, toggle};
use crate::{
    live::{Event, LiveAction},
    notifications::{NotificationAction, NotificationService, app_identity, local_time},
};
use canopa_notify_client::{
    DoNotDisturbReason, Suppression,
    preferences::{
        AppPreferences, Delivery, DoNotDisturbPreferences, DoNotDisturbRequest, LockScreen,
        NotificationPreferences, PreferenceSource, Schedule, WEEKDAYS,
    },
};
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, row, text},
};

pub(super) type NotificationsPage = LivePage<NotificationService>;

pub(super) fn message(event: Event<NotificationService>) -> Message {
    Message::Notifications(event)
}

fn apply(action: NotificationAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

const DAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DELIVERY: [(&str, Option<Delivery>); 4] = [
    ("Default", None),
    ("Show banners", Some(Delivery::Allow)),
    ("Notification Center only", Some(Delivery::Silence)),
    ("Mute", Some(Delivery::Drop)),
];
const LOCK_SCREEN: [(&str, LockScreen); 3] = [
    ("Hide", LockScreen::Hidden),
    ("Name only", LockScreen::NameOnly),
    ("Show content", LockScreen::Content),
];
/// Installed applications shown for a search, at most.
const MATCHES: usize = 12;

/// An installed application: its App Identity and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct App {
    pub(super) identity: String,
    pub(super) name: String,
}

/// Installed applications, for naming and finding applications.
pub(super) fn installed() -> anyhow::Result<Vec<App>> {
    let mut apps: Vec<_> = canopa_apps::discover_installed_applications()?
        .into_iter()
        .map(|app| App {
            identity: app_identity(&app.id),
            name: app.name,
        })
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.identity == b.identity);
    Ok(apps)
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    Search(String),
    Installed(Result<Vec<App>, String>),
}

/// `HH:MM` every half hour, plus `current` when it is in between.
fn times(current: Option<&str>) -> Vec<String> {
    let mut times: Vec<String> = (0..48)
        .map(|half| format!("{:02}:{:02}", half / 2, (half % 2) * 30))
        .collect();
    if let Some(current) = current
        && !times.iter().any(|time| time == current)
    {
        times.push(current.to_owned());
        times.sort();
    }
    times
}

fn when(millis: i64) -> String {
    match local_time(millis) {
        Some((clock, true)) => clock,
        Some((clock, false)) => format!("tomorrow {clock}"),
        None => "later".into(),
    }
}

impl State {
    pub(super) fn notifications_edit(&mut self, edit: Edit) -> Task<Message> {
        match edit {
            Edit::Search(query) => self.notification_query = query.chars().take(100).collect(),
            Edit::Installed(Ok(apps)) => self.notification_apps = Some(apps),
            Edit::Installed(Err(_)) => self.notification_apps = Some(Vec::new()),
        }
        Task::none()
    }

    /// The preferences with the change waiting on that control, so a quick
    /// second change builds on the first.
    fn dnd_preferences(&self, state: &NotificationPreferences) -> DoNotDisturbPreferences {
        match self.notifications.live.pending("dnd-preferences") {
            Some(NotificationAction::Preferences(Some(preferences))) => preferences.clone(),
            _ => state.do_not_disturb_preferences.clone(),
        }
    }

    fn set_dnd(
        &self,
        state: &NotificationPreferences,
        change: impl FnOnce(&mut DoNotDisturbPreferences),
    ) -> Message {
        let mut preferences = self.dnd_preferences(state);
        change(&mut preferences);
        apply(NotificationAction::Preferences(Some(preferences)))
    }

    fn do_not_disturb_section<'a>(
        &'a self,
        state: &'a NotificationPreferences,
    ) -> Element<'a, Message> {
        let page = &self.notifications;
        let status = &state.do_not_disturb;
        let active = match page.live.pending("dnd") {
            Some(NotificationAction::DoNotDisturb(request)) => {
                !matches!(request, DoNotDisturbRequest::Off)
            }
            _ => status.active,
        };
        let description = match (status.active, status.reason, status.until) {
            (false, ..) => match status.next_change {
                Some(next) => format!("Off. Turns on at {}.", when(next)),
                None => "Off. Notifications show banners and play sounds.".into(),
            },
            (true, Some(DoNotDisturbReason::Manual), _) => "On until you turn it off.".into(),
            (true, Some(DoNotDisturbReason::Schedule), Some(until)) => {
                format!("On by the schedule until {}.", when(until))
            }
            (true, _, Some(until)) => format!("On until {}.", when(until)),
            (true, ..) => "On.".into(),
        };
        let preferences = self.dnd_preferences(state);
        let mut body = column![
            toggle(
                "notifications-dnd",
                "Do not disturb",
                active,
                apply(NotificationAction::DoNotDisturb(if active {
                    DoNotDisturbRequest::Off
                } else {
                    DoNotDisturbRequest::On
                })),
            ),
            muted(description),
            row![
                self.action(
                    "settings-button-for-1-hour",
                    "For 1 hour",
                    Some(apply(NotificationAction::DoNotDisturb(
                        DoNotDisturbRequest::For { minutes: 60 }
                    ))),
                    false,
                ),
                self.action(
                    "settings-button-for-3-hours",
                    "For 3 hours",
                    Some(apply(NotificationAction::DoNotDisturb(
                        DoNotDisturbRequest::For { minutes: 180 }
                    ))),
                    false,
                ),
                self.action(
                    "settings-button-until-tomorrow-morning",
                    "Until tomorrow morning",
                    Some(apply(NotificationAction::DoNotDisturb(
                        DoNotDisturbRequest::UntilTomorrow
                    ))),
                    false,
                ),
            ]
            .spacing(10),
            page.control_status("dnd", message),
        ]
        .spacing(10);

        // The schedule.
        let scheduled = preferences.schedule.is_some();
        body = body.push(toggle(
            "notifications-schedule",
            "Turn on automatically every day",
            scheduled,
            self.set_dnd(state, |preferences| {
                preferences.schedule = if scheduled {
                    None
                } else {
                    Some(Schedule {
                        start: "22:00".into(),
                        end: "07:00".into(),
                        days: None,
                    })
                }
            }),
        ));
        if let Some(schedule) = &preferences.schedule {
            let start = schedule.start.clone();
            let end = schedule.end.clone();
            let from = choice(
                "notifications-schedule-from",
                times(Some(&start)),
                Some(start.clone()),
                move |time| {
                    self.set_dnd(state, |preferences| {
                        if let Some(schedule) = &mut preferences.schedule {
                            schedule.start = time;
                        }
                    })
                },
            );
            let to = choice(
                "notifications-schedule-to",
                times(Some(&end)),
                Some(end.clone()),
                move |time| {
                    self.set_dnd(state, |preferences| {
                        if let Some(schedule) = &mut preferences.schedule {
                            schedule.end = time;
                        }
                    })
                },
            );
            let mut days = row![].spacing(8);
            for (index, day) in WEEKDAYS.iter().enumerate() {
                let selected = schedule
                    .days
                    .as_ref()
                    .is_none_or(|days| days.iter().any(|d| d == day));
                days = days.push(toggle(
                    format!("notifications-day-{day}"),
                    DAY_LABELS[index],
                    selected,
                    self.set_dnd(state, |preferences| {
                        if let Some(schedule) = &mut preferences.schedule {
                            let mut chosen: Vec<String> = schedule
                                .days
                                .clone()
                                .unwrap_or_else(|| WEEKDAYS.map(str::to_owned).to_vec());
                            if selected {
                                chosen.retain(|d| d != day);
                            } else {
                                chosen.push((*day).to_owned());
                            }
                            chosen.sort_by_key(|d| WEEKDAYS.iter().position(|w| w == d));
                            schedule.days = (chosen.len() < WEEKDAYS.len()).then_some(chosen);
                        }
                    }),
                ));
            }
            body = body.push(
                column![
                    row![text("From"), from, text("to"), to]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    days,
                    muted(if schedule.end <= schedule.start {
                        "The quiet time ends the next morning."
                    } else {
                        "The quiet time ends the same day."
                    }),
                ]
                .spacing(8),
            );
        }
        let alarms = preferences.break_through.alarms;
        let calls = preferences.break_through.calls;
        body = body
            .push(text("While Do not disturb is on, still show"))
            .push(toggle(
                "notifications-alarms",
                "Alarms",
                alarms,
                self.set_dnd(state, |preferences| {
                    preferences.break_through.alarms = !alarms;
                }),
            ))
            .push(toggle(
                "notifications-calls",
                "Incoming calls",
                calls,
                self.set_dnd(state, |preferences| {
                    preferences.break_through.calls = !calls;
                }),
            ))
            .push(page.control_status("dnd-preferences", message));
        match state.do_not_disturb_source {
            PreferenceSource::Configuration => {
                body = body.push(muted(
                    "These come from config.kdl. Changing them here overrides that file.",
                ));
            }
            PreferenceSource::User => {
                body = body.push(self.action(
                    "settings-button-use-the-settings-from-config-kdl",
                    "Use the settings from config.kdl",
                    Some(apply(NotificationAction::Preferences(None))),
                    false,
                ));
            }
            PreferenceSource::Default => {}
        }
        settings_group("Do not disturb", body)
    }

    fn suppression_section<'a>(
        &'a self,
        state: &'a NotificationPreferences,
    ) -> Element<'a, Message> {
        let preferences = self.dnd_preferences(state);
        let (fullscreen, sharing) = (
            preferences.suppress_fullscreen,
            preferences.suppress_screen_sharing,
        );
        let now = match state.suppression {
            Some(Suppression::ScreenSharing) => {
                "Banners are hidden now because the screen is being shared."
            }
            Some(Suppression::Fullscreen) => {
                "Banners are hidden now because an application is fullscreen."
            }
            None => "Notifications still reach the Notification Center.",
        };
        settings_group(
            "Hide banners automatically",
            column![
                toggle(
                    "notifications-fullscreen",
                    "While an application is fullscreen",
                    fullscreen,
                    self.set_dnd(state, |preferences| {
                        preferences.suppress_fullscreen = !fullscreen;
                    }),
                ),
                toggle(
                    "notifications-sharing",
                    "While sharing or recording the screen (content is hidden)",
                    sharing,
                    self.set_dnd(state, |preferences| {
                        preferences.suppress_screen_sharing = !sharing;
                    }),
                ),
                muted(now),
            ]
            .spacing(10),
        )
    }

    /// One application's rules. `name` is the installed name when known.
    fn app_rules<'a>(
        &'a self,
        state: &'a NotificationPreferences,
        identity: &'a str,
        name: Option<&'a str>,
    ) -> Element<'a, Message> {
        let page = &self.notifications;
        let entry = state.app(identity);
        let control = NotificationAction::App {
            identity: identity.to_owned(),
            delivery: None,
            preferences: AppPreferences::default(),
        }
        .control();
        // The values set here, with a change still on its way.
        let (delivery, user) = match page.live.pending(&control) {
            Some(NotificationAction::App {
                delivery,
                preferences,
                ..
            }) => (*delivery, *preferences),
            _ => entry.map_or((None, AppPreferences::default()), |entry| {
                (entry.delivery, entry.user)
            }),
        };
        let sound = user
            .sound
            .unwrap_or_else(|| entry.is_none_or(|entry| entry.sound));
        let lock_screen = user
            .lock_screen
            .unwrap_or_else(|| entry.map_or(LockScreen::NameOnly, |entry| entry.lock_screen));
        let set = move |delivery: Option<Delivery>, preferences: AppPreferences| {
            apply(NotificationAction::App {
                identity: identity.to_owned(),
                delivery,
                preferences,
            })
        };
        let key = identity.to_lowercase();
        let delivery_label = DELIVERY
            .iter()
            .find(|(_, value)| *value == delivery)
            .map(|(label, _)| (*label).to_owned());
        let lock_label = LOCK_SCREEN
            .iter()
            .find(|(_, value)| *value == lock_screen)
            .map(|(label, _)| (*label).to_owned());
        let mut card = column![
            row![
                column![
                    text(name.unwrap_or(identity).to_owned()),
                    muted(identity.to_owned())
                ]
                .spacing(2)
                .width(Length::Fill),
                self.action(
                    "settings-button-reset",
                    "Reset",
                    (delivery.is_some() || user != AppPreferences::default())
                        .then(|| set(None, AppPreferences::default())),
                    false,
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                text("Notifications").width(Length::Fixed(160.)),
                choice(
                    format!("notifications-app-{key}-delivery"),
                    DELIVERY
                        .iter()
                        .map(|(label, _)| (*label).to_owned())
                        .collect(),
                    delivery_label,
                    move |label| {
                        let delivery = DELIVERY
                            .iter()
                            .find(|(name, _)| *name == label)
                            .and_then(|(_, value)| *value);
                        set(delivery, user)
                    },
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            toggle(
                format!("notifications-app-{key}-sound"),
                "Play sounds",
                sound,
                set(
                    delivery,
                    AppPreferences {
                        sound: Some(!sound),
                        ..user
                    }
                ),
            ),
            row![
                text("On the lock screen").width(Length::Fixed(160.)),
                choice(
                    format!("notifications-app-{key}-lock"),
                    LOCK_SCREEN
                        .iter()
                        .map(|(label, _)| (*label).to_owned())
                        .collect(),
                    lock_label,
                    move |label| {
                        let lock_screen = LOCK_SCREEN
                            .iter()
                            .find(|(name, _)| *name == label)
                            .map(|(_, value)| *value);
                        set(
                            delivery,
                            AppPreferences {
                                lock_screen,
                                ..user
                            },
                        )
                    },
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(8);
        if entry.is_some_and(|entry| entry.configured != AppPreferences::default()) {
            card = card.push(muted("config.kdl also sets rules for this application."));
        }
        card = card.push(page.control_status(&control, message));
        iced::widget::container(card)
            .padding(12)
            .width(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }

    fn applications_section<'a>(
        &'a self,
        state: &'a NotificationPreferences,
    ) -> Element<'a, Message> {
        let installed = self.notification_apps.as_deref().unwrap_or_default();
        let name = |identity: &str| {
            installed
                .iter()
                .find(|app| app.identity == identity)
                .map(|app| app.name.as_str())
        };
        let mut body = column![muted(
            "Lock-screen visibility applies to urgent notifications, the only ones the lock screen shows."
        )]
        .spacing(12);
        if state.apps.is_empty() {
            body = body.push(muted(
                "No application has its own rules yet. Find one below to set them.",
            ));
        }
        for entry in &state.apps {
            body = body.push(self.app_rules(state, &entry.app_identity, name(&entry.app_identity)));
        }
        body = body.push(field(
            "notifications-search",
            "Find an application",
            &self.notification_query,
            |value| Message::NotificationsForm(Edit::Search(value)),
        ));
        let query = self.notification_query.trim().to_lowercase();
        if !query.is_empty() {
            let matches: Vec<_> = installed
                .iter()
                .filter(|app| state.app(&app.identity).is_none())
                .filter(|app| {
                    app.name.to_lowercase().contains(&query)
                        || app.identity.to_lowercase().contains(&query)
                })
                .take(MATCHES)
                .collect();
            if matches.is_empty() {
                body = body.push(muted(if self.notification_apps.is_none() {
                    "Reading installed applications…"
                } else {
                    "No other installed application matches."
                }));
            }
            for app in matches {
                body = body.push(self.app_rules(state, &app.identity, Some(&app.name)));
            }
        }
        settings_group("Applications", body)
    }

    pub(super) fn notifications_view(&self) -> Element<'_, Message> {
        self.notifications.view(message, |state| {
            column![
                self.do_not_disturb_section(state),
                self.suppression_section(state),
                self.applications_section(state),
            ]
            .spacing(14)
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_times_include_odd_values() {
        assert_eq!(times(None).len(), 48);
        let odd = times(Some("22:15"));
        assert_eq!(odd.len(), 49);
        assert!(odd.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
