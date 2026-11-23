//! The Notifications page: Do Not Disturb (on, for a while, on a schedule,
//! what breaks through), automatic banner suppression, and each
//! application's delivery, sound and lock-screen rule. Canopa Notify owns all
//! of it; changes apply at once.
use super::{Message, State, chip, field, kit, live_page::LivePage, pick, settings_group, toggle};
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
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length, Task,
    widget::{column, text},
};

pub(super) type NotificationsPage = LivePage<NotificationService>;

pub(super) fn message(event: Event<NotificationService>) -> Message {
    Message::Notifications(event)
}

fn apply(action: NotificationAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(13).style(canopa_ui::muted).into()
}

/// A weekday of the schedule (`mon` … `sun`) in the selected language.
fn day_label(day: &str) -> String {
    match day.parse() {
        Ok(day) => canopa_i18n::format::weekday(day, canopa_i18n::format::Length::Medium),
        Err(_) => day.to_owned(),
    }
}

fn delivery_options() -> Vec<(Option<Delivery>, String)> {
    vec![
        (None, fl!("settings-notifications-delivery-default")),
        (
            Some(Delivery::Allow),
            fl!("settings-notifications-delivery-banners"),
        ),
        (
            Some(Delivery::Silence),
            fl!("settings-notifications-delivery-center"),
        ),
        (
            Some(Delivery::Drop),
            fl!("settings-notifications-delivery-mute"),
        ),
    ]
}

fn lock_screen_options() -> Vec<(LockScreen, String)> {
    vec![
        (LockScreen::Hidden, fl!("settings-notifications-lock-hide")),
        (
            LockScreen::NameOnly,
            fl!("settings-notifications-lock-name"),
        ),
        (
            LockScreen::Content,
            fl!("settings-notifications-lock-content"),
        ),
    ]
}

