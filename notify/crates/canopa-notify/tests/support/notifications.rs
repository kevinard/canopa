#![allow(
    dead_code,
    reason = "Shared fixtures are also imported directly by individual test binaries"
)]

use canopa_notify::NotificationInput;

pub fn notification(app_name: impl Into<String>, summary: impl Into<String>) -> NotificationInput {
    NotificationInput {
        app_name: app_name.into(),
        summary: summary.into(),
        ..NotificationInput::default()
    }
}

pub fn notification_with_body(
    app_name: impl Into<String>,
    summary: impl Into<String>,
    body: impl Into<String>,
) -> NotificationInput {
    NotificationInput {
        body: body.into(),
        ..notification(app_name, summary)
    }
}

pub fn blank_notification(app_name: impl Into<String>) -> NotificationInput {
    NotificationInput {
        app_name: app_name.into(),
        summary: "   ".to_string(),
        body: "\n\t".to_string(),
        ..NotificationInput::default()
    }
}
