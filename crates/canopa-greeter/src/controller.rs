use crate::{
    accounts::{self, Account},
    config::{Config, valid_user},
    protocol::{Command, Event, Kind},
};
use anyhow::Result;
use canopa_session_ui::view::{
    AuthenticationPanel, Control, ControlKind, Emblem, Prompt, SessionView,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    User,
    Waiting,
    Prompt,
    Canceling,
    Starting,
    Failed,
    Done,
    Fatal,
}
const FIRST_OPTION: usize = 100;
const OPTIONS_PER_PAGE: usize = 3;
const CLOSE_CHOOSER: usize = 30;
const PREVIOUS_PAGE: usize = 31;
const NEXT_PAGE: usize = 32;
/// Shows the username field in place of the account list.
pub const ENTER_USERNAME: usize = 5;
/// Account rows on the sign-in screen: this plus the account's index.
pub const FIRST_ACCOUNT: usize = 200;
/// The sign-in screen lists at most this many accounts; the chooser has the rest.
const LISTED_ACCOUNTS: usize = 4;

#[derive(Clone, Copy)]
pub enum Choice {
    Account,
    Desktop,
}

struct Chooser {
    kind: Choice,
    // Keep the displayed choices tied to their IDs across live catalog updates.
    options: Vec<ChoiceOption>,
    selected: usize,
    current: Option<usize>,
}

struct ChoiceOption {
    key: String,
    title: String,
    detail: Option<String>,
}