/// A schedule time (`HH:MM`) as the formats locale writes it.
fn time_label(time: &str) -> String {
    match format!("{time}:00").parse() {
        Ok(time) => canopa_i18n::format::time(time),
        Err(_) => time.to_owned(),
    }
}
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
        Some((clock, true)) => time_label(&clock),
        Some((clock, false)) => fl!("settings-notifications-tomorrow", time = time_label(&clock)),
        None => fl!("settings-notifications-later"),
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
                Some(next) => fl!("settings-dnd-off-until", time = when(next)),
                None => fl!("settings-dnd-off"),
            },
            (true, Some(DoNotDisturbReason::Manual), _) => fl!("settings-dnd-on-manual"),
            (true, Some(DoNotDisturbReason::Schedule), Some(until)) => {
                fl!("settings-dnd-on-schedule", time = when(until))
            }
            (true, _, Some(until)) => fl!("settings-dnd-on-until", time = when(until)),
            (true, ..) => fl!("settings-dnd-on"),
        };
        let preferences = self.dnd_preferences(state);
        let mut body: Vec<Element<'_, Message>> = vec![super::described_toggle(
            "notifications-dnd",
            fl!("settings-dnd"),
            Some(description),
            active,
            apply(NotificationAction::DoNotDisturb(if active {
                DoNotDisturbRequest::Off
            } else {
                DoNotDisturbRequest::On
            })),
        )];
        body.push(
            column![
                text(fl!("settings-dnd-quick")),
                direction::row([
                    self.action(
                        "settings-button-for-1-hour",
                        fl!("settings-dnd-for-hours", hours = 1),
                        Some(apply(NotificationAction::DoNotDisturb(
                            DoNotDisturbRequest::For { minutes: 60 },
                        ))),
                        false,
                    ),
                    self.action(
                        "settings-button-for-3-hours",
                        fl!("settings-dnd-for-hours", hours = 3),
                        Some(apply(NotificationAction::DoNotDisturb(
                            DoNotDisturbRequest::For { minutes: 180 },
                        ))),
                        false,
                    ),
                    self.action(
                        "settings-button-until-tomorrow-morning",
                        fl!("settings-dnd-until-tomorrow"),
                        Some(apply(NotificationAction::DoNotDisturb(
                            DoNotDisturbRequest::UntilTomorrow,
                        ))),
                        false,
                    ),
                ])
                .spacing(8),
                page.control_status("dnd", message),
            ]
            .spacing(8)
            .into(),
        );

        // The schedule.
        let scheduled = preferences.schedule.is_some();
        let mut schedule_rows = column![toggle(
            "notifications-schedule",
            fl!("settings-dnd-schedule"),
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
        )]
        .spacing(12);
        if let Some(schedule) = &preferences.schedule {
            let start = schedule.start.clone();
            let end = schedule.end.clone();
            let labelled = |current: &str| -> Vec<(String, String)> {
                times(Some(current))
                    .into_iter()
                    .map(|time| {
                        let label = time_label(&time);
                        (time, label)
                    })
                    .collect()
            };
            let from = pick(
                "notifications-schedule-from",
                labelled(&start),
                Some(start.clone()),
                move |time| {
                    self.set_dnd(state, |preferences| {
                        if let Some(schedule) = &mut preferences.schedule {
                            schedule.start = time;
                        }
                    })
                },
            );
            let to = pick(
                "notifications-schedule-to",
                labelled(&end),
                Some(end.clone()),
                move |time| {
                    self.set_dnd(state, |preferences| {
                        if let Some(schedule) = &mut preferences.schedule {
                            schedule.end = time;
                        }
                    })
                },
            );
            let mut days = Vec::new();
            for day in &WEEKDAYS {
                let selected = schedule
                    .days
                    .as_ref()
                    .is_none_or(|days| days.iter().any(|d| d == day));
                days.push(chip(
                    format!("notifications-day-{day}"),
                    day_label(day),
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
            schedule_rows = schedule_rows.push(
                column![
                    direction::row([
                        text(fl!("settings-dnd-from")).into(),
                        from,
                        text(fl!("settings-dnd-to")).into(),
                        to,
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center),
                    direction::row(days).spacing(6),
                    muted(if schedule.end <= schedule.start {
                        fl!("settings-dnd-ends-next-morning")
                    } else {
                        fl!("settings-dnd-ends-same-day")
                    }),
                ]
                .spacing(10),
            );
        }
        body.push(schedule_rows.into());
        let alarms = preferences.break_through.alarms;
        let calls = preferences.break_through.calls;
        let mut through: Vec<Element<'_, Message>> = Vec::new();
        through.push(toggle(
            "notifications-alarms",
            fl!("settings-dnd-alarms"),
            alarms,
            self.set_dnd(state, |preferences| {
                preferences.break_through.alarms = !alarms;
            }),
        ));
        through.push(
            column![
                toggle(
                    "notifications-calls",
                    fl!("settings-dnd-calls"),
                    calls,
                    self.set_dnd(state, |preferences| {
                        preferences.break_through.calls = !calls;
                    }),
                ),
                page.control_status("dnd-preferences", message),
            ]
            .into(),
        );
        match state.do_not_disturb_source {
            PreferenceSource::Configuration => {
                through.push(muted(fl!("settings-dnd-from-config")));
            }
            PreferenceSource::User => {
                through.push(self.action(
                    "settings-button-use-the-settings-from-config-kdl",
                    fl!("settings-dnd-use-config"),
                    Some(apply(NotificationAction::Preferences(None))),
                    false,
                ));
            }
            PreferenceSource::Default => {}
        }
        column![
            settings_group(fl!("settings-dnd-section"), kit::rows(body)),
            settings_group(fl!("settings-dnd-break-through"), kit::rows(through)),
        ]
        .spacing(28)
        .into()
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
            Some(Suppression::ScreenSharing) => fl!("settings-suppression-now-sharing"),
            Some(Suppression::Fullscreen) => fl!("settings-suppression-now-fullscreen"),
            None => fl!("settings-suppression-none"),
        };
        super::noted_group(
            fl!("settings-suppression"),
            kit::rows([
                toggle(
                    "notifications-fullscreen",
                    fl!("settings-suppression-fullscreen"),
                    fullscreen,
                    self.set_dnd(state, |preferences| {
                        preferences.suppress_fullscreen = !fullscreen;
                    }),
                ),
                toggle(
                    "notifications-sharing",
                    fl!("settings-suppression-sharing"),
                    sharing,
                    self.set_dnd(state, |preferences| {
                        preferences.suppress_screen_sharing = !sharing;
                    }),
                ),
            ]),
            [now],
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
        let mut card = column![
            direction::row([
                column![
                    super::heading(name.unwrap_or(identity).to_owned(), kit::size::BODY),
                    muted(identity.to_owned())
                ]
                .spacing(2)
                .align_x(direction::start())
                .width(Length::Fill)
                .into(),
                self.action(
                    "settings-button-reset",
                    fl!("settings-notifications-reset"),
                    (delivery.is_some() || user != AppPreferences::default())
                        .then(|| set(None, AppPreferences::default())),
                    false,
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            direction::row([
                text(fl!("settings-notifications-delivery"))
                    .width(Length::Fill)
                    .align_x(direction::text_start())
                    .into(),
                pick(
                    format!("notifications-app-{key}-delivery"),
                    delivery_options(),
                    Some(delivery),
                    move |delivery| set(delivery, user),
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
            toggle(
                format!("notifications-app-{key}-sound"),
                fl!("settings-notifications-sounds"),
                sound,
                set(
                    delivery,
                    AppPreferences {
                        sound: Some(!sound),
                        ..user
                    }
                ),
            ),
            direction::row([
                text(fl!("settings-notifications-lock"))
                    .width(Length::Fill)
                    .align_x(direction::text_start())
                    .into(),
                pick(
                    format!("notifications-app-{key}-lock"),
                    lock_screen_options(),
                    Some(lock_screen),
                    move |lock_screen| {
                        set(
                            delivery,
                            AppPreferences {
                                lock_screen: Some(lock_screen),
                                ..user
                            },
                        )
                    },
                ),
            ])
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .spacing(8);
        if entry.is_some_and(|entry| entry.configured != AppPreferences::default()) {
            card = card.push(muted(fl!("settings-notifications-configured")));
        }
        card = card.push(page.control_status(&control, message));
        card.spacing(10).into()
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
        let mut body: Vec<Element<'_, Message>> = Vec::new();
        if state.apps.is_empty() {
            body.push(super::kit::empty(fl!("settings-notifications-no-rules")));
        }
        for entry in &state.apps {
            body.push(self.app_rules(state, &entry.app_identity, name(&entry.app_identity)));
        }
        body.push(field(
            "notifications-search",
            fl!("settings-notifications-search"),
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
                body.push(muted(if self.notification_apps.is_none() {
                    fl!("settings-notifications-reading-apps")
                } else {
                    fl!("settings-notifications-no-match")
                }));
            }
            for app in matches {
                body.push(self.app_rules(state, &app.identity, Some(&app.name)));
            }
        }
        super::noted_group(
            fl!("settings-notifications-apps"),
            kit::rows(body),
            [fl!("settings-notifications-apps-hint")],
        )
    }

    pub(super) fn notifications_view(&self) -> Element<'_, Message> {
        self.notifications.view(message, |state| {
            column![
                self.do_not_disturb_section(state),
                self.suppression_section(state),
                self.applications_section(state),
            ]
            .spacing(28)
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
