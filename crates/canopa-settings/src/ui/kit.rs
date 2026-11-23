//! Settings' visual vocabulary: the type scale, sections and cards, buttons,
//! fields and the page glyphs. Pages compose these instead of styling
//! widgets themselves, so every page reads the same way.
use super::Message;
use canopa_ui::colors;
use canopa_ui::direction;
use iced::{
    Alignment, Background, Border, Color, Element, Font, Length, Shadow, Theme, Vector,
    font::Weight,
    widget::{
        button, column, container, overlay::menu, pick_list, rule, slider, svg, text, text_input,
    },
};
use std::sync::OnceLock;

/// Logical sizes of the Settings type scale.
pub(super) mod size {
    pub const PAGE_TITLE: u32 = 26;
    pub const SECTION: u32 = 16;
    pub const BODY: u32 = 14;
    pub const CAPTION: u32 = 13;
    pub const SMALL: u32 = 12;
}
/// The widest a page's content column grows; wider windows centre it.
pub(super) const CONTENT_WIDTH: f32 = 680.0;
pub(super) const CARD_RADIUS: f32 = 12.0;
pub(super) const CONTROL_RADIUS: f32 = 8.0;
/// Horizontal inset of a card's rows.
pub(super) const ROW_PADDING: [u16; 2] = [12, 16];
/// The least height of a row's content inside a card.
pub(super) const ROW_HEIGHT: f32 = 34.0;
/// The narrowest a trailing pick list is drawn, so choices line up.
/// The width of a slider's name, so tracks start at one edge.
pub(super) const SLIDER_LABEL: f32 = 150.0;
pub(super) const PICK_WIDTH: f32 = 240.0;

static FONT: OnceLock<Font> = OnceLock::new();

/// Record the appearance's font before the window opens.
pub(super) fn set_font(font: Font) {
    let _ = FONT.set(font);
}
pub(super) fn font() -> Font {
    FONT.get().copied().unwrap_or(Font::DEFAULT)
}
/// The application font at a heavier weight: the same family, so headings
/// never fall back to another face.
pub(super) fn strong_font(weight: Weight) -> Font {
    Font { weight, ..font() }
}

/// `from` moved `amount` of the way toward `to`, opaque.
pub(super) fn blend(from: Color, to: Color, amount: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: 1.0,
    }
}
fn dark(theme: &Theme) -> bool {
    theme.extended_palette().is_dark
}
/// A divider quiet enough to separate rows inside a card.
pub(super) fn hairline(theme: &Theme) -> Color {
    let p = colors(theme);
    blend(p.surface, p.border, if dark(theme) { 0.45 } else { 0.5 })
}
/// A switch whose off state still reads against a card.
pub(super) fn switch_style(
    theme: &Theme,
    status: iced::widget::toggler::Status,
) -> iced::widget::toggler::Style {
    use iced::widget::toggler::Status;
    let mut style = canopa_ui::switch_style(theme, status);
    let p = colors(theme);
    let off = matches!(
        status,
        Status::Active { is_toggled: false } | Status::Hovered { is_toggled: false }
    );
    if off {
        let hovered = matches!(status, Status::Hovered { .. });
        style.background = blend(p.surface, p.text, if hovered { 0.5 } else { 0.44 }).into();
        style.foreground = if dark(theme) {
            p.text.into()
        } else {
            Color::WHITE.into()
        };
    }
    style
}

