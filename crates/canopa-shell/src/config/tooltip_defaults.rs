//! Built-in default tooltip templates per module kind.
//!
//! Returned as `Option<FormatTemplate>` so callers can pass the result
//! directly to `TooltipSetting::resolve`. `None` means "no default" and is
//! treated identically to `TooltipSetting::Disabled` at runtime.

use super::{FormatTemplate, StyledSegment, TextTemplate};

fn template(raw: &str, allowed: &[&str]) -> FormatTemplate {
    let segs = raw
        .split_whitespace()
        .map(|s| {
            StyledSegment::new(
                TextTemplate::parse(s, allowed, "tooltip-default")
                    .expect("built-in default tooltip must parse"),
            )
        })
        .collect();
    FormatTemplate::from_styled(segs)
}

/// A translated default: the message's text with its placeables left as
/// `{name}` placeholders, so a language can place the percent sign
/// (`45 %` in French). Falls back to `english` if the translation is not a
/// valid template.
fn translated(text: String, english: &str, allowed: &[&str]) -> FormatTemplate {
    let text: String = text
        .chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect();
    match TextTemplate::parse(&text, allowed, "tooltip-default") {
        Ok(parsed) => FormatTemplate::from_styled(vec![StyledSegment::new(parsed)]),
        Err(_) => template(english, allowed),
    }
}

pub fn battery_default() -> Option<FormatTemplate> {
    Some(translated(
        fl!(
            "shell-tooltip-battery",
            value = "{value}",
            state = "{state}"
        ),
        "{value}% {state}",
        &["icon", "value", "percent", "state"],
    ))
}

pub fn volume_default() -> Option<FormatTemplate> {
    Some(translated(
        fl!("shell-tooltip-volume", value = "{value}"),
        "{value}%",
        &["icon", "value", "percent", "state"],
    ))
}

pub fn clock_default() -> Option<FormatTemplate> {
    Some(template("{date}", &["icon", "value", "time", "date"]))
}

pub fn notifications_default() -> Option<FormatTemplate> {
    Some(template("{label}", &["icon", "value", "label", "state"]))
}

pub fn idle_inhibitor_default() -> Option<FormatTemplate> {
    Some(template("{label}", &["icon", "value", "label", "state"]))
}

pub fn network_default() -> Option<FormatTemplate> {
    Some(template("{label}", &["icon", "value", "label", "state"]))
}

pub fn focused_window_default() -> Option<FormatTemplate> {
    Some(template("{title}", &["icon", "value", "title"]))
}

pub fn workspaces_default() -> Option<FormatTemplate> {
    None
}

pub fn button_default() -> Option<FormatTemplate> {
    None
}

pub fn script_default() -> Option<FormatTemplate> {
    None
}

pub fn tray_default() -> Option<FormatTemplate> {
    Some(template("{title}", &["title", "description"]))
}

pub fn media_default() -> Option<FormatTemplate> {
    Some(template(
        "{label} · {player} · {state}",
        &["label", "player", "state"],
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_defaults_parse() {
        let _ = super::battery_default();
        let _ = super::volume_default();
        let _ = super::clock_default();
        let _ = super::network_default();
        let _ = super::focused_window_default();
        let _ = super::workspaces_default();
        let _ = super::button_default();
        let _ = super::script_default();
        let _ = super::tray_default();
    }
}
