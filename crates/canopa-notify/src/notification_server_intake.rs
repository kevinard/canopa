use crate::{
    NotificationAction, NotificationImageData, NotificationInput, NotificationPosition,
    NotificationSound, NotificationTimeout, NotificationVisualReference, Urgency,
};
use std::collections::{HashMap, HashSet};
use zbus::zvariant::OwnedValue;

#[derive(Clone, Debug)]
pub struct NotificationServerRequest {
    pub app_name: String,
    pub replaces_id: u32,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<String>,
    pub hints: HashMap<String, OwnedValue>,
    pub expire_timeout: i32,
}

impl NotificationServerRequest {
    /// Decode content only. The transport must resolve `replaces_id` against its
    /// live registry and assign the internal replacement target separately.
    /// Oversized text/action input is rejected before normalization or hint decoding.
    pub fn into_notification_input(
        self,
    ) -> Result<NotificationInput, crate::NotificationLimitExceeded> {
        crate::input_limits::validate_server_request(&self)?;
        let Self {
            app_name,
            replaces_id: _,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        } = self;
        let (image_data, image_reference) = notification_image(&hints);
        let action_icons = bool_hint(&hints, "action-icons");

        let input = NotificationInput {
            app_name,
            desktop_entry: desktop_entry_hint(&hints),
            replaces_id: None,
            app_icon: NotificationVisualReference::from_sender_reference(&app_icon),
            summary,
            body,
            expire_timeout: NotificationTimeout::from_freedesktop_expire_timeout(expire_timeout),
            urgency: urgency_hint(&hints),
            actions: notification_actions(actions, action_icons),
            category: string_hint(&hints, "category"),
            progress: progress_hint(&hints),
            resident: resident_hint(&hints),
            transient: bool_hint(&hints, "transient"),
            suppress_sound: bool_hint(&hints, "suppress-sound"),
            sound: sound_hint(&hints),
            requested_position: requested_position_hint(&hints),
            image_data,
            image_reference,
        };
        crate::input_limits::validate_input(&input)?;
        Ok(input)
    }
}

fn desktop_entry_hint(hints: &HashMap<String, OwnedValue>) -> Option<String> {
    string_hint(hints, "desktop-entry")
}

fn string_hint(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    hints
        .get(key)
        .and_then(|value| value.downcast_ref::<String>().ok())
}

