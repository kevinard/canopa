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

pub fn battery_default() -> Option<FormatTemplate> {
    Some(template(
        "{value}% {state}",
        &["icon", "value", "percent", "state"],
    ))
}

pub fn volume_default() -> Option<FormatTemplate> {
    Some(template("{value}%", &["icon", "value", "percent", "state"]))
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