/// The sidebar sits between the page background and its cards.
pub(super) fn sidebar_background(theme: &Theme) -> Color {
    let p = colors(theme);
    if dark(theme) {
        blend(p.background, p.surface, 0.45)
    } else {
        blend(p.background, p.surface, 0.35)
    }
}
/// The accent washed into a surface: selected rows and segments.
pub(super) fn selection(theme: &Theme) -> Color {
    let p = colors(theme);
    blend(p.surface, p.accent, if dark(theme) { 0.26 } else { 0.2 })
}
/// Foreground for text drawn on [`selection`]: the accent when it reads
/// there, the text colour otherwise.
pub(super) fn on_selection(theme: &Theme) -> Color {
    let p = colors(theme);
    let rgb = |c: Color| {
        canopa_theme::Rgb(
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
    };
    if rgb(p.accent).contrast(rgb(selection(theme))) >= 4.5 {
        p.accent
    } else {
        p.text
    }
}

pub(super) fn card(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        // Lifted a little further from the page in the dark appearance.
        background: Some(
            if dark(theme) {
                blend(p.surface, p.text, 0.025)
            } else {
                p.surface
            }
            .into(),
        ),
        text_color: Some(p.text),
        border: Border {
            color: hairline(theme),
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        shadow: if dark(theme) {
            Shadow::default()
        } else {
            Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.05),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            }
        },
        ..Default::default()
    }
}
/// A tinted note inside a card or above the page: notices and warnings.
pub(super) fn note(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(blend(p.surface, p.accent, 0.08).into()),
        text_color: Some(p.text),
        border: Border {
            color: blend(p.surface, p.accent, 0.35),
            width: 1.0,
            radius: CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}
pub(super) fn divider(theme: &Theme) -> iced::widget::rule::Style {
    iced::widget::rule::Style {
        color: hairline(theme),
        radius: 0.0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    }
}

/// Rows of a card, separated by hairlines. Each row gets the same vertical
/// rhythm, so cards built from rows line up across pages.
pub(super) fn rows<'a>(
    items: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    let items: Vec<_> = items.into_iter().collect();
    let last = items.len().saturating_sub(1);
    let mut list = column![];
    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            list = list.push(rule::horizontal(1).style(divider));
        }
        // The card's own padding frames the first and last rows.
        let padding = iced::Padding {
            top: if index == 0 { 0.0 } else { 7.0 },
            bottom: if index == last { 0.0 } else { 7.0 },
            ..iced::Padding::ZERO
        };
        // Every row is at least as tall as a row with a control, so switches,
        // values and pick lists share one rhythm.
        list = list.push(
            container(
                direction::row([
                    container(item).width(Length::Fill).into(),
                    iced::widget::space().height(ROW_HEIGHT).into(),
                ])
                .align_y(Alignment::Center),
            )
            .padding(padding)
            .width(Length::Fill),
        );
    }
    list.width(Length::Fill).into()
}

/// A setting: its name and an optional explanation lead, the control trails.
pub(super) fn row<'a>(
    label: impl Into<String>,
    description: Option<String>,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut lead = column![
        text(label.into())
            .size(size::BODY)
            .width(Length::Fill)
            .align_x(direction::text_start())
    ]
    .spacing(2)
    .width(Length::Fill);
    if let Some(description) = description.filter(|d| !d.is_empty()) {
        lead = lead.push(
            text(description)
                .size(size::CAPTION)
                .style(canopa_ui::muted)
                .width(Length::Fill)
                .align_x(direction::text_start()),
        );
    }
    direction::row([lead.into(), control.into()])
        .spacing(16)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// A slider setting: its name and current value above the track.
pub(super) fn slider_row<'a>(
    label: impl Into<String>,
    value: impl Into<String>,
    slider: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    // One line: the name, the track, then the value, so every slider in
    // the app lines up the same way.
    direction::row([
        text(label.into())
            .size(size::BODY)
            .width(Length::Fixed(SLIDER_LABEL))
            .align_x(direction::text_start())
            .into(),
        container(slider).width(Length::Fill).into(),
        text(value.into())
            .size(size::CAPTION)
            .style(canopa_ui::muted)
            .width(Length::Fixed(80.))
            .align_x(direction::text_end())
            .into(),
    ])
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Mutually exclusive choices drawn as one control; the chosen segment
/// carries the accent wash.
pub(super) fn segmented<'a>(
    segments: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    container(direction::row(segments).spacing(2))
        .padding(3)
        .style(|theme: &Theme| {
            let p = colors(theme);
            container::Style {
                background: Some(field_surface(theme).into()),
                border: Border {
                    color: blend(hairline(theme), p.border, 0.2),
                    width: 1.0,
                    radius: (CONTROL_RADIUS + 2.0).into(),
                },
                ..Default::default()
            }
        })
        .into()
}

