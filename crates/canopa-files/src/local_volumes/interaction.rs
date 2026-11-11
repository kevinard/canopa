//! Helper-main-context interaction. Answers apply only to the latest question.
use super::{
    command_wire::{Answer, Question, Remember},
    *,
};

type Publish = Rc<dyn Fn(u64, Option<Question>)>;

#[derive(Default)]
struct Pending {
    serial: u64,
    question: Option<Question>,
    closed: bool,
}

pub(super) struct Interaction {
    pub operation: gio::MountOperation,
    pending: Rc<RefCell<Pending>>,
    publish: Publish,
}

impl Interaction {
    pub fn new(publish: impl Fn(u64, Option<Question>) + 'static) -> Self {
        let operation = gio::MountOperation::new();
        let pending = Rc::new(RefCell::new(Pending::default()));
        let publish: Publish = Rc::new(publish);
        let state = pending.clone();
        let notice = publish.clone();
        operation.connect_ask_password(move |operation, message, username, domain, flags| {
            operation.stop_signal_emission_by_name("ask-password");
            offer(
                operation,
                &state,
                &notice,
                Question::Password {
                    message: message.into(),
                    username: username.into(),
                    domain: domain.into(),
                    flags: flags.bits(),
                },
            );
        });
        for (signal, blocked, choices_index) in
            [("ask-question", false, 2), ("show-processes", true, 3)]
        {
            let state = pending.clone();
            let notice = publish.clone();
            operation.connect_local(signal, false, move |values| {
                let operation = values[0]
                    .get::<gio::MountOperation>()
                    .expect("MountOperation signal owner");
                operation.stop_signal_emission_by_name(signal);
                // GIO registers the choices parameter as GStrv, including for
                // show-processes. No borrowed C array escapes this callback.
                let choices = values[choices_index].get::<glib::StrV>();
                let message = values[1].get::<String>();
                match (message, choices) {
                    (Ok(message), Ok(choices)) => offer(
                        &operation,
                        &state,
                        &notice,
                        Question::Choice {
                            message,
                            choices: choices.iter().map(|s| s.to_string()).collect(),
                            blocked,
                        },
                    ),
                    _ => operation.reply(gio::MountOperationResult::Aborted),
                }
                None
            });
        }
        let state = pending.clone();
        let notice = publish.clone();
        operation.connect_aborted(move |op| {
            state.borrow_mut().closed = true;
            clear(&state, &notice);
            op.set_password(None);
        });
        Self {
            operation,
            pending,
            publish,
        }
    }

    pub fn answer(&self, serial: u64, answer: Answer) -> io::Result<()> {
        let state = self.pending.borrow();
        if state.serial != serial {
            return Err(io::Error::other("This device question has changed."));
        }
        match (state.question.as_ref(), answer) {
            (
                Some(Question::Password { flags, .. }),
                Answer::Password {
                    username,
                    domain,
                    password,
                    anonymous,
                    pim,
                    hidden_volume,
                    system_volume,
                    save,
                },
            ) => {
                if [&username, &domain, &password]
                    .iter()
                    .any(|s| s.len() > 4096 || s.contains('\0'))
                    || anonymous && flags & gio::AskPasswordFlags::ANONYMOUS_SUPPORTED.bits() == 0
                    || (pim != 0 || hidden_volume || system_volume)
                        && flags & gio::AskPasswordFlags::TCRYPT.bits() == 0
                    || save != Remember::Never
                        && flags & gio::AskPasswordFlags::SAVING_SUPPORTED.bits() == 0
                {
                    return Err(io::Error::other("Invalid device credentials."));
                }
                // Anonymous connections never forward credential drafts from
                // any frontend to the platform provider.
                self.operation
                    .set_username((!anonymous).then_some(username.as_str()));
                self.operation
                    .set_domain((!anonymous).then_some(domain.as_str()));
                self.operation
                    .set_password((!anonymous).then_some(password.as_str()));
                self.operation.set_anonymous(anonymous);
                // GVFS stores a kept password in the Secret Service itself
                // (G_PASSWORD_SAVE_PERMANENTLY); Files never writes it anywhere.
                self.operation.set_password_save(if anonymous {
                    gio::PasswordSave::Never
                } else {
                    save.platform()
                });
                self.operation.set_pim(pim);
                self.operation.set_is_tcrypt_hidden_volume(hidden_volume);
                self.operation.set_is_tcrypt_system_volume(system_volume);
            }
            (Some(Question::Choice { choices, .. }), Answer::Choice { choice })
                if choice < choices.len() =>
            {
                self.operation.set_choice(choice as i32);
            }
            _ => {
                return Err(io::Error::other(
                    "The answer does not match the current device question.",
                ));
            }
        }
        drop(state);
        clear(&self.pending, &self.publish);
        self.operation.reply(gio::MountOperationResult::Handled);
        Ok(())
    }

