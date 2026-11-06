use super::{Element, Message, State, column, field_id, text};

impl State {
    pub(super) fn shortcuts_view(&self) -> Element<'_, Message> {
        let editor = match &self.shortcuts {
            None => return text("Loading keyboard shortcuts…").into(),
            Some(Err(error)) => return text(error).into(),
            Some(Ok(editor)) => editor,
        };
        let mut content = column![
            text("Edit the keyboard shortcuts shipped with Canopa. Use Niri key names such as Mod+Shift+Space. Mod is normally Super."),
            field_id(
                "settings-shortcut-search",
                "Search shortcuts",
                &self.shortcut_query,
                Message::ShortcutSearch,
            ),
        ].spacing(14);
        let editable = self.paths.active() && !self.busy;
        if editable {
            content = content.push(self.action(
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
                field_id(
                    format!("settings-shortcut-{index}"),
                    &shortcut.label,
                    &shortcut.key,
                    move |value| Message::ShortcutKey(index, value),
                )
            } else {
                text(format!("{} · {}", shortcut.label, shortcut.key)).into()
            });
        }
        if !found {
            content = content.push(text("No shortcuts match your search."));
        }
        content.into()
    }
}