/// A small status pill in the accent wash, such as "In use".
pub(super) fn badge<'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(size::SMALL)
            .font(strong_font(Weight::Semibold))
            .style(|theme: &Theme| text::Style {
                color: Some(on_selection(theme)),
            }),
    )
    .padding([3, 10])
    .style(|theme: &Theme| container::Style {
        background: Some(selection(theme).into()),
        border: iced::border::rounded(u32::MAX),
        ..Default::default()
    })
    .into()
}

/// How much a [`status`] message asks for attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Notice {
    /// Something to know, such as a search in progress.
    Info,
    /// Something missing or stopped that limits the page.
    Warning,
}

/// A state of the page rather than a setting: a glyph, a title, an optional
/// explanation and an optional action, on a tinted panel.
pub(super) fn status<'a>(
    notice: Notice,
    title: impl Into<String>,
    body: Option<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    notice_line(notice, title, body, action, true)
}

/// A [`status`] as one row among others in a card: no box of its own.
pub(super) fn status_line<'a>(notice: Notice, title: impl Into<String>) -> Element<'a, Message> {
    notice_line(notice, title, None, None, false)
}

fn notice_line<'a>(
    notice: Notice,
    title: impl Into<String>,
    body: Option<String>,
    action: Option<Element<'a, Message>>,
    boxed: bool,
) -> Element<'a, Message> {
    let mut words = column![
        text(title.into())
            .size(size::BODY)
            .width(Length::Fill)
            .align_x(direction::text_start())
    ]
    .spacing(2)
    .width(Length::Fill);
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        words = words.push(caption(body));
    }
    let mut row = vec![
        glyph(
            if notice == Notice::Warning {
                "warning"
            } else {
                "info"
            },
            18.0,
            if notice == Notice::Warning {
                Tint::Warning
            } else {
                Tint::Muted
            },
        ),
        words.into(),
    ];
    if let Some(action) = action {
        row.push(action);
    }
    let line = direction::row(row)
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    if !boxed {
        return line.into();
    }
    container(line)
        .padding([10, 12])
        .width(Length::Fill)
        .style(move |theme: &Theme| notice_style(theme, notice))
        .into()
}

