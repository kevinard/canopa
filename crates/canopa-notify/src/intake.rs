use crate::{
    AcceptedNotification, DeliveryOutcome, NotificationBodyMarkup, NotificationBodyRun,
    NotificationBodyStyle, NotificationImageData, NotificationInput, NotificationReceipt,
};

const MAX_BODY_MARKUP_RUNS: usize = 256;

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
    let body_markup = interpreted_body_markup(&input.body);
    let body = body_markup.plain_text();
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
            body_markup,
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

pub(crate) fn interpreted_body_markup(body: &str) -> NotificationBodyMarkup {
    let mut parser = BodyMarkupParser::new(body);
    parser.parse();
    let runs = styled_chars_to_runs(parser.output);
    let markup = NotificationBodyMarkup::from_runs(runs);

    if markup.runs.len() > MAX_BODY_MARKUP_RUNS {
        NotificationBodyMarkup::plain(markup.plain_text())
    } else {
        markup
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StyleCounts {
    bold: u16,
    italic: u16,
    underline: u16,
}

impl StyleCounts {
    fn style(self) -> NotificationBodyStyle {
        NotificationBodyStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            underline: self.underline > 0,
        }
    }

    fn open(&mut self, tag_name: &str) {
        match tag_name {
            "b" => self.bold = self.bold.saturating_add(1),
            "i" => self.italic = self.italic.saturating_add(1),
            "u" => self.underline = self.underline.saturating_add(1),
            _ => {}
        }
    }

    fn close(&mut self, tag_name: &str) {
        match tag_name {
            "b" => self.bold = self.bold.saturating_sub(1),
            "i" => self.italic = self.italic.saturating_sub(1),
            "u" => self.underline = self.underline.saturating_sub(1),
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StyledChar {
    ch: char,
    style: NotificationBodyStyle,
}

struct BodyMarkupParser<'a> {
    input: &'a str,
    index: usize,
    output: Vec<StyledChar>,
    style: StyleCounts,
}

impl<'a> BodyMarkupParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            index: 0,
            output: Vec::new(),
            style: StyleCounts::default(),
        }
    }

    fn parse(&mut self) {
        while self.index < self.input.len() {
            if self.remaining().starts_with('<') {
                self.parse_tag_or_discard_markup_start();
            } else {
                self.parse_text();
            }
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.index..]
    }

    fn parse_tag_or_discard_markup_start(&mut self) {
        let remaining = self.remaining();
        if let Some(end) = remaining.find('>') {
            let tag = &remaining[1..end];
            self.index += end + 1;
            self.handle_tag(tag);
            return;
        }

        if let Some(tag_name_len) = malformed_known_tag_name_len(remaining) {
            let tag = &remaining[1..1 + tag_name_len];
            self.index += 1 + tag_name_len;
            self.handle_tag(tag);
            return;
        }

        self.index += '<'.len_utf8();
    }

    fn parse_text(&mut self) {
        let remaining = self.remaining();
        let text_end = remaining.find('<').unwrap_or(remaining.len());
        let text = &remaining[..text_end];
        self.index += text_end;
        self.push_text(&decode_xml_entities(text));
    }

    fn handle_tag(&mut self, tag: &str) {
        let tag = tag.trim();
        if tag.is_empty() {
            return;
        }

        let closing = tag.starts_with('/');
        let tag_without_direction = tag.trim_start_matches('/').trim();
        let tag_name = tag_name(tag_without_direction);
        let tag_name = tag_name.to_ascii_lowercase();
        let self_closing = tag_without_direction.trim_end().ends_with('/');

        match tag_name.as_str() {
            "b" | "i" | "u" => {
                if self_closing {
                    return;
                }

                if closing {
                    self.style.close(&tag_name);
                } else {
                    self.style.open(&tag_name);
                }
            }
            "br" => self.push_newline(),
            "p" | "div" => self.push_newline(),
            "img" if !closing => {
                if let Some(alt) = attribute_value(tag_without_direction, "alt") {
                    self.push_text(&alt);
                }
            }
            "a" | "img" => {}
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        let style = self.style.style();
        self.output
            .extend(text.chars().map(|ch| StyledChar { ch, style }));
    }

    fn push_newline(&mut self) {
        if !self.output.last().is_some_and(|ch| ch.ch == '\n') {
            self.output.push(StyledChar {
                ch: '\n',
                style: NotificationBodyStyle::default(),
            });
        }
    }
}

fn malformed_known_tag_name_len(value: &str) -> Option<usize> {
    let after_open = value.strip_prefix('<')?;
    let after_direction = after_open.strip_prefix('/').unwrap_or(after_open);
    let name_len = after_direction
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_alphanumeric())
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if name_len == 0 {
        return None;
    }

    let name = after_direction[..name_len].to_ascii_lowercase();
    matches!(
        name.as_str(),
        "b" | "i" | "u" | "a" | "img" | "br" | "p" | "div"
    )
    .then_some(if after_open.starts_with('/') {
        name_len + 1
    } else {
        name_len
    })
}

