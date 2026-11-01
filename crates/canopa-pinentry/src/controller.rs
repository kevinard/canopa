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
    first: Option<Secret>,
    details: Option<usize>,
    selected: usize,
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
            first: None,
            details: None,
            selected,
        };
        this.refresh();
        Ok(this)
    }
    pub fn refresh(&mut self) {
        self.ui.custom.clear();
        self.ui
            .custom
            .insert("heading".into(), "Canopa Pinentry".into());
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
                echo: false,
            })
        };
        if let Some(page) = self.details {
            self.ui.controls.push(Control {
                id: 2,
                label: "Back (Enter)".into(),
            });
            self.ui.controls.push(Control {
                id: 4,
                label: format!("Next page (Right) · {} / {}", page + 1, self.pages()),
            });
            self.ui.controls.push(Control {
                id: 5,
                label: "Previous page (Left)".into(),
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
                label: "Details (Right)".into(),
            });
        }
        for control in &mut self.ui.controls {
            if control.id == self.selected {
                control.label = format!("› {}", control.label);
            }
        }
    }
    fn details_text(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}",
            self.request.title,
            self.request.description,
            self.request.prompt,
            self.request.error,
            self.request.repeat.as_deref().unwrap_or("")
        )
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
        match id {
            2 => self.accept(),
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
                self.finish(Response::Pin {
                    secret: first,
                    repeated: true,
                });
            } else {
                self.first = Some(std::mem::replace(
                    &mut self.ui.secret,
                    Secret::with_capacity(8193),
                ));
                self.ui.message.clear();
                self.ui.failed = false;
            }
        } else {
            let secret = std::mem::replace(&mut self.ui.secret, Secret::with_capacity(8193));
            self.finish(Response::Pin {
                secret,
                repeated: false,
            });
        }
    }
    pub fn finish(&mut self, response: Response) {
        self.clear_input();
        self.first = None;
        self.response = Some(response);
    }
    pub fn clear_input(&mut self) {
        self.ui.secret.clear();
    }
    pub fn backspace(&mut self) {
        self.ui.secret.pop();
    }
    pub fn input(&mut self, text: &str) {
        if self.request.kind != Kind::Pin || self.details.is_some() || self.response.is_some() {
            return;
        }
        if self.ui.secret.count() + text.chars().count() > 2048 {
            self.ui.message = "Passphrase limit: 2048 characters. Input was rejected.".into();
            self.ui.failed = true;
        } else {
            self.ui.secret.push(text);
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
