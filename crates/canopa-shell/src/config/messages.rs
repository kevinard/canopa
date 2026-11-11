//! Message IDs in configuration.
//!
//! The shipped defaults name Shell messages (`label-id "shell-module-lock"`,
//! `item label-id="shell-menu-cancel"`) instead of English text, so built-in
//! buttons, menus and tooltips follow the person's language. A person's own
//! `label`, `title`, `prompt` or `tooltip` stays literal. IDs resolve when the
//! configuration is loaded, in the language the Shell started with.

/// The text of the Shell message `id`, or `None` when the Shell has no such
/// message.
pub(crate) fn text(id: &str) -> Option<String> {
    let messages = crate::i18n::LOADER.get();
    messages.has(id).then(|| messages.get(id))
}

/// The text of a message used as a format template: each `{ $name }`
/// variable becomes the template placeholder `{name}`, so
/// `shell-tooltip-workspace = Workspace { $index }` gives `Workspace {index}`.
pub(crate) fn template(id: &str, placeholders: &[&str]) -> Option<String> {
    let messages = crate::i18n::LOADER.get();
    if !messages.has(id) {
        return None;
    }
    let args: std::collections::HashMap<&str, String> = placeholders
        .iter()
        .map(|name| (*name, format!("{{{name}}}")))
        .collect();
    // Bidi isolation marks around placeables would end up in the template.
    Some(
        messages
            .get_args(id, args)
            .chars()
            .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
            .collect(),
    )
}

/// The help shown for an unknown message ID.
pub(crate) fn unknown_help() -> String {
    "message IDs name the Shell's built-in texts; see Translated labels in docs/configuration.md, or use a literal value"
        .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_resolve_in_the_shell_catalog() {
        assert_eq!(super::text("shell-menu-cancel").as_deref(), Some("Cancel"));
        assert_eq!(super::text("shell-no-such-message"), None);
        assert_eq!(
            super::template("shell-tooltip-workspace", &["index", "name"]).as_deref(),
            Some("Workspace {index}")
        );
    }
}
