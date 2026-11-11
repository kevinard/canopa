use super::{Element, Message, State, column, field, settings_group, text};

impl State {
    pub(super) fn shortcuts_view(&self) -> Element<'_, Message> {
        let editor = match &self.shortcuts {
            None => return text("Loading keyboard shortcuts…").into(),
            Some(Err(error)) => {
                return settings_group(
                    "Shortcuts unavailable",
                    text(error).style(canopa_ui::muted),
                );
            }
            Some(Ok(editor)) => editor,
        };
        let mut content = column![
            text("Edit the keyboard shortcuts shipped with Canopa. Use Niri key names such as Mod+Shift+Space. Mod is normally Super. Leave a shortcut empty to turn it off. Shortcuts you have not changed follow Canopa updates."),
            field(
                "settings-shortcut-search",
                "Search shortcuts",
                &self.shortcut_query,
                Message::ShortcutSearch,
            ),
        ].spacing(14);
        if editor.migration_pending() {
            content = content.push(text(
                "Your shortcuts are stored as a full copy of an older set of defaults. Saving keeps only your changes, so new and updated default shortcuts reach you again. A backup of niri.kdl is kept next to it.",
            ));
        }
        let editable = self.paths.active() && !self.busy;
        if editable {
            content = content.push(self.action(
                "settings-button-reset-to-defaults",
                "Reset to defaults",
                Some(Message::ResetShortcuts),
                false,
            ));
        } else if !self.paths.active() {
            content = content.push(text(
                "Log into the Canopa session to save keyboard shortcuts.",
            ));
        }
        let query = self.shortcut_query.trim().to_lowercase();
        let mut found = false;
        for (index, shortcut) in editor.rows.iter().enumerate() {
            if !shortcut.label.to_lowercase().contains(&query)
                && !shortcut.saved_key.to_lowercase().contains(&query)
            {
                continue;
            }
            found = true;
            content = content.push(if editable || self.busy {
                field(
                    format!("settings-shortcut-{index}"),
                    &shortcut.label,
                    &shortcut.key,
                    move |value| Message::ShortcutKey(index, value),
                )
            } else {
                text(format!(
                    "{} · {}",
                    shortcut.label,
                    if shortcut.key.is_empty() {
                        "Off"
                    } else {
                        &shortcut.key
                    }
                ))
                .into()
            });
        }
        if !found {
            content = content.push(text("No shortcuts match your search."));
        }
        content.into()
    }
}
