//! Files browser chrome, using the shared Canopa appearance roles.
use super::{Desktop, Element, Message};
use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Color, Font, Length, Theme,
    font::Weight,
    widget::{column, container, responsive, row, rule, scrollable, space, text, tooltip},
};
use std::{cell::RefCell, collections::HashMap};

pub(super) const SIDEBAR_WIDTH: f32 = 204.0;

pub(super) fn ink(appearance: &Appearance, role: ColorRole) -> Color {
    canopa_ui::color(appearance.role(role))
}

pub(super) fn divider(theme: &Theme) -> rule::Style {
    let mut color = canopa_ui::colors(theme).border;
    color.a = 0.64;
    rule::Style {
        color,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

pub(super) fn bar(appearance: &Appearance, top: bool) -> container::Style {
    let mut tint = ink(appearance, ColorRole::Raised);
    tint.a = 0.56;
    let mut frame = canopa_ui::frame(appearance, appearance.style(UiKind::Panel));
    let radius = frame.border.radius;
    frame.border = iced::Border::default().rounded(if top {
        iced::border::Radius {
            bottom_left: 0.0,
            bottom_right: 0.0,
            ..radius
        }
    } else {
        iced::border::Radius {
            top_left: 0.0,
            top_right: 0.0,
            ..radius
        }
    });
    frame.background = Some(tint.into());
    frame
}

pub(super) fn highlight(appearance: &Appearance, role: ColorRole) -> container::Style {
    container::Style::default()
        .background(ink(appearance, role))
        .border(iced::Border::default().rounded(8))
}

pub(super) fn dialog_heading<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: impl Into<String>,
    subtitle: impl Into<String>,
) -> Element<'a, Message> {
    heading_with_action(desktop, icon, title, subtitle, None)
}

pub(super) fn heading_with_action<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut bold = canopa_ui::font(appearance);
    bold.weight = Weight::Semibold;
    let title = text(title.into())
        .font(bold)
        .size(appearance.typography().title_size.px())
        .wrapping(text::Wrapping::WordOrGlyph);
    heading(desktop, icon, title.into(), subtitle.into(), action)
}

pub(super) fn heading_line<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: String,
    subtitle: String,
    action: Element<'a, Message>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut bold = canopa_ui::font(appearance);
    bold.weight = Weight::Semibold;
    let title = single_line(
        title,
        bold,
        appearance.typography().title_size.px(),
        None,
        true,
    );
    heading(desktop, icon, title, subtitle, Some(action))
}

