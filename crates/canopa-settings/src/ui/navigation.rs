//! The sidebar: registry groups, a search field and keyboard page movement.
use super::{Message, State, focus, kit};
use crate::registry::{self, Entry};
use canopa_ui::{
    accessibility::{Node, semantic},
    direction,
};
use iced::{
    Element, Length,
    widget::{column, container, scrollable, text, text_input},
};

/// Focus id of the search field (Ctrl+F).
pub(super) const SEARCH: &str = "settings-search";
/// Scroll id of the page list.
pub(super) const SCROLL: &str = "settings-navigation-scroll";

/// Bring the selected page's link into view: the list is scrolled in
/// proportion to the link's place in it.
pub(super) fn reveal(page: crate::Page) -> iced::Task<Message> {
    let pages = registry::navigation();
    let index = pages
        .iter()
        .position(|entry| entry.page == page)
        .unwrap_or(0);
    let fraction = if pages.len() > 1 {
        index as f32 / (pages.len() - 1) as f32
    } else {
        0.0
    };
    iced::widget::operation::snap_to(
        SCROLL,
        scrollable::RelativeOffset {
            x: 0.0,
            y: fraction,
        },
    )
}

/// Focus id of a page's sidebar link; stable across translations.
pub(super) fn link_id(entry: &Entry) -> String {
    format!("settings-page-{}", entry.id)
}

impl State {
    pub(super) fn navigation(&self) -> Element<'_, Message> {
        let enabled = !self.busy && self.trial.is_none();
        let mut list = column![].spacing(1);
        if self.search.trim().is_empty() {
            let pages = registry::navigation();
            for (group, entries) in registry::groups() {
                list = list.push(
                    container(
                        text(group.title())
                            .size(kit::size::SMALL)
                            .font(kit::strong_font(iced::font::Weight::Semibold))
                            .style(canopa_ui::muted)
                            .width(Length::Fill)
                            .align_x(direction::text_start()),
                    )
                    .padding(iced::Padding::from([0, 10]).top(9).bottom(3)),
                );
                for entry in entries {
                    list = list.push(self.page_link(entry, &pages, enabled));
                }
            }
        } else {
            let found = registry::search(&self.search);
            list = list.push(semantic(
                Node::status(match found.len() {
                    0 => fl!("settings-search-none"),
                    count => fl!("settings-search-found", count = count),
                })
                .key("settings-search-status"),
                // A plain space lays out as 0×0 and stays; a fixed zero height
                // would be dropped by Column::push, and the status with it.
                iced::widget::space(),
            ));
            if found.is_empty() {
                list = list.push(
                    container(
                        text(fl!("settings-search-none"))
                            .size(13)
                            .style(canopa_ui::muted)
                            .width(Length::Fill)
                            .align_x(direction::text_start()),
                    )
                    .padding([8, 12]),
                );
            }
            for entry in &found {
                list = list.push(self.page_link(entry, &found, enabled));
            }
        }
        semantic(
            Node::navigation(fl!("settings-sidebar-navigation")).key("settings-navigation"),
            container(
                column![
                    semantic(
                        Node::search_input(fl!("settings-search-placeholder"), &self.search)
                            .key("settings-search-field")
                            .placeholder(fl!("settings-search-placeholder"))
                            .shortcut("Control+F")
                            .controls("settings-pages")
                            .on_text(Message::Search),
                        // The magnifier is drawn over the field's leading padding.
                        iced::widget::stack![
                            text_input(&fl!("settings-search-placeholder"), &self.search)
                                .id(SEARCH)
                                .align_x(direction::start())
                                .on_input(Message::Search)
                                .on_submit(Message::SearchSubmit)
                                .padding(if direction::is_rtl() {
                                    iced::Padding::from([8, 12]).right(34)
                                } else {
                                    iced::Padding::from([8, 12]).left(34)
                                })
                                .style(kit::search_style),
                            container(kit::glyph("search", 16.0, kit::Tint::Muted))
                                .height(Length::Fill)
                                .width(Length::Fill)
                                .align_x(direction::start())
                                .align_y(iced::alignment::Vertical::Center)
                                .padding([0, 11]),
                        ]
                    ),
                    semantic(
                        Node::list(fl!("settings-sidebar-pages")).key("settings-pages"),
                        canopa_ui::scroll_speed(
                            scrollable(list)
                                .id(SCROLL)
                                .direction(scrollable::Direction::Vertical(
                                    scrollable::Scrollbar::new()
                                        .width(4)
                                        .scroller_width(4)
                                        .margin(0)
                                ))
                                .style(canopa_ui::scrollbar)
                                .height(Length::Fill)
                        )
                    ),
                ]
                .spacing(12),
            )
            .width(236)
            .height(Length::Fill)
            .padding(iced::Padding::from([14, 12]))
            .style(|theme| container::Style::default().background(kit::sidebar_background(theme))),
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
        let title = entry.title();
        semantic(
            Node::option(title.clone(), entry.page == self.page)
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
                {
                    let selected = entry.page == self.page;
                    iced::widget::button(
                        direction::row([
                            kit::glyph(
                                entry.id,
                                18.0,
                                if selected {
                                    kit::Tint::Accent
                                } else {
                                    kit::Tint::Muted
                                },
                            ),
                            text(title)
                                .size(kit::size::BODY)
                                .font(if selected {
                                    kit::strong_font(iced::font::Weight::Semibold)
                                } else {
                                    kit::font()
                                })
                                .width(Length::Fill)
                                .align_x(direction::text_start())
                                .into(),
                        ])
                        .spacing(11)
                        .align_y(iced::Alignment::Center),
                    )
                    .on_press_maybe(open)
                    .style(move |theme, status| kit::navigation_style(theme, status, selected))
                    .width(Length::Fill)
                    .padding([5, 10])
                },
            ),
        )
    }
}
