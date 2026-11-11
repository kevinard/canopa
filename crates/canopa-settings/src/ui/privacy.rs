//! The Privacy page: what applications may use through portals (location,
//! camera, microphone, speakers, screenshots, the background, notifications,
//! screen-sharing restore grants), with allow, deny and revoke, and what
//! starts at login.
use super::{Message, State, focus, live_page::LivePage, settings_group, toggle};
use crate::{
    live::{Event, LiveAction},
    privacy::{Access, Grant, PrivacyAction, PrivacyService, PrivacyState, store_missing},
};
use canopa_ui::direction;
use iced::{
    Alignment, Element, Length,
    widget::{column, text},
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

fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    canopa_ui::accessibility::semantic(
        canopa_ui::accessibility::Node::button(label.clone())
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
        // The portal's accuracy level: none, country, city, neighborhood,
        // street or exact.
        (Access::Location, Some(true)) => fl!(
            "settings-privacy-location-allowed",
            accuracy = grant
                .permissions
                .first()
                .map_or("exact", String::as_str)
                .to_lowercase()
        ),
        (Access::ScreenCast | Access::RemoteDesktop, _) => fl!("settings-privacy-restore"),
        (_, Some(true)) => fl!("settings-privacy-allowed"),
        (_, Some(false)) => fl!("settings-privacy-denied"),
        (_, None) => fl!("settings-privacy-asks"),
    }
}

impl State {
    fn grant_row<'a>(&'a self, state: &'a PrivacyState, grant: &'a Grant) -> Element<'a, Message> {
        let page = &self.privacy;
        let control = PrivacyAction::Revoke(grant.clone()).control();
        let key = control.replace(['/', '.'], "-");
        let mut row = vec![
            column![
                text(state.name(&grant.app).to_owned()),
                muted(status(grant))
            ]
            .spacing(2)
            .align_x(direction::start())
            .width(Length::Fill)
            .into(),
        ];
        if grant.access.switchable() {
            let allowed = grant.allowed() == Some(true);
            row.push(row_button(
                format!("privacy-{key}-switch"),
                if allowed {
                    fl!("settings-privacy-deny")
                } else {
                    fl!("settings-privacy-allow")
                },
                Some(apply(PrivacyAction::Set(grant.clone(), !allowed))),
            ));
        }
        row.push(row_button(
            format!("privacy-{key}-revoke"),
            fl!("settings-privacy-forget"),
            Some(apply(PrivacyAction::Revoke(grant.clone()))),
        ));
        column![
            direction::row(row).spacing(10).align_y(Alignment::Center),
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
                    fl!("settings-privacy-permissions"),
                    muted(fl!(
                        "settings-privacy-store-missing-hint",
                        reason = store_missing()
                    )),
                ));
            }
            Some(grants) => {
                let mut list = column![muted(fl!("settings-privacy-permissions-hint"))].spacing(12);
                for access in Access::ALL {
                    let mine: Vec<_> = grants
                        .iter()
                        .filter(|grant| grant.access == access)
                        .collect();
                    let mut section = column![text(access.title()).size(16)].spacing(8);
                    if mine.is_empty() {
                        section = section.push(muted(fl!("settings-privacy-none-asked")));
                    }
                    for grant in mine {
                        section = section.push(self.grant_row(state, grant));
                    }
                    list = list.push(section);
                }
                list = list.push(muted(fl!("settings-privacy-screen-hint")));
                body = body.push(settings_group(fl!("settings-privacy-permissions"), list));
            }
        }
        let mut startup = column![].spacing(8);
        if state.autostart.is_empty() {
            startup = startup.push(muted(fl!("settings-privacy-no-autostart")));
        }
        for entry in &state.autostart {
            let control = PrivacyAction::Autostart(entry.id.clone(), true).control();
            let enabled = match self.privacy.live.pending(&control) {
                Some(PrivacyAction::Autostart(_, value)) => *value,
                _ => entry.enabled,
            };
            let label = if entry.sandboxed {
                fl!(
                    "settings-privacy-autostart-sandboxed",
                    name = entry.name.as_str()
                )
            } else if entry.system {
                fl!(
                    "settings-privacy-autostart-system",
                    name = entry.name.as_str()
                )
            } else {
                entry.name.clone()
            };
            startup = startup
                .push(toggle(
                    format!("privacy-autostart-{}", entry.id.replace('.', "-")),
                    label,
                    enabled,
                    apply(PrivacyAction::Autostart(entry.id.clone(), !enabled)),
                ))
                .push(self.privacy.control_status(&control, message));
        }
        body.push(settings_group(fl!("settings-privacy-autostart"), startup))
            .into()
    }

    pub(super) fn privacy_view(&self) -> Element<'_, Message> {
        self.privacy
            .view(message, |state| self.privacy_sections(state))
    }
}
