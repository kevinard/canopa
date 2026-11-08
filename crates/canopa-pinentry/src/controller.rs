use crate::protocol::{Kind, Request, Response};
use anyhow::Result;
use canopa_session_ui::{
    secret::Secret,
    settings::Settings,
    view::{Control, Prompt, SessionView},
};

pub struct Controller {
    pub ui: SessionView,
    pub request: Request,
    pub response: Option<Response>,
    heading: &'static str,
    first: Option<Secret>,
    details: Option<usize>,
    selected: usize,
    pub revision: u64,
    pub pending: Option<crate::agent::Query>,
    checking: bool,
    quality: Option<i16>,
    generated: bool,
    revealed: bool,
    generated_seen: bool,
    checked: Option<(Secret, bool)>,
}
fn label(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '_' {
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else if !c.is_control() {
            result.push(c);
        }
    }
    result
}
fn excerpt(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        out.push('…');
    }
    out
}
// Match the shared text widget's 64-column limit before assigning its four lines.
// Paging by characters would lose context containing many explicit line breaks.
fn lines(text: &str) -> Vec<String> {
    text.lines()
        .flat_map(|line| {
            let chars: Vec<_> = line.chars().filter(|c| !c.is_control()).collect();
            chars
                .chunks(64)
                .map(|chunk| chunk.iter().collect())
                .collect::<Vec<_>>()
        })
        .collect()
}
impl Controller {
    pub fn new(settings: &Settings, request: Request) -> Result<Self> {
        Self::new_named(settings, request, "Canopa Pinentry")
    }