pub struct Controller {
    pub ui: SessionView,
    pub phase: Phase,
    pub session: usize,
    pub user: String,
    pub generation: u64,
    started: Instant,
    retry_after: Instant,
    sessions: Vec<crate::config::Session>,
    auth_session: Option<crate::config::Session>,
    pub preferences: crate::preferences::Preferences,
    pub power_menu: crate::power::Menu,
    pub power_capabilities: crate::power::Capabilities,
    chooser: Option<Chooser>,
    session_edited: bool,
    accounts: Vec<Account>,
    user_edited: bool,
    /// A username is being typed, so the field shows instead of the chosen account.
    typing: bool,
    /// The status still shows "Starting authentication…".
    starting: bool,
    /// The startup selection is the remembered last user, not yet signed in as.
    remembered: bool,
    /// The current attempt started by itself and nobody has touched it yet.
    automatic: bool,
    /// Open the power menu once the current attempt's cancellation completes.
    power_after_cancel: bool,
}
impl Controller {
    pub fn new(config: &Config) -> Result<Self> {
        let user = if config.default_user.is_empty() {
            config.users.first().cloned().unwrap_or_default()
        } else {
            config.default_user.clone()
        };
        let mut ui = SessionView::new(&config.view, user.clone())?;
        ui.authentication_panel = Some(AuthenticationPanel {
            height_reserve: 300.,
            heading: fl!("greeter-sign-in"),
            subtitle: String::new(),
            emblem: Emblem::Canopa,
            dialog: false,
            choices: &[(1, Emblem::Accounts), (2, Emblem::Cycle)],
            utilities: &[20, 23, CLOSE_CHOOSER, PREVIOUS_PAGE, NEXT_PAGE],
            disclosure: None,
            list_actions: false,
            checked_control: None,
            choice_details: Default::default(),
            account: None,
            confirm_identity: false,
        });
        let mut this = Self {
            ui,
            phase: Phase::User,
            session: 0,
            user,
            generation: 0,
            started: Instant::now(),
            retry_after: Instant::now(),
            sessions: config.sessions.clone(),
            auth_session: None,
            preferences: Default::default(),
            power_menu: Default::default(),
            power_capabilities: Default::default(),
            chooser: None,
            session_edited: false,
            accounts: accounts::merge(&config.users, Vec::new()),
            user_edited: false,
            typing: false,
            starting: false,
            remembered: false,
            automatic: false,
            power_after_cancel: false,
        };
        this.choose_user(&if config.sessions.is_empty() {
            fl!("greeter-no-sessions-help")
        } else {
            String::new()
        });
        this.refresh(config);
        Ok(this)
    }
    pub fn apply_preferences(
        &mut self,
        config: &Config,
        preferences: crate::preferences::Preferences,
    ) {
        self.preferences = preferences;
        if config.default_user.is_empty()
            && let Some(user) = &self.preferences.last_user
        {
            self.user = user.clone();
            self.choose_user("");
            self.remembered = true;
        }
        self.recall_session();
        self.refresh(config);
    }
    fn recall_session(&mut self) {
        self.session = self
            .preferences
            .sessions
            .get(&self.user)
            .and_then(|id| self.sessions.iter().position(|s| &s.id == id))
            .unwrap_or(0);
    }
    pub fn apply_sessions(&mut self, sessions: Vec<crate::config::Session>) -> Option<Command> {
        if matches!(self.phase, Phase::Starting | Phase::Done) {
            return None;
        }
        let selected = self.sessions.get(self.session).map(|s| s.id.clone());
        let same = self.auth_session.as_ref().is_none_or(|active| {
            sessions.iter().any(|session| {
                session.id == active.id
                    && session.label == active.label
                    && session.argv == active.argv
                    && session.env == active.env
                    && session.desktop_file == active.desktop_file
            })
        });
        self.sessions = sessions;
        self.session = selected
            .and_then(|id| self.sessions.iter().position(|s| s.id == id))
            .unwrap_or(0);
        if !same && matches!(self.phase, Phase::Waiting | Phase::Prompt) {
            self.cancel()
        } else {
            None
        }
    }
    pub fn power_action(&mut self, id: usize) -> Option<crate::power::Action> {
        use crate::power::{Action, Menu};
        match (id, self.power_menu) {
            (20, Menu::Closed) if matches!(self.phase, Phase::User | Phase::Failed) => {
                let action = if self.power_capabilities.shutdown {
                    Action::Shutdown
                } else if self.power_capabilities.restart {
                    Action::Restart
                } else {
                    return None;
                };
                if self.phase == Phase::User {
                    self.user = self.ui.secret.text().to_owned();
                }
                self.ui.secret.clear();
                self.power_menu = Menu::Choose(action);
            }
            (21 | 22, Menu::Choose(_)) => {
                let action = if id == 21 {
                    Action::Shutdown
                } else {
                    Action::Restart
                };
                if self.power_capabilities.allows(action) {
                    self.power_menu = Menu::Confirm(action);
                }
            }
            (24, Menu::Confirm(action)) if self.power_capabilities.allows(action) => {
                self.power_menu = Menu::Pending(action);
                return Some(action);
            }
            (23, Menu::Choose(_) | Menu::Confirm(_)) => {
                self.power_menu = Menu::Closed;
                self.choose_user("");
            }
            _ => {}
        }
        None
    }
    pub fn power_finished(&mut self, success: bool) {
        if matches!(self.power_menu, crate::power::Menu::Pending(_)) {
            self.power_menu = crate::power::Menu::Closed;
            self.choose_user(&if success {
                fl!("greeter-power-accepted")
            } else {
                fl!("greeter-power-denied")
            });
        }
    }
    fn choose_user(&mut self, message: &str) {
        self.phase = Phase::User;
        self.typing = false;
        self.starting = false;
        self.ui.secret.clear();
        self.ui.secret.push(&self.user);
        self.ui.prompt = Some(Prompt {
            id: 0,
            text: fl!("greeter-username-prompt"),
            echo: true,
        });
        self.ui.busy = false;
        self.ui.failed = false;
        self.ui.message = message.into();
    }
    pub fn refresh(&mut self, config: &Config) {
        use crate::power::{Action, Menu};
        self.ui.hide_input = self.power_menu != Menu::Closed || self.chooser.is_some();
        self.ui.primary_control = (!self.ui.hide_input && !self.ui.busy).then_some(3);
        let panel = self
            .ui
            .authentication_panel
            .as_mut()
            .expect("greeter panel");
        panel.list_actions = self.chooser.is_some();
        panel.choice_details.clear();
        panel.checked_control = self
            .chooser
            .as_ref()
            .and_then(|chooser| chooser.current)
            .map(|index| FIRST_OPTION + index);
        (panel.heading, panel.emblem) = match self.chooser.as_ref().map(|chooser| chooser.kind) {
            Some(Choice::Account) => (fl!("greeter-choose-an-account"), Emblem::Accounts),
            Some(Choice::Desktop) => (fl!("greeter-choose-a-desktop"), Emblem::Display),
            None if self.ui.hide_input => (fl!("greeter-power-options"), Emblem::Power),
            None => (fl!("greeter-sign-in"), Emblem::Canopa),
        };
        if let Some(chooser) = &self.chooser {
            let start = chooser.selected / OPTIONS_PER_PAGE * OPTIONS_PER_PAGE;
            let end = (start + OPTIONS_PER_PAGE).min(chooser.options.len());
            self.ui.controls = chooser.options[start..end]
                .iter()
                .enumerate()
                .map(|(offset, option)| {
                    let index = start + offset;
                    let id = FIRST_OPTION + index;
                    if let Some(detail) = &option.detail {
                        panel.choice_details.insert(id, detail.clone());
                    }
                    let mut control = Control::new(id, option.title.clone());
                    control.selected = chooser.selected == index;
                    control
                })
                .collect();
            if start > 0 {
                self.ui
                    .controls
                    .push(Control::new(PREVIOUS_PAGE, fl!("greeter-previous-choices")));
            }
            if end < chooser.options.len() {
                self.ui
                    .controls
                    .push(Control::new(NEXT_PAGE, fl!("greeter-more-choices")));
            }
            self.ui.controls.push(
                Control::new(CLOSE_CHOOSER, fl!("greeter-back-to-sign-in"))
                    .with_kind(ControlKind::Back),
            );
            return;
        }
        if let Menu::Choose(action) | Menu::Confirm(action) | Menu::Pending(action) =
            self.power_menu
        {
            self.ui.prompt = None;
            self.ui.busy = matches!(self.power_menu, Menu::Pending(_));
            self.ui.controls.clear();
            match self.power_menu {
                Menu::Choose(_) => {
                    self.ui.message = fl!("greeter-power-choose-hint");
                    for (id, value) in [(21, Action::Shutdown), (22, Action::Restart)] {
                        if self.power_capabilities.allows(value) {
                            let mut control = Control::new(id, value.label());
                            control.selected = action == value;
                            self.ui.controls.push(control);
                        }
                    }
                }
                Menu::Confirm(_) => {
                    self.ui.message = action.warning();
                    self.ui
                        .controls
                        .push(Control::new(24, action.confirm_label()));
                }
                Menu::Pending(_) => self.ui.message = fl!("greeter-power-pending"),
                Menu::Closed => {}
            }
            if !self.ui.busy {
                self.ui.controls.push(Control::new(23, fl!("greeter-back")));
            }
            return;
        }

        let displayed_user = if self.phase == Phase::User {
            self.ui.secret.text()
        } else {
            &self.user
        };
        let account = self.accounts.iter().find(|a| a.username == displayed_user);
        let chooser_label = if displayed_user.is_empty() {
            fl!("greeter-enter-username-placeholder")
        } else {
            account
                .map(Account::chooser_label)
                .unwrap_or_else(|| displayed_user.to_owned())
        };
        let account_is_username = chooser_label == displayed_user;
        self.ui.username = account
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| displayed_user.to_owned());
        let known = self.phase != Phase::User || account.is_some();
        // A chosen account signs in with one action; typing switches to a username.
        if let Some(panel) = self.ui.authentication_panel.as_mut() {
            panel.confirm_identity = self.phase == Phase::User && !self.typing && account.is_some();
        }
        self.ui.set_avatar_initials(
            (config.avatars_enabled() && known && !self.ui.username.is_empty())
                .then(|| crate::avatars::initials(&self.ui.username)),
        );
        // The form names the account once it is known; otherwise it asks to sign in.
        if known && !self.ui.username.is_empty() {
            let name = self.ui.username.clone();
            panel_heading(&mut self.ui, name);
        }
        let chosen = self.phase == Phase::User && !self.typing && account.is_some();
        let (account_label, account_text) = if chosen {
            (
                fl!(
                    "greeter-change-account-for",
                    account = chooser_label.as_str()
                ),
                fl!("greeter-change-account"),
            )
        } else if self.phase == Phase::User && account_is_username {
            (fl!("greeter-choose-account"), fl!("greeter-choose-account"))
        } else if self.phase == Phase::User {
            (
                fl!(
                    "greeter-choose-account-for",
                    account = chooser_label.as_str()
                ),
                fl!("greeter-choose-account"),
            )
        } else {
            (
                fl!(
                    "greeter-change-account-for",
                    account = chooser_label.as_str()
                ),
                fl!("greeter-change-account"),
            )
        };
        self.ui.controls = vec![
            Control::new(1, account_label)
                .with_kind(ControlKind::AccountChooser { text: account_text }),
            config.sessions.get(self.session).map_or_else(
                || Control::new(2, fl!("greeter-no-sessions")),
                |session| {
                    Control::new(2, fl!("greeter-desktop", session = session.label.as_str()))
                        .with_kind(ControlKind::DesktopChooser {
                            session: session.label.clone(),
                        })
                },
            ),
            Control::new(
                3,
                match self.phase {
                    Phase::User if !self.typing && account.is_some() => fl!("greeter-sign-in"),
                    Phase::User => fl!("greeter-continue"),
                    Phase::Prompt => fl!("greeter-submit-response"),
                    Phase::Failed => fl!("greeter-retry"),
                    Phase::Starting => fl!("greeter-starting-session"),
                    Phase::Canceling => fl!("greeter-cancelling"),
                    _ => fl!("greeter-authenticating"),
                },
            ),
        ];
        if self.phase == Phase::User && self.accounts.is_empty() {
            self.ui.controls.retain(|control| control.id != 1);
        }
        if chosen {
            let current_user = account.map(|a| a.username.clone());
            // The account list: Enter or a row signs in; typing enters a username.
            let current = self
                .accounts
                .iter()
                .position(|a| Some(a.username.as_str()) == current_user.as_deref())
                .unwrap_or(0);
            let start = current
                .saturating_sub(LISTED_ACCOUNTS - 1)
                .min(self.accounts.len().saturating_sub(LISTED_ACCOUNTS));
            let rows = self
                .accounts
                .iter()
                .enumerate()
                .skip(start)
                .take(LISTED_ACCOUNTS)
                .map(|(index, account)| {
                    let label = account.chooser_label();
                    let mut control = Control::new(
                        FIRST_ACCOUNT + index,
                        fl!("greeter-sign-in-as", account = label.as_str()),
                    )
                    .with_kind(ControlKind::Account {
                        name: account.display_name.clone(),
                        detail: (account.display_name != account.username)
                            .then(|| account.username.clone()),
                        initials: crate::avatars::initials(&account.display_name),
                    });
                    control.selected = index == current;
                    control
                });
            let mut controls: Vec<_> = rows.collect();
            controls.push(
                Control::new(ENTER_USERNAME, fl!("greeter-not-listed")).with_kind(
                    ControlKind::AccountChooser {
                        text: fl!("greeter-not-listed"),
                    },
                ),
            );
            self.ui.controls.retain(|control| control.id == 2);
            controls.append(&mut self.ui.controls);
            self.ui.controls = controls;
            self.ui.primary_control = None;
        }
        if config.power_controls
            && matches!(
                self.phase,
                Phase::User | Phase::Failed | Phase::Waiting | Phase::Prompt
            )
            && (self.power_capabilities.shutdown || self.power_capabilities.restart)
        {
            self.ui
                .controls
                .push(Control::new(20, fl!("greeter-power-control")).with_kind(ControlKind::Power));
        }
    }
    pub fn avatar_request(&self, config: &Config) -> (String, Vec<std::path::PathBuf>) {
        let username = if self.phase == Phase::User {
            self.ui.secret.text()
        } else {
            &self.user
        };
        let icon = self
            .accounts
            .iter()
            .find(|a| a.username == username)
            .and_then(|a| a.icon_file.as_deref());
        (username.into(), config.avatars.paths(username, icon))
    }
    pub fn submit(&mut self) -> Option<Command> {
        if self.power_menu != crate::power::Menu::Closed || self.chooser.is_some() {
            return None;
        }
        match self.phase {
            Phase::User | Phase::Failed => {
                if self.sessions.is_empty() {
                    self.ui.message = fl!("greeter-no-sessions-help");
                    return None;
                }
                if Instant::now() < self.retry_after {
                    return None;
                }
                if self.phase == Phase::User {
                    self.user_edited = true;
                    self.user = self.ui.secret.text().to_owned();
                }
                self.ui.secret.clear();
                if !valid_user(&self.user) {
                    self.ui.failed = true;
                    self.ui.message = fl!("greeter-invalid-username");
                    return None;
                }
                if !self.session_edited {
                    self.recall_session();
                }
                self.auth_session = self.sessions.get(self.session).cloned();
                self.generation = self.generation.wrapping_add(1);
                self.started = Instant::now();
                self.phase = Phase::Waiting;
                self.ui.prompt = None;
                self.ui.busy = true;
                self.ui.failed = false;
                self.ui.message = fl!("greeter-starting-authentication");
                self.starting = true;
                Some(Command::Begin {
                    generation: self.generation,
                    user: self.user.clone(),
                })
            }
            Phase::Prompt => {
                self.phase = Phase::Waiting;
                self.ui.prompt = None;
                self.ui.busy = true;
                Some(Command::Answer {
                    generation: self.generation,
                    answer: std::mem::take(&mut self.ui.secret),
                })
            }
            _ => None,
        }
    }
    pub fn cancel(&mut self) -> Option<Command> {
        // A person cancelled: an automatic attempt does not restart itself.
        self.automatic = false;
        match self.phase {
            Phase::Starting | Phase::Done | Phase::Fatal | Phase::Canceling => None,
            Phase::User | Phase::Failed => {
                self.user_edited = true;
                self.user.clear();
                self.choose_user(&fl!("greeter-enter-username"));
                None
            }
            _ => {
                self.generation = self.generation.wrapping_add(1);
                self.phase = Phase::Canceling;
                self.ui.secret.clear();
                self.ui.prompt = None;
                self.ui.busy = true;
                self.ui.message = fl!("greeter-cancelling-authentication");
                self.starting = false;
                Some(Command::Cancel {
                    generation: self.generation,
                })
            }
        }
    }
    pub fn escape(&mut self) -> Option<Command> {
        if self.power_menu != crate::power::Menu::Closed {
            self.power_action(23);
            return None;
        }
        if !self.ui.secret.is_empty() {
            self.clear_input();
            None
        } else {
            self.cancel()
        }
    }
    pub fn session_next(&mut self, config: &Config) -> Option<Command> {
        if config.sessions.is_empty() {
            return None;
        }
        if matches!(
            self.phase,
            Phase::Starting | Phase::Done | Phase::Fatal | Phase::Canceling
        ) {
            return None;
        }
        self.session_edited = true;
        self.session = (self.session + 1) % config.sessions.len();
        if matches!(self.phase, Phase::Waiting | Phase::Prompt) {
            self.cancel()
        } else {
            None
        }
    }
    pub fn user_next(&mut self, previous: bool) {
        if self.phase != Phase::User || self.accounts.is_empty() {
            return;
        }
        let n = self.accounts.len();
        let index = self
            .accounts
            .iter()
            .position(|a| a.username == self.ui.secret.text())
            .map(|i| (i + if previous { n - 1 } else { 1 }) % n)
            .unwrap_or(if previous { n - 1 } else { 0 });
        self.pick_account(index);
    }

    fn pick_account(&mut self, index: usize) {
        self.user_edited = true;
        self.user = self.accounts[index].username.clone();
        self.session_edited = false;
        self.recall_session();
        self.choose_user("");
    }

    pub fn open_chooser(&mut self, kind: Choice) {
        // The desktop can also be changed while a password is asked for.
        let desktop_during_prompt = matches!(kind, Choice::Desktop) && self.phase == Phase::Prompt;
        if (self.phase != Phase::User && !desktop_during_prompt)
            || self.power_menu != crate::power::Menu::Closed
        {
            return;
        }
        self.automatic = false;
        let (options, current) = match kind {
            Choice::Account => (
                self.accounts
                    .iter()
                    .map(|account| ChoiceOption {
                        key: account.username.clone(),
                        title: account.display_name.clone(),
                        detail: (account.username != account.display_name)
                            .then(|| account.username.clone()),
                    })
                    .collect::<Vec<_>>(),
                self.accounts
                    .iter()
                    .position(|account| account.username == self.ui.secret.text()),
            ),
            Choice::Desktop => (
                self.sessions
                    .iter()
                    .map(|session| ChoiceOption {
                        key: session.id.clone(),
                        title: session.label.clone(),
                        detail: session.comment.clone(),
                    })
                    .collect(),
                Some(self.session),
            ),
        };
        if !options.is_empty() {
            let current = current.filter(|index| *index < options.len());
            self.chooser = Some(Chooser {
                kind,
                options,
                selected: current.unwrap_or(0),
                current,
            });
        }
    }

    pub fn choosing(&self) -> bool {
        self.chooser.is_some()
    }

    pub fn close_chooser(&mut self) {
        self.chooser = None;
    }

    pub fn move_choice(&mut self, previous: bool) {
        if let Some(chooser) = &mut self.chooser {
            let count = chooser.options.len();
            chooser.selected = (chooser.selected + if previous { count - 1 } else { 1 }) % count;
        }
    }

    pub fn confirm_choice(&mut self) {
        if let Some(chooser) = &self.chooser {
            self.chooser_action(FIRST_OPTION + chooser.selected);
        }
    }

    pub fn chooser_action(&mut self, id: usize) {
        let Some(chooser) = &mut self.chooser else {
            return;
        };
        let start = chooser.selected / OPTIONS_PER_PAGE * OPTIONS_PER_PAGE;
        match id {
            CLOSE_CHOOSER => self.close_chooser(),
            PREVIOUS_PAGE => chooser.selected = start.saturating_sub(OPTIONS_PER_PAGE),
            NEXT_PAGE => {
                chooser.selected = (start + OPTIONS_PER_PAGE).min(chooser.options.len() - 1)
            }
            id if id >= FIRST_OPTION
                && (self.phase == Phase::User
                    || (self.phase == Phase::Prompt
                        && matches!(chooser.kind, Choice::Desktop))) =>
            {
                let Some(option) = chooser.options.get(id - FIRST_OPTION) else {
                    return;
                };
                let index = match chooser.kind {
                    Choice::Account => self
                        .accounts
                        .iter()
                        .position(|account| account.username == option.key),
                    Choice::Desktop => self
                        .sessions
                        .iter()
                        .position(|session| session.id == option.key),
                };
                let kind = chooser.kind;
                self.close_chooser();
                match (kind, index) {
                    (Choice::Account, Some(index)) => self.pick_account(index),
                    (Choice::Desktop, Some(index)) => {
                        self.session = index;
                        self.session_edited = true;
                        // Chosen during the password step: this attempt starts it.
                        if self.phase == Phase::Prompt {
                            self.auth_session = self.sessions.get(index).cloned();
                        }
                    }
                    _ => self.ui.message = fl!("greeter-choice-unavailable"),
                }
            }
            _ => {}
        }
    }

    /// Begin signing in as the remembered last user, once, so the screen opens
    /// on their password. Returns nothing when there is no remembered user.
    pub fn start_remembered(&mut self) -> Option<Command> {
        if !std::mem::take(&mut self.remembered)
            || self.phase != Phase::User
            || self.typing
            || self.power_menu != crate::power::Menu::Closed
            || self.chooser.is_some()
        {
            return None;
        }
        let command = self.submit();
        self.automatic = command.is_some();
        command
    }

    /// The power control during the password step: cancel the attempt, then
    /// open the power menu.
    pub fn power_from_prompt(&mut self) -> Option<Command> {
        if !matches!(self.phase, Phase::Waiting | Phase::Prompt) {
            return None;
        }
        let command = self.cancel();
        self.power_after_cancel = command.is_some();
        command
    }

    /// Sign in as a listed account: choose it and begin authentication.
    pub fn sign_in_as(&mut self, index: usize) -> Option<Command> {
        if self.phase != Phase::User
            || self.power_menu != crate::power::Menu::Closed
            || index >= self.accounts.len()
        {
            return None;
        }
        // A desktop chosen on this screen applies to the account signed in as.
        let chosen = self.session_edited.then_some(self.session);
        self.pick_account(index);
        if let Some(session) = chosen {
            self.session = session;
            self.session_edited = true;
        }
        self.submit()
    }

    /// Replace the account list with an empty username field.
    pub fn enter_username(&mut self) {
        if self.phase == Phase::User && self.power_menu == crate::power::Menu::Closed {
            self.user_edited = true;
            self.user.clear();
            self.choose_user("");
            self.typing = true;
        }
    }

    pub fn apply_accounts(&mut self, config: &Config, detected: Vec<Account>) {
        self.accounts = accounts::merge(&config.users, detected);
        if self.phase == Phase::User
            && !self.user_edited
            && self.user.is_empty()
            && self.ui.secret.is_empty()
            && let Some(first) = self.accounts.first()
        {
            self.user = first.username.clone();
            let message = self.ui.message.clone();
            self.choose_user(&message);
        }
    }
    pub fn clear_input(&mut self) {
        self.automatic = false;
        if self.phase == Phase::User {
            self.user_edited = true;
            self.typing = true;
        }
        self.ui.secret.clear();
    }
    pub fn input(&mut self, text: &str) {
        self.automatic = false;
        if matches!(self.phase, Phase::User | Phase::Prompt) {
            if self.phase == Phase::User && !text.is_empty() {
                self.user_edited = true;
                // Typing over a chosen account starts a new username.
                if !self.typing {
                    self.ui.secret.clear();
                    self.typing = true;
                }
            }
            self.ui.secret.push(text);
        }
    }
    pub fn backspace(&mut self) {
        self.edit(canopa_session_ui::secret::Edit::Backspace);
    }
    pub fn edit(&mut self, edit: canopa_session_ui::secret::Edit) {
        self.automatic = false;
        if matches!(self.phase, Phase::User | Phase::Prompt) {
            if self.phase == Phase::User {
                self.user_edited = true;
                self.typing = true;
            }
            self.ui.secret.edit(edit);
        }
    }
    pub fn event(&mut self, event: Event, _config: &Config) -> Option<Command> {
        if matches!(event.kind, Kind::Fatal) {
            self.phase = Phase::Fatal;
            self.ui.secret.clear();
            self.ui.prompt = None;
            self.ui.message = fl!("greeter-service-unavailable");
            return None;
        }
        if event.generation != self.generation {
            return None;
        }
        // Any event of this attempt replaces "Starting authentication…".
        let starting = std::mem::take(&mut self.starting);
        match event.kind {
            Kind::Prompt { text, echo } if self.phase == Phase::Waiting => {
                self.ui.secret.clear();
                self.ui.prompt = Some(Prompt {
                    id: event.generation,
                    text,
                    echo,
                });
                self.phase = Phase::Prompt;
                self.ui.busy = false;
                if starting {
                    self.ui.message.clear();
                }
            }
            Kind::Message { text, error } if self.phase == Phase::Waiting => {
                self.ui.message = text;
                self.ui.failed = error;
            }
            Kind::Authenticated if self.phase == Phase::Waiting => {
                self.phase = Phase::Starting;
                self.ui.secret.clear();
                self.ui.prompt = None;
                self.ui.message = fl!("greeter-starting-session");
                let Some(session) = self.auth_session.clone() else {
                    self.phase = Phase::Fatal;
                    return None;
                };
                return Some(Command::Commit {
                    generation: self.generation,
                    session,
                });
            }
            Kind::Started if self.phase == Phase::Starting => {
                self.phase = Phase::Done;
                if let Some(session) = &self.auth_session {
                    self.preferences.record_success(&self.user, &session.id);
                }
            }
            Kind::Cancelled(message) => {
                let restart = self.automatic && !self.power_after_cancel;
                self.automatic = false;
                if std::mem::take(&mut self.power_after_cancel) {
                    self.choose_user("");
                    self.power_action(20);
                } else if restart {
                    // Nobody is at the screen: ask again rather than show a timeout.
                    self.choose_user("");
                    let command = self.submit();
                    self.automatic = command.is_some();
                    return command;
                } else {
                    self.choose_user(&message);
                }
            }
            Kind::Failed(message) => {
                self.retry_after =
                    Instant::now() + Duration::from_secs(2).saturating_sub(self.started.elapsed());
                self.phase = Phase::Failed;
                self.ui.secret.clear();
                self.ui.prompt = None;
                self.ui.busy = false;
                self.ui.failed = true;
                self.ui.message = message;
            }
            _ => {}
        }
        None
    }
}
fn panel_heading(ui: &mut SessionView, heading: String) {
    if let Some(panel) = ui.authentication_panel.as_mut() {
        panel.heading = heading;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_commit_only_after_handoff_and_catalog_changes_cannot_redirect_authentication() {
        let mut config = configured();
        config.users = vec!["alice".into(), "bob".into()];
        let mut other = config.sessions[0].clone();
        other.id = "other".into();
        other.label = "Other".into();
        config.sessions.push(other);
        let mut preferences = crate::preferences::Preferences::default();
        preferences.record_success("bob", "other");
        let mut c = Controller::new(&config).unwrap();
        c.apply_preferences(&config, preferences.clone());
        assert_eq!(c.user, "bob");
        assert_eq!(c.session, 1);
        c.user_next(true);
        assert_eq!(c.user, "alice");
        assert_eq!(c.session, 0);
        assert!(matches!(c.submit(), Some(Command::Begin { .. })));
        let generation = c.generation;
        let commit = c.event(
            Event {
                generation,
                kind: Kind::Authenticated,
            },
            &config,
        );
        assert!(matches!(commit,Some(Command::Commit{session,..}) if session.id=="test"));
        assert_eq!(c.preferences, preferences);
        c.event(
            Event {
                generation,
                kind: Kind::Started,
            },
            &config,
        );
        assert_eq!(c.preferences.last_user.as_deref(), Some("alice"));
        assert_eq!(c.preferences.sessions["alice"], "test");
        let mut c = Controller::new(&config).unwrap();
        c.submit();
        let old = c.generation;
        let mut changed = config.sessions.clone();
        changed[0].comment = Some("Presentation-only update".into());
        assert!(c.apply_sessions(changed.clone()).is_none());
        changed[0].argv = vec!["different-program".into()];
        assert!(matches!(
            c.apply_sessions(changed),
            Some(Command::Cancel { .. })
        ));
        assert!(
            c.event(
                Event {
                    generation: old,
                    kind: Kind::Authenticated
                },
                &config
            )
            .is_none()
        );
        assert!(c.phase == Phase::Canceling);
    }
    #[test]
    fn power_requires_a_permitted_choice_and_separate_confirmation_and_blocks_login_submission() {
        let _english = crate::i18n::english();
        use crate::power::{Action, Capabilities, Menu};
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        assert!(c.power_action(20).is_none());
        assert_eq!(c.power_menu, Menu::Closed);
        c.power_capabilities = Capabilities {
            shutdown: true,
            restart: false,
        };
        assert!(c.power_action(20).is_none());
        assert_eq!(c.power_menu, Menu::Choose(Action::Shutdown));
        assert!(c.submit().is_none());
        assert!(c.ui.secret.is_empty());
        assert!(c.power_action(22).is_none());
        assert_eq!(c.power_menu, Menu::Choose(Action::Shutdown));
        assert!(c.power_action(21).is_none());
        assert_eq!(c.power_menu, Menu::Confirm(Action::Shutdown));
        assert_eq!(c.power_action(24), Some(Action::Shutdown));
        assert!(c.power_action(24).is_none());
        c.power_finished(false);
        assert_eq!(c.power_menu, Menu::Closed);
        assert!(c.ui.message.contains("denied"));
    }
    #[test]
    fn sign_in_screen_is_translated_and_rows_carry_their_meaning() {
        let _french = crate::i18n::French::select();
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        c.refresh(&config);
        let panel = c.ui.authentication_panel.as_ref().unwrap();
        assert_eq!(panel.heading, "Se connecter");
        // A known account is named; the field still holds its username.
        c.apply_accounts(&config, vec![alice()]);
        c.refresh(&config);
        let panel = c.ui.authentication_panel.as_ref().unwrap();
        assert_eq!(panel.heading, "Alice Example");
        assert_eq!(c.ui.secret.text(), "alice");
        assert_eq!(
            c.ui.prompt.as_ref().unwrap().text,
            "Nom d\u{2019}utilisateur\u{a0}:"
        );
        // The account list: rows that sign in, manual entry, then the desktop.
        assert_eq!(
            c.ui.controls[0].label,
            "Se connecter en tant que Alice Example (alice)"
        );
        assert!(c.ui.controls[0].selected);
        assert!(matches!(
            &c.ui.controls[0].kind,
            ControlKind::Account { name, detail: Some(detail), initials }
                if name == "Alice Example" && detail == "alice" && initials == "AE"
        ));
        assert!(matches!(
            &c.ui.controls[1].kind,
            ControlKind::AccountChooser { text } if text == "Compte absent\u{a0}?"
        ));
        assert_eq!(c.ui.controls[2].label, "Bureau\u{a0}: Test");
        assert_eq!(
            c.ui.controls[2].kind,
            ControlKind::DesktopChooser {
                session: "Test".into()
            }
        );
        // Typing a username shows the field and the account chooser link.
        c.input("b");
        c.refresh(&config);
        assert!(matches!(
            &c.ui.controls[0].kind,
            ControlKind::AccountChooser { text } if text == "Choisir un compte"
        ));
        c.open_chooser(Choice::Account);
        c.refresh(&config);
        let back = c.ui.controls.last().unwrap();
        assert_eq!(back.kind, ControlKind::Back);
        assert_eq!(back.label, "Retour à la connexion");
        assert!(c.ui.controls[0].selected);
    }
    #[test]
    fn a_chosen_account_signs_in_with_one_action_and_typing_starts_a_new_username() {
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice()]);
        c.refresh(&config);
        let confirming =
            |c: &Controller| c.ui.authentication_panel.as_ref().unwrap().confirm_identity;
        assert!(confirming(&c));
        assert_eq!(c.ui.secret.text(), "alice");
        // Typing replaces the chosen account's username and shows the field.
        c.input("b");
        c.refresh(&config);
        assert!(!confirming(&c));
        assert_eq!(c.ui.secret.text(), "b");
        c.input("ob");
        assert_eq!(c.ui.secret.text(), "bob");
        // Choosing the account again returns to the one-action sign-in.
        c.user_next(false);
        c.refresh(&config);
        assert!(confirming(&c));
        assert_eq!(c.ui.secret.text(), "alice");
        assert!(matches!(c.submit(), Some(Command::Begin { user, .. }) if user == "alice"));
    }
    #[test]
    fn listed_accounts_sign_in_directly_and_manual_entry_starts_empty() {
        let mut config = configured();
        let mut other = config.sessions[0].clone();
        other.id = "other".into();
        config.sessions.push(other);
        let bob = Account {
            username: "bob".into(),
            display_name: "Bob Example".into(),
            uid: Some(1001),
            icon_file: None,
        };
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice(), bob]);
        c.refresh(&config);
        // An explicitly chosen desktop survives signing in from a row.
        c.session_next(&config);
        let desktop = c.session;
        assert_eq!(desktop, 1);
        assert!(matches!(c.sign_in_as(1), Some(Command::Begin { user, .. }) if user == "bob"));
        assert_eq!(c.session, desktop);
        assert!(
            c.sign_in_as(0).is_none(),
            "an attempt is already in progress"
        );
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice()]);
        c.refresh(&config);
        assert!(c.sign_in_as(5).is_none());
        c.enter_username();
        c.refresh(&config);
        assert!(c.ui.secret.is_empty());
        assert!(!c.ui.authentication_panel.as_ref().unwrap().confirm_identity);
        assert!(
            c.ui.controls
                .iter()
                .all(|control| !matches!(control.kind, ControlKind::Account { .. }))
        );
    }
    fn remembered(config: &Config) -> Controller {
        let mut preferences = crate::preferences::Preferences::default();
        preferences.record_success("bob", "other");
        let mut c = Controller::new(config).unwrap();
        c.apply_preferences(config, preferences);
        c
    }
    fn two_desktops() -> Config {
        let mut config = configured();
        let mut other = config.sessions[0].clone();
        other.id = "other".into();
        config.sessions.push(other);
        config
    }
    fn prompt(c: &mut Controller, config: &Config) {
        let generation = c.generation;
        c.event(
            Event {
                generation,
                kind: Kind::Prompt {
                    text: "Password:".into(),
                    echo: false,
                },
            },
            config,
        );
    }
    fn cancelled(c: &mut Controller, config: &Config) -> Option<Command> {
        let generation = c.generation;
        c.event(
            Event {
                generation,
                kind: Kind::Cancelled("Authentication timed out".into()),
            },
            config,
        )
    }
    #[test]
    fn a_remembered_user_opens_on_their_password_once() {
        let config = two_desktops();
        let mut c = remembered(&config);
        assert!(matches!(c.start_remembered(), Some(Command::Begin { user, .. }) if user == "bob"));
        assert!(c.start_remembered().is_none(), "only once per startup");
        prompt(&mut c, &config);
        // Escape returns to the account list and nothing starts by itself again.
        assert!(matches!(c.cancel(), Some(Command::Cancel { .. })));
        assert!(cancelled(&mut c, &config).is_none());
        assert_eq!(c.phase, Phase::User);
        assert!(c.start_remembered().is_none());
        // No remembered user, or an administrator's default user: no automatic start.
        assert!(
            Controller::new(&config)
                .unwrap()
                .start_remembered()
                .is_none()
        );
        let mut fixed = config.clone();
        fixed.default_user = "alice".into();
        assert!(remembered(&fixed).start_remembered().is_none());
    }
    #[test]
    fn an_untouched_automatic_attempt_restarts_quietly_but_a_touched_one_does_not() {
        let config = two_desktops();
        let mut c = remembered(&config);
        c.start_remembered();
        prompt(&mut c, &config);
        let first = c.generation;
        // The greetd deadline passed with nobody at the screen.
        let restart = cancelled(&mut c, &config);
        assert!(
            matches!(restart, Some(Command::Begin { user, generation }) if user == "bob" && generation > first)
        );
        assert_ne!(c.ui.message, "Authentication timed out");
        assert!(!c.ui.failed);
        prompt(&mut c, &config);
        c.input("x");
        assert!(cancelled(&mut c, &config).is_none());
        assert_eq!(c.ui.message, "Authentication timed out");
        assert_eq!(c.phase, Phase::User);
    }
    #[test]
    fn power_and_desktop_stay_available_during_the_password_step() {
        let config = two_desktops();
        let mut c = remembered(&config);
        c.power_capabilities = crate::power::Capabilities {
            shutdown: true,
            restart: true,
        };
        c.start_remembered();
        prompt(&mut c, &config);
        c.refresh(&config);
        assert!(c.ui.controls.iter().any(|control| control.id == 20));
        // The desktop chooser applies to this attempt without cancelling it.
        c.open_chooser(Choice::Desktop);
        assert!(c.choosing());
        c.chooser_action(FIRST_OPTION);
        assert_eq!(c.phase, Phase::Prompt);
        c.input("password");
        assert!(matches!(c.submit(), Some(Command::Answer { .. })));
        let generation = c.generation;
        let commit = c.event(
            Event {
                generation,
                kind: Kind::Authenticated,
            },
            &config,
        );
        assert!(matches!(commit, Some(Command::Commit { session, .. }) if session.id == "test"));
        // Power cancels the attempt first, then opens its menu.
        let mut c = remembered(&config);
        c.power_capabilities = crate::power::Capabilities {
            shutdown: true,
            restart: true,
        };
        c.start_remembered();
        prompt(&mut c, &config);
        assert!(matches!(
            c.power_from_prompt(),
            Some(Command::Cancel { .. })
        ));
        assert!(
            cancelled(&mut c, &config).is_none(),
            "no restart after a person acted"
        );
        assert!(matches!(c.power_menu, crate::power::Menu::Choose(_)));
    }
    fn configured() -> Config {
        let (mut config, _) = Config::load(&canopa_config::Request::Defaults).unwrap();
        config.sessions.push(crate::config::Session {
            id: "test".into(),
            label: "Test".into(),
            argv: vec!["true".into()],
            env: Vec::new(),
            comment: None,
            desktop_file: None,
        });
        config
    }
    fn alice() -> Account {
        Account {
            username: "alice".into(),
            display_name: "Alice Example".into(),
            uid: Some(1000),
            icon_file: None,
        }
    }
    #[test]
    fn chooser_focus_does_not_change_the_current_account_before_confirmation() {
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice(), Account::manual("bob")]);
        c.open_chooser(Choice::Account);
        c.move_choice(false);
        c.refresh(&config);

        assert_eq!(
            c.ui.authentication_panel.as_ref().unwrap().checked_control,
            Some(FIRST_OPTION)
        );
        assert_eq!(c.ui.secret.text(), "alice");
        c.confirm_choice();
        assert_eq!(c.ui.secret.text(), "bob");
    }

    #[test]
    fn avatar_sources_follow_canonical_selection_and_never_read_password_input() {
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        let mut a = alice();
        a.icon_file = Some("/faces/alice.png".into());
        let mut b = Account::manual("bob");
        b.icon_file = Some("/faces/bob.png".into());
        c.apply_accounts(&config, vec![a, b]);
        assert_eq!(
            c.avatar_request(&config).1[0],
            std::path::PathBuf::from("/faces/alice.png")
        );
        c.user_next(false);
        assert_eq!(c.avatar_request(&config).0, "bob");
        c.submit();
        c.event(
            Event {
                generation: c.generation,
                kind: Kind::Prompt {
                    text: "Password".into(),
                    echo: false,
                },
            },
            &config,
        );
        c.input("private-password");
        c.apply_accounts(&config, vec![alice()]);
        c.refresh(&config);
        assert_eq!(c.avatar_request(&config).0, "bob");
        assert_eq!(c.ui.secret.text(), "private-password");
        assert!(c.phase == Phase::Prompt);
    }
    #[test]
    fn discovered_display_names_never_replace_the_canonical_authentication_username_or_active_prompt()
     {
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice()]);
        c.refresh(&config);
        assert_eq!(c.ui.username, "Alice Example");
        assert!(c.ui.controls[0].label.contains("alice"));
        assert!(matches!(c.submit(),Some(Command::Begin{user,..}) if user=="alice"));
        let generation = c.generation;
        c.event(
            Event {
                generation,
                kind: Kind::Prompt {
                    text: "Password:".into(),
                    echo: false,
                },
            },
            &config,
        );
        c.input("test-response");
        c.apply_accounts(&config, Vec::new());
        assert!(c.phase == Phase::Prompt);
        assert_eq!(c.generation, generation);
        assert_eq!(c.user, "alice");
        assert_eq!(c.ui.secret.text(), "test-response");
    }
    #[test]
    fn late_discovery_preserves_typed_default_and_explicitly_cleared_usernames() {
        let config = configured();
        let mut c = Controller::new(&config).unwrap();
        c.input("remote-user");
        c.apply_accounts(&config, vec![alice()]);
        assert_eq!(c.ui.secret.text(), "remote-user");
        c.user_next(false);
        assert_eq!(c.ui.secret.text(), "alice");
        let mut c = Controller::new(&config).unwrap();
        c.clear_input();
        c.apply_accounts(&config, vec![alice()]);
        assert!(c.ui.secret.is_empty());
        let mut config = config;
        config.default_user = "configured-default".into();
        let mut c = Controller::new(&config).unwrap();
        c.apply_accounts(&config, vec![alice()]);
        assert_eq!(c.ui.secret.text(), "configured-default");
    }
    #[test]
    fn cancelled_or_old_success_cannot_commit_a_desktop_and_new_prompts_clear_secrets() {
        let (mut config, _) = Config::load(&canopa_config::Request::Defaults).unwrap();
        config.sessions.push(crate::config::Session {
            id: "test".into(),
            label: "Test".into(),
            argv: vec!["true".into()],
            env: Vec::new(),
            comment: None,
            desktop_file: None,
        });
        let mut c = Controller::new(&config).unwrap();
        c.ui.secret.clear();
        c.input("alice");
        assert!(matches!(c.submit(), Some(Command::Begin { .. })));
        let first = c.generation;
        c.event(
            Event {
                generation: first,
                kind: Kind::Prompt {
                    text: "Password".into(),
                    echo: false,
                },
            },
            &config,
        );
        c.input("secret");
        c.submit();
        c.event(
            Event {
                generation: first,
                kind: Kind::Prompt {
                    text: "OTP".into(),
                    echo: true,
                },
            },
            &config,
        );
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.prompt.as_ref().unwrap().echo);
        let cancel = c.cancel();
        assert!(matches!(cancel, Some(Command::Cancel { .. })));
        assert!(
            c.event(
                Event {
                    generation: first,
                    kind: Kind::Authenticated
                },
                &config
            )
            .is_none()
        );
        assert!(c.phase == Phase::Canceling);
        c.event(
            Event {
                generation: c.generation,
                kind: Kind::Cancelled("cancelled".into()),
            },
            &config,
        );
        assert!(c.phase == Phase::User);
    }
}