fn heading<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: Element<'a, Message>,
    subtitle: String,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let mut labels = column![title].width(Length::Fill).spacing(3);
    if !subtitle.is_empty() {
        labels = labels.push(
            text(subtitle)
                .font(font)
                .size(appearance.typography().label_size.px() + 1.0)
                .color(ink(appearance, ColorRole::Muted))
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    let symbol = container(desktop.icons.chrome(icon, 21.0, 1.0))
        .padding(9)
        .style(move |_| highlight(appearance, ColorRole::AccentSurface));
    let mut content: Vec<Element<'a, Message>> = vec![symbol.into(), labels.into()];
    if let Some(action) = action {
        content.push(action);
    }
    container(
        canopa_ui::direction::row(content)
            .spacing(12)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding([4, 0])
    .into()
}

pub(super) fn empty_state<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let (title, subtitle) = (title.into(), subtitle.into());
    let appearance = &desktop.configuration.appearance;
    let symbol = container(desktop.icons.chrome(icon, 25.0, 0.9))
        .padding(12)
        .style(move |_| highlight(appearance, ColorRole::AccentSurface));
    let description = text(subtitle)
        .font(canopa_ui::font(appearance))
        .size(appearance.typography().body_size.px())
        .color(ink(appearance, ColorRole::Muted))
        .wrapping(text::Wrapping::WordOrGlyph)
        .align_x(iced::alignment::Horizontal::Center);
    let mut content = column![symbol, section_title(appearance, title), description]
        .align_x(iced::Alignment::Center)
        .spacing(10)
        .padding(14)
        .max_width(380);
    if let Some(action) = action {
        content = content.push(action);
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .into()
}

pub(super) fn dismiss_id(window: iced::window::Id) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-utility-close-{window:?}"))
}

pub(super) fn dismiss(desktop: &Desktop, window: iced::window::Id) -> Element<'_, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let message = Some(Message::Close(window));
    let button = canopa_ui::styled_action(
        text("×")
            .font(font)
            .size(19)
            .align_x(iced::Alignment::Center),
        message.clone(),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .width(32)
    .height(32)
    .padding(4);
    tooltip(
        super::accessibility_ui::named_button(
            super::accessibility_ui::focus_key(&dismiss_id(window)),
            fl!("files-close-window"),
            message.clone(),
            super::focus::control(dismiss_id(window), message, appearance, button),
        ),
        text(fl!("files-close-window"))
            .font(font)
            .size(appearance.typography().label_size.px()),
        tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_millis(400))
    .into()
}

pub(super) fn catalogue_row(appearance: &Appearance, selected: bool) -> container::Style {
    if selected {
        highlight(appearance, ColorRole::AccentSurface)
    } else {
        container::Style::default().border(iced::Border {
            width: 1.0,
            radius: 8.0.into(),
            color: ink(appearance, ColorRole::Border).scale_alpha(if appearance.high_contrast() {
                1.0
            } else {
                0.26
            }),
        })
    }
}

pub(super) fn selection_mark<'a>(
    appearance: &'a Appearance,
    selected: bool,
) -> Element<'a, Message> {
    container(
        text(if selected { "✓" } else { "" })
            .font(canopa_ui::font(appearance))
            .size(12)
            .color(ink(appearance, ColorRole::OnAccent)),
    )
    .width(17)
    .height(17)
    .align_x(iced::Alignment::Center)
    .align_y(iced::Alignment::Center)
    .style(move |_| container::Style {
        background: selected.then(|| ink(appearance, ColorRole::Accent).into()),
        border: iced::Border {
            width: 1.0,
            color: ink(
                appearance,
                if selected {
                    ColorRole::Accent
                } else {
                    ColorRole::Border
                },
            ),
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .into()
}

pub(super) fn section_title<'a>(
    appearance: &Appearance,
    title: impl Into<String>,
) -> text::Text<'a> {
    let mut font = canopa_ui::font(appearance);
    font.weight = Weight::Semibold;
    text(title.into())
        .font(font)
        .size(appearance.typography().body_size.px() + 1.0)
}

pub(super) fn card<'a>(
    appearance: &'a Appearance,
    body: impl Into<Element<'a, Message>>,
) -> container::Container<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding(14)
        .style(move |_| canopa_ui::frame(appearance, appearance.style(UiKind::Card)))
}

pub(super) fn catalogue<'a>(
    appearance: &'a Appearance,
    title: String,
    instruction: String,
    listing: Element<'a, Message>,
    inspector: Option<(iced::widget::Id, f32, Element<'a, Message>)>,
) -> Element<'a, Message> {
    let guidance = inspector.is_none().then(|| {
        text(instruction)
            .font(canopa_ui::font(appearance))
            .size(appearance.typography().label_size.px())
            .style(canopa_ui::muted)
    });
    let header = row![
        section_title(appearance, title),
        space().width(Length::Fill),
        guidance
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let listing = column![header, listing]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill);
    let inspector = inspector.map(|(id, width, details)| {
        scrollable(card(appearance, details))
            .id(id)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(6)
                    .scroller_width(6)
                    .spacing(6),
            ))
            .width(width)
            .height(Length::Fill)
    });
    // The catalogue scroll state stays in the first child when an item is selected.
    row![listing, inspector]
        .spacing(12)
        .height(Length::Fill)
        .into()
}

pub(super) fn centered<'a>(
    body: impl Into<Element<'a, Message>>,
    maximum: f32,
) -> container::Container<'a, Message> {
    container(container(body).width(Length::Fill).max_width(maximum))
        .align_x(iced::Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
}

// Visible rows and breadcrumbs reuse measurements across cursor movement,
// scrolling and editor keystrokes. Keep storage bounded across folder changes.
type Widths = HashMap<(Font, u32), HashMap<String, f32>>;
thread_local! {
    static WIDTHS: RefCell<(usize, Widths)> = RefCell::default();
}

