//! The Privacy page: what applications may use through portals (location,
//! camera, microphone, speakers, screenshots, the background, notifications,
//! screen-sharing restore grants), with allow, deny and revoke, and what
//! starts at login.
use super::{Message, State, focus, live_page::LivePage, settings_group, toggle};
use crate::{
    live::{Event, LiveAction},
    privacy::{Access, Grant, PrivacyAction, PrivacyService, PrivacyState, STORE_MISSING},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, row, text},
};

pub(super) type PrivacyPage = LivePage<PrivacyService>;

pub(super) fn message(event: Event<PrivacyService>) -> Message {
    Message::Privacy(event)
}

fn apply(action: PrivacyAction) -> Message {
    message(Event::Apply(action))
}

fn muted<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label).size(14).style(canopa_ui::muted).into()
}

fn row_button<'a>(id: String, label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label)
            .key(id.clone())
            .disabled(message.is_none())
            .on_click(message.clone()),
        focus::control(
            id,
            message.clone(),
            None,
            None,
            false,
            canopa_ui::action(text(label), message, canopa_ui::Action::Normal, false)
                .padding([6, 12]),
        ),
    )
}

fn status(grant: &Grant) -> String {
    match (grant.access, grant.allowed()) {
        (Access::Location, Some(true)) => format!(
            "Allowed ({})",
            grant
                .permissions
                .first()
                .map_or("exact", String::as_str)
                .to_lowercase()
        ),
        (Access::ScreenCast | Access::RemoteDesktop, _) => {
            "Can restore its session without asking".into()
        }
        (_, Some(true)) => "Allowed".into(),
        (_, Some(false)) => "Denied".into(),
        (_, None) => "Asks each time".into(),
    }
}

impl State {
    fn grant_row<'a>(&'a self, state: &'a PrivacyState, grant: &'a Grant) -> Element<'a, Message> {
        let page = &self.privacy;
        let control = PrivacyAction::Revoke(grant.clone()).control();
        let key = control.replace(['/', '.'], "-");
        let mut buttons = row![].spacing(8);
        if grant.access.switchable() {
            let allowed = grant.allowed() == Some(true);
            buttons = buttons.push(row_button(
                format!("privacy-{key}-switch"),
                if allowed { "Deny" } else { "Allow" },
                Some(apply(PrivacyAction::Set(grant.clone(), !allowed))),
            ));
        }
        buttons = buttons.push(row_button(
            format!("privacy-{key}-revoke"),
            "Forget",
            Some(apply(PrivacyAction::Revoke(grant.clone()))),
        ));
        column![
            row![
                column![
                    text(state.name(&grant.app).to_owned()),
                    muted(status(grant))
                ]
                .spacing(2)
                .width(Length::Fill),
                buttons,
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            page.control_status(&control, message),
        ]
        .spacing(4)
        .into()
    }

    fn privacy_sections<'a>(&'a self, state: &'a PrivacyState) -> Element<'a, Message> {
        let mut body = column![].spacing(14);
        match &state.grants {
            None => {
                body = body.push(settings_group(
                    "Application permissions",
                    muted(format!(
                        "{STORE_MISSING} It records what sandboxed applications were allowed through portals."
                    )),
                ));
            }
            Some(grants) => {
                let mut list = column![muted(
                    "Sandboxed applications ask through the desktop portal. Forget removes the answer, so the application asks again."
                )]
                .spacing(12);
                for access in Access::ALL {
                    let mine: Vec<_> = grants
                        .iter()
                        .filter(|grant| grant.access == access)
                        .collect();
                    let mut section = column![text(access.title()).size(16)].spacing(8);
                    if mine.is_empty() {
                        section = section.push(muted("No application has asked."));
                    }
                    for grant in mine {
                        section = section.push(self.grant_row(state, grant));
                    }
                    list = list.push(section);
                }
                list = list.push(muted(
                    "Canopa asks every time an application wants to share or control the screen, and does not remember those answers.",
                ));
                body = body.push(settings_group("Application permissions", list));
            }
        }
        let mut startup = column![].spacing(8);
        if state.autostart.is_empty() {
            startup = startup.push(muted("Nothing starts automatically when you log in."));
        }
        for entry in &state.autostart {
            let control = PrivacyAction::Autostart(entry.id.clone(), true).control();
            let enabled = match self.privacy.live.pending(&control) {
                Some(PrivacyAction::Autostart(_, value)) => *value,
                _ => entry.enabled,
            };
            let mut label = entry.name.clone();
            if entry.sandboxed {
                label.push_str(" (added by the application)");
            } else if entry.system {
                label.push_str(" (installed for everyone)");
            }
            startup = startup
                .push(toggle(
                    format!("privacy-autostart-{}", entry.id.replace('.', "-")),
                    label,
                    enabled,
                    apply(PrivacyAction::Autostart(entry.id.clone(), !enabled)),
                ))
                .push(self.privacy.control_status(&control, message));
        }
        body.push(settings_group("Start at login", startup)).into()
    }

    pub(super) fn privacy_view(&self) -> Element<'_, Message> {
        self.privacy
            .view(message, |state| self.privacy_sections(state))
    }
}
