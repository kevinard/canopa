//! Suspend, hibernate, reboot and power off from menus and buttons.
//!
//! Requests go to logind (see [`crate::session`]). Its `Can*` answers decide
//! which actions the Shell shows; a confirmation names applications holding a
//! block inhibitor; and failures are reported through Notify.
use std::borrow::Cow;
use std::collections::HashMap;

use iced::Task;

use super::{App, Message};
use crate::config::{MenuConfig, MenuItemConfig, PanelAction};
use crate::notice::NoticeSlot;
use crate::session::{Capabilities, Failure, Inhibitor, SessionAction};

#[derive(Debug, Clone)]
pub enum Event {
    Capabilities(Capabilities),
    Blockers(SessionAction, Vec<Inhibitor>),
    Finished(SessionAction, Result<(), Failure>),
}

#[derive(Debug, Default)]
pub(super) struct SessionRuntime {
    pub(super) capabilities: Capabilities,
    pub(super) blockers: HashMap<SessionAction, Vec<Inhibitor>>,
    /// One request at a time: a second click while polkit is asking is ignored.
    pub(super) pending: Option<SessionAction>,
}

fn item_actions(items: &[MenuItemConfig], out: &mut Vec<SessionAction>) {
    for item in items {
        if let Some(PanelAction::Session(action)) = item.action {
            out.push(action);
        }
        item_actions(&item.children, out);
    }
}

fn menu_actions(menu: &MenuConfig) -> Vec<SessionAction> {
    let mut actions = Vec::new();
    item_actions(&menu.items, &mut actions);
    actions
}

fn retain_offered(items: &mut Vec<MenuItemConfig>, capabilities: &Capabilities) {
    items.retain(|item| {
        !matches!(item.action, Some(PanelAction::Session(action)) if !capabilities.offers(action))
    });
    for item in items {
        retain_offered(&mut item.children, capabilities);
    }
}

/// Menus as shown: items for refused session actions are left out. A free
/// function so callers can borrow the configuration beside the foreground.
pub(super) fn offered_menus<'a>(
    menus: &'a [MenuConfig],
    capabilities: &Capabilities,
) -> Cow<'a, [MenuConfig]> {
    let refused = |menu: &MenuConfig| {
        menu_actions(menu)
            .into_iter()
            .any(|action| !capabilities.offers(action))
    };
    if !menus.iter().any(refused) {
        return Cow::Borrowed(menus);
    }
    Cow::Owned(
        menus
            .iter()
            .cloned()
            .map(|mut menu| {
                retain_offered(&mut menu.items, capabilities);
                menu
            })
            .collect(),
    )
}

async fn system(connection: Option<zbus::Connection>) -> Option<zbus::Connection> {
    match connection {
        Some(connection) => Some(connection),
        None => crate::session::system().await,
    }
}

impl App {
    fn uses_session_actions(&self) -> bool {
        self.config
            .menus
            .iter()
            .any(|menu| !menu_actions(menu).is_empty())
            || self.config.modules.iter().any(|module| {
                module
                    .actions
                    .bindings
                    .iter()
                    .any(|binding| matches!(binding.action, PanelAction::Session(_)))
            })
    }

    pub(super) fn refresh_session_capabilities(&self) -> Task<Message> {
        if !self.uses_session_actions() {
            return Task::none();
        }
        self.query_session_capabilities()
    }

    /// Re-reads `Can*` even when no menu or button offers a session action;
    /// the launcher's session rows need the answers too.
    fn query_session_capabilities(&self) -> Task<Message> {
        let connection = self.shared.dbus.system.clone();
        Task::perform(
            async move {
                match system(connection).await {
                    Some(connection) => crate::session::capabilities(&connection).await,
                    None => Capabilities::default(),
                }
            },
            |capabilities| Message::Session(Event::Capabilities(capabilities)),
        )
    }