    pub(crate) fn new_named(
        settings: &Settings,
        request: Request,
        heading: &'static str,
    ) -> Result<Self> {
        let mut ui = SessionView::new(settings, String::new())?;
        ui.secret = Secret::with_capacity(8193);
        ui.message = excerpt(&request.error, 96);
        ui.failed = !request.error.is_empty();
        let selected = if request.focus_cancel && request.kind == Kind::Confirm {
            3
        } else {
            2
        };
        let mut this = Self {
            ui,
            request,
            response: None,
            heading,
            first: None,
            details: None,
            selected,
            revision: 0,
            pending: None,
            checking: false,
            quality: None,
            generated: false,
            revealed: false,
            generated_seen: false,
            checked: None,
        };
        this.refresh();
        Ok(this)
    }
    pub fn refresh(&mut self) {
        self.ui.interaction_epoch = (self.revision, u64::from(self.checking));
        self.ui.busy = self.checking;
        self.ui.custom.clear();
        self.ui.custom.insert("heading".into(), self.heading.into());
        self.ui.controls.clear();
        let description: Vec<_> = if let Some(page) = self.details {
            lines(&self.details_text())
                .into_iter()
                .skip(page * 8)
                .take(8)
                .collect()
        } else {
            lines(&self.request.description)
                .into_iter()
                .take(8)
                .collect()
        };
        for (index, chunk) in description.chunks(4).enumerate() {
            self.ui
                .custom
                .insert(format!("message-{index}"), chunk.join("\n"));
        }
        self.ui.hide_input = self.details.is_some() || self.request.kind != Kind::Pin;
        self.ui.primary_control =
            (self.details.is_none() && self.request.kind != Kind::Confirm).then_some(2);
        self.ui.prompt = if self.ui.hide_input {
            None
        } else {
            let text = if self.first.is_some() {
                self.request
                    .repeat
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Repeat passphrase:")
            } else {
                &self.request.prompt
            };
            Some(Prompt {
                id: 1,
                text: excerpt(&label(text), 96),
                echo: self.generated && self.revealed,
            })
        };
        if let Some(page) = self.details {
            self.ui.controls.push(Control {
                id: 2,
                label: "Back (Enter)".into(),
            });
            self.ui.controls.push(Control {
                id: 4,
                label: format!("Next page (Page Down) · {} / {}", page + 1, self.pages()),
            });
            self.ui.controls.push(Control {
                id: 5,
                label: "Previous page (Page Up)".into(),
            });
        } else {
            self.ui.controls.push(Control {
                id: 2,
                label: label(&self.request.ok),
            });
            if self.request.kind == Kind::Confirm && !self.request.notok.is_empty() {
                self.ui.controls.push(Control {
                    id: 6,
                    label: label(&self.request.notok),
                });
            }
            if self.request.kind != Kind::Message {
                self.ui.controls.push(Control {
                    id: 3,
                    label: label(&self.request.cancel),
                });
            }
            self.ui.controls.push(Control {
                id: 4,
                label: "Details (Page Down)".into(),
            });
        }
        if self.details.is_none() && self.request.kind == Kind::Pin && self.first.is_none() {
            if let Some(label) = &self.request.generate {
                self.ui.controls.push(Control {
                    id: 7,
                    label: label.clone(),
                });
            }
            if self.generated {
                self.ui.controls.push(Control {
                    id: 8,
                    label: if self.revealed {
                        "Hide generated passphrase"
                    } else {
                        "Reveal generated passphrase"
                    }
                    .into(),
                });
            }
        }
        if let Some(quality) = self.quality
            && !self.ui.failed
            && !self.checking
            && !self.generated
        {
            self.ui.message = format!(
                "{}: {quality}%{}",
                self.request.quality.as_deref().unwrap_or("Quality"),
                if quality < 0 {
                    " · agent rejects this passphrase"
                } else {
                    ""
                }
            );
        }
        for control in &mut self.ui.controls {
            if control.id == self.selected {
                control.label = format!("› {}", control.label);
            }
        }
    }
    fn details_text(&self) -> String {
        [
            self.request.title.as_str(),
            self.request.description.as_str(),
            self.request.prompt.as_str(),
            self.request.error.as_str(),
            self.request.repeat.as_deref().unwrap_or(""),
            self.request.quality_hint.as_str(),
            self.request.generate_hint.as_str(),
            self.request.constraints_hint_short.as_str(),
            self.request.constraints_hint.as_str(),
        ]
        .join("\n")
    }
    fn pages(&self) -> usize {
        lines(&self.details_text()).len().div_ceil(8).max(1)
    }
    pub fn cycle(&mut self) {
        let index = self
            .ui
            .controls
            .iter()
            .position(|c| c.id == self.selected)
            .unwrap_or(0);
        self.selected = self.ui.controls[(index + 1) % self.ui.controls.len()].id;
    }
    pub fn activate(&mut self) {
        self.action(self.selected);
    }
    pub fn action(&mut self, id: usize) {
        if self.response.is_some() {
            return;
        }
        if self.checking && id != 3 {
            return;
        }
        match id {
            7 if !self.checking
                && self.details.is_none()
                && self.first.is_none()
                && self.request.generate.is_some() =>
            {
                self.changed();
                self.forget_generation();
                self.ui.secret.clear();
                self.checking = true;
                self.pending = Some(crate::agent::Query {
                    revision: self.revision,
                    kind: crate::agent::Kind::Generate,
                });
                self.ui.message = "Requesting a passphrase from the agent…".into();
            }
            8 if self.generated => {
                self.revealed = !self.revealed;
                self.generated_seen = true;
            }
            2 if !self.checking => self.accept(),
            3 => self.finish(Response::Canceled),
            6 if self.request.kind == Kind::Confirm => self.finish(Response::NotConfirmed),
            4 => {
                self.clear_input();
                self.first = None;
                self.details = Some(self.details.map_or(0, |page| (page + 1) % self.pages()));
                self.selected = 2;
            }
            5 => {
                if let Some(page) = self.details {
                    self.details = Some((page + self.pages() - 1) % self.pages());
                }
            }
            _ => {}
        }
        self.refresh();
    }
    fn accept(&mut self) {
        if self.details.take().is_some() {
            self.selected = 2;
            return;
        }
        if self.request.kind != Kind::Pin {
            self.finish(Response::Accepted);
            return;
        }
        if self.generated && !self.generated_seen {
            self.ui.message =
                "Reveal and record the generated passphrase before continuing.".into();
            return;
        }
        if self.request.repeat.is_some() {
            if let Some(first) = self.first.take() {
                let mut difference = first.as_bytes().len() ^ self.ui.secret.as_bytes().len();
                for index in 0..first.as_bytes().len().max(self.ui.secret.as_bytes().len()) {
                    difference |= usize::from(
                        first.as_bytes().get(index).copied().unwrap_or(0)
                            ^ self.ui.secret.as_bytes().get(index).copied().unwrap_or(0),
                    );
                }
                if difference != 0 {
                    self.clear_input();
                    self.ui.message = excerpt(&self.request.repeat_error, 96);
                    self.ui.failed = true;
                    return;
                }
                self.ui.secret.clear();
                self.complete_pin(first, true);
            } else {
                self.first = Some(std::mem::replace(
                    &mut self.ui.secret,
                    Secret::with_capacity(8193),
                ));
                self.generated = false;
                self.revealed = false;
                self.generated_seen = false;
                self.changed();
                self.ui.message.clear();
                self.ui.failed = false;
            }
        } else {
            let secret = std::mem::replace(&mut self.ui.secret, Secret::with_capacity(8193));
            self.complete_pin(secret, false);
        }
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.quality = None;
        self.pending = None;
    }
    fn forget_generation(&mut self) {
        if self.generated && !self.ui.failed {
            self.ui.message.clear();
        }
        self.generated = false;
        self.revealed = false;
        self.generated_seen = false;
    }
    fn copy(secret: &Secret) -> Secret {
        let mut copy = Secret::with_capacity(8193);
        copy.push(secret.text());
        copy
    }
    pub fn quality_query(&mut self) {
        if self.request.quality.is_some()
            && !self.checking
            && self.details.is_none()
            && self.first.is_none()
            && !self.ui.secret.is_empty()
            && !self.generated
        {
            self.pending = Some(crate::agent::Query {
                revision: self.revision,
                kind: crate::agent::Kind::Quality(Self::copy(&self.ui.secret)),
            });
        }
    }
    fn complete_pin(&mut self, secret: Secret, repeated: bool) {
        self.forget_generation();
        if self.request.constraints {
            self.changed();
            self.checking = true;
            if secret.as_bytes().len() > 300 {
                self.checking = false;
                self.ui.failed = true;
                self.ui.message =
                    "Passphrase exceeds the agent constraint check's 300-byte limit.".into();
                return;
            }
            self.pending = Some(crate::agent::Query {
                revision: self.revision,
                kind: crate::agent::Kind::Check(Self::copy(&secret)),
            });
            self.checked = Some((secret, repeated));
            self.ui.message = "Checking the agent's passphrase requirements…".into();
        } else {
            self.finish(Response::Pin { secret, repeated });
        }
    }
    pub fn inquiry_result(&mut self, revision: u64, answer: crate::agent::Answer) {
        if revision != self.revision || self.response.is_some() {
            return;
        }
        match answer {
            crate::agent::Answer::Quality(quality) => {
                self.quality = quality;
                if quality.is_none() {
                    self.ui.message = "Agent quality feedback unavailable.".into();
                }
            }
            crate::agent::Answer::Check(result) => {
                self.checking = false;
                if matches!(result, Ok(None)) {
                    if let Some((secret, repeated)) = self.checked.take() {
                        self.finish(Response::Pin { secret, repeated });
                    }
                } else {
                    self.checked = None;
                    self.ui.failed = true;
                    self.ui.message = match result { Ok(Some(reason)) => excerpt(&reason, 160), _ => "Could not verify the agent's passphrase requirements. Try again or cancel.".into() };
                }
            }
            crate::agent::Answer::Generated(secret) => {
                self.checking = false;
                self.ui.secret.clear();
                if let Some(secret) = secret {
                    self.ui.secret = secret;
                    self.generated = true;
                    self.revealed = false;
                    self.ui.failed = false;
                    self.ui.message =
                        "Reveal and record this generated passphrase before continuing.".into();
                } else {
                    self.ui.failed = true;
                    self.ui.message = "The agent could not generate a passphrase.".into();
                }
            }
        }
        self.refresh();
    }
    pub fn finish(&mut self, response: Response) {
        self.checking = false;
        self.clear_input();
        self.first = None;
        self.checked = None;
        self.pending = None;
        self.checking = false;
        self.response = Some(response);
    }
    pub fn clear_input(&mut self) {
        if self.checking {
            return;
        }
        self.changed();
        self.forget_generation();
        self.ui.secret.clear();
    }
    pub fn backspace(&mut self) {
        self.edit(canopa_session_ui::secret::Edit::Backspace);
    }
    pub fn edit(&mut self, edit: canopa_session_ui::secret::Edit) {
        if self.request.kind == Kind::Pin
            && self.details.is_none()
            && self.response.is_none()
            && !self.checking
        {
            let length = self.ui.secret.as_bytes().len();
            self.ui.secret.edit(edit);
            if self.ui.secret.as_bytes().len() != length {
                self.changed();
                if self.ui.secret.is_empty() {
                    self.forget_generation();
                }
            }
        }
    }
    pub fn input(&mut self, text: &str) {
        if self.request.kind != Kind::Pin
            || self.details.is_some()
            || self.response.is_some()
            || self.checking
            || text.is_empty()
            || text.chars().any(char::is_control)
        {
            return;
        }
        if self.ui.secret.count()
            - self.ui.secret.text()[self.ui.secret.selection()]
                .chars()
                .count()
            + text.chars().count()
            > 2048
        {
            self.ui.message = "Passphrase limit: 2048 characters. Input was rejected.".into();
            self.ui.failed = true;
        } else {
            let replaces_all = self.ui.secret.selection() == (0..self.ui.secret.as_bytes().len());
            self.ui.secret.push(text);
            self.changed();
            if replaces_all {
                self.forget_generation();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller(request: Request) -> Controller {
        let (settings, _) = crate::config::load(&canopa_config::Request::Defaults).unwrap();
        Controller::new(&settings, request).unwrap()
    }
    #[test]
    fn generated_values_require_reveal_after_caret_changes_and_partial_edits() {
        use canopa_session_ui::secret::Edit;
        let mut c = controller(Request {
            generate: Some("Generate".into()),
            ..Default::default()
        });
        let generate = |c: &mut Controller| {
            c.action(7);
            let query = c.pending.take().unwrap();
            let mut secret = Secret::default();
            secret.push("unseen generated value");
            c.inquiry_result(
                query.revision,
                crate::agent::Answer::Generated(Some(secret)),
            );
        };
        generate(&mut c);
        c.edit(Edit::Left(false));
        c.edit(Edit::SelectAll);
        c.input("");
        c.action(2);
        assert!(
            c.response.is_none(),
            "caret/selection/empty input bypassed reveal"
        );
        c.edit(Edit::End(false));
        c.edit(Edit::Backspace);
        c.input("replacement suffix");
        c.action(2);
        assert!(c.response.is_none(), "partial edits bypassed reveal");
        c.action(8);
        c.action(8);
        c.action(2);
        assert!(matches!(c.response.take(), Some(Response::Pin { .. })));
        generate(&mut c);
        c.edit(Edit::SelectAll);
        c.input("entirely typed by the user");
        c.action(2);
        assert!(
            matches!(c.response.take(), Some(Response::Pin { secret, .. }) if secret.text()=="entirely typed by the user")
        );
        generate(&mut c);
        c.clear_input();
        c.input("fresh typed value");
        c.action(2);
        assert!(
            matches!(c.response.take(), Some(Response::Pin { secret, .. }) if secret.text()=="fresh typed value")
        );
    }
    #[test]
    fn agent_feedback_is_revision_scoped_and_generation_and_constraints_require_explicit_steps() {
        let mut c = controller(Request {
            quality: Some("Quality".into()),
            generate: Some("Generate".into()),
            constraints: true,
            ..Default::default()
        });
        c.input("old draft");
        c.quality_query();
        let old = c.pending.take().unwrap().revision;
        c.input(" edited");
        c.inquiry_result(old, crate::agent::Answer::Quality(Some(99)));
        assert!(c.quality.is_none());
        c.action(7);
        let generated = c.pending.take().unwrap().revision;
        c.inquiry_result(
            generated,
            crate::agent::Answer::Generated(Some(Controller::copy(&{
                let mut s = Secret::default();
                s.push("generated phrase");
                s
            }))),
        );
        c.action(2);
        assert!(c.response.is_none() && c.pending.is_none());
        c.action(8);
        assert!(c.ui.prompt.as_ref().unwrap().echo);
        c.action(8);
        assert!(!c.ui.prompt.as_ref().unwrap().echo);
        c.action(2);
        let check = c.pending.take().unwrap();
        assert!(
            matches!(check.kind, crate::agent::Kind::Check(secret) if secret.text()=="generated phrase")
        );
        c.inquiry_result(
            check.revision,
            crate::agent::Answer::Check(Ok(Some("Agent refused this passphrase".into()))),
        );
        assert!(c.response.is_none() && c.ui.failed && c.checked.is_none());
        c.input("acceptable phrase");
        c.action(2);
        let check = c.pending.take().unwrap();
        assert!(c.response.is_none());
        c.inquiry_result(check.revision, crate::agent::Answer::Check(Ok(None)));
        assert!(
            matches!(c.response.take(), Some(Response::Pin {secret,..}) if secret.text()=="acceptable phrase")
        );
        c.inquiry_result(old, crate::agent::Answer::Generated(None));
        assert!(!c.generated);
    }
    #[test]
    fn repeat_mismatch_restarts_and_only_matching_pair_returns_a_secret() {
        let mut c = controller(Request {
            repeat: Some("Again:".into()),
            ..Request::default()
        });
        c.input("first");
        c.action(2);
        assert!(c.ui.secret.is_empty());
        c.input("wrong");
        c.action(2);
        assert!(c.response.is_none());
        assert!(c.ui.failed);
        assert!(c.first.is_none());
        c.input("second");
        c.action(2);
        c.input("second");
        c.action(2);
        assert!(c.ui.secret.is_empty());
        assert!(
            matches!(c.response,Some(Response::Pin {ref secret,repeated:true}) if secret.text()=="second")
        );
    }
    #[test]
    fn long_unicode_input_is_bounded_and_cancellation_erases_both_entries() {
        let mut c = controller(Request {
            repeat: Some(String::new()),
            ..Request::default()
        });
        c.input(&"🔒".repeat(2048));
        assert_eq!(c.ui.secret.as_bytes().len(), 8192);
        c.input("x");
        assert_eq!(c.ui.secret.count(), 2048);
        assert!(c.ui.failed);
        c.action(2);
        c.input("other");
        c.action(3);
        assert!(c.first.is_none());
        assert!(c.ui.secret.is_empty());
    }
    #[test]
    fn confirmation_default_focus_and_details_cannot_submit_hidden_input() {
        let mut c = controller(Request {
            kind: Kind::Confirm,
            focus_cancel: true,
            ..Request::default()
        });
        c.input("discarded");
        assert!(c.ui.secret.is_empty());
        assert!(c.ui.hide_input);
        c.activate();
        assert!(matches!(c.response, Some(Response::Canceled)));
        let mut c = controller(Request {
            description: "context ".repeat(150),
            ..Request::default()
        });
        c.input("discarded");
        c.action(4);
        assert!(c.ui.secret.is_empty());
        c.input("ignored");
        c.action(2);
        assert!(c.response.is_none());
        assert!(c.ui.secret.is_empty());
        assert!(!c.ui.hide_input);
    }
    #[test]
    fn details_preserve_context_with_more_lines_than_the_visible_widgets() {
        let context: Vec<_> = (0..36).map(|i| format!("Context line {i:03}")).collect();
        let mut c = controller(Request {
            description: context.join("\n"),
            ..Request::default()
        });
        c.action(4);
        let mut visible = String::new();
        for _ in 0..c.pages() {
            for text in c.ui.custom.values() {
                visible.push_str(text);
                visible.push('\n');
            }
            c.action(4);
        }
        for line in context {
            assert!(visible.contains(&line), "lost a request context line");
        }
    }
}