pub(super) fn text_width(content: &str, font: Font, size: f32) -> f32 {
    WIDTHS.with_borrow_mut(|(count, widths)| {
        let key = (font, size.to_bits());
        if let Some(width) = widths.get(&key).and_then(|labels| labels.get(content)) {
            return *width;
        }
        let width = measure_text(content, font, size);
        if *count >= 4096 {
            widths.clear();
            *count = 0;
        }
        widths
            .entry(key)
            .or_default()
            .insert(content.to_owned(), width);
        *count += 1;
        width
    })
}

fn measure_text(content: &str, font: Font, size: f32) -> f32 {
    use iced::advanced::text::{self, Paragraph};

    iced::advanced::graphics::text::Paragraph::with_text(text::Text {
        content,
        bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
        size: iced::Pixels(size),
        line_height: text::LineHeight::default(),
        font,
        align_x: text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
    .min_bounds()
    .width
    .ceil()
}

pub(super) fn single_line<'a>(
    label: impl Into<String>,
    font: Font,
    size: f32,
    color: Option<Color>,
    reveal: bool,
) -> Element<'a, Message> {
    let label = label.into();
    responsive(move |bounds| {
        let fits = text_width(&label, font, size) <= bounds.width;
        let shown = if fits {
            label.clone()
        } else {
            let mut boundaries: Vec<_> = label.char_indices().map(|(index, _)| index).collect();
            boundaries.push(label.len());
            let limit = boundaries.partition_point(|&end| {
                text_width(&format!("{}…", &label[..end]), font, size) <= bounds.width
            });
            format!("{}…", &label[..boundaries[limit.saturating_sub(1)]])
        };
        let line = container(
            text(shown)
                .font(font)
                .size(size)
                .shaping(text::Shaping::Advanced)
                .wrapping(text::Wrapping::None)
                .style(move |_| text::Style { color }),
        )
        .width(Length::Fill)
        .clip(true);
        if reveal && !fits {
            tooltip(
                line,
                text(label.clone()).font(font).size(size),
                tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(450))
            .into()
        } else {
            line.into()
        }
    })
    .height(Length::Shrink)
    .into()
}

pub(super) fn section<'a>(
    appearance: &Appearance,
    label: impl Into<String>,
) -> Element<'a, Message> {
    let label = label.into();
    let mut font = canopa_ui::font(appearance);
    font.weight = Weight::Semibold;
    container(
        text(label)
            .font(font)
            .size(appearance.typography().label_size.px() + 1.0)
            .color(ink(appearance, ColorRole::Muted)),
    )
    .padding([6, 10])
    .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Icon {
    Back,
    Forward,
    ActiveForward,
    ActiveBack,
    Up,
    Search,
    Eye,
    More,
    Folder,
    ActiveFolder,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Drive,
    Trash,
    Recovery,
    Eject,
    Columns,
    List,
    Grid,
    Edit,
    Places,
    Settings,
    Activity,
    Terminal,
    Info,
    Question,
    Network,
    Phone,
    Camera,
}

impl Icon {
    pub(super) const ALL: [Self; 32] = [
        Self::Back,
        Self::Forward,
        Self::ActiveForward,
        Self::ActiveBack,
        Self::Up,
        Self::Search,
        Self::Eye,
        Self::More,
        Self::Folder,
        Self::ActiveFolder,
        Self::Home,
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Pictures,
        Self::Drive,
        Self::Trash,
        Self::Recovery,
        Self::Eject,
        Self::Columns,
        Self::List,
        Self::Grid,
        Self::Edit,
        Self::Places,
        Self::Settings,
        Self::Activity,
        Self::Terminal,
        Self::Info,
        Self::Question,
        Self::Network,
        Self::Phone,
        Self::Camera,
    ];

    /// The icon drawn for `self` in the current writing direction: arrows and
    /// chevrons point the other way in right-to-left languages.
    pub(super) fn directional(self) -> Self {
        if !canopa_ui::direction::is_rtl() {
            return self;
        }
        match self {
            Self::Back => Self::Forward,
            Self::Forward => Self::Back,
            Self::ActiveForward => Self::ActiveBack,
            Self::ActiveBack => Self::ActiveForward,
            other => other,
        }
    }