    /// Opening Quick settings re-reads `Can*`; opening a confirmation lists
    /// the applications that would block it.
    pub(super) fn refresh_session_state_for(
        &mut self,
        content: &crate::surface::panel::PanelContent,
    ) -> Task<Message> {
        use crate::surface::panel::PanelContent;
        match content {
            PanelContent::BarPanel => self.refresh_session_capabilities(),
            PanelContent::Menu(name) => {
                let Some(menu) = self.config.menus.iter().find(|menu| &menu.name == name) else {
                    return Task::none();
                };
                if matches!(
                    menu.source.as_ref().map(|source| &source.kind),
                    Some(crate::config::MenuSourceKind::Launcher(_))
                ) {
                    return self.query_session_capabilities();
                }
                let mut actions = menu_actions(menu);
                actions.dedup();
                let connection = self.shared.dbus.system.clone();
                Task::batch(actions.into_iter().map(|action| {
                    // Never show a previous answer for a new confirmation.
                    self.session.blockers.remove(&action);
                    let connection = connection.clone();
                    Task::perform(
                        async move {
                            let Some(connection) = system(connection).await else {
                                return Vec::new();
                            };
                            crate::session::blockers(&connection, action)
                                .await
                                .unwrap_or_default()
                        },
                        move |held| Message::Session(Event::Blockers(action, held)),
                    )
                }))
            }
            _ => Task::none(),
        }
    }

    pub(super) fn request_session_action(&mut self, action: SessionAction) -> Task<Message> {
        if self.session.pending.is_some() {
            return Task::none();
        }
        self.session.pending = Some(action);
        tracing::info!(target: "canopa_shell::session", action = action.id(), "requesting session action from logind");
        let connection = self.shared.dbus.system.clone();
        Task::perform(
            async move {
                match system(connection).await {
                    Some(connection) => crate::session::request(&connection, action).await,
                    None => Err(Failure::Unavailable),
                }
            },
            move |result| Message::Session(Event::Finished(action, result)),
        )
    }

    pub(super) fn handle_session_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Capabilities(capabilities) => {
                self.session.capabilities = capabilities;
                Task::none()
            }
            Event::Blockers(action, held) => {
                self.session.blockers.insert(action, held);
                Task::none()
            }
            Event::Finished(action, result) => {
                if self.session.pending == Some(action) {
                    self.session.pending = None;
                }
                match result {
                    Ok(()) => {
                        self.notices.clear(NoticeSlot::Session);
                        Task::none()
                    }
                    Err(failure) => {
                        tracing::warn!(target: "canopa_shell::session", action = action.id(), ?failure, "session action failed");
                        self.notices.show(
                            NoticeSlot::Session,
                            crate::session::notice(&failure, action),
                        );
                        if let Failure::Blocked(held) = failure {
                            self.session.blockers.insert(action, held);
                        }
                        // logind may have changed its mind (for example a new
                        // inhibitor or a removed swap device).
                        self.refresh_session_capabilities()
                    }
                }
            }
        }
    }

    /// Whether a module's click leads to an action logind offers. A button
    /// whose session action, directly or through its confirmation menu, is
    /// refused is hidden.
    pub(super) fn module_offered(&self, id: &crate::modules::ModuleId) -> bool {
        let capabilities = &self.session.capabilities;
        match self
            .modules
            .action_for(id, &crate::config::ActionTrigger::Click)
        {
            Some(PanelAction::Session(action)) => capabilities.offers(*action),
            Some(PanelAction::OpenMenu(name) | PanelAction::ToggleMenu(name)) => {
                let actions = self
                    .config
                    .menus
                    .iter()
                    .find(|menu| &menu.name == name)
                    .map(menu_actions)
                    .unwrap_or_default();
                actions.is_empty()
                    || actions
                        .into_iter()
                        .any(|action| capabilities.offers(action))
            }
            _ => true,
        }
    }

    /// A confirmation's prompt, followed by the applications blocking it.
    pub(super) fn session_prompt(
        &self,
        content: &crate::surface::panel::PanelContent,
        prompt: Option<String>,
    ) -> Option<String> {
        let crate::surface::panel::PanelContent::Menu(name) = content else {
            return prompt;
        };
        let held: Vec<_> = self
            .config
            .menus
            .iter()
            .find(|menu| &menu.name == name)
            .map(menu_actions)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|action| self.session.blockers.get(&action))
            .flatten()
            .cloned()
            .collect();
        if held.is_empty() {
            return prompt;
        }
        let blocked = format!("Blocked by {}.", crate::session::blockers_sentence(&held));
        Some(match prompt {
            Some(prompt) => format!("{prompt}\n{blocked}"),
            None => blocked,
        })
    }
}

#[cfg(test)]
mod tests;
