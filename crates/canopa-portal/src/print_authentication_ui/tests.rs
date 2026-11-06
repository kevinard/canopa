use super::*;
use crate::print::authentication::{Endpoint, Transport};
fn form() -> Form {
    Form::new(
        Request {
            context: Context {
                app_id: "org.canopa.Test".into(),
                parent_window: String::new(),
                title: "Document".into(),
                modal: true,
            },
            channel: 3,
            challenge: Challenge {
                version: 1,
                interactive: true,
                id: 1,
                endpoint: Endpoint {
                    server: "office.example".into(),
                    port: 631,
                    transport: Transport::Tls,
                },
                kind: Kind::Server {
                    limits: crate::print::authentication::ServerLimits::Legacy,
                    realm: "Office".into(),
                    resource: "/".into(),
                },
                attempt: 1,
                suggested_user: "initial".into(),
            },
        },
        Theme::Dark,
        Arc::default(),
    )
}
#[test]
fn opening_and_editing_do_not_approve_and_explicit_authenticate_moves_secret_values() {
    let mut f = form();
    let _ = f.update(Message::Opened);
    assert!(!f.completed);
    let message = Message::Value(1, Arc::new(Secret::from_string("synthetic-private".into())));
    assert!(!format!("{message:?}").contains("synthetic-private"));
    let _ = f.update(message);
    assert!(f.output.lock().unwrap().is_none());
    let _ = f.update(Message::Authenticate);
    assert!(f.completed && f.values.is_empty());
    let _ = f.view(window::Id::unique());
    let values = f.output.lock().unwrap().take().unwrap();
    assert_eq!(values.values[1].bytes(), b"synthetic-private");
    let _ = f.update(Message::Cancel);
    assert!(f.output.lock().unwrap().is_none());
}
#[test]
fn invalid_values_cannot_authenticate_and_cancel_clears_the_form() {
    let mut f = form();
    let _ = f.update(Message::Value(
        1,
        Arc::new(Secret::from_string("x".repeat(190))),
    ));
    assert!(!f.valid());
    let _ = f.update(Message::Authenticate);
    assert!(!f.completed && f.output.lock().unwrap().is_none());
    let _ = f.update(Message::Cancel);
    assert!(f.completed && f.values.is_empty() && f.output.lock().unwrap().is_none());
    let _ = f.view(window::Id::unique());
    let _ = f.update(Message::Authenticate);
    assert!(f.output.lock().unwrap().is_none());
}

#[test]
fn cache_only_challenges_cannot_open_an_authentication_surface() {
    let mut f = form();
    assert!(f.request.validate().is_ok());
    f.request.challenge.interactive = false;
    assert!(f.request.validate().is_err());
}

#[test]
fn credential_form_uses_worker_capacity_in_bytes_without_truncating_input() {
    let mut f = form();
    let user = format!("{}u", "é".repeat(63));
    let password = format!("{}p", "é".repeat(511));
    let _ = f.update(Message::Value(
        0,
        Arc::new(Secret::from_string(user.clone())),
    ));
    let _ = f.update(Message::Value(
        1,
        Arc::new(Secret::from_string(password.clone())),
    ));
    assert!(!f.valid());
    let Kind::Server { limits, .. } = &mut f.request.challenge.kind else {
        unreachable!()
    };
    *limits = crate::print::authentication::ServerLimits::CanopaV1;
    assert!(f.valid());
    let _ = f.update(Message::Authenticate);
    let values = f.output.lock().unwrap().take().unwrap();
    assert_eq!(values.values[0].bytes(), user.as_bytes());
    assert_eq!(values.values[1].bytes(), password.as_bytes());
}
