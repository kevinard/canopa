//! Shell presentation of Notify's bounded summary. Notify owns all policy and counts.
use super::render::ModuleRender;
use super::view::{IconLabel, IconLabelLayout};
use crate::app::{Message, messages::ModuleRuntimeEvent};
use crate::config::{NotificationsConfig, Style};
use crate::modules::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceMapper, ModuleDataSourceState, ModuleInstanceOutcome,
};
use crate::modules::{Module, ModuleId, ModuleKind, ModuleState};
use canopa_notify_client::{DoNotDisturbReason, Summary, SummarySubscription, Suppression};

/// "Until 08:00", "Scheduled until 07:00" or "Until turned off", in local time.
fn dnd_detail(summary: &Summary) -> Option<String> {
    if !summary.do_not_disturb {
        return None;
    }
    let until = summary
        .do_not_disturb_until
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|until| {
            let until = until.with_timezone(&chrono::Local);
            if until.date_naive() == chrono::Local::now().date_naive() {
                until.format("%H:%M").to_string()
            } else {
                until.format("%a %H:%M").to_string()
            }
        });
    Some(match (summary.do_not_disturb_reason, until) {
        (Some(DoNotDisturbReason::Schedule), Some(until)) => format!("Scheduled until {until}"),
        (_, Some(until)) => format!("Until {until}"),
        _ => "Until turned off".to_owned(),
    })
}

fn suppression_detail(summary: &Summary) -> Option<&'static str> {
    match summary.suppression? {
        Suppression::Fullscreen => Some("Fullscreen: banners are saved quietly"),
        Suppression::ScreenSharing => Some("Screen sharing: banners are hidden"),
    }
}

pub(super) struct NotificationsModule {
    pub(super) id: ModuleId,
    pub(super) config: NotificationsConfig,
}

impl Module for NotificationsModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn kind(&self) -> ModuleKind {
        ModuleKind::Notifications
    }
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
    fn data_source(&self) -> Option<&dyn ModuleDataSourceMapper> {
        Some(self)
    }
}

impl ModuleDataSourceMapper for NotificationsModule {
    fn data_source_identity(&self) -> ModuleDataSourceIdentity {
        ModuleDataSourceIdentity::Notifications
    }
    fn map_data_source_state(&self, state: &ModuleDataSourceState) -> ModuleInstanceOutcome {
        match state {
            ModuleDataSourceState::Notifications(summary) => {
                ModuleInstanceOutcome::Visible(ModuleState::Notifications(*summary))
            }
            _ => ModuleInstanceOutcome::Unavailable,
        }
    }
}

fn summary(state: &ModuleState) -> Option<Summary> {
    if let ModuleState::Notifications(summary) = state {
        Some(*summary)
    } else {
        None
    }
}

struct Presentation {
    icon: &'static str,
    count: String,
    status: &'static str,
    dnd: bool,
    help: String,
}

impl Presentation {
    fn for_state(state: &ModuleState) -> Self {
        match summary(state) {
            Some(summary) => Self {
                icon: if summary.do_not_disturb {
                    "󰂛"
                } else {
                    "󰂚"
                },
                count: summary.requires_attention.to_string(),
                status: if summary.do_not_disturb {
                    "dnd"
                } else {
                    "available"
                },
                dnd: summary.do_not_disturb,
                help: format!(
                    "{} require attention · DND {}{}{} · Click to open · Right-click to toggle DND",
                    summary.requires_attention,
                    if summary.do_not_disturb { "on" } else { "off" },
                    dnd_detail(&summary)
                        .map(|detail| format!(" ({})", detail.to_lowercase()))
                        .unwrap_or_default(),
                    suppression_detail(&summary)
                        .map(|detail| format!(" · {detail}"))
                        .unwrap_or_default(),
                ),
            },
            None => Self {
                icon: "󰂜",
                count: "?".into(),
                status: "unavailable",
                dnd: false,
                help: "Notifications unavailable · Reconnecting".into(),
            },
        }
    }

    fn values(self) -> Vec<(&'static str, String)> {
        vec![
            ("icon", self.icon.into()),
            ("value", self.count),
            ("state", self.status.into()),
            ("label", self.help),
        ]
    }

    fn label(self) -> String {
        if self.dnd {
            format!("{} · DND", self.count)
        } else {
            self.count
        }
    }
}

impl IconLabel for NotificationsModule {
    fn fallback_view(
        &self,
        state: &ModuleState,
        layout: IconLabelLayout,
        style: &Style,
    ) -> iced::Element<'_, Message> {
        if layout == IconLabelLayout::Collapsed {
            super::view::stacked_icon_label(
                self.icon(state),
                Presentation::for_state(state).count,
                self.color(state),
                style,
            )
        } else {
            super::view::default_icon_label_view(self, state, layout, style)
        }
    }

    fn icon(&self, state: &ModuleState) -> String {
        Presentation::for_state(state).icon.into()
    }
    fn label(&self, state: &ModuleState) -> String {
        Presentation::for_state(state).label()
    }

    fn styled_segments_for_layout(
        &self,
        state: &ModuleState,
        layout: IconLabelLayout,
        style: &Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let values = Presentation::for_state(state)
            .values()
            .into_iter()
            .map(|(key, value)| (key, std::borrow::Cow::Owned(value)))
            .collect::<Vec<_>>();
        super::view::render_format_segments(
            &self.config.format,
            layout.into(),
            &values,
            None,
            style,
        )
    }
    fn color(&self, state: &ModuleState) -> iced::Color {
        match summary(state) {
            None => crate::theme::palette().warning,
            Some(s) if s.do_not_disturb => crate::theme::palette().muted,
            Some(s) if s.requires_attention > 0 => crate::theme::palette().accent,
            _ => crate::theme::palette().text,
        }
    }
}