    pub fn abort(&self) {
        self.pending.borrow_mut().closed = true;
        let asked = self.pending.borrow().question.is_some();
        clear(&self.pending, &self.publish);
        if asked {
            self.operation.reply(gio::MountOperationResult::Aborted);
        }
        self.operation.set_password(None);
    }

    pub fn retire(&self) {
        self.pending.borrow_mut().closed = true;
        clear(&self.pending, &self.publish);
        self.operation.set_password(None);
    }
}

fn clear(state: &RefCell<Pending>, publish: &Publish) {
    let mut state = state.borrow_mut();
    if state.question.take().is_some() {
        let serial = state.serial;
        drop(state);
        publish(serial, None);
    }
}

fn offer(
    operation: &gio::MountOperation,
    pending: &RefCell<Pending>,
    publish: &Publish,
    question: Question,
) {
    let valid = match &question {
        Question::Password {
            message,
            username,
            domain,
            ..
        } => message.len() <= 16384 && username.len() <= 4096 && domain.len() <= 4096,
        Question::Choice {
            message, choices, ..
        } => {
            message.len() <= 16384
                && !choices.is_empty()
                && choices.len() <= 64
                && choices.iter().all(|s| s.len() <= 4096)
        }
    };
    let mut state = pending.borrow_mut();
    if state.closed {
        drop(state);
        operation.reply(gio::MountOperationResult::Aborted);
        return;
    }
    let Some(serial) = state.serial.checked_add(1).filter(|_| valid) else {
        drop(state);
        clear(pending, publish);
        operation.reply(gio::MountOperationResult::Aborted);
        return;
    };
    state.serial = serial;
    state.question = Some(question.clone());
    drop(state);
    publish(serial, Some(question));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(operation: &gio::MountOperation, choices: &[&str]) {
        operation.emit_by_name::<()>(
            "ask-question",
            &[&"Private device question", &glib::StrV::from(choices)],
        );
    }

    #[test]
    fn changed_questions_refuse_old_or_invalid_answers_without_replying() {
        let notices = Rc::new(RefCell::new(Vec::new()));
        let observed = notices.clone();
        let interaction =
            Interaction::new(move |id, question| observed.borrow_mut().push((id, question)));
        let replies = Rc::new(RefCell::new(Vec::new()));
        let observed = replies.clone();
        interaction
            .operation
            .connect_reply(move |op, result| observed.borrow_mut().push((result, op.choice())));
        choice(&interaction.operation, &["First", "Second"]);
        choice(&interaction.operation, &["Latest"]);
        assert!(interaction.answer(1, Answer::Choice { choice: 0 }).is_err());
        assert!(interaction.answer(2, Answer::Choice { choice: 1 }).is_err());
        assert!(replies.borrow().is_empty());
        interaction.answer(2, Answer::Choice { choice: 0 }).unwrap();
        assert_eq!(
            *replies.borrow(),
            vec![(gio::MountOperationResult::Handled, 0)]
        );
        assert!(interaction.answer(2, Answer::Choice { choice: 0 }).is_err());
        assert_eq!(notices.borrow().last().unwrap().0, 2);
        assert!(notices.borrow().last().unwrap().1.is_none());
    }

    fn credentials(anonymous: bool) -> Answer {
        Answer::Password {
            username: "private-user".into(),
            domain: "private-domain".into(),
            password: "private-secret".into(),
            anonymous,
            pim: 0,
            hidden_volume: false,
            system_volume: false,
            save: Remember::Never,
        }
    }

    #[test]
    fn credentials_are_applied_to_the_actual_operation_without_password_saving() {
        let interaction = Interaction::new(|_, _| {});
        let replied = Rc::new(Cell::new(false));
        let observed = replied.clone();
        interaction.operation.connect_reply(move |op, result| {
            assert_eq!(result, gio::MountOperationResult::Handled);
            assert_eq!(op.username().as_deref(), Some("private-user"));
            assert_eq!(op.domain().as_deref(), Some("private-domain"));
            assert_eq!(op.password().as_deref(), Some("private-secret"));
            assert_eq!(op.password_save(), gio::PasswordSave::Never);
            observed.set(true);
        });
        interaction.operation.emit_by_name::<()>(
            "ask-password",
            &[
                &"Unlock private device",
                &"",
                &"",
                &(gio::AskPasswordFlags::NEED_PASSWORD
                    | gio::AskPasswordFlags::NEED_USERNAME
                    | gio::AskPasswordFlags::NEED_DOMAIN),
            ],
        );
        assert!(interaction.answer(1, credentials(true)).is_err());
        assert!(!replied.get());
        interaction.answer(1, credentials(false)).unwrap();
        assert!(replied.get());
        interaction.abort();
        assert!(interaction.operation.password().is_none());
    }

    #[test]
    fn abort_and_provider_abort_retire_the_question_and_refuse_late_answers() {
        let interaction = Interaction::new(|_, _| {});
        let replies = Rc::new(RefCell::new(Vec::new()));
        let observed = replies.clone();
        interaction
            .operation
            .connect_reply(move |_, result| observed.borrow_mut().push(result));
        choice(&interaction.operation, &["Continue"]);
        interaction.abort();
        assert_eq!(*replies.borrow(), vec![gio::MountOperationResult::Aborted]);
        assert!(interaction.answer(1, Answer::Choice { choice: 0 }).is_err());
        let provider = Interaction::new(|_, _| {});
        choice(&provider.operation, &["Try again"]);
        provider.operation.emit_by_name::<()>("aborted", &[]);
        assert!(provider.answer(1, Answer::Choice { choice: 0 }).is_err());
        assert_eq!(replies.borrow().len(), 1);
    }

    #[test]
    fn anonymous_connections_discard_credentials_before_the_platform_reply() {
        let interaction = Interaction::new(|_, _| {});
        let replied = Rc::new(Cell::new(false));
        let observed = replied.clone();
        interaction
            .operation
            .connect_reply(move |operation, result| {
                assert_eq!(result, gio::MountOperationResult::Handled);
                assert!(operation.is_anonymous());
                assert!(operation.username().is_none());
                assert!(operation.domain().is_none());
                assert!(operation.password().is_none());
                assert_eq!(operation.password_save(), gio::PasswordSave::Never);
                observed.set(true);
            });
        interaction.operation.emit_by_name::<()>(
            "ask-password",
            &[
                &"Private anonymous connection",
                &"",
                &"",
                &(gio::AskPasswordFlags::NEED_PASSWORD
                    | gio::AskPasswordFlags::ANONYMOUS_SUPPORTED),
            ],
        );
        interaction.answer(1, credentials(true)).unwrap();
        assert!(replied.get());
    }

    #[test]
    fn remembered_passwords_are_left_to_gio_only_when_the_backend_can_keep_them() {
        let ask = |interaction: &Interaction, flags: gio::AskPasswordFlags| {
            interaction.operation.emit_by_name::<()>(
                "ask-password",
                &[&"Password for me on box", &"me", &"", &flags],
            );
        };
        let remember = |save| Answer::Password {
            username: "me".into(),
            domain: String::new(),
            password: "private-secret".into(),
            anonymous: false,
            pim: 0,
            hidden_volume: false,
            system_volume: false,
            save,
        };
        let unsupported = Interaction::new(|_, _| {});
        ask(&unsupported, gio::AskPasswordFlags::NEED_PASSWORD);
        assert!(
            unsupported
                .answer(1, remember(Remember::Permanently))
                .is_err(),
            "a backend that cannot keep it never receives a save request"
        );
        for (save, expected) in [
            (Remember::Permanently, gio::PasswordSave::Permanently),
            (Remember::ForSession, gio::PasswordSave::ForSession),
            (Remember::Never, gio::PasswordSave::Never),
        ] {
            let interaction = Interaction::new(|_, _| {});
            let saved = Rc::new(Cell::new(None));
            let observed = saved.clone();
            interaction
                .operation
                .connect_reply(move |op, _| observed.set(Some(op.password_save())));
            ask(
                &interaction,
                gio::AskPasswordFlags::NEED_PASSWORD | gio::AskPasswordFlags::SAVING_SUPPORTED,
            );
            interaction.answer(1, remember(save)).unwrap();
            assert_eq!(saved.get(), Some(expected));
        }
    }

    #[test]
    fn blocking_process_questions_use_the_same_generation_bound_choices() {
        let notices = Rc::new(RefCell::new(Vec::new()));
        let observed = notices.clone();
        let interaction =
            Interaction::new(move |id, question| observed.borrow_mut().push((id, question)));
        // We render the platform's message and choices, never kill listed PIDs.
        // A null GArray is enough to exercise the actual registered GIO signal.
        let processes = glib::Value::from_type(glib::Type::from_name("GArray").unwrap());
        interaction.operation.emit_by_name_with_values(
            "show-processes",
            &[
                "Private device is busy".to_value(),
                processes,
                glib::StrV::from(["Cancel", "Try again"]).to_value(),
            ],
        );
        assert!(
            matches!(notices.borrow().last(), Some((1, Some(Question::Choice { blocked: true, choices, .. }))) if choices.len() == 2)
        );
        interaction.answer(1, Answer::Choice { choice: 1 }).unwrap();
        assert_eq!(interaction.operation.choice(), 1);
    }
}