fn urgency_hint(hints: &HashMap<String, OwnedValue>) -> Urgency {
    match hints
        .get("urgency")
        .and_then(|value| value.downcast_ref::<u8>().ok())
    {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

fn progress_hint(hints: &HashMap<String, OwnedValue>) -> Option<i64> {
    hints
        .get("value")
        .and_then(|value| value.downcast_ref::<i32>().ok())
        .map(i64::from)
}

fn resident_hint(hints: &HashMap<String, OwnedValue>) -> bool {
    bool_hint(hints, "resident")
}

fn bool_hint(hints: &HashMap<String, OwnedValue>, key: &str) -> bool {
    hints
        .get(key)
        .and_then(|value| value.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

fn sound_hint(hints: &HashMap<String, OwnedValue>) -> Option<NotificationSound> {
    string_hint(hints, "sound-file")
        .and_then(NotificationSound::file)
        .or_else(|| string_hint(hints, "sound-name").and_then(NotificationSound::name))
}

fn requested_position_hint(hints: &HashMap<String, OwnedValue>) -> Option<NotificationPosition> {
    let x = hints
        .get("x")
        .and_then(|value| value.downcast_ref::<i32>().ok())?;
    let y = hints
        .get("y")
        .and_then(|value| value.downcast_ref::<i32>().ok())?;
    Some(NotificationPosition { x, y })
}

fn notification_image(
    hints: &HashMap<String, OwnedValue>,
) -> (
    Option<NotificationImageData>,
    Option<NotificationVisualReference>,
) {
    if let Some(image_data) = raw_image_hint(hints, &["image-data", "image_data"]) {
        return (Some(image_data), None);
    }

    if let Some(image_reference) = image_reference_hint(hints) {
        return (None, Some(image_reference));
    }

    (raw_image_hint(hints, &["icon_data"]), None)
}

fn raw_image_hint(
    hints: &HashMap<String, OwnedValue>,
    keys: &[&str],
) -> Option<NotificationImageData> {
    for key in keys {
        let Some(value) = hints.get(*key) else {
            continue;
        };
        let Ok((width, height, rowstride, has_alpha, bits_per_sample, channels, data)) =
            value.downcast_ref::<(i32, i32, i32, bool, i32, i32, Vec<u8>)>()
        else {
            continue;
        };

        if let Some(image) = NotificationImageData::from_freedesktop(
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        ) {
            return Some(image);
        }
    }

    None
}

fn image_reference_hint(
    hints: &HashMap<String, OwnedValue>,
) -> Option<NotificationVisualReference> {
    string_hint(hints, "image-path")
        .or_else(|| string_hint(hints, "image_path"))
        .and_then(|value| NotificationVisualReference::from_sender_reference(&value))
}

fn notification_actions(actions: Vec<String>, action_icons: bool) -> Vec<NotificationAction> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for chunk in actions.chunks(2) {
        let [key, label] = chunk else {
            continue;
        };
        let label = label.trim();
        if key.is_empty() || (label.is_empty() && key != "default") || !seen.insert(key.clone()) {
            continue;
        }

        let mut action = NotificationAction::new(key, label);
        if action_icons && let Some(icon) = NotificationVisualReference::icon_name(key) {
            action = action.with_icon(icon);
        }
        normalized.push(action);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{Structure, Value};

    #[test]
    fn maps_freedesktop_request_into_notification_input() {
        let input = NotificationServerRequest {
            app_name: "Example Mail".to_string(),
            replaces_id: 42,
            app_icon: "mail-unread".to_string(),
            summary: "Inbox".to_string(),
            body: "One unread message".to_string(),
            actions: vec![
                "default".to_string(),
                "Open".to_string(),
                "dismiss".to_string(),
                "Dismiss".to_string(),
                "ignored-tail".to_string(),
            ],
            hints: HashMap::from([
                (
                    "desktop-entry".to_string(),
                    string_value("example-mail.desktop"),
                ),
                ("category".to_string(), string_value("email")),
                ("urgency".to_string(), OwnedValue::from(2_u8)),
                ("value".to_string(), OwnedValue::from(73_i32)),
            ]),
            expire_timeout: 2_500,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(input.app_name, "Example Mail");
        assert_eq!(input.desktop_entry.as_deref(), Some("example-mail.desktop"));
        assert_eq!(input.replaces_id, None);
        assert_eq!(input.summary, "Inbox");
        assert_eq!(input.body, "One unread message");
        assert_eq!(
            input.expire_timeout,
            NotificationTimeout::Milliseconds(2_500)
        );
        assert_eq!(input.urgency, Urgency::Critical);
        assert_eq!(input.category.as_deref(), Some("email"));
        assert_eq!(input.progress, Some(73));
        assert_eq!(
            input.app_icon,
            NotificationVisualReference::icon_name("mail-unread")
        );
        assert_eq!(
            input
                .actions
                .iter()
                .map(|action| (action.key.as_str(), action.label.as_str()))
                .collect::<Vec<_>>(),
            vec![("default", "Open"), ("dismiss", "Dismiss")]
        );
    }

    #[test]
    fn normalizes_freedesktop_action_pairs_into_valid_first_seen_actions() {
        let input = NotificationServerRequest {
            app_name: "Example Mail".to_string(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: "Inbox".to_string(),
            body: String::new(),
            actions: vec![
                "".to_string(),
                "Empty key".to_string(),
                "open".to_string(),
                "  Open   now  ".to_string(),
                "reply".to_string(),
                "   ".to_string(),
                "open".to_string(),
                "Second open label".to_string(),
                "Open".to_string(),
                "Case distinct key".to_string(),
                "tail".to_string(),
            ],
            hints: HashMap::new(),
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(
            input
                .actions
                .iter()
                .map(|action| (action.key.as_str(), action.label.as_str()))
                .collect::<Vec<_>>(),
            vec![("open", "Open   now"), ("Open", "Case distinct key")]
        );
    }

    #[test]
    fn maps_action_icons_hint_to_action_icon_references() {
        let input = NotificationServerRequest {
            app_name: "Example Mail".to_string(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: "Inbox".to_string(),
            body: String::new(),
            actions: vec![
                "mail-reply-sender".to_string(),
                "Reply".to_string(),
                "mail-archive".to_string(),
                "Archive".to_string(),
            ],
            hints: HashMap::from([("action-icons".to_string(), OwnedValue::from(true))]),
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(
            input.actions[0].icon,
            NotificationVisualReference::icon_name("mail-reply-sender")
        );
        assert_eq!(
            input.actions[1].icon,
            NotificationVisualReference::icon_name("mail-archive")
        );
    }

    #[test]
    fn applies_freedesktop_default_values() {
        let input = NotificationServerRequest {
            app_name: "Calendar".to_string(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: "Meeting".to_string(),
            body: String::new(),
            actions: Vec::new(),
            hints: HashMap::new(),
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(input.desktop_entry, None);
        assert_eq!(input.replaces_id, None);
        assert_eq!(input.expire_timeout, NotificationTimeout::ServerDefault);
        assert_eq!(input.urgency, Urgency::Normal);
        assert_eq!(input.category, None);
        assert_eq!(input.progress, None);
        assert_eq!(input.app_icon, None);
        assert!(input.actions.is_empty());
    }

    #[test]
    fn decodes_only_boolean_true_resident_hint() {
        assert!(
            request_with_hints(HashMap::from([(
                "resident".to_string(),
                OwnedValue::from(true)
            )]))
            .resident
        );
        assert!(
            !request_with_hints(HashMap::from([(
                "resident".to_string(),
                OwnedValue::from(false)
            )]))
            .resident
        );
        assert!(!request_with_hints(HashMap::new()).resident);
        assert!(
            !request_with_hints(HashMap::from([(
                "resident".to_string(),
                string_value("true")
            )]))
            .resident
        );
    }

    #[test]
    fn decodes_optional_behavior_hints_without_requiring_presentation_support() {
        let input = request_with_hints(HashMap::from([
            ("transient".to_string(), OwnedValue::from(true)),
            ("suppress-sound".to_string(), OwnedValue::from(true)),
            (
                "sound-name".to_string(),
                string_value("message-new-instant"),
            ),
            ("sound-file".to_string(), string_value("/tmp/message.oga")),
            ("x".to_string(), OwnedValue::from(32_i32)),
            ("y".to_string(), OwnedValue::from(64_i32)),
        ]));

        assert!(input.transient);
        assert!(input.suppress_sound);
        assert_eq!(
            input.sound,
            Some(NotificationSound::File("/tmp/message.oga".to_string()))
        );
        assert_eq!(
            input.requested_position,
            Some(NotificationPosition { x: 32, y: 64 })
        );
    }

    #[test]
    fn maps_freedesktop_visual_hints_into_notification_input() {
        let input = NotificationServerRequest {
            app_name: "Photos".to_string(),
            replaces_id: 0,
            app_icon: "file:///tmp/photos.png".to_string(),
            summary: "Screenshot".to_string(),
            body: String::new(),
            actions: Vec::new(),
            hints: HashMap::from([
                (
                    "image-data".to_string(),
                    OwnedValue::try_from(Value::from(Structure::from((
                        1_i32,
                        1_i32,
                        4_i32,
                        true,
                        8_i32,
                        4_i32,
                        vec![1_u8, 2, 3, 4],
                    ))))
                    .unwrap(),
                ),
                (
                    "image-path".to_string(),
                    string_value("file:///tmp/fallback.jpg"),
                ),
            ]),
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(
            input.app_icon,
            NotificationVisualReference::local_path("/tmp/photos.png")
        );
        assert!(input.image_data.is_some());
        assert_eq!(input.image_reference, None);
    }

    #[test]
    fn maps_image_path_when_raw_image_data_is_missing() {
        let input = NotificationServerRequest {
            app_name: "Photos".to_string(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: "Screenshot".to_string(),
            body: String::new(),
            actions: Vec::new(),
            hints: HashMap::from([(
                "image_path".to_string(),
                string_value("file:///tmp/screenshot.jpg"),
            )]),
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap();

        assert_eq!(input.image_data, None);
        assert_eq!(
            input.image_reference,
            NotificationVisualReference::local_path("/tmp/screenshot.jpg")
        );
    }

    fn request_with_hints(hints: HashMap<String, OwnedValue>) -> NotificationInput {
        NotificationServerRequest {
            app_name: "Calendar".to_string(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: "Meeting".to_string(),
            body: String::new(),
            actions: Vec::new(),
            hints,
            expire_timeout: -1,
        }
        .into_notification_input()
        .unwrap()
    }

    fn string_value(value: &'static str) -> OwnedValue {
        OwnedValue::try_from(Value::from(value)).unwrap()
    }
}
