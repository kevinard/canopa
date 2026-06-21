use crate::{
    AcceptedNotification, DeliveryOutcome, NotificationImageData, NotificationInput,
    NotificationReceipt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NotificationIntakeResult {
    Accepted(AcceptedNotificationInput),
    NoOpDropped(NotificationReceipt),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcceptedNotificationInput {
    pub notification: AcceptedNotification,
    pub image_data: Option<NotificationImageData>,
}

pub(crate) fn intake_notification(input: NotificationInput) -> NotificationIntakeResult {
    let app_identity = derive_app_identity(&input);
    let body = plain_text_body(&input.body);
    let has_image = input.image_data.is_some() || input.image_reference.is_some();

    if input.summary.trim().is_empty()
        && body.trim().is_empty()
        && input.actions.is_empty()
        && !has_image
        && input.category.is_none()
        && input.progress.is_none()
    {
        return NotificationIntakeResult::NoOpDropped(NotificationReceipt {
            notification_id: 0,
            delivery_outcome: DeliveryOutcome::Dropped,
        });
    }

    NotificationIntakeResult::Accepted(AcceptedNotificationInput {
        notification: AcceptedNotification {
            app_identity,
            app_name: input.app_name,
            replaces_id: input.replaces_id,
            app_icon: input.app_icon,
            summary: input.summary,
            body,
            raw_body: input.body,
            urgency: input.urgency,
            category: input.category,
            progress: input.progress,
            image_reference: input.image_reference,
            has_image_payload: false,
            actions: input.actions,
            resident: input.resident,
        },
        image_data: input.image_data,
    })
}

fn derive_app_identity(input: &NotificationInput) -> String {
    input
        .desktop_entry
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(input.app_name.as_str())
        .to_string()
}

fn plain_text_body(body: &str) -> String {
    let mut output = String::new();
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag = String::new();
            for tag_ch in chars.by_ref() {
                if tag_ch == '>' {
                    break;
                }
                tag.push(tag_ch);
            }

            let tag_name = tag
                .trim()
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches('/');

            if matches!(tag_name.to_ascii_lowercase().as_str(), "br" | "p" | "div") {
                push_newline(&mut output);
            }
            continue;
        }

        output.push(ch);
    }

    decode_basic_entities(&output)
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn push_newline(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NotificationAction;

    #[test]
    fn intake_derives_app_identity_from_trimmed_desktop_entry() {
        let result = intake_notification(NotificationInput {
            app_name: "Example Mail".to_string(),
            desktop_entry: Some("  example-mail.desktop  ".to_string()),
            summary: "Inbox".to_string(),
            ..NotificationInput::default()
        });

        let accepted = accepted(result);
        assert_eq!(accepted.app_identity, "example-mail.desktop");
        assert_eq!(accepted.app_name, "Example Mail");
    }

    #[test]
    fn intake_falls_back_to_app_name_without_trimming_display_metadata() {
        let result = intake_notification(NotificationInput {
            app_name: "  Sender Name  ".to_string(),
            desktop_entry: Some(" \t ".to_string()),
            summary: "Visible".to_string(),
            ..NotificationInput::default()
        });

        let accepted = accepted(result);
        assert_eq!(accepted.app_identity, "  Sender Name  ");
        assert_eq!(accepted.app_name, "  Sender Name  ");
    }

    #[test]
    fn intake_renders_notification_body_while_retaining_raw_body() {
        let result = intake_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            body: "<b>Hello</b><br/>World &amp; friends".to_string(),
            ..NotificationInput::default()
        });

        let accepted = accepted(result);
        assert_eq!(accepted.body, "Hello\nWorld & friends");
        assert_eq!(accepted.raw_body, "<b>Hello</b><br/>World &amp; friends");
    }

    #[test]
    fn intake_accepts_blank_notification_image_notifications() {
        let result = intake_notification(NotificationInput {
            app_name: "Blank Sender".to_string(),
            summary: "   ".to_string(),
            body: "\n\t".to_string(),
            image_data: Some(test_image_data()),
            ..NotificationInput::default()
        });

        match result {
            NotificationIntakeResult::Accepted(input) => {
                assert_eq!(input.notification.summary, "   ");
                assert!(!input.notification.has_image_payload);
                assert!(input.image_data.is_some());
            }
            NotificationIntakeResult::NoOpDropped(_) => panic!("expected accepted intake result"),
        }
    }

    #[test]
    fn intake_rejects_blank_app_icon_only_notifications_as_no_op() {
        let result = intake_notification(NotificationInput {
            app_name: "Blank Sender".to_string(),
            app_icon: crate::NotificationVisualReference::icon_name("mail-unread"),
            ..NotificationInput::default()
        });

        assert_eq!(
            result,
            NotificationIntakeResult::NoOpDropped(NotificationReceipt {
                notification_id: 0,
                delivery_outcome: DeliveryOutcome::Dropped,
            })
        );
    }

    #[test]
    fn intake_accepts_actions_category_or_progress_as_meaningful() {
        for input in [
            NotificationInput {
                app_name: "Action Sender".to_string(),
                actions: vec![NotificationAction::new("open", "Open")],
                ..NotificationInput::default()
            },
            NotificationInput {
                app_name: "Category Sender".to_string(),
                category: Some("transfer".to_string()),
                ..NotificationInput::default()
            },
            NotificationInput {
                app_name: "Progress Sender".to_string(),
                progress: Some(95),
                ..NotificationInput::default()
            },
        ] {
            assert!(matches!(
                intake_notification(input),
                NotificationIntakeResult::Accepted(_)
            ));
        }
    }

    fn accepted(result: NotificationIntakeResult) -> AcceptedNotification {
        match result {
            NotificationIntakeResult::Accepted(input) => input.notification,
            NotificationIntakeResult::NoOpDropped(_) => panic!("expected accepted intake result"),
        }
    }

    fn test_image_data() -> NotificationImageData {
        NotificationImageData::from_freedesktop(1, 1, 4, true, 8, 4, vec![255, 0, 0, 255]).unwrap()
    }
}
