use crate::native::{Event, Runtime};
use canopa_session_ui::{
    settings::Settings,
    view::{
        AuthenticationPanel, Control, Emblem, Prompt, REQUEST_PAGE_LINES, SessionView,
        request_lines, request_summary,
    },
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Idle,
    Waiting,
    Prompt,
    Failed,
    Cancelling,
}
pub struct Controller {
    pub ui: SessionView,
    pub phase: Phase,
    pub generation: u64,
    pub serial: u64,
    uids: Vec<u32>,
    selected: usize,
    identity: Option<crate::identity::Identity>,
    retry_after: Instant,
    action: String,
    description: String,
    subject_pid: Option<u32>,
    policy_publisher: String,
    notice: String,
    prompt: Option<Prompt>,
    details: Option<usize>,
}
impl Controller {
    pub fn new(settings: &Settings) -> anyhow::Result<Self> {
        let mut ui = SessionView::new(settings, String::new())?;
        ui.authentication_panel = Some(AuthenticationPanel {
            height_reserve: 0.,
            heading: "Authorize system change",
            eyebrow: "SYSTEM AUTHORIZATION",
            emblem: Emblem::Shield,
            dialog: true,
            choices: &[(1, Emblem::Cycle)],
            utilities: &[4],
            disclosure: Some(4),
            list_actions: false,
            checked_control: None,
            choice_details: Default::default(),
        });
        Ok(Self {
            ui,
            phase: Phase::Idle,
            generation: 0,
            serial: 0,
            uids: Vec::new(),
            selected: 0,
            identity: None,
            retry_after: Instant::now(),
            action: String::new(),
            description: String::new(),
            subject_pid: None,
            policy_publisher: String::new(),
            notice: String::new(),
            prompt: None,
            details: None,
        })
    }
    pub fn selected_uid(&self) -> Option<u32> {
        self.uids
            .get(self.selected)
            .copied()
            .filter(|_| self.active())
    }
    pub fn identity(&mut self, mut identity: crate::identity::Identity) {
        if Some(identity.uid) == self.selected_uid() {
            self.ui.set_user_avatar(
                identity
                    .rgba
                    .take()
                    .and_then(|bytes| image::RgbaImage::from_raw(128, 128, bytes))
                    .map(std::sync::Arc::new),
            );
            self.identity = Some(identity);
            self.refresh();
        }
    }
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Begin {
                generation,
                action,
                message,
                subject_pid,
                uids,
                selected,
            } if generation > self.generation => {
                self.identity = None;
                self.ui.set_user_avatar(None);
                self.generation = generation;
                self.serial = 0;
                self.uids = uids;
                self.selected = selected.min(self.uids.len().saturating_sub(1));
                self.ui.secret.clear();
                self.prompt = None;
                self.action = action;
                self.subject_pid = subject_pid;
                self.policy_publisher.clear();
                self.description = message
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect();
                self.details = None;
                self.phase = Phase::Waiting;
                self.notice = "Waiting for authentication…".into();
                self.ui.failed = false;
            }
            Event::Waiting {
                generation,
                serial,
                selected,
            } if self.active()
                && generation == self.generation
                && serial >= self.serial
                && self.phase != Phase::Cancelling =>
            {
                self.serial = serial;
                self.selected = selected.min(self.uids.len().saturating_sub(1));
                self.phase = Phase::Waiting;
                self.ui.secret.clear();
                self.prompt = None;
                self.notice = "Waiting for authentication…".into();
                self.ui.failed = false;
            }
            Event::Prompt {
                generation,
                serial,
                text,
                echo,
            } if self.active()
                && generation == self.generation
                && serial > self.serial
                && self.phase != Phase::Cancelling =>
            {
                self.serial = serial;
                self.ui.secret.clear();
                self.prompt = Some(Prompt {
                    id: serial,
                    text,
                    echo,
                });
                self.phase = Phase::Prompt;
                self.notice.clear();
                self.ui.failed = false;
            }
            Event::Info {
                generation,
                text,
                error,
            } if self.active()
                && generation == self.generation
                && self.phase != Phase::Cancelling =>
            {
                self.notice = text;
                self.ui.failed = error;
            }
            Event::Failed { generation }
                if self.active()
                    && generation == self.generation
                    && self.phase != Phase::Cancelling =>
            {
                self.ui.secret.clear();
                self.prompt = None;
                self.phase = Phase::Failed;
                self.retry_after = Instant::now() + Duration::from_secs(2);
                self.notice = "Authentication failed. Try again.".into();
                self.ui.failed = true;
            }
            Event::PolicyPublisher { generation, name }
                if generation == self.generation && self.active() =>
            {
                self.policy_publisher = name
                    .chars()
                    .filter(|character| !character.is_control())
                    .collect::<String>()
                    .trim()
                    .to_owned();
            }
            Event::Closed { generation } if generation == self.generation => self.clear(),
            _ => {}
        }
        self.refresh();
    }
    fn clear(&mut self) {
        self.phase = Phase::Idle;
        self.ui.secret.clear();
        self.prompt = None;
        self.ui.custom.clear();
        self.action.clear();
        self.description.clear();
        self.subject_pid = None;
        self.policy_publisher.clear();
        self.details = None;
        self.notice.clear();
        self.ui.username.clear();
        self.uids.clear();
        self.identity = None;
        self.ui.set_user_avatar(None);
    }
    pub fn refresh(&mut self) {
        self.ui.interaction_epoch = (self.generation, self.serial);
        let panel = self.ui.authentication_panel.as_mut().expect("dialog panel");
        panel.disclosure = self.details.is_none().then_some(4);
        panel.heading = if self.details.is_some() {
            "Request details"
        } else {
            "Authorize system change"
        };
        self.ui.custom.clear();
        self.ui.prompt = if self.details.is_none() {
            self.prompt.as_ref().map(|p| Prompt {
                id: p.id,
                text: p.text.clone(),
                echo: p.echo,
            })
        } else {
            None
        };
        let (display, page_count) = if let Some(page) = self.details {
            let account = self
                .identity
                .as_ref()
                .filter(|i| Some(i.uid) == self.selected_uid())
                .map(crate::identity::Identity::label)
                .or_else(|| self.selected_uid().map(|uid| format!("UID {uid}")))
                .unwrap_or_default();
            let process = self
                .subject_pid
                .map_or_else(String::new, |pid| format!("Request subject: Process {pid}"));
            let publisher = if self.policy_publisher.is_empty() {
                String::new()
            } else {
                format!("Policy publisher: {}", self.policy_publisher)
            };
            let full = format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                self.action,
                self.description,
                account,
                publisher,
                process,
                self.prompt.as_ref().map(|p| p.text.as_str()).unwrap_or(""),
                self.notice
            );
            let lines = request_lines(&full);
            let count = lines.len().div_ceil(REQUEST_PAGE_LINES).max(1);
            (
                lines
                    .into_iter()
                    .skip((page % count) * REQUEST_PAGE_LINES)
                    .take(REQUEST_PAGE_LINES)
                    .collect::<Vec<_>>(),
                count,
            )
        } else {
            self.ui.custom.insert("action".into(), self.action.clone());
            if !self.description.is_empty()
                && !self.description.contains('\n')
                && self.description.chars().count() <= 40
            {
                self.ui
                    .custom
                    .insert("request-title".into(), self.description.clone());
                (Vec::new(), 1)
            } else {
                (request_summary(&self.description), 1)
            }
        };
        for (index, part) in display.chunks(4).enumerate() {
            self.ui
                .custom
                .insert(format!("message-{index}"), part.join("\n"));
        }
        self.ui.message = if self.details.is_some() {
            "Details · Page Up/Down: page · Enter: back".into()
        } else {
            self.notice.clone()
        };
        let identity = self
            .identity
            .as_ref()
            .filter(|i| Some(i.uid) == self.selected_uid());
        let label = if let Some(identity) = identity {
            if identity.name.is_empty() {
                identity.username.clone()
            } else {
                identity.name.clone()
            }
        } else if let Some(uid) = self.selected_uid() {
            if uid == 0 {
                "root".into()
            } else if uid == unsafe { libc::getuid() } {
                "Your account".into()
            } else {
                format!("User ID {uid}")
            }
        } else {
            String::new()
        };
        self.ui.username = label.clone();
        let account = format!("Account: {label}");
        if self.uids.len() == 1 && self.details.is_none() {
            self.ui.custom.insert("account".into(), account.clone());
            if let Some(pid) = self.subject_pid {
                self.ui
                    .custom
                    .insert("subject-process".into(), format!("Process {pid}"));
            }
            if !self.policy_publisher.is_empty() {
                self.ui
                    .custom
                    .insert("policy-publisher".into(), self.policy_publisher.clone());
            }
        }
        self.ui.busy = self.phase == Phase::Waiting || self.details.is_some();
        self.ui.primary_control = (matches!(self.phase, Phase::Prompt | Phase::Failed)
            && self.details.is_none())
        .then_some(2);
        self.ui.controls = vec![
            Control {
                id: 2,
                label: if self.phase == Phase::Failed {
                    "Retry"
                } else if self.phase == Phase::Prompt && self.details.is_none() {
                    "Authenticate"
                } else {
                    "Waiting…"
                }
                .into(),
            },
            Control {
                id: 3,
                label: "Cancel".into(),
            },
            Control {
                id: 4,
                label: "Policy details".into(),
            },
        ];
        if self.uids.len() > 1 && self.details.is_none() {
            self.ui.controls.insert(
                0,
                Control {
                    id: 1,
                    label: account,
                },
            );
        }
        if let Some(page) = self.details {
            self.ui.controls = vec![
                Control {
                    id: 2,
                    label: "Back to authentication".into(),
                },
                Control {
                    id: 4,
                    label: format!("Next page ({}/{page_count})", page % page_count + 1),
                },
                Control {
                    id: 3,
                    label: "Cancel".into(),
                },
            ];
        }
    }
    pub fn input(&mut self, text: &str) {
        if self.phase == Phase::Prompt && self.details.is_none() {
            self.ui.secret.push(text);
        }
    }
    pub fn backspace(&mut self) {
        self.edit(canopa_session_ui::secret::Edit::Backspace);
    }
    pub fn edit(&mut self, edit: canopa_session_ui::secret::Edit) {
        if self.phase == Phase::Prompt && self.details.is_none() {
            self.ui.secret.edit(edit);
        }
    }
    pub fn clear_input(&mut self) {
        self.ui.secret.clear();
    }
    pub fn submit(&mut self) {
        if self.details.take().is_some() {
            self.refresh();
            return;
        }
        if self.phase == Phase::Prompt && self.details.is_none() {
            let secret = std::mem::take(&mut self.ui.secret);
            Runtime::command(self.generation, self.serial, 3, 0, Some(&secret));
            self.phase = Phase::Waiting;
            self.prompt = None;
            self.notice = "Authenticating…".into();
        } else if self.phase == Phase::Failed && Instant::now() >= self.retry_after {
            Runtime::command(self.generation, self.serial, 2, self.selected, None);
            self.serial += 1;
            self.phase = Phase::Waiting;
            self.notice = "Authenticating…".into();
            self.ui.failed = false;
        }
        self.refresh();
    }
    pub fn select(&mut self) {
        if self.details.is_some()
            || self.uids.len() < 2
            || !matches!(self.phase, Phase::Prompt | Phase::Failed)
            || (self.phase == Phase::Failed && Instant::now() < self.retry_after)
        {
            return;
        }
        self.identity = None;
        self.ui.set_user_avatar(None);
        self.selected = (self.selected + 1) % self.uids.len();
        self.ui.secret.clear();
        self.prompt = None;
        Runtime::command(self.generation, self.serial, 2, self.selected, None);
        self.serial += 1;
        self.phase = Phase::Waiting;
        self.refresh();
    }
    pub fn cancel(&mut self) {
        if !self.active() || self.phase == Phase::Cancelling {
            return;
        }
        self.ui.secret.clear();
        self.prompt = None;
        self.details = None;
        self.phase = Phase::Cancelling;
        self.notice = "Cancelling authentication…".into();
        Runtime::command(self.generation, self.serial, 1, 0, None);
        self.refresh();
    }
    pub fn details(&mut self) {
        if !self.active() || self.phase == Phase::Cancelling {
            return;
        }
        self.ui.secret.clear();
        self.details = Some(self.details.map(|p| p + 1).unwrap_or(0));
        self.refresh();
    }
    pub fn previous_page(&mut self) {
        if let Some(page) = &mut self.details {
            *page = page.saturating_sub(1);
            self.refresh();
        }
    }
    pub fn preview(settings: &Settings) -> anyhow::Result<Self> {
        let mut c = Self::new(settings)?;
        c.event(Event::Begin {
            generation: 1,
            action: "org.freedesktop.NetworkManager.settings.modify.system".into(),
            message: "Change DNS servers".into(),
            subject_pid: Some(2481),
            uids: vec![unsafe { libc::getuid() }],
            selected: 0,
        });
        c.event(Event::Prompt {
            generation: 1,
            serial: 1,
            text: "Password:".into(),
            echo: false,
        });
        c.event(Event::PolicyPublisher {
            generation: 1,
            name: "NetworkManager".into(),
        });
        c.identity(crate::identity::Identity {
            uid: unsafe { libc::getuid() },
            username: "alex".into(),
            name: "Alex Morgan".into(),
            rgba: None,
        });
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller() -> Controller {
        let (settings, _) = crate::config::load(&canopa_config::Request::Defaults).unwrap();
        let mut controller = Controller::preview(&settings).unwrap();
        controller.identity = None;
        controller.refresh();
        controller
    }
    #[test]
    fn metadata_never_adds_an_identity_or_changes_its_uid() {
        let mut c = controller();
        let uid = c.selected_uid().unwrap();
        let data = |uid, name: &str| crate::identity::Identity {
            uid,
            username: "alice".into(),
            name: name.into(),
            rgba: None,
        };
        c.identity(data(uid.wrapping_add(1), "Foreign"));
        assert!(!c.ui.username.contains("Foreign"));
        c.identity(data(uid, "Alice Example"));
        assert!(c.ui.username.contains("Alice Example"));
        assert_eq!(c.selected_uid(), Some(uid));
        c.event(Event::Closed {
            generation: c.generation,
        });
        c.identity(data(uid, "Stale"));
        assert!(c.ui.username.is_empty());
    }
    #[test]
    fn fresh_prompts_clear_input_and_closed_or_cancelled_requests_cannot_be_resurrected() {
        let mut c = controller();
        c.input("private password");
        c.event(Event::Prompt {
            generation: 1,
            serial: 2,
            text: "One-time code".into(),
            echo: true,
        });
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.prompt.as_ref().unwrap().echo);
        c.input("123456");
        c.cancel();
        assert!(c.ui.secret.is_empty());
        c.event(Event::Prompt {
            generation: 1,
            serial: 3,
            text: "stale".into(),
            echo: false,
        });
        assert_eq!(c.phase, Phase::Cancelling);
        c.event(Event::Closed { generation: 1 });
        assert_eq!(c.phase, Phase::Idle);
        c.event(Event::Prompt {
            generation: 1,
            serial: 4,
            text: "stale".into(),
            echo: false,
        });
        assert_eq!(c.phase, Phase::Idle);
        c.event(Event::Begin {
            generation: 2,
            action: "org.test.second".into(),
            message: "Second request".into(),
            subject_pid: None,
            uids: vec![0, 1000],
            selected: 0,
        });
        c.event(Event::Closed { generation: 1 });
        assert!(c.active());
        c.event(Event::Prompt {
            generation: 2,
            serial: 1,
            text: "Password:".into(),
            echo: false,
        });
        c.input("new");
        c.event(Event::Prompt {
            generation: 1,
            serial: 100,
            text: "stale".into(),
            echo: true,
        });
        assert_eq!(c.ui.secret.text(), "new");
    }
    #[test]
    fn switching_identity_erases_input_and_rejects_old_prompt_serials() {
        let mut c = controller();
        c.uids = vec![0, 1000];
        c.input("must be erased");
        c.select();
        assert!(c.ui.secret.is_empty());
        assert_eq!(c.phase, Phase::Waiting);
        c.event(Event::Prompt {
            generation: 1,
            serial: 1,
            text: "old".into(),
            echo: false,
        });
        assert_eq!(c.phase, Phase::Waiting);
        c.event(Event::Prompt {
            generation: 1,
            serial: 3,
            text: "new".into(),
            echo: false,
        });
        assert_eq!(c.phase, Phase::Prompt);
    }
    #[test]
    fn request_details_are_paged_without_secret_content_or_authentication_submission() {
        let mut c = controller();
        c.description = "a".repeat(512);
        c.action = "org.test.action".into();
        c.prompt.as_mut().unwrap().text = "b".repeat(2048);
        c.input("private password");
        c.details();
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.prompt.is_none());
        assert!(
            c.ui.custom
                .values()
                .all(|v| !v.contains("private password"))
        );
        let first = c.ui.custom.clone();
        c.details();
        assert_ne!(first, c.ui.custom);
        c.input("cannot enter a password in details");
        assert!(c.ui.secret.is_empty());
        c.submit();
        assert_eq!(c.phase, Phase::Prompt);
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.prompt.is_some());
    }
}
