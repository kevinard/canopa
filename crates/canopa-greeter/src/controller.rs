use crate::{
    accounts::{self, Account},
    config::{Config, valid_user},
    protocol::{Command, Event, Kind},
};
use anyhow::Result;
use canopa_session_ui::view::{Control, Prompt, SessionView};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, PartialEq, Eq)]
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
pub struct Controller {
    pub ui: SessionView,
    pub phase: Phase,
    pub session: usize,
    pub user: String,
    pub generation: u64,
    started: Instant,
    retry_after: Instant,
    has_sessions: bool,
    accounts: Vec<Account>,
    user_edited: bool,
}
impl Controller {
    pub fn new(config: &Config) -> Result<Self> {
        let user = if config.default_user.is_empty() {
            config.users.first().cloned().unwrap_or_default()
        } else {
            config.default_user.clone()
        };
        let mut this = Self {
            ui: SessionView::new(&config.view, user.clone())?,
            phase: Phase::User,
            session: 0,
            user,
            generation: 0,
            started: Instant::now(),
            retry_after: Instant::now(),
            has_sessions: !config.sessions.is_empty(),
            accounts: accounts::merge(&config.users, Vec::new()),
            user_edited: false,
        };
        this.choose_user(if config.sessions.is_empty() {
            "No desktop sessions available. Install a session or configure one."
        } else {
            "Enter: continue · Tab: session · ↑/↓: user"
        });
        this.refresh(config);
        Ok(this)
    }
    fn choose_user(&mut self, message: &str) {
        self.phase = Phase::User;
        self.ui.secret.clear();
        self.ui.secret.push(&self.user);
        self.ui.prompt = Some(Prompt {
            id: 0,
            text: "Username:".into(),
            echo: true,
        });
        self.ui.busy = false;
        self.ui.failed = false;
        self.ui.message = message.into();
    }
    pub fn refresh(&mut self, config: &Config) {
        let displayed_user = if self.phase == Phase::User {
            self.ui.secret.text()
        } else {
            &self.user
        };
        let account = self.accounts.iter().find(|a| a.username == displayed_user);
        let chooser_label = if displayed_user.is_empty() {
            "enter username".into()
        } else {
            account
                .map(Account::chooser_label)
                .unwrap_or_else(|| displayed_user.to_owned())
        };
        self.ui.username = account
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| displayed_user.to_owned());
        self.ui.set_avatar_initials(
            config
                .avatars_enabled()
                .then(|| crate::avatars::initials(&self.ui.username)),
        );
        self.ui.controls = vec![
            Control {
                id: 1,
                label: format!("User: {chooser_label}"),
            },
            Control {
                id: 2,
                label: config
                    .sessions
                    .get(self.session)
                    .map(|s| format!("Session: {}   ‹ Tab ›", s.label))
                    .unwrap_or_else(|| "No desktop sessions available".into()),
            },
            Control {
                id: 3,
                label: match self.phase {
                    Phase::User => "Continue",
                    Phase::Prompt => "Submit response",
                    Phase::Failed => "Retry",
                    Phase::Starting => "Starting session…",
                    Phase::Canceling => "Cancelling…",
                    _ => "Authenticating…",
                }
                .into(),
            },
        ];
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
        match self.phase {
            Phase::User | Phase::Failed => {
                if !self.has_sessions {
                    self.ui.message =
                        "No desktop sessions available. Install a session or configure one.".into();
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
                    self.ui.message = "Enter a valid username".into();
                    return None;
                }
                self.generation = self.generation.wrapping_add(1);
                self.started = Instant::now();
                self.phase = Phase::Waiting;
                self.ui.prompt = None;
                self.ui.busy = true;
                self.ui.failed = false;
                self.ui.message = "Starting authentication…".into();
                Some(Command::Begin {
                    generation: self.generation,
                    user: self.user.clone(),
                })
            }
            Phase::Prompt => {
                self.phase = Phase::Waiting;
                self.ui.prompt = None;
                Some(Command::Answer {
                    generation: self.generation,
                    answer: std::mem::take(&mut self.ui.secret),
                })
            }
            _ => None,
        }
    }
    pub fn cancel(&mut self) -> Option<Command> {
        match self.phase {
            Phase::Starting | Phase::Done | Phase::Fatal | Phase::Canceling => None,
            Phase::User | Phase::Failed => {
                self.user_edited = true;
                self.user.clear();
                self.choose_user("Enter a username");
                None
            }
            _ => {
                self.generation = self.generation.wrapping_add(1);
                self.phase = Phase::Canceling;
                self.ui.secret.clear();
                self.ui.prompt = None;
                self.ui.busy = true;
                self.ui.message = "Cancelling authentication…".into();
                Some(Command::Cancel {
                    generation: self.generation,
                })
            }
        }
    }
    pub fn escape(&mut self) -> Option<Command> {
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
        self.user_edited = true;
        let n = self.accounts.len();
        let index = self
            .accounts
            .iter()
            .position(|a| a.username == self.ui.secret.text())
            .map(|i| (i + if previous { n - 1 } else { 1 }) % n)
            .unwrap_or(if previous { n - 1 } else { 0 });
        self.user = self.accounts[index].username.clone();
        self.choose_user("Enter: continue · Tab: session · ↑/↓: user");
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
        if self.phase == Phase::User {
            self.user_edited = true;
        }
        self.ui.secret.clear();
    }
    pub fn input(&mut self, text: &str) {
        if matches!(self.phase, Phase::User | Phase::Prompt) {
            if self.phase == Phase::User && !text.is_empty() {
                self.user_edited = true;
            }
            self.ui.secret.push(text);
        }
    }
    pub fn backspace(&mut self) {
        if matches!(self.phase, Phase::User | Phase::Prompt) {
            if self.phase == Phase::User {
                self.user_edited = true;
            }
            self.ui.secret.pop();
        }
    }
    pub fn event(&mut self, event: Event, config: &Config) -> Option<Command> {
        if matches!(event.kind, Kind::Fatal) {
            self.phase = Phase::Fatal;
            self.ui.secret.clear();
            self.ui.prompt = None;
            self.ui.message = "Login service unavailable".into();
            return None;
        }
        if event.generation != self.generation {
            return None;
        }
        match event.kind {
            Kind::Prompt { text, echo } if self.phase == Phase::Waiting => {
                self.ui.secret.clear();
                self.ui.prompt = Some(Prompt {
                    id: event.generation,
                    text,
                    echo,
                });
                self.phase = Phase::Prompt;
                if self.ui.message == "Starting authentication…" {
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
                self.ui.message = "Starting session…".into();
                let Some(session) = config.sessions.get(self.session).cloned() else {
                    self.phase = Phase::Fatal;
                    return None;
                };
                return Some(Command::Commit {
                    generation: self.generation,
                    session,
                });
            }
            Kind::Started if self.phase == Phase::Starting => self.phase = Phase::Done,
            Kind::Cancelled(message) => self.choose_user(&message),
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
#[cfg(test)]
mod tests {
    use super::*;
    fn configured() -> Config {
        let (mut config, _) = Config::load(&canopa_config::Request::Defaults).unwrap();
        config.sessions.push(crate::config::Session {
            id: "test".into(),
            label: "Test".into(),
            argv: vec!["true".into()],
            env: Vec::new(),
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
