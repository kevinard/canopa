use super::*;
use iced::widget::column;
impl State {
    pub(super) fn launcher_view(&self) -> Element<'_, Message> {
        let model = &self.launcher;
        let mut body = column![
            text(
                "Enable extensions and choose their launcher keywords. Changes apply automatically."
            ),
            text("Use Tab on a launcher result to pin it or assign an alias.")
        ]
        .spacing(14);
        if !model.loaded {
            return body
                .push(text("Loading launcher preferences…"))
                .push(self.clipboard_settings())
                .into();
        }
        if model.catalog.extensions.is_empty() {
            body = body.push(text(
                "Start Canopa Shell to discover configured extensions and their latest status.",
            ));
        }
        for ext in &model.catalog.extensions {
            let pref = model.draft.extensions.get(&ext.id);
            let enabled = pref.map(|p| p.enabled).unwrap_or(ext.enabled);
            let keyword = pref.map(|p| p.keyword.as_str()).unwrap_or(&ext.keyword);
            let id = ext.id.clone();
            let keyword_id = ext.id.clone();
            let mut card = column![
                toggle(
                    format!("launcher-enabled-{id}"),
                    &ext.name,
                    enabled,
                    Message::LauncherEnabled(id, !enabled)
                ),
                text(&ext.description),
                field(
                    format!("extension-keyword-{}", ext.id),
                    "Keyword",
                    keyword,
                    move |value| Message::LauncherKeyword(keyword_id.clone(), value)
                )
            ]
            .spacing(8);
            if !ext.actions.is_empty() {
                card = card.push(text(format!("Actions: {}", ext.actions.join(" · "))));
            }
            if !ext.status.is_empty() {
                card = card.push(text(format!("Last status: {}", ext.status)));
            }
            body = body.push(container(card).padding(12).style(canopa_ui::surface));
        }
        body=body.push(text("Web searches").size(20)).push(text("Only pasted URLs and explicit search keywords offer browser actions. A request starts when you select the result.")).push(field("launcher-search-template","Default search URL · include {query}",&model.draft.web_template,Message::LauncherTemplate));
        for (i, search) in model.draft.searches.iter().enumerate() {
            body = body.push(
                column![
                    field(
                        format!("search-keyword-{i}"),
                        "Search keyword",
                        &search.keyword,
                        move |value| Message::LauncherSearch(i, false, value)
                    ),
                    field(
                        format!("search-template-{i}"),
                        "Search URL · include {query}",
                        &search.template,
                        move |value| Message::LauncherSearch(i, true, value)
                    ),
                    self.action(
                        "settings-button-remove-search-shortcut",
                        "Remove search shortcut",
                        Some(Message::LauncherRemoveSearch(i)),
                        false
                    )
                ]
                .spacing(8),
            );
        }
        body = body.push(self.action(
            "settings-button-add-search-shortcut",
            "Add search shortcut",
            (model.draft.searches.len() < 16).then_some(Message::LauncherAddSearch),
            false,
        ));
        if !model.draft.shortcuts.is_empty() {
            body = body.push(text("Pins and aliases").size(20));
            for s in &model.draft.shortcuts {
                body = body.push(text(format!(
                    "{}{}{}",
                    s.label,
                    if s.pinned { " · pinned" } else { "" },
                    if s.alias.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", s.alias)
                    }
                )));
            }
        }
        body.push(self.clipboard_settings()).into()
    }

    fn clipboard_settings(&self) -> Element<'_, Message> {
        let mut body = column![
            text("Clipboard history").size(20),
            text("Use clip in Applications to find recent copies. History lasts for this Shell session."),
        ]
        .spacing(12);
        if let Some(editor) = &self.clipboard {
            body = body
                .push(toggle(
                    "launcher-clipboard-recording",
                    "Record clipboard history",
                    editor.recording,
                    Message::ClipboardRecording(!editor.recording),
                ))
                .push(toggle(
                    "launcher-clipboard-images",
                    "Include images when recording",
                    editor.images,
                    Message::ClipboardImages(!editor.images),
                ))
                .push(field(
                    "launcher-clipboard-limit",
                    "Maximum recent entries (1–64)",
                    &editor.limit,
                    Message::ClipboardLimit,
                ))
                .push(self.action(
                    "settings-button-save-clipboard-settings",
                    "Save clipboard settings",
                    (!self.busy && self.trial.is_none()).then_some(Message::ClipboardSave),
                    true,
                ));
        } else if self.clipboard_notice.is_empty() {
            body = body.push(text("Loading clipboard settings…"));
        }
        if !self.clipboard_notice.is_empty() {
            body = body.push(text(&self.clipboard_notice));
        }
        container(body).padding(12).style(canopa_ui::surface).into()
    }
}
