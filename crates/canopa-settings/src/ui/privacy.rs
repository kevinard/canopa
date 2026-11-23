//! The Privacy page: what applications may use through portals (location,
//! camera, microphone, speakers, screenshots, the background, notifications,
//! screen-sharing restore grants), with allow, deny and revoke, and what
//! starts at login.
use super::{
    Message, State, described_toggle, focus, heading, kit, live_page::LivePage, settings_group,
};
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
    text(label).size(13).style(canopa_ui::muted).into()
}

fn treated_button<'a>(
    id: String,
    label: String,
    message: Option<Message>,
    treatment: super::kit::Treatment,
) -> Element<'a, Message> {
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
            iced::widget::button(text(label))
                .on_press_maybe(message)
                .style(move |theme, status| super::kit::button_style(theme, status, treatment))
                .padding([7, 14]),
        ),
    )
}

fn row_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Secondary)
}
/// An action that removes or forgets something.
fn danger_button<'a>(id: String, label: String, message: Option<Message>) -> Element<'a, Message> {
    treated_button(id, label, message, super::kit::Treatment::Destructive)
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
        row.push(danger_button(
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
        let mut body = column![].spacing(28);
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
                let mut list: Vec<Element<'_, Message>> = Vec::new();
                // Only the kinds of access some application asked for.
                for access in Access::ALL {
                    let mine: Vec<_> = grants
                        .iter()
                        .filter(|grant| grant.access == access)
                        .collect();
                    if mine.is_empty() {
                        continue;
                    }
                    let mut section = column![heading(access.title(), kit::size::BODY)].spacing(8);
                    for grant in mine {
                        section = section.push(self.grant_row(state, grant));
                    }
                    list.push(section.into());
                }
                body = body.push(
                    column![
                        if grants.is_empty() {
                            settings_group(
                                fl!("settings-privacy-permissions"),
                                kit::empty(fl!("settings-privacy-nothing-asked-title")),
                            )
                        } else {
                            settings_group(fl!("settings-privacy-permissions"), kit::rows(list))
                        },
                        // How answers are kept: Forget is explained only
                        // where there is something to forget.
                        kit::footnote(if grants.is_empty() {
                            fl!("settings-privacy-screen-hint")
                        } else {
                            format!(
                                "{} {}",
                                fl!("settings-privacy-permissions-hint"),
                                fl!("settings-privacy-screen-hint")
                            )
                        }),
                    ]
                    .spacing(8),
                );
            }
        }
        let mut startup: Vec<Element<'_, Message>> = Vec::new();
        if state.autostart.is_empty() {
            startup.push(super::kit::empty(fl!("settings-privacy-no-autostart")));
        }
        for entry in &state.autostart {
            let control = PrivacyAction::Autostart(entry.id.clone(), true).control();
            let enabled = match self.privacy.live.pending(&control) {
                Some(PrivacyAction::Autostart(_, value)) => *value,
                _ => entry.enabled,
            };
            let origin = if entry.sandboxed {
                Some(fl!("settings-privacy-autostart-sandboxed-hint"))
            } else if entry.system {
                Some(fl!("settings-privacy-autostart-system-hint"))
            } else {
                None
            };
            startup.push(
                column![
                    described_toggle(
                        format!("privacy-autostart-{}", entry.id.replace('.', "-")),
                        entry.name.clone(),
                        origin,
                        enabled,
                        apply(PrivacyAction::Autostart(entry.id.clone(), !enabled)),
                    ),
                    self.privacy.control_status(&control, message),
                ]
                .into(),
            );
        }
        body.push(settings_group(
            fl!("settings-privacy-autostart"),
            kit::rows(startup),
        ))
        .into()
    }

    pub(super) fn privacy_view(&self) -> Element<'_, Message> {
        self.privacy
            .view(message, |state| self.privacy_sections(state))
    }
}