impl ModuleRender for NotificationsModule {
    fn bar_panel_activation(
        &self,
        state: &ModuleState,
        window: iced::window::Id,
    ) -> Option<super::InteractionEvent> {
        summary(state).map(|_| super::InteractionEvent::RightClicked(window))
    }
    fn render_bar_panel_with_style(
        &self,
        state: &ModuleState,
        _: &Style,
        context: super::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, Message>> {
        let summary = summary(state);
        let description = summary.map_or_else(
            || "Notifications unavailable".to_owned(),
            |s| match (dnd_detail(&s), suppression_detail(&s)) {
                (Some(detail), _) => format!("Banners are paused · {detail}"),
                (None, Some(suppression)) => suppression.to_owned(),
                (None, None) => "Banners are allowed".to_owned(),
            },
        );
        Some(super::view::panel_control(
            include_bytes!("../../assets/moon.svg"),
            "Do Not Disturb",
            description,
            summary.map_or(super::view::PanelControlEnd::Status("Unavailable"), |s| {
                super::view::PanelControlEnd::Toggle(s.do_not_disturb)
            }),
            super::render::PanelModuleContext {
                primary: summary.and(context.primary),
                ..context
            },
        ))
    }

    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn has_content(&self, _state: &ModuleState, _mode: crate::config::FormatMode) -> bool {
        true
    }
    fn tooltip_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        Presentation::for_state(state).values()
    }
}

/// One source shared by every notifications module. No detached tasks or event
/// backlog: dropping the Iced subscription closes its socket and retry timer.
pub(crate) fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::run(|| {
        iced::futures::stream::unfold((None, false), |(connection, retry)| async move {
            let (connection, snapshot) = next(connection, retry).await;
            let retry = connection.is_none();
            Some((
                Message::Modules(ModuleRuntimeEvent::Notifications(snapshot)),
                (connection, retry),
            ))
        })
    })
}

async fn next(
    connection: Option<SummarySubscription>,
    retry: bool,
) -> (Option<SummarySubscription>, Option<Summary>) {
    if let Some(mut connection) = connection {
        return match connection.next().await {
            Ok(Some(snapshot)) => (Some(connection), Some(snapshot)),
            _ => (None, None),
        };
    }
    if retry {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let Ok(socket) = canopa_notify_client::default_socket() else {
        return (None, None);
    };
    match SummarySubscription::connect(&socket).await {
        Ok((connection, snapshot)) => (Some(connection), Some(snapshot)),
        Err(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_pending_and_dnd_remain_visible_and_distinguishable() {
        let module = NotificationsModule {
            id: ModuleId::new("notifications"),
            config: NotificationsConfig::default(),
        };
        for state in [ModuleState::Empty, ModuleState::Unavailable] {
            assert!(module.has_content(&state, crate::config::FormatMode::Collapsed));
            assert_eq!(Presentation::for_state(&state).status, "unavailable");
            assert_eq!(module.label(&state), "?");
        }
        let state = ModuleState::Notifications(Summary {
            instance: [1; 16],
            requires_attention: 12,
            do_not_disturb: true,
            ..Default::default()
        });
        assert_eq!(module.label(&state), "12 · DND");
        assert!(
            Presentation::for_state(&state)
                .help
                .contains("12 require attention · DND on (until turned off)")
        );
    }

    #[test]
    fn timer_schedule_and_suppression_details_are_described() {
        let until = chrono::Local::now() + chrono::Duration::minutes(5);
        let timer = Summary {
            do_not_disturb: true,
            do_not_disturb_reason: Some(DoNotDisturbReason::Timer),
            do_not_disturb_until: Some(until.timestamp_millis()),
            ..Default::default()
        };
        let clock = until.format("%H:%M").to_string();
        let expected = if until.date_naive() == chrono::Local::now().date_naive() {
            format!("Until {clock}")
        } else {
            format!("Until {}", until.format("%a %H:%M"))
        };
        assert_eq!(dnd_detail(&timer).unwrap(), expected);
        let schedule = Summary {
            do_not_disturb_reason: Some(DoNotDisturbReason::Schedule),
            ..timer
        };
        assert!(
            dnd_detail(&schedule)
                .unwrap()
                .starts_with("Scheduled until ")
        );
        let sharing = Summary {
            suppression: Some(Suppression::ScreenSharing),
            ..Default::default()
        };
        assert_eq!(dnd_detail(&sharing), None);
        assert!(
            Presentation::for_state(&ModuleState::Notifications(sharing))
                .help
                .contains("Screen sharing: banners are hidden")
        );
    }
}