    pub(super) fn svg(self) -> String {
        let path = match self {
            Self::Back | Self::ActiveBack => "m14 6-6 6 6 6",
            Self::Forward | Self::ActiveForward => "m10 6 6 6-6 6",
            Self::Up => "M12 19V5m-6 6 6-6 6 6",
            Self::Search => "M16.2 16.2 21 21M18 11a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z",
            Self::Eye => {
                "M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6S2 12 2 12Zm12.5 0a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z"
            }
            Self::More => "M5 12h.01m6.99 0h.01m6.99 0h.01",
            Self::Folder | Self::ActiveFolder => {
                "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"
            }
            Self::Home => "m3 10 9-7 9 7m-2-1v11H5V9m5 11v-6h4v6",
            Self::Desktop => "M3 5h18v12H3Zm9 12v4m-4 0h8",
            Self::Documents => "M6 3h8l4 4v14H6Zm8 0v5h4m-9 4h6m-6 4h6",
            Self::Downloads => "M12 3v12m-4-4 4 4 4-4M5 16v4h14v-4",
            Self::Pictures => "M3 5h18v14H3Zm1 12 5-5 3 3 3-3 5 5M15 9h.01",
            Self::Drive => "M5 5h14l2 10v4H3v-4Zm-2 10h18m-4 2h.01",
            Self::Trash => "M4 6h16M9 6V3h6v3m3 0-1 15H7L6 6m4 4v7m4-7v7",
            Self::Recovery => "M3 10a9 9 0 1 1 1 7m-1-13v6h6m3-4v6l3 2",
            Self::Eject => "m6 14 6-8 6 8Zm0 4h12",
            Self::Columns => "M3 5h18v14H3Zm6 0v14m6-14v14",
            Self::List => "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
            Self::Grid => "M4 4h6v6H4Zm10 0h6v6h-6ZM4 14h6v6H4Zm10 0h6v6h-6Z",
            Self::Edit => "m15 5 4 4M4 20l4-.8L20 7l-3-3L5 16Z",
            Self::Places => "M3 5h18v14H3Zm6 0v14M5.5 9h1m-1 3h1m-1 3h1",
            Self::Settings => {
                "M4 6h7m4 0h5M4 12h3m4 0h9M4 18h11m4 0h1M11 4h4v4h-4ZM7 10h4v4H7Zm8 6h4v4h-4Z"
            }
            Self::Activity => "M3 12h4l3-7 4 14 3-7h4",
            Self::Terminal => "M3 5h18v14H3Zm4 4 3 3-3 3m6 0h4",
            Self::Info => "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Zm-10-1v6m0-10h.01",
            Self::Question => {
                "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0ZM9 9a3 3 0 1 1 4 3c-1 1-1 1-1 2m0 3h.01"
            }
            Self::Network => "M4 4h16v6H4Zm0 10h16v6H4ZM8 7h.01M8 17h.01",
            Self::Phone => "M7 2h10v20H7Zm4 16h2",
            Self::Camera => "M3 7h4l2-3h6l2 3h4v13H3Zm12 6a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
        };
        let weight = if self == Self::More { 3.5 } else { 1.9 };
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="{path}" fill="none" stroke="currentColor" stroke-width="{weight}" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn arrows_and_chevrons_point_the_other_way_right_to_left() {
        if !crate::i18n::tests::isolated(
            "desktop::chrome::tests::arrows_and_chevrons_point_the_other_way_right_to_left",
        ) {
            return;
        }
        assert_eq!(Icon::Back.directional(), Icon::Back);
        let _rtl = crate::i18n::tests::Selected::pseudo();
        assert_eq!(Icon::Back.directional(), Icon::Forward);
        assert_eq!(Icon::Forward.directional(), Icon::Back);
        assert_eq!(Icon::ActiveForward.directional(), Icon::ActiveBack);
        assert_eq!(Icon::Home.directional(), Icon::Home);
    }
}