/// Every notice the same box: an accent wash for information, a warning
/// wash for something that limits the page.
fn notice_style(theme: &Theme, notice: Notice) -> container::Style {
    let p = colors(theme);
    let tint = if notice == Notice::Warning {
        p.warning
    } else {
        p.accent
    };
    container::Style {
        background: Some(blend(p.surface, tint, 0.08).into()),
        text_color: Some(p.text),
        border: Border {
            color: blend(p.surface, tint, 0.3),
            width: 1.0,
            radius: CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// A section whose only content is a [`status`]: the notice is the card.
pub(super) fn status_card<'a>(
    notice: Notice,
    title: impl Into<String>,
    body: Option<String>,
) -> Element<'a, Message> {
    let title = title.into();
    let mut words = column![
        text(title)
            .size(size::BODY)
            .width(Length::Fill)
            .align_x(direction::text_start())
    ]
    .spacing(2)
    .width(Length::Fill);
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        words = words.push(caption(body));
    }
    container(
        direction::row([
            glyph(
                if notice == Notice::Warning {
                    "warning"
                } else {
                    "info"
                },
                18.0,
                if notice == Notice::Warning {
                    Tint::Warning
                } else {
                    Tint::Muted
                },
            ),
            words.into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(ROW_PADDING)
    .width(Length::Fill)
    .style(move |theme: &Theme| {
        let mut style = notice_style(theme, notice);
        style.border.radius = CARD_RADIUS.into();
        style
    })
    .into()
}

/// Waiting for a service: a quiet, centred line with three dots, the same
/// shape as an empty list.
pub(super) fn loading<'a>(value: impl Into<String>) -> Element<'a, Message> {
    container(
        column![
            text("\u{2022} \u{2022} \u{2022}")
                .size(size::SECTION)
                .style(|theme: &Theme| text::Style {
                    color: Some(colors(theme).accent),
                }),
            text(value.into()).size(size::BODY).style(canopa_ui::muted),
        ]
        .spacing(6)
        .align_x(Alignment::Center),
    )
    .padding([28, 16])
    .center_x(Length::Fill)
    .style(card)
    .into()
}

/// An empty list: one quiet, centred line inside the card that would hold it.
pub(super) fn empty<'a>(value: impl Into<String>) -> Element<'a, Message> {
    container(
        text(value.into())
            .size(size::BODY)
            .style(canopa_ui::muted)
            .align_x(iced::alignment::Horizontal::Center)
            .width(Length::Fill),
    )
    .padding([8, 0])
    .width(Length::Fill)
    .into()
}

/// A section's title, aligned with the cards of every other section.
pub(super) fn section_heading<'a>(label: impl Into<String>) -> Element<'a, Message> {
    text(label.into())
        .size(size::SECTION)
        .font(strong_font(Weight::Semibold))
        .width(Length::Fill)
        .align_x(direction::text_start())
        .into()
}

/// Text under a card, outside it: footnotes about the whole section.
pub(super) fn footnote<'a>(value: impl Into<String>) -> Element<'a, Message> {
    // Indented like the card's own text, a little below the card.
    container(caption(value))
        .padding(iced::Padding::from([0, 16]).top(2))
        .into()
}

/// A slider for a value centred on zero, such as balance: no side is filled.
pub(super) fn centered_slider_style(theme: &Theme, status: slider::Status) -> slider::Style {
    let mut style = slider_style(theme, status);
    style.rail.backgrounds.0 = style.rail.backgrounds.1;
    style
}

/// Secondary text under a control or at the end of a card.
pub(super) fn caption<'a>(value: impl Into<String>) -> Element<'a, Message> {
    text(value.into())
        .size(size::CAPTION)
        .style(canopa_ui::muted)
        .width(Length::Fill)
        .align_x(direction::text_start())
        .into()
}

/// How a push button presents itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Treatment {
    /// The one action a section or page leads with.
    Primary,
    /// Every other action: a filled, outlined button.
    Secondary,
    /// The chosen segment of a segmented control.
    Selected,
    /// An action that removes or forgets something.
    Destructive,
    /// A borderless action inside dense rows.
    Flat,
}
impl From<canopa_ui::Action> for Treatment {
    fn from(action: canopa_ui::Action) -> Self {
        match action {
            canopa_ui::Action::Primary => Treatment::Primary,
            canopa_ui::Action::Normal => Treatment::Secondary,
            canopa_ui::Action::Quiet => Treatment::Secondary,
            canopa_ui::Action::Destructive => Treatment::Destructive,
        }
    }
}

pub(super) fn button_style(
    theme: &Theme,
    status: button::Status,
    treatment: Treatment,
) -> button::Style {
    let p = colors(theme);
    let disabled = status == button::Status::Disabled;
    let hovered = status == button::Status::Hovered;
    let pressed = status == button::Status::Pressed;
    let lean = if pressed {
        0.14
    } else if hovered {
        0.08
    } else {
        0.0
    };
    let (background, foreground, border) = match treatment {
        // Unavailable actions lose their fill and most of their contrast.
        // A faint fill without a border: plainly not pressable.
        _ if disabled && treatment != Treatment::Selected => (
            blend(p.surface, p.text, 0.04),
            p.disabled,
            Color::TRANSPARENT,
        ),
        Treatment::Primary => (
            blend(p.accent, p.text, lean * if dark(theme) { 1.6 } else { 0.9 }),
            p.on_accent,
            Color::TRANSPARENT,
        ),
        Treatment::Selected => (
            selection(theme),
            on_selection(theme),
            blend(p.surface, p.accent, 0.5),
        ),
        Treatment::Secondary | Treatment::Destructive => (
            blend(p.raised, p.text, lean),
            if disabled {
                p.disabled
            } else if treatment == Treatment::Destructive {
                p.danger
            } else {
                p.text
            },
            if disabled {
                hairline(theme)
            } else {
                blend(p.raised, p.border, 0.45)
            },
        ),
        Treatment::Flat => (
            if lean > 0.0 {
                blend(p.surface, p.text, lean * 0.8)
            } else {
                Color::TRANSPARENT
            },
            if disabled { p.disabled } else { p.text },
            Color::TRANSPARENT,
        ),
    };
    button::Style {
        background: Some(background.into()),
        text_color: foreground,
        border: Border {
            color: border,
            width: if border == Color::TRANSPARENT {
                0.0
            } else {
                1.0
            },
            radius: CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// A sidebar entry: a quiet row, filled with the accent wash when selected.
pub(super) fn navigation_style(
    theme: &Theme,
    status: button::Status,
    selected: bool,
) -> button::Style {
    let p = colors(theme);
    let base = sidebar_background(theme);
    let background = if selected {
        Some(selection(theme).into())
    } else {
        match status {
            button::Status::Hovered => Some(blend(base, p.text, 0.07).into()),
            button::Status::Pressed => Some(blend(base, p.text, 0.11).into()),
            _ => None,
        }
    };
    button::Style {
        background,
        text_color: if status == button::Status::Disabled && !selected {
            p.muted
        } else {
            p.text
        },
        border: Border {
            radius: CONTROL_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The surface of a text field and a closed pick list.
fn field_surface(theme: &Theme) -> Color {
    let p = colors(theme);
    if dark(theme) {
        blend(p.background, p.surface, 0.55)
    } else {
        p.surface
    }
}

pub(super) fn text_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let p = colors(theme);
    let border = match status {
        text_input::Status::Focused { .. } => p.accent,
        text_input::Status::Hovered => blend(hairline(theme), p.text, 0.3),
        _ => blend(hairline(theme), p.border, 0.35),
    };
    text_input::Style {
        background: Background::Color(if matches!(status, text_input::Status::Disabled) {
            p.surface
        } else {
            field_surface(theme)
        }),
        border: Border {
            color: border,
            width: if matches!(status, text_input::Status::Focused { .. }) {
                1.5
            } else {
                1.0
            },
            radius: CONTROL_RADIUS.into(),
        },
        icon: p.muted,
        placeholder: blend(p.muted, field_surface(theme), 0.25),
        value: if matches!(status, text_input::Status::Disabled) {
            p.disabled
        } else {
            p.text
        },
        selection: p.accent.scale_alpha(0.35),
    }
}

/// The sidebar search: a filled pill without a resting border.
pub(super) fn search_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let p = colors(theme);
    let mut style = text_input_style(theme, status);
    style.background = Background::Color(blend(sidebar_background(theme), p.text, 0.07));
    if !matches!(status, text_input::Status::Focused { .. }) {
        style.border.color = Color::TRANSPARENT;
    }
    style
}

pub(super) fn pick_list_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let p = colors(theme);
    let open = matches!(status, pick_list::Status::Opened { .. });
    let hovered = matches!(status, pick_list::Status::Hovered);
    pick_list::Style {
        text_color: p.text,
        placeholder_color: p.muted,
        handle_color: p.muted,
        background: Background::Color(if hovered {
            blend(p.raised, p.text, 0.06)
        } else {
            p.raised
        }),
        border: Border {
            color: if open {
                p.accent
            } else {
                blend(p.raised, p.border, 0.45)
            },
            width: 1.0,
            radius: CONTROL_RADIUS.into(),
        },
    }
}

pub(super) fn menu_style(theme: &Theme) -> menu::Style {
    let p = colors(theme);
    menu::Style {
        background: Background::Color(p.raised),
        border: Border {
            color: blend(p.raised, p.border, 0.5),
            width: 1.0,
            radius: CONTROL_RADIUS.into(),
        },
        text_color: p.text,
        selected_text_color: p.text,
        selected_background: Background::Color(blend(p.raised, p.accent, 0.28)),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub(super) fn slider_style(theme: &Theme, status: slider::Status) -> slider::Style {
    let p = colors(theme);
    let engaged = matches!(status, slider::Status::Hovered | slider::Status::Dragged);
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                p.accent.into(),
                blend(p.surface, p.text, if dark(theme) { 0.16 } else { 0.14 }).into(),
            ),
            width: 4.0,
            border: Border {
                radius: 2.0.into(),
                ..Default::default()
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: if engaged { 9.0 } else { 8.0 },
            },
            background: if dark(theme) {
                p.text.into()
            } else {
                Color::WHITE.into()
            },
            border_width: 2.0,
            border_color: p.accent,
        },
    }
}

/// A page glyph, drawn in the current text colour.
pub(super) fn glyph<'a>(name: &str, size: f32, tint: Tint) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(glyph_source(name).into_bytes()))
        .width(size)
        .height(size)
        .style(move |theme: &Theme, _| svg::Style {
            color: Some(match tint {
                Tint::Warning => colors(theme).warning,
                Tint::Muted => colors(theme).muted,
                Tint::Accent => on_selection(theme),
            }),
        })
        .into()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tint {
    Warning,
    Muted,
    Accent,
}

/// Outline glyphs on a 24-unit grid, one stroke weight throughout.
fn glyph_source(name: &str) -> String {
    let body = match name {
        "displays" => {
            r#"<rect x="3" y="4" width="18" height="12.5" rx="2"/><path d="M8.5 20.5h7M12 16.5v4"/>"#
        }
        "input" => {
            r#"<rect x="2.5" y="6" width="19" height="12" rx="2.5"/><path d="M6.5 10h.01M10 10h.01M14 10h.01M17.5 10h.01M6.5 14h.01M17.5 14h.01M9.5 14h5"/>"#
        }
        "network" => {
            r#"<path d="M2.5 9.2a13.5 13.5 0 0 1 19 0M5.6 12.6a9 9 0 0 1 12.8 0M8.8 16a4.5 4.5 0 0 1 6.4 0"/><path d="M12 19.6h.01"/>"#
        }
        "bluetooth" => r#"<path d="M7 7.5l10 9-5 4.5V3l5 4.5-10 9"/>"#,
        "printers" => {
            r#"<path d="M7 9V3.5h10V9"/><rect x="3" y="9" width="18" height="8" rx="2"/><path d="M7 14h10v6.5H7z"/>"#
        }
        "sound" => {
            r#"<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z"/><path d="M15.5 9a4.2 4.2 0 0 1 0 6M18.2 6.3a8 8 0 0 1 0 11.4"/>"#
        }
        "personal" => {
            r#"<path d="M12 3a9 9 0 1 0 0 18c1 0 1.7-.8 1.7-1.7 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 0-1 .8-1.7 1.7-1.7H17a4 4 0 0 0 4-4C21 6.7 17 3 12 3z"/><path d="M7.5 12h.01M9.5 7.8h.01M14.5 7.8h.01M17 11.5h.01" stroke-width="2.6"/>"#
        }
        "accessibility" => {
            r#"<circle cx="12" cy="12" r="9.5"/><circle cx="12" cy="7.3" r="1.4" fill="black" stroke="none"/><path d="M7.5 10.2l4.5 1 4.5-1M12 11.2v3.3l-2.6 4M12 14.5l2.6 4"/>"#
        }
        "power" => {
            r#"<rect x="2.5" y="7" width="16.5" height="10" rx="2.2"/><path d="M21.5 10.5v3"/><path d="M11.4 9.2l-2 2.9h3l-2 2.9"/>"#
        }
        "date-time" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.2 2"/>"#,
        "region" => {
            r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a13.5 13.5 0 0 1 0 18M12 3a13.5 13.5 0 0 0 0 18"/>"#
        }
        "users" => {
            r#"<circle cx="9" cy="8.5" r="3.5"/><path d="M2.5 19.5c.8-3.3 3.4-5 6.5-5s5.7 1.7 6.5 5"/><path d="M15.8 5.3a3.4 3.4 0 0 1 0 6.4M17.8 14.7c1.9.7 3.2 2.4 3.7 4.8"/>"#
        }
        "privacy" => {
            r#"<path d="M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z"/><path d="M9 12l2.1 2.1L15 10.2"/>"#
        }
        "about" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5M12 7.8h.01"/>"#,
        "notifications" => {
            r#"<path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 1.8h-15z"/><path d="M10 20.8a2.2 2.2 0 0 0 4 0"/>"#
        }
        "default-apps" => {
            r#"<rect x="3.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.8"/>"#
        }
        "launcher" => {
            r#"<path d="M12 2.8l1.9 4.9 5 1.9-5 1.9L12 16.4l-1.9-4.9-5-1.9 5-1.9z"/><path d="M18.5 15.5l.8 2 2 .8-2 .8-.8 2-.8-2-2-.8 2-.8z"/>"#
        }
        "shortcuts" => {
            r#"<path d="M9 9V6.5A2.5 2.5 0 1 0 6.5 9H9zm0 0v6m0-6h6m-6 6H6.5A2.5 2.5 0 1 0 9 17.5V15zm0 0h6m0-6V6.5A2.5 2.5 0 1 1 17.5 9H15zm0 0v6m0 0h2.5a2.5 2.5 0 1 1-2.5 2.5V15z"/>"#
        }
        "widgets" => {
            r#"<rect x="3" y="3.5" width="18" height="17" rx="2.5"/><path d="M3 8.5h18M9.5 8.5v12"/>"#
        }
        "chevron-down" => r#"<path d="M6 9.5l6 6 6-6"/>"#,
        "chevron-right" => r#"<path d="M9.5 6l6 6-6 6"/>"#,
        "chevron-up" => r#"<path d="M6 14.5l6-6 6 6"/>"#,
        "pencil" => r#"<path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16z"/><path d="M13.5 6.5l4 4"/>"#,
        "close" => r#"<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>"#,
        "info" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5M12 7.8h.01"/>"#,
        "search" => r#"<circle cx="10.8" cy="10.8" r="6.3"/><path d="M15.5 15.5l4.8 4.8"/>"#,
        "check" => r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#,
        "warning" => r#"<path d="M12 3.5l9.5 16.5h-19z"/><path d="M12 10v4.5M12 17.3h.01"/>"#,
        "plug" => r#"<path d="M9 3v5M15 3v5M6.5 8h11v3a5.5 5.5 0 0 1-11 0zM12 16.5V21"/>"#,
        _ => {
            r#"<path d="M9.5 3h5M10.5 3v6.2L5.2 18a2 2 0 0 0 1.7 3h10.2a2 2 0 0 0 1.7-3l-5.3-8.8V3"/><path d="M7.5 15h9"/>"#
        }
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_its_own_glyph() {
        let fallback = glyph_source("");
        for entry in crate::registry::PAGES {
            if entry.id == "live-example" {
                continue;
            }
            assert_ne!(glyph_source(entry.id), fallback, "{}", entry.id);
        }
    }

    #[test]
    fn selected_text_reads_on_its_wash() {
        for color_scheme in [
            canopa_theme::ColorScheme::Dark,
            canopa_theme::ColorScheme::Light,
        ] {
            let theme = canopa_ui::theme(canopa_theme::Appearance {
                color_scheme,
                ..Default::default()
            });
            let rgb = |c: Color| {
                canopa_theme::Rgb(
                    (c.r * 255.0).round() as u8,
                    (c.g * 255.0).round() as u8,
                    (c.b * 255.0).round() as u8,
                )
            };
            assert!(rgb(on_selection(&theme)).contrast(rgb(selection(&theme))) >= 4.5);
        }
    }
}
