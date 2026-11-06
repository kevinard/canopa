//! Explicit per-action consent and editable triggers in the shared Iced theme.
use crate::global_shortcuts::{
    self, Binding,
    editor::{Decision, Feedback, Request},
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{checkbox, column, container, row, scrollable, text, text_input},
    window,
};
use std::io::Write;
use tokio::sync::watch;

struct Draft {
    id: String,
    description: String,
    enabled: bool,
    trigger: String,
}
struct RecordingDraft {
    row: usize,
    generation: u64,
    handle: window::keyboard_shortcuts::Recording,
    state: window::keyboard_shortcuts::RecordingState,
}
struct Dialog {
    window: Option<window::Id>,
    inhibitor: Option<window::keyboard_shortcuts::Inhibitor>,
    inhibition: Option<window::keyboard_shortcuts::Status>,
    recording: Option<RecordingDraft>,
    record_generation: u64,
    request: Request,
    rows: Vec<Draft>,
    name: Option<String>,
    theme: Theme,
    generation: u64,
    busy: bool,
    error: Option<String>,
    failed: Vec<String>,
    completed: bool,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Inhibitor(Option<window::keyboard_shortcuts::Inhibitor>),
    Inhibition(window::keyboard_shortcuts::Status),
    Name(Option<String>),
    Feedback(Feedback),
    Enabled(usize, bool),
    Trigger(usize, String),
    Record(usize),
    StopRecording,
    Recorded(u64, window::keyboard_shortcuts::RecordingState),
    Apply,
    Cancel,
    Event(Event),
}
fn label(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
        .collect()
}
fn normalize(value: &str) -> String {
    let parts: Vec<_> = value.split('+').map(str::trim).collect();
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            if i + 1 < parts.len() {
                match part.to_ascii_lowercase().as_str() {
                    "ctrl" | "control" => "CTRL".into(),
                    "alt" => "ALT".into(),
                    "shift" => "SHIFT".into(),
                    "super" | "logo" => "LOGO".into(),
                    "num" => "NUM".into(),
                    _ => part.to_string(),
                }
            } else if part.eq_ignore_ascii_case("enter") {
                "Return".into()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}
impl Dialog {
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let escape = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (escape || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }
    fn new(request: Request, theme: Theme) -> Self {
        let rows = request
            .actions
            .0
            .iter()
            .map(|action| {
                let current = request.current.iter().find(|b| b.id == action.id);
                let suggested =
                    current.or_else(|| request.defaults.iter().find(|b| b.id == action.id));
                Draft {
                    id: action.id.clone(),
                    description: label(&action.description),
                    enabled: request.configuring && current.is_some(),
                    trigger: suggested.map(|b| b.trigger.clone()).unwrap_or_default(),
                }
            })
            .collect();
        Self {
            window: None,
            inhibitor: None,
            inhibition: None,
            recording: None,
            record_generation: 0,
            generation: request.generation,
            request,
            rows,
            name: None,
            theme,
            busy: false,
            error: None,
            failed: Vec::new(),
            completed: false,
        }
    }
    fn write(&self, decision: &Decision) -> anyhow::Result<()> {
        let mut output = std::io::stdout().lock();
        serde_json::to_writer(&mut output, decision)?;
        output.write_all(b"\n")?;
        output.flush()?;
        Ok(())
    }
    fn selected(&self) -> Vec<Binding> {
        self.rows
            .iter()
            .filter(|r| r.enabled)
            .map(|r| Binding {
                id: r.id.clone(),
                trigger: normalize(&r.trigger),
            })
            .collect()
    }
    fn stop_recording(&mut self) {
        if let Some(recording) = self.recording.take() {
            recording.handle.cancel();
        }
    }
    fn recording_result(
        &mut self,
        generation: u64,
        state: window::keyboard_shortcuts::RecordingState,
    ) {
        use window::keyboard_shortcuts::{RecordingCancelReason as R, RecordingState as S};
        let Some(recording) = self
            .recording
            .as_mut()
            .filter(|recording| recording.generation == generation)
        else {
            return;
        };
        recording.state = state.clone();
        match state {
            S::Recorded(value) => {
                let row = recording.row;
                self.recording = None;
                if value.len() <= 256
                    && let Some(draft) = self.rows.get_mut(row)
                {
                    draft.trigger = value;
                    self.failed.retain(|id| id != &draft.id);
                    self.error = None;
                }
            }
            S::Cancelled(reason) => {
                self.recording = None;
                self.error = match reason {
                    R::User => None,
                    R::Unrepresentable => Some("This key combination is unavailable in the current keyboard layout. Choose another shortcut.".into()),
                    R::KeymapChanged => Some("The keyboard setup changed. Record the shortcut again.".into()),
                    _ => Some("Keyboard recording was interrupted. Focus this window and try again.".into()),
                };
            }
            S::Closed => {
                self.recording = None;
                self.error =
                    Some("Keyboard recording is no longer available in this window.".into());
            }
            _ => {}
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Opened => {
                if let Some(id) = self.window {
                    return window::inhibit_keyboard_shortcuts(id).map(Message::Inhibitor);
                }
            }
            Message::Inhibitor(inhibitor) => {
                self.inhibitor = inhibitor;
                if let Some(inhibitor) = &self.inhibitor {
                    self.inhibition = Some(inhibitor.status());
                    if let Some(changes) = inhibitor.changes() {
                        return Task::run(changes, Message::Inhibition);
                    }
                }
            }
            Message::Inhibition(status) => {
                self.inhibition = Some(status);
                if status != window::keyboard_shortcuts::Status::Active {
                    self.stop_recording();
                }
            }
            Message::Record(index)
                if !self.busy && self.recording.is_none() && index < self.rows.len() =>
            {
                let Some(generation) = self.record_generation.checked_add(1) else {
                    return Task::none();
                };
                let handle = self
                    .inhibitor
                    .as_ref()
                    .and_then(|inhibitor| inhibitor.record());
                let Some(handle) = handle else {
                    self.error = Some("Keyboard recording is unavailable. Enter a shortcut or focus this window and try again.".into());
                    return Task::none();
                };
                let Some(changes) = handle.changes() else {
                    handle.cancel();
                    return Task::none();
                };
                self.record_generation = generation;
                self.recording = Some(RecordingDraft {
                    row: index,
                    generation,
                    state: handle.state(),
                    handle,
                });
                self.error = None;
                return Task::run(changes, move |state| Message::Recorded(generation, state));
            }
            Message::StopRecording => self.stop_recording(),
            Message::Recorded(generation, state) => self.recording_result(generation, state),
            Message::Name(name) => self.name = name,
            Message::Feedback(feedback) => {
                if feedback.busy || feedback.generation != self.generation {
                    self.stop_recording();
                }
                self.generation = feedback.generation;
                self.busy = feedback.busy;
                self.error = feedback.error.map(|error| match error.as_str() {
                    "conflict"=>"A selected shortcut is already in use. Choose another trigger.",
                    "stale_generation"=>"Available shortcuts changed. Review your choices and try again.",
                    "unresolved"=>"A selected key combination is unavailable in the current keyboard layout.",
                    _=>"These shortcuts could not be applied. Review the selected keys and try again.",
                }.to_owned());
                self.failed = feedback.failed;
            }
            Message::Enabled(index, value) if !self.busy && self.recording.is_none() => {
                if let Some(row) = self.rows.get_mut(index) {
                    row.enabled = value;
                }
            }
            Message::Trigger(index, value) if !self.busy && self.recording.is_none() => {
                if value.len() <= 256
                    && let Some(row) = self.rows.get_mut(index)
                {
                    row.trigger = value;
                }
            }
            Message::Apply if !self.busy && self.recording.is_none() => {
                let bindings = self.selected();
                if self.request.actions.definitions(&bindings).is_err() {
                    self.error = Some("Choose a key for every selected shortcut.".into());
                    return Task::none();
                }
                if self
                    .write(&Decision::Apply {
                        generation: self.generation,
                        bindings,
                    })
                    .is_err()
                {
                    self.completed = true;
                    return iced::exit();
                }
                self.busy = true;
                self.error = None;
                self.failed.clear();
            }
            Message::Cancel | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.stop_recording();
                let _ = self.write(&Decision::Cancel);
                self.completed = true;
                return iced::exit();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })) => {
                return self.update(if self.recording.is_some() {
                    Message::StopRecording
                } else {
                    Message::Cancel
                });
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let mut body = column![
            text(if self.request.configuring {
                "Configure global shortcuts"
            } else {
                "Allow global shortcuts?"
            })
            .size(24),
            text(self.name.as_deref().unwrap_or(&self.request.app_id)).size(18),
            text("Choose which shortcuts this app can use while other windows are focused.")
                .size(14)
        ]
        .spacing(12);
        if self.name.is_some() {
            body = body.push(text(&self.request.app_id).size(12).style(canopa_ui::muted));
        }
        let mut actions = column![].spacing(10);
        for (index, draft) in self.rows.iter().enumerate() {
            let mut toggle = checkbox(draft.enabled)
                .label(&draft.description)
                .style(canopa_ui::checkbox_style);
            if !self.busy && self.recording.is_none() {
                toggle = toggle.on_toggle(move |value| Message::Enabled(index, value));
            }
            let active = self
                .recording
                .as_ref()
                .filter(|recording| recording.row == index);
            let trigger: Element<'_, Message> = if self.recording.is_some() {
                container(text(if draft.trigger.is_empty() {
                    "No shortcut chosen"
                } else {
                    &draft.trigger
                }))
                .padding(6)
                .width(Length::Fill)
                .into()
            } else {
                let mut input = text_input("CTRL+ALT+a or LOGO+F9", &draft.trigger);
                if !self.busy {
                    input = input.on_input(move |value| Message::Trigger(index, value));
                }
                input.width(Length::Fill).into()
            };
            let can_record = !self.busy
                && self.recording.is_none()
                && self.inhibition == Some(window::keyboard_shortcuts::Status::Active);
            let record = canopa_ui::action(
                text(if active.is_some() {
                    "Cancel recording"
                } else {
                    "Record keys"
                }),
                if active.is_some() {
                    Some(Message::StopRecording)
                } else {
                    can_record.then_some(Message::Record(index))
                },
                canopa_ui::Action::Quiet,
                false,
            );
            let mut item = column![
                toggle,
                row![trigger, record].spacing(8),
                text(label(&draft.id)).size(12).style(canopa_ui::muted)
            ]
            .spacing(6);
            if let Some(recording) = active {
                use window::keyboard_shortcuts::RecordingState as S;
                let prompt = match recording.state {
                    S::Preparing => "Preparing keyboard recording…",
                    S::WaitingForRelease => "Release held keys to begin.",
                    S::Ready => "Press the shortcut keys.",
                    S::Collecting => "Release the keys to finish.",
                    _ => "Finishing keyboard recording…",
                };
                item = item.push(text(prompt).size(13));
            }
            if self.failed.contains(&draft.id) {
                item = item
                    .push(text("This shortcut is unavailable. Choose another trigger.").size(13));
            }
            actions = actions.push(
                container(item)
                    .padding(12)
                    .width(Length::Fill)
                    .style(canopa_ui::surface),
            );
        }
        if self.rows.is_empty() {
            actions = actions.push(text("This app requested no shortcuts."));
        }
        body = body.push(scrollable(actions).height(Length::Fill));
        body=body.push(text("Use CTRL, ALT, SHIFT or LOGO (Super), followed by a key name. Example: CTRL+ALT+a.").size(12).style(canopa_ui::muted));
        if let Some(error) = &self.error {
            body = body.push(text(label(error)).size(14));
        }
        body = body.push(
            row![
                canopa_ui::action(
                    text("Cancel"),
                    (!self.busy).then_some(Message::Cancel),
                    canopa_ui::Action::Quiet,
                    false
                )
                .width(Length::Fill),
                canopa_ui::action(
                    text(if self.busy {
                        "Applying…"
                    } else if self.request.configuring {
                        "Save shortcuts"
                    } else {
                        "Allow selected"
                    }),
                    (!self.busy && self.recording.is_none()).then_some(Message::Apply),
                    canopa_ui::Action::Normal,
                    false
                )
                .width(Length::Fill),
            ]
            .spacing(12),
        );
        container(body)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
pub fn run(
    request: Request,
    appearance: canopa_theme::Appearance,
    updates: watch::Receiver<Option<Feedback>>,
) -> anyhow::Result<()> {
    request.actions.definitions(&request.defaults)?;
    request.actions.definitions(&request.current)?;
    anyhow::ensure!(
        request.actions.0.len() <= global_shortcuts::MAX_ACTIONS,
        "Too many shortcut actions"
    );
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(680.0, 640.0),
                min_size: Some(Size::new(440.0, 360.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let (id, opening) = window::open_dialog_with_activation(
                request
                    .parent_window
                    .strip_prefix("wayland:")
                    .unwrap_or_default()
                    .to_owned(),
                true,
                request.activation_token.clone(),
                settings,
            );
            let app_id = request.app_id.clone();
            let metadata = Task::perform(
                crate::worker::run(move || {
                    canopa_apps::discover_choice_metadata()
                        .ok()?
                        .into_iter()
                        .find(|a| {
                            a.id == app_id || a.id.strip_suffix(".desktop") == Some(app_id.as_str())
                        })
                        .map(|a| label(&a.name))
                }),
                |result| Message::Name(result.ok().flatten()),
            );
            let controls =
                futures_util::stream::unfold(Some(updates.clone()), |state| async move {
                    let mut receiver = state?;
                    let value = if receiver.changed().await.is_ok() {
                        receiver.borrow_and_update().clone()
                    } else {
                        None
                    };
                    Some(match value {
                        Some(value) => (Message::Feedback(value), Some(receiver)),
                        None => (Message::Cancel, None),
                    })
                });
            let mut dialog = Dialog::new(request.clone(), theme.clone());
            dialog.window = Some(id);
            (
                dialog,
                Task::batch([
                    opening.map(|_| Message::Opened),
                    metadata,
                    Task::run(controls, |message| message),
                ]),
            )
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|_: &Dialog, _| "Global shortcuts — Canopa".to_owned())
    .theme(|dialog: &Dialog, _| dialog.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(configuring: bool) -> Request {
        Request {
            app_id: "example.App".into(),
            parent_window: String::new(),
            activation_token: None,
            configuring,
            actions: global_shortcuts::Actions(vec![global_shortcuts::Action {
                id: "record".into(),
                description: "Record".into(),
                preferred_trigger: Some("CTRL+r".into()),
            }]),
            defaults: vec![Binding {
                id: "record".into(),
                trigger: "F9".into(),
            }],
            current: if configuring {
                vec![Binding {
                    id: "record".into(),
                    trigger: "F10".into(),
                }]
            } else {
                Vec::new()
            },
            generation: 4,
        }
    }
    #[test]
    fn bind_suggestions_start_unchecked_and_configure_uses_only_the_live_set() {
        let theme = canopa_ui::theme(canopa_theme::Appearance::default());
        let bind = Dialog::new(request(false), theme.clone());
        assert!(bind.selected().is_empty());
        assert_eq!(bind.rows[0].trigger, "F9");
        let configuring = Dialog::new(request(true), theme);
        assert_eq!(configuring.selected()[0].trigger, "F10");
    }
    #[test]
    fn conflict_feedback_refreshes_generation_without_erasing_the_users_draft() {
        let theme = canopa_ui::theme(canopa_theme::Appearance::default());
        let mut dialog = Dialog::new(request(true), theme);
        let _ = dialog.update(Message::Trigger(0, "ctrl+alt+a".into()));
        dialog.busy = true;
        let _ = dialog.update(Message::Feedback(Feedback {
            generation: 8,
            current: Vec::new(),
            error: Some("conflict".into()),
            failed: vec!["record".into()],
            busy: false,
        }));
        assert_eq!(dialog.generation, 8);
        assert!(!dialog.busy);
        assert_eq!(dialog.selected()[0].trigger, "CTRL+ALT+a");
        assert_eq!(dialog.failed, vec!["record"]);
    }
    fn recording(
        generation: u64,
    ) -> (
        RecordingDraft,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use window::keyboard_shortcuts::{Recording, RecordingDriver, RecordingState as S};
        struct Fake(Arc<AtomicBool>);
        impl RecordingDriver for Fake {
            fn state(&self) -> S {
                if self.0.load(Ordering::SeqCst) {
                    S::Cancelled(window::keyboard_shortcuts::RecordingCancelReason::User)
                } else {
                    S::Ready
                }
            }
            fn cancel(&self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let (_, updates) = iced::futures::channel::mpsc::channel(0);
        (
            RecordingDraft {
                row: 0,
                generation,
                handle: Recording::new(Fake(cancelled.clone()), updates),
                state: S::Ready,
            },
            cancelled,
        )
    }
    #[test]
    fn a_recorded_trigger_does_not_enable_an_unchecked_action_or_grant_authority() {
        use window::keyboard_shortcuts::RecordingState as S;
        let mut dialog = Dialog::new(
            request(false),
            canopa_ui::theme(canopa_theme::Appearance::default()),
        );
        dialog.recording = Some(recording(1).0);
        let _ = dialog.update(Message::Trigger(0, "F12".into()));
        let _ = dialog.update(Message::Enabled(0, true));
        let _ = dialog.update(Message::Apply);
        assert!(!dialog.busy);
        assert!(!dialog.rows[0].enabled);
        assert_eq!(dialog.rows[0].trigger, "F9");
        let _ = dialog.update(Message::Recorded(1, S::Recorded("CTRL+a".into())));
        assert!(dialog.recording.is_none());
        assert!(!dialog.completed);
        assert!(!dialog.busy);
        assert_eq!(dialog.rows[0].trigger, "CTRL+a");
        assert!(dialog.selected().is_empty());
    }
    #[test]
    fn cancellation_and_late_results_preserve_the_old_draft_and_checkbox() {
        use window::keyboard_shortcuts::{RecordingCancelReason as R, RecordingState as S};
        let mut dialog = Dialog::new(
            request(true),
            canopa_ui::theme(canopa_theme::Appearance::default()),
        );
        let (draft, cancelled) = recording(1);
        dialog.recording = Some(draft);
        let _ = dialog.update(Message::StopRecording);
        assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!dialog.completed);
        dialog.recording = Some(recording(2).0);
        let _ = dialog.update(Message::Recorded(1, S::Recorded("F12".into())));
        assert_eq!(dialog.rows[0].trigger, "F10");
        assert!(dialog.recording.is_some());
        let _ = dialog.update(Message::Recorded(2, S::Cancelled(R::User)));
        assert!(dialog.recording.is_none());
        assert_eq!(dialog.rows[0].trigger, "F10");
        assert!(dialog.rows[0].enabled);
        assert!(!dialog.completed);
        assert!(dialog.error.is_none());
    }
}
