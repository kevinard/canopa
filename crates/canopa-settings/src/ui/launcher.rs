use super::*;
use iced::widget::column;
impl State {
    pub(super) fn launcher_view(&self) -> Element<'_, Message> {
        let model = &self.launcher;
        let mut body = column![
            text(fl!("settings-launcher-intro")),
            text(fl!("settings-launcher-pin-hint"))
        ]
        .spacing(14);
        if !model.loaded {
            return body
                .push(text(fl!("settings-launcher-loading")))
                .push(self.clipboard_settings())
                .into();
        }
        if model.catalog.extensions.is_empty() {
            body = body.push(text(fl!("settings-launcher-no-extensions")));
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
                    // The extension's own name and description.
                    &ext.name,
                    enabled,
                    Message::LauncherEnabled(id, !enabled)
                ),
                text(&ext.description),
                field(
                    format!("extension-keyword-{}", ext.id),
                    fl!("settings-launcher-keyword"),
                    keyword,
                    move |value| Message::LauncherKeyword(keyword_id.clone(), value)
                )
            ]
            .spacing(8);
            if !ext.actions.is_empty() {
                card = card.push(text(fl!(
                    "settings-launcher-actions",
                    actions = ext.actions.join(" · ")
                )));
            }
            if !ext.status.is_empty() {
                card = card.push(text(fl!(
                    "settings-launcher-status",
                    status = ext.status.as_str()
                )));
            }
            body = body.push(container(card).padding(12).style(canopa_ui::surface));
        }
        body = body
            .push(text(fl!("settings-launcher-web")).size(20))
            .push(text(fl!("settings-launcher-web-hint")))
            .push(field(
                "launcher-search-template",
                fl!("settings-launcher-default-url", placeholder = "{query}"),
                &model.draft.web_template,
                Message::LauncherTemplate,
            ));
        for (i, search) in model.draft.searches.iter().enumerate() {
            body = body.push(
                column![
                    field(
                        format!("search-keyword-{i}"),
                        fl!("settings-launcher-search-keyword"),
                        &search.keyword,
                        move |value| Message::LauncherSearch(i, false, value)
                    ),
                    field(
                        format!("search-template-{i}"),
                        fl!("settings-launcher-search-url", placeholder = "{query}"),
                        &search.template,
                        move |value| Message::LauncherSearch(i, true, value)
                    ),
                    self.action(
                        "settings-button-remove-search-shortcut",
                        fl!("settings-launcher-remove-search"),
                        Some(Message::LauncherRemoveSearch(i)),
                        false
                    )
                ]
                .spacing(8),
            );
        }
        body = body.push(self.action(
            "settings-button-add-search-shortcut",
            fl!("settings-launcher-add-search"),
            (model.draft.searches.len() < 16).then_some(Message::LauncherAddSearch),
            false,
        ));
        if !model.draft.shortcuts.is_empty() {
            body = body.push(text(fl!("settings-launcher-pins")).size(20));
            for s in &model.draft.shortcuts {
                let pinned = if s.pinned { "yes" } else { "no" };
                body = body.push(text(if s.alias.is_empty() {
                    fl!(
                        "settings-launcher-pin",
                        name = s.label.as_str(),
                        pinned = pinned
                    )
                } else {
                    fl!(
                        "settings-launcher-pin-alias",
                        name = s.label.as_str(),
                        pinned = pinned,
                        alias = s.alias.as_str()
                    )
                }));
            }
        }
        body.push(self.clipboard_settings()).into()
    }

    fn clipboard_settings(&self) -> Element<'_, Message> {
        let mut body = column![
            text(fl!("settings-clipboard")).size(20),
            text(fl!("settings-clipboard-hint")),
        ]
        .spacing(12);
        if let Some(editor) = &self.clipboard {
            body = body
                .push(toggle(
                    "launcher-clipboard-recording",
                    fl!("settings-clipboard-record"),
                    editor.recording,
                    Message::ClipboardRecording(!editor.recording),
                ))
                .push(toggle(
                    "launcher-clipboard-images",
                    fl!("settings-clipboard-images"),
                    editor.images,
                    Message::ClipboardImages(!editor.images),
                ))
                .push(field(
                    "launcher-clipboard-limit",
                    fl!("settings-clipboard-limit", limit = "64"),
                    &editor.limit,
                    Message::ClipboardLimit,
                ))
                .push(self.action(
                    "settings-button-save-clipboard-settings",
                    fl!("settings-clipboard-save"),
                    (!self.busy && self.trial.is_none()).then_some(Message::ClipboardSave),
                    true,
                ));
        } else if self.clipboard_notice.is_empty() {
            body = body.push(text(fl!("settings-clipboard-loading")));
        }
        if !self.clipboard_notice.is_empty() {
            body = body.push(text(&self.clipboard_notice));
        }
        container(body).padding(12).style(canopa_ui::surface).into()
    }
}
