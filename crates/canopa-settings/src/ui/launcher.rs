use super::*;
use iced::widget::column;

/// Search engines offered for the default web search: (name, address).
pub(super) const ENGINES: [(&str, &str); 7] = [
    ("DuckDuckGo", "https://duckduckgo.com/?q={query}"),
    ("Google", "https://www.google.com/search?q={query}"),
    ("Bing", "https://www.bing.com/search?q={query}"),
    ("Brave Search", "https://search.brave.com/search?q={query}"),
    ("Startpage", "https://www.startpage.com/do/search?q={query}"),
    ("Ecosia", "https://www.ecosia.org/search?q={query}"),
    ("Qwant", "https://www.qwant.com/?q={query}"),
];

/// A compact button showing only a glyph, named for assistive technology.
fn icon_button<'a>(
    key: String,
    glyph: &str,
    label: String,
    message: Message,
) -> Element<'a, Message> {
    semantic(
        Node::button(label)
            .key(key.clone())
            .on_click(Some(message.clone())),
        focus::control(
            key,
            Some(message.clone()),
            None,
            None,
            false,
            styled_button(
                kit::glyph(glyph, 16.0, kit::Tint::Muted),
                Some(message),
                kit::Treatment::Flat,
            )
            .padding(7),
        ),
    )
}
impl State {
    pub(super) fn launcher_view(&self) -> Element<'_, Message> {
        let model = &self.launcher;
        if !model.loaded {
            return column![
                settings_group(
                    fl!("settings-launcher-extensions"),
                    kit::caption(fl!("settings-launcher-loading")),
                ),
                self.clipboard_settings(),
            ]
            .spacing(28)
            .into();
        }
        let mut extensions: Vec<Element<'_, Message>> = Vec::new();
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
                kit::caption(ext.description.clone()),
                field(
                    format!("extension-keyword-{}", ext.id),
                    fl!("settings-launcher-keyword"),
                    keyword,
                    move |value| Message::LauncherKeyword(keyword_id.clone(), value)
                )
            ]
            .spacing(8);
            if !ext.actions.is_empty() {
                card = card.push(kit::caption(fl!(
                    "settings-launcher-actions",
                    actions = ext.actions.join(" · ")
                )));
            }
            if !ext.status.is_empty() {
                card = card.push(kit::caption(fl!(
                    "settings-launcher-status",
                    status = ext.status.as_str()
                )));
            }
            extensions.push(card.into());
        }
        // The default search: a listed engine, or an address typed in full.
        let template = model.draft.web_template.trim();
        let engine = ENGINES.iter().position(|(_, address)| *address == template);
        let custom = self.custom_search || engine.is_none();
        let mut web: Vec<Element<'_, Message>> = vec![kit::row(
            fl!("settings-launcher-engine"),
            Some(fl!("settings-launcher-engine-hint")),
            pick(
                "launcher-search-engine",
                ENGINES
                    .iter()
                    .enumerate()
                    .map(|(index, (name, _))| (Some(index), (*name).to_owned()))
                    .chain([(None, fl!("settings-launcher-engine-custom"))])
                    .collect(),
                Some(if custom { None } else { engine }),
                Message::LauncherEngine,
            ),
        )];
        if custom {
            web.push(
                column![
                    super::bare_field(
                        "launcher-search-template",
                        fl!("settings-launcher-default-url", placeholder = "{query}"),
                        "https://example.org/search?q={query}",
                        &model.draft.web_template,
                        Message::LauncherTemplate,
                        Length::Fill,
                    ),
                    kit::caption(fl!(
                        "settings-launcher-default-url",
                        placeholder = "{query}"
                    )),
                ]
                .spacing(6)
                .into(),
            );
        }
        // Keywords: labels once above the list, then one line per keyword.
        let mut keywords: Vec<Element<'_, Message>> = Vec::new();
        if !model.draft.searches.is_empty() {
            keywords.push(
                direction::row([
                    text(fl!("settings-launcher-search-keyword"))
                        .size(kit::size::CAPTION)
                        .style(canopa_ui::muted)
                        .width(Length::Fixed(120.))
                        .into(),
                    text(fl!("settings-launcher-search-url", placeholder = "{query}"))
                        .size(kit::size::CAPTION)
                        .style(canopa_ui::muted)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(12)
                .into(),
            );
        }
        for (i, search) in model.draft.searches.iter().enumerate() {
            keywords.push(
                direction::row([
                    super::bare_field(
                        format!("search-keyword-{i}"),
                        fl!("settings-launcher-search-keyword"),
                        "wiki",
                        &search.keyword,
                        move |value| Message::LauncherSearch(i, false, value),
                        Length::Fixed(120.),
                    ),
                    super::bare_field(
                        format!("search-template-{i}"),
                        fl!("settings-launcher-search-url", placeholder = "{query}"),
                        "https://example.org/search?q={query}",
                        &search.template,
                        move |value| Message::LauncherSearch(i, true, value),
                        Length::Fill,
                    ),
                    icon_button(
                        format!("settings-button-remove-search-shortcut-{i}"),
                        "close",
                        fl!(
                            "settings-launcher-remove-named",
                            keyword = search.keyword.trim()
                        ),
                        Message::LauncherRemoveSearch(i),
                    ),
                ])
                .spacing(12)
                .align_y(Alignment::Center)
                .into(),
            );
        }
        if model.draft.searches.is_empty() {
            keywords.push(kit::empty(fl!("settings-launcher-no-keywords")));
        }
        keywords.push(
            direction::row([
                container(kit::caption(fl!("settings-launcher-keywords-hint")))
                    .width(Length::Fill)
                    .into(),
                self.action(
                    "settings-button-add-search-shortcut",
                    fl!("settings-launcher-add-search"),
                    (model.draft.searches.len() < 16).then_some(Message::LauncherAddSearch),
                    false,
                ),
            ])
            .spacing(12)
            .align_y(Alignment::Center)
            .into(),
        );
        let mut body = column![
            column![
                if model.catalog.extensions.is_empty() {
                    settings_group(
                        fl!("settings-launcher-extensions"),
                        kit::empty(fl!("settings-launcher-no-extensions")),
                    )
                } else {
                    settings_group(fl!("settings-launcher-extensions"), kit::rows(extensions))
                },
                // How extensions work, once there are some to enable.
                if model.catalog.extensions.is_empty() {
                    iced::widget::space().into()
                } else {
                    kit::footnote(fl!("settings-launcher-intro"))
                },
            ]
            .spacing(8),
            settings_group(fl!("settings-launcher-web"), kit::rows(web)),
            super::noted_group(
                fl!("settings-launcher-keywords"),
                column(keywords).spacing(10),
                [fl!("settings-launcher-web-hint")],
            ),
        ]
        .spacing(28);
        if !model.draft.shortcuts.is_empty() {
            let pins = model.draft.shortcuts.iter().map(|s| {
                let pinned = if s.pinned { "yes" } else { "no" };
                Element::from(text(if s.alias.is_empty() {
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
                }))
            });
            body = body.push(settings_group(
                fl!("settings-launcher-pins"),
                kit::rows(pins),
            ));
        }
        body.push(self.clipboard_settings()).into()
    }

    fn clipboard_settings(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        if let Some(editor) = &self.clipboard {
            rows.push(toggle(
                "launcher-clipboard-recording",
                fl!("settings-clipboard-record"),
                editor.recording,
                Message::ClipboardRecording(!editor.recording),
            ));
            // The other options only matter while history is recorded.
            if editor.recording {
                rows.push(toggle(
                    "launcher-clipboard-images",
                    fl!("settings-clipboard-images"),
                    editor.images,
                    Message::ClipboardImages(!editor.images),
                ));
                rows.push(kit::row(
                    fl!("settings-clipboard-limit", limit = "64"),
                    None,
                    super::bare_field(
                        "launcher-clipboard-limit",
                        fl!("settings-clipboard-limit", limit = "64"),
                        "64",
                        &editor.limit,
                        Message::ClipboardLimit,
                        Length::Fixed(96.),
                    ),
                ));
            }
        } else if self.clipboard_notice.is_empty() {
            rows.push(text(fl!("settings-clipboard-loading")).into());
        }
        if !self.clipboard_notice.is_empty() {
            rows.push(text(&self.clipboard_notice).into());
        }
        super::noted_group(
            fl!("settings-clipboard"),
            kit::rows(rows),
            [fl!("settings-clipboard-hint")],
        )
    }
}
