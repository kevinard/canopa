//! The sidebar: registry groups, a search field and keyboard page movement.
use super::{Message, State, focus, heading};
use crate::registry::{self, Entry};
use canopa_ui::accessibility::{Node, semantic};
use iced::{
    Element, Length,
    widget::{column, container, scrollable, text, text_input},
};

/// Focus id of the search field (Ctrl+F).
pub(super) const SEARCH: &str = "settings-search";

/// Focus id of a page's sidebar link; stable across translations.
pub(super) fn link_id(entry: &Entry) -> String {
    format!("settings-page-{}", entry.id)
}

impl State {
    pub(super) fn navigation(&self) -> Element<'_, Message> {
        let enabled = !self.busy && self.trial.is_none();
        let mut list = column![].spacing(4);
        if self.search.trim().is_empty() {
            let pages = registry::navigation();
            for (group, entries) in registry::groups() {
                list = list.push(
                    container(text(group.title()).size(12).style(canopa_ui::muted))
                        .padding(iced::Padding::from([0, 12]).top(10)),
                );
                for entry in entries {
                    list = list.push(self.page_link(entry, &pages, enabled));
                }
            }
        } else {
            let found = registry::search(&self.search);
            list = list.push(semantic(
                Node::status(match found.len() {
                    0 => "No matching settings".to_owned(),
                    1 => "1 matching page".to_owned(),
                    n => format!("{n} matching pages"),
                })
                .key("settings-search-status"),
                // A plain space lays out as 0×0 and stays; a fixed zero height
                // would be dropped by Column::push, and the status with it.
                iced::widget::space(),
            ));
            if found.is_empty() {
                list = list.push(
                    container(
                        text("No matching settings")
                            .size(13)
                            .style(canopa_ui::muted),
                    )
                    .padding([8, 12]),
                );
            }
            for entry in &found {
                list = list.push(self.page_link(entry, &found, enabled));
            }
        }
        semantic(
            Node::navigation("Settings pages").key("settings-navigation"),
            container(
                column![
                    column![
                        text("Canopa").size(13).style(canopa_ui::muted),
                        heading("Settings", 24)
                    ]
                    .spacing(3)
                    .padding([8, 8]),
                    semantic(
                        Node::search_input("Search settings", &self.search)
                            .key("settings-search-field")
                            .placeholder("Search settings")
                            .shortcut("Control+F")
                            .controls("settings-pages")
                            .on_text(Message::Search),
                        text_input("Search settings", &self.search)
                            .id(SEARCH)
                            .on_input(Message::Search)
                            .on_submit(Message::SearchSubmit)
                            .padding(8)
                    ),
                    semantic(
                        Node::list("Pages").key("settings-pages"),
                        canopa_ui::scroll_speed(
                            scrollable(list)
                                .id("settings-navigation-scroll")
                                .height(Length::Fill)
                        )
                    ),
                ]
                .spacing(14),
            )
            .width(220)
            .height(Length::Fill)
            .padding(14)
            .style(|theme| {
                container::Style::default().background(canopa_ui::colors(theme).surface)
            }),
        )
    }

    /// A sidebar link. Up and Down move through `siblings`, the list shown.
    fn page_link<'a>(
        &self,
        entry: &'static Entry,
        siblings: &[&'static Entry],
        enabled: bool,
    ) -> Element<'a, Message> {
        let index = siblings
            .iter()
            .position(|sibling| sibling.page == entry.page)
            .unwrap_or(0);
        let len = siblings.len().max(1);
        let neighbour =
            |offset: usize| enabled.then(|| Message::Page(siblings[(index + offset) % len].page));
        let open = enabled.then_some(Message::Page(entry.page));
        semantic(
            Node::option(entry.title, entry.page == self.page)
                .key(link_id(entry))
                .position(index + 1, len)
                .disabled(!enabled)
                .on_click(open.clone()),
            focus::control(
                link_id(entry),
                open.clone(),
                // Down (and Left) move to the next page, Up (and Right) to the previous one.
                neighbour(1),
                neighbour(len - 1),
                self.busy,
                canopa_ui::action(
                    text(entry.title),
                    open,
                    if entry.page == self.page {
                        canopa_ui::Action::Normal
                    } else {
                        canopa_ui::Action::Quiet
                    },
                    false,
                )
                .width(Length::Fill)
                .padding([8, 12]),
            ),
        )
    }
}