fn tag_name(tag: &str) -> &str {
    tag.split(|ch: char| ch.is_whitespace() || ch == '/')
        .next()
        .unwrap_or_default()
}

fn attribute_value(tag: &str, attribute_name: &str) -> Option<String> {
    let mut index = tag_name(tag).len();
    while index < tag.len() {
        index += tag[index..]
            .chars()
            .take_while(|ch| ch.is_whitespace() || *ch == '/')
            .map(char::len_utf8)
            .sum::<usize>();
        if index >= tag.len() {
            break;
        }

        let name_start = index;
        while index < tag.len() {
            let ch = tag[index..].chars().next().unwrap();
            if ch.is_whitespace() || ch == '=' || ch == '/' {
                break;
            }
            index += ch.len_utf8();
        }
        let name = &tag[name_start..index];

        index += tag[index..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        if !tag[index..].starts_with('=') {
            continue;
        }
        index += '='.len_utf8();
        index += tag[index..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();

        let (value, next_index) = attribute_raw_value(tag, index);
        index = next_index;

        if name.eq_ignore_ascii_case(attribute_name) {
            return Some(decode_xml_entities(value));
        }
    }

    None
}

fn attribute_raw_value(tag: &str, index: usize) -> (&str, usize) {
    let Some(first) = tag[index..].chars().next() else {
        return ("", index);
    };

    if first == '"' || first == '\'' {
        let value_start = index + first.len_utf8();
        let rest = &tag[value_start..];
        match rest.find(first) {
            Some(end) => (&rest[..end], value_start + end + first.len_utf8()),
            None => (rest, tag.len()),
        }
    } else {
        let value_start = index;
        let value_len = tag[value_start..]
            .char_indices()
            .take_while(|(_, ch)| !ch.is_whitespace() && *ch != '/')
            .map(|(index, ch)| index + ch.len_utf8())
            .last()
            .unwrap_or(0);
        (
            &tag[value_start..value_start + value_len],
            value_start + value_len,
        )
    }
}

fn decode_xml_entities(value: &str) -> String {
    let mut output = String::new();
    let mut index = 0;

    while index < value.len() {
        let ch = value[index..].chars().next().unwrap();
        if ch == '&' {
            if let Some(end) = value[index + 1..].find(';') {
                let entity_end = index + 1 + end;
                let entity = &value[index + 1..entity_end];
                if let Some(decoded) = decode_xml_entity(entity) {
                    output.push(decoded);
                    index = entity_end + ';'.len_utf8();
                    continue;
                }
            }
        }

        output.push(ch);
        index += ch.len_utf8();
    }

    output
}

fn decode_xml_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        entity => decode_numeric_entity(entity),
    }
}

fn decode_numeric_entity(entity: &str) -> Option<char> {
    let value = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        u32::from_str_radix(hex, 16).ok()?
    } else if let Some(decimal) = entity.strip_prefix('#') {
        decimal.parse::<u32>().ok()?
    } else {
        return None;
    };

    char::from_u32(value)
}

