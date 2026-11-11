use super::{Element, Message, State, column, field, settings_group, text};
use canopa_ui::direction;

impl State {
    pub(super) fn shortcuts_view(&self) -> Element<'_, Message> {
        let editor = match &self.shortcuts {
            None => return text(fl!("settings-shortcuts-loading")).into(),
            Some(Err(error)) => {
                return settings_group(
                    fl!("settings-shortcuts-unavailable"),
                    text(error).style(canopa_ui::muted),
                );
            }
            Some(Ok(editor)) => editor,
        };
        let mut content = column![
            text(fl!("settings-shortcuts-intro")),
            field(
                "settings-shortcut-search",
                fl!("settings-shortcuts-search"),
                &self.shortcut_query,
                Message::ShortcutSearch,
            ),
        ]
        .spacing(14);
        if editor.migration_pending() {
            content = content.push(text(fl!("settings-shortcuts-migration")));
        }
        let editable = self.paths.active() && !self.busy;
        if editable {
            content = content.push(self.action(
                "settings-button-reset-to-defaults",
                fl!("settings-shortcuts-reset"),
                Some(Message::ResetShortcuts),
                false,
            ));
        } else if !self.paths.active() {
            content = content.push(text(fl!("settings-shortcuts-needs-session")));
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
                    shortcut.label.clone(),
                    &shortcut.key,
                    move |value| Message::ShortcutKey(index, value),
                )
            } else {
                text(if shortcut.key.is_empty() {
                    fl!("settings-shortcut-off", action = shortcut.label.as_str())
                } else {
                    fl!(
                        "settings-shortcut-key",
                        action = shortcut.label.as_str(),
                        key = shortcut.key.as_str()
                    )
                })
                .width(iced::Length::Fill)
                .align_x(direction::text_start())
                .into()
            });
        }
        if !found {
            content = content.push(text(fl!("settings-shortcuts-none-match")));
        }
        content.into()
    }
}
