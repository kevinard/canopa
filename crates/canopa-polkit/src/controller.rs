use crate::native::{Event, Runtime};
use canopa_session_ui::{
    settings::Settings,
    view::{
        AuthenticationPanel, Control, ControlKind, Emblem, Prompt, SessionView, request_summary,
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
    /// The requesting program's name, as the agent read it for this request.
    subject_name: Option<String>,
    policy_publisher: String,
    notice: String,
    prompt: Option<Prompt>,
    details: Option<usize>,
    /// After a failure the field stays open: a response typed there is kept
    /// across the restarted conversation and sent when its prompt arrives.
    carry: bool,
    submit_on_prompt: bool,
}
impl Controller {
    pub fn new(settings: &Settings) -> anyhow::Result<Self> {
        let mut ui = SessionView::new(settings, String::new())?;
        ui.authentication_panel = Some(AuthenticationPanel {
            height_reserve: 0.,
            heading: fl!("polkit-heading"),
            subtitle: fl!("polkit-subtitle"),
            emblem: Emblem::Shield,
            dialog: true,
            choices: &[(1, Emblem::Cycle)],
            utilities: &[4],
            disclosure: Some(4),
            list_actions: false,
            checked_control: None,
            choice_details: Default::default(),
            account: None,
            confirm_identity: false,
            facts: Vec::new(),
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
            subject_name: None,
            policy_publisher: String::new(),
            notice: String::new(),
            prompt: None,
            details: None,
            carry: false,
            submit_on_prompt: false,
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
                self.subject_name = None;
                self.policy_publisher.clear();
                self.description = message
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect();
                self.details = None;
                self.carry = false;
                self.submit_on_prompt = false;
                self.phase = Phase::Waiting;
                self.notice = fl!("polkit-waiting-for-authentication");
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
                if !self.carry {
                    self.ui.secret.clear();
                    self.prompt = None;
                    self.notice = fl!("polkit-waiting-for-authentication");
                    self.ui.failed = false;
                }
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
                self.ui.mask_response();
                let carried = std::mem::take(&mut self.carry);
                if !carried {
                    self.ui.secret.clear();
                    self.notice.clear();
                    self.ui.failed = false;
                }
                self.prompt = Some(Prompt {
                    id: serial,
                    text,
                    echo,
                });
                self.phase = Phase::Prompt;
                if std::mem::take(&mut self.submit_on_prompt) {
                    self.submit();
                    return;
                }
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
                self.ui.mask_response();
                self.carry = false;
                self.submit_on_prompt = false;
                self.phase = Phase::Failed;
                self.retry_after = Instant::now() + Duration::from_secs(2);
                self.notice = fl!("polkit-failed");
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
        self.carry = false;
        self.submit_on_prompt = false;
        self.ui.secret.clear();
        self.prompt = None;
        self.ui.custom.clear();
        self.action.clear();
        self.description.clear();
        self.subject_pid = None;
        self.subject_name = None;
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
        // The decision stays the heading in the details; they are its subtitle.
        panel.heading = if self.details.is_some() && !self.description.is_empty() {
            self.description.clone()
        } else if self.details.is_some() {
            fl!("polkit-request-details")
        } else {
            fl!("polkit-heading")
        };
        panel.account = None;
        // Who asked comes first, under what is asked.
        panel.subtitle = match (&self.subject_name, self.details.is_some()) {
            // The details keep the decision in view; the requester is a row there.
            (_, true) => fl!("polkit-request-details"),
            (Some(name), _) => fl!("polkit-requested-by", name = name.as_str()),
            _ => fl!("polkit-subtitle"),
        };
        self.ui.custom.clear();
        self.ui.prompt = if self.details.is_none() {
            self.prompt.as_ref().map(|p| Prompt {
                id: p.id,
                // PAM's plain "Password:" asks for an administrator's password here.
                text: if !p.echo
                    && p.text
                        .trim()
                        .trim_end_matches(':')
                        .trim()
                        .eq_ignore_ascii_case("password")
                {
                    fl!("polkit-admin-password")
                } else {
                    p.text.clone()
                },
                echo: p.echo,
            })
        } else {
            None
        };
        panel.facts.clear();
        let selected_uid = self.selected_uid();
        let mut facts = Vec::new();
        let display = if self.details.is_some() {
            let account = self
                .identity
                .as_ref()
                .filter(|i| Some(i.uid) == selected_uid)
                // The person first; the login name and UID on a secondary line.
                .map(|identity| {
                    let secondary = fl!(
                        "polkit-identity",
                        username = identity.username.as_str(),
                        uid = identity.uid.to_string()
                    );
                    if identity.name.is_empty() || identity.name == identity.username {
                        secondary
                    } else {
                        format!("{}\n{secondary}", identity.name)
                    }
                })
                .or_else(|| selected_uid.map(|uid| fl!("polkit-uid", uid = uid.to_string())))
                .unwrap_or_default();
            let process =
                self.subject_pid
                    .map_or_else(String::new, |pid| match &self.subject_name {
                        // The name first; the process on a secondary line.
                        Some(name) => {
                            format!("{name}\n{}", fl!("polkit-process", pid = pid.to_string()))
                        }
                        None => fl!("polkit-process", pid = pid.to_string()),
                    });
            // What is asked, by whom and as whom first; identifiers after.
            facts = [
                (fl!("polkit-fact-requester"), process),
                (fl!("polkit-fact-account"), account),
                (fl!("polkit-fact-action"), self.action.clone()),
                (fl!("polkit-fact-publisher"), self.policy_publisher.clone()),
            ]
            .into_iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .collect();
            Vec::new()
        } else {
            self.ui.custom.insert("action".into(), self.action.clone());
            if !self.description.is_empty()
                && !self.description.contains('\n')
                && self.description.chars().count() <= 40
            {
                self.ui
                    .custom
                    .insert("request-title".into(), self.description.clone());
                Vec::new()
            } else {
                request_summary(&self.description)
            }
        };
        if let Some(panel) = self.ui.authentication_panel.as_mut() {
            panel.facts = facts;
        }
        for (index, part) in display.chunks(4).enumerate() {
            self.ui
                .custom
                .insert(format!("message-{index}"), part.join("\n"));
        }
        self.ui.message = if self.details.is_some() {
            String::new()
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
                fl!("polkit-your-account")
            } else {
                fl!("polkit-user-id", uid = uid.to_string())
            }
        } else {
            String::new()
        };
        self.ui.username = label.clone();
        let account = canopa_session_ui::view::account_label(&label);
        if self.uids.len() == 1 && self.details.is_none() {
            self.ui.custom.insert("account".into(), account.clone());
            if let Some(panel) = self.ui.authentication_panel.as_mut() {
                panel.account = Some(label.clone());
            }
            if let Some(pid) = self.subject_pid {
                self.ui.custom.insert(
                    "subject-process".into(),
                    fl!("polkit-process", pid = pid.to_string()),
                );
            }
            if let Some(name) = &self.subject_name {
                self.ui.custom.insert("subject-name".into(), name.clone());
            }
            self.ui
                .custom
                .insert("account-caption".into(), fl!("polkit-admin-required"));
            self.ui
                .custom
                .insert("field-hint".into(), fl!("polkit-password-hint"));
            if !self.policy_publisher.is_empty() {
                self.ui
                    .custom
                    .insert("policy-publisher".into(), self.policy_publisher.clone());
            }
        }
        self.ui.busy = self.phase == Phase::Waiting && self.details.is_none();
        // The default action keeps its place while polkit checks a response.
        self.ui.primary_control = (self.phase != Phase::Cancelling).then_some(2);
        self.ui.controls = vec![
            Control::new(
                2,
                if self.phase == Phase::Failed && self.prompt.is_none() {
                    fl!("polkit-retry")
                } else if self.phase == Phase::Waiting && self.prompt.is_some() {
                    fl!("polkit-checking")
                } else {
                    fl!("polkit-authenticate")
                },
            ),
            Control::new(3, fl!("polkit-cancel")),
            Control::new(4, fl!("polkit-policy-details")),
        ];
        if let Some(accept) = self.ui.controls.first_mut() {
            accept.enabled = matches!(self.phase, Phase::Prompt | Phase::Failed);
        }
        if self.uids.len() > 1 && self.details.is_none() {
            self.ui
                .controls
                .insert(0, Control::new(1, account).with_kind(ControlKind::Identity));
        }
        if self.details.is_some() {
            // Going back sits where "Request details" was, so the footer keeps
            // its layout; it has keyboard focus, and Cancel stays at the end.
            let mut back =
                Control::new(2, fl!("polkit-back-to-authentication")).with_kind(ControlKind::Back);
            back.selected = true;
            self.ui.controls = vec![back, Control::new(3, fl!("polkit-cancel"))];
            self.ui.primary_control = None;
            if let Some(panel) = self.ui.authentication_panel.as_mut() {
                panel.disclosure = Some(2);
            }
        }
    }
    /// The field accepts a response while prompted, and after a failure for
    /// the restarted conversation.
    fn accepts_input(&self) -> bool {
        self.details.is_none()
            && (self.phase == Phase::Prompt
                || self.phase == Phase::Failed
                || self.phase == Phase::Waiting && self.carry)
    }
    pub fn input(&mut self, text: &str) {
        if self.accepts_input() {
            self.ui.secret.push(text);
            if self.phase == Phase::Failed {
                // The new attempt starts now; the error has been read.
                self.ui.failed = false;
                self.notice.clear();
                self.refresh();
            }
        }
    }
    pub fn backspace(&mut self) {
        self.edit(canopa_session_ui::secret::Edit::Backspace);
    }
    pub fn select_input(&mut self, anchor: usize, cursor: usize) {
        if self.accepts_input() {
            self.ui.secret.set_selection(anchor, cursor);
        }
    }
    pub fn edit(&mut self, edit: canopa_session_ui::secret::Edit) {
        if self.accepts_input() {
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
            // The field keeps as many (meaningless) masked characters while
            // polkit checks, so the response does not seem to vanish.
            self.ui
                .secret
                .push(&"x".repeat(secret.text().chars().count().min(64)));
            // The field stays, quiet, with a spinner, until polkit answers.
            self.phase = Phase::Waiting;
            self.notice.clear();
            self.ui.failed = false;
        } else if self.phase == Phase::Failed && Instant::now() >= self.retry_after {
            // Restart the conversation and send what was typed when it asks.
            self.carry = !self.ui.secret.is_empty();
            self.submit_on_prompt = self.carry;
            Runtime::command(self.generation, self.serial, 2, self.selected, None);
            self.serial += 1;
            self.phase = Phase::Waiting;
            self.notice.clear();
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
        self.carry = false;
        self.submit_on_prompt = false;
        self.phase = Phase::Cancelling;
        self.notice = fl!("polkit-cancelling");
        Runtime::command(self.generation, self.serial, 1, 0, None);
        self.refresh();
    }
    pub fn details(&mut self) {
        if !self.active() || self.phase == Phase::Cancelling {
            return;
        }
        self.ui.secret.clear();
        self.details = Some(0);
        self.refresh();
    }
    pub fn previous_page(&mut self) {
        if let Some(page) = &mut self.details {
            *page = page.saturating_sub(1);
            self.refresh();
        }
    }
    /// Name the program that asked, for the current request only.
    pub fn subject_name(&mut self, generation: u64, name: Option<String>) {
        if generation == self.generation && self.active() {
            self.subject_name = name;
        }
    }

    pub fn preview(settings: &Settings) -> anyhow::Result<Self> {
        let mut c = Self::new(settings)?;
        c.event(Event::Begin {
            generation: 1,
            action: "org.freedesktop.NetworkManager.settings.modify.system".into(),
            message: fl!("polkit-preview-message"),
            subject_pid: Some(2481),
            uids: vec![unsafe { libc::getuid() }],
            selected: 0,
        });
        c.event(Event::Prompt {
            generation: 1,
            serial: 1,
            text: fl!("polkit-preview-prompt"),
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
        c.subject_name(1, Some("Advanced Network Configuration".into()));
        c.refresh();
        Ok(c)
    }
}

/// The requesting program's application name from its desktop entry, else its
/// program name. Read once per request for display; the process can choose it.
pub fn process_name(pid: u32) -> Option<String> {
    let program = canopa_session_ui::process::program_name(pid)?;
    let applications = canopa_apps::discover_choice_metadata().unwrap_or_default();
    Some(application_name(&applications, &program).unwrap_or(program))
}

/// The display name of the desktop entry whose program is `program`.
pub fn application_name(
    applications: &[canopa_apps::DesktopApplication],
    program: &str,
) -> Option<String> {
    applications
        .iter()
        .find(|app| {
            app.exec_program.as_deref() == Some(program)
                || app.startup_wm_class.as_deref() == Some(program)
        })
        .map(|app| {
            app.name
                .chars()
                .filter(|c| !c.is_control())
                .take(48)
                .collect()
        })
        .filter(|name: &String| !name.trim().is_empty())
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

    /// Changes the process-wide selection; restores English even on panic.
    struct French(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl Drop for French {
        fn drop(&mut self) {
            canopa_i18n::select(canopa_i18n::Selection::default());
        }
    }
    fn french() -> French {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
        let fr: canopa_i18n::LanguageIdentifier = "fr".parse().unwrap();
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(std::slice::from_ref(&fr)),
            time: fr.clone(),
            numbers: fr,
            pseudo: false,
        });
        French(guard)
    }

    #[test]
    fn dialog_labels_are_translated_and_the_account_is_data() {
        let _french = french();
        let (settings, _) = crate::config::load(&canopa_config::Request::Defaults).unwrap();
        let c = Controller::preview(&settings).unwrap();
        let panel = c.ui.authentication_panel.as_ref().unwrap();
        assert_eq!(panel.heading, "Autoriser une modification du système");
        assert_eq!(panel.account.as_deref(), Some("Alex Morgan"));
        assert_eq!(c.ui.custom["account"], "Compte\u{a0}: Alex Morgan");
        assert_eq!(c.ui.custom["subject-process"], "Processus 2481");
        let labels: Vec<_> = c.ui.controls.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Autoriser", "Annuler", "Détails de la demande"]);
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
    fn request_details_are_listed_without_secret_content_or_authentication_submission() {
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
        let facts = &c.ui.authentication_panel.as_ref().unwrap().facts;
        assert!(facts.iter().any(|(_, value)| value == "org.test.action"));
        assert!(
            facts
                .iter()
                .all(|(_, value)| !value.contains("private password") && !value.contains("bbbb"))
        );
        c.input("cannot enter a password in details");
        assert!(c.ui.secret.is_empty());
        c.submit();
        assert_eq!(c.phase, Phase::Prompt);
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.prompt.is_some());
    }
    #[test]
    fn a_failure_keeps_the_field_and_sends_the_next_response_when_asked_again() {
        let mut c = controller();
        let generation = c.generation;
        c.event(Event::Failed { generation });
        assert_eq!(c.phase, Phase::Failed);
        assert!(
            c.ui.prompt.is_some(),
            "the field stays for the next attempt"
        );
        assert!(c.ui.failed);
        c.retry_after = Instant::now();
        c.input("second attempt");
        assert!(!c.ui.failed, "typing starts a new attempt");
        c.submit();
        assert_eq!(c.phase, Phase::Waiting);
        assert_eq!(c.ui.secret.text(), "second attempt");
        let serial = c.serial;
        c.event(Event::Waiting {
            generation,
            serial,
            selected: 0,
        });
        assert_eq!(c.ui.secret.text(), "second attempt");
        c.event(Event::Prompt {
            generation,
            serial: serial + 1,
            text: "Password:".into(),
            echo: false,
        });
        assert_eq!(c.phase, Phase::Waiting, "the kept response was sent");
        // Only a same-length mask placeholder remains while polkit checks.
        assert_eq!(c.ui.secret.text(), "x".repeat("second attempt".len()));
    }
    #[test]
    fn requesters_are_named_by_their_desktop_entry_or_program() {
        let editor = canopa_apps::DesktopApplication {
            id: "nm-connection-editor".into(),
            name: "Advanced Network Configuration".into(),
            exec_program: Some("nm-connection-editor".into()),
            ..Default::default()
        };
        let apps = [editor];
        assert_eq!(
            application_name(&apps, "nm-connection-editor").as_deref(),
            Some("Advanced Network Configuration")
        );
        assert_eq!(application_name(&apps, "pkexec"), None);
        let mut c = controller();
        c.subject_name(c.generation, Some("Advanced Network Configuration".into()));
        c.refresh();
        assert_eq!(
            c.ui.custom["subject-name"],
            "Advanced Network Configuration"
        );
        // A later request starts without the previous requester's name.
        c.subject_name(c.generation + 1, Some("Stale".into()));
        c.refresh();
        assert_ne!(
            c.ui.custom.get("subject-name").map(String::as_str),
            Some("Stale")
        );
    }
}