fn styled_chars_to_runs(chars: Vec<StyledChar>) -> Vec<NotificationBodyRun> {
    let mut lines: Vec<Vec<StyledChar>> = vec![Vec::new()];
    for styled in chars {
        if styled.ch == '\n' {
            lines.push(Vec::new());
        } else if let Some(line) = lines.last_mut() {
            line.push(styled);
        }
    }

    let mut normalized = Vec::new();
    for (index, line) in lines.into_iter().enumerate() {
        if index > 0 {
            normalized.push(StyledChar {
                ch: '\n',
                style: NotificationBodyStyle::default(),
            });
        }

        let start = line
            .iter()
            .position(|styled| !styled.ch.is_whitespace())
            .unwrap_or(line.len());
        let end = line
            .iter()
            .rposition(|styled| !styled.ch.is_whitespace())
            .map(|index| index + 1)
            .unwrap_or(start);
        normalized.extend_from_slice(&line[start..end]);
    }

    while normalized
        .first()
        .is_some_and(|styled| styled.ch.is_whitespace())
    {
        normalized.remove(0);
    }
    while normalized
        .last()
        .is_some_and(|styled| styled.ch.is_whitespace())
    {
        normalized.pop();
    }

    let mut runs = Vec::new();
    let mut current_text = String::new();
    let mut current_style: Option<NotificationBodyStyle> = None;

    for styled in normalized {
        match current_style {
            Some(style) if style == styled.style => current_text.push(styled.ch),
            Some(style) => {
                runs.push(NotificationBodyRun::styled(
                    std::mem::take(&mut current_text),
                    style,
                ));
                current_text.push(styled.ch);
                current_style = Some(styled.style);
            }
            None => {
                current_text.push(styled.ch);
                current_style = Some(styled.style);
            }
        }
    }

    if let Some(style) = current_style {
        runs.push(NotificationBodyRun::styled(current_text, style));
    }

    runs
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
        assert_eq!(
            accepted.body_markup,
            NotificationBodyMarkup::from_runs(vec![
                NotificationBodyRun::styled(
                    "Hello",
                    NotificationBodyStyle {
                        bold: true,
                        ..NotificationBodyStyle::default()
                    },
                ),
                NotificationBodyRun::plain("\nWorld & friends"),
            ])
        );
        assert_eq!(accepted.raw_body, "<b>Hello</b><br/>World &amp; friends");
    }

    #[test]
    fn intake_accumulates_nested_body_markup_and_coalesces_runs() {
        let accepted = accepted(intake_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            body: "<B>Bold <i>both</I></b><u> under</u>".to_string(),
            ..NotificationInput::default()
        }));

        assert_eq!(accepted.body, "Bold both under");
        assert_eq!(
            accepted.body_markup.runs,
            vec![
                NotificationBodyRun::styled(
                    "Bold ",
                    NotificationBodyStyle {
                        bold: true,
                        ..NotificationBodyStyle::default()
                    },
                ),
                NotificationBodyRun::styled(
                    "both",
                    NotificationBodyStyle {
                        bold: true,
                        italic: true,
                        ..NotificationBodyStyle::default()
                    },
                ),
                NotificationBodyRun::styled(
                    " under",
                    NotificationBodyStyle {
                        underline: true,
                        ..NotificationBodyStyle::default()
                    },
                ),
            ]
        );
    }

    #[test]
    fn intake_keeps_link_text_and_image_alt_without_targets() {
        let accepted = accepted(intake_notification(NotificationInput {
            app_name: "Reporter".to_string(),
            summary: "Report".to_string(),
            body: r#"<a href="https://example.invalid/report">Open</a> <img src="secret.png" ALT="chart &amp; notes"/>"#.to_string(),
            ..NotificationInput::default()
        }));

        assert_eq!(accepted.body, "Open chart & notes");
        assert_eq!(
            accepted.body_markup,
            NotificationBodyMarkup::plain("Open chart & notes")
        );
        assert!(
            !serde_json::to_string(&accepted.body_markup)
                .unwrap()
                .contains("example.invalid")
        );
        assert!(
            !serde_json::to_string(&accepted.body_markup)
                .unwrap()
                .contains("secret.png")
        );
    }

    #[test]
    fn intake_decodes_numeric_entities_without_decoding_html_named_entities() {
        let accepted = accepted(intake_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            body: "A &#38; B &#x1F44D; &nbsp;".to_string(),
            ..NotificationInput::default()
        }));

        assert_eq!(accepted.body, "A & B 👍 &nbsp;");
    }

    #[test]
    fn intake_degrades_malformed_markup_to_visible_text() {
        let accepted = accepted(intake_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            body: "<b>Hello</i> <wat>friend</wat>".to_string(),
            ..NotificationInput::default()
        }));

        assert_eq!(accepted.body, "Hello friend");
        assert_eq!(
            accepted.body_markup.runs,
            vec![NotificationBodyRun::styled(
                "Hello friend",
                NotificationBodyStyle {
                    bold: true,
                    ..NotificationBodyStyle::default()
                },
            )]
        );
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

    #[test]
    fn intake_uses_visible_body_fallback_for_noop_detection() {
        assert_eq!(
            intake_notification(NotificationInput {
                app_name: "No Visible Body".to_string(),
                body: r#"<b></b><img src="ignored.png"/>"#.to_string(),
                ..NotificationInput::default()
            }),
            NotificationIntakeResult::NoOpDropped(NotificationReceipt {
                notification_id: 0,
                delivery_outcome: DeliveryOutcome::Dropped,
            })
        );

        assert!(matches!(
            intake_notification(NotificationInput {
                app_name: "Visible Alt".to_string(),
                body: r#"<img src="ignored.png" alt="Screenshot ready"/>"#.to_string(),
                ..NotificationInput::default()
            }),
            NotificationIntakeResult::Accepted(_)
        ));
        assert!(matches!(
            intake_notification(NotificationInput {
                app_name: "Visible Link".to_string(),
                body: r#"<a href="ignored">Open report</a>"#.to_string(),
                ..NotificationInput::default()
            }),
            NotificationIntakeResult::Accepted(_)
        ));
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
