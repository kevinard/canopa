//! Files browser chrome, using the shared Canopa appearance roles.
use super::{Desktop, Element, Message};
#[cfg(test)]
use crate::browser_ui::text_layout::ellipsize;
pub(super) use crate::browser_ui::text_layout::{
    file_name_line, single_line, text_width, tooltip_style, two_lines,
};
pub(super) use crate::browser_ui::{Icon, bar, bar_tint, divider, sidebar_current, window_surface};
use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Color, Length, Theme,
    font::Weight,
    widget::{column, container, row, scrollable, space, text, tooltip},
};

pub(super) const SIDEBAR_WIDTH: f32 = 204.0;
/// Dialog and footer buttons match the height of a text field beside them.
pub(super) const DIALOG_BUTTON_PADDING: [f32; 2] = [9.0, 18.0];

pub(super) fn ink(appearance: &Appearance, role: ColorRole) -> Color {
    canopa_ui::color(appearance.role(role))
}

/// How far a browser recedes behind another Files window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Recession {
    None,
    /// Another Files window, such as Quick Look or Properties, is active.
    Inactive,
    /// A confirmation or name prompt that belongs to this browser is open.
    Prompt,
}

/// The dimming laid over a browser so the active Files window reads as the
/// surface in use. It only paints; the browser keeps receiving input.
pub(super) fn scrim(appearance: &Appearance, recession: Recession) -> container::Style {
    let dark = appearance.color_scheme == canopa_theme::ColorScheme::Dark;
    let alpha = match (recession, dark) {
        (Recession::None, _) => return container::Style::default(),
        // Every Files window over a browser dims it alike, so Properties,
        // Quick Look and prompts read as one family.
        (Recession::Inactive | Recession::Prompt, true) => 0.34,
        (Recession::Inactive | Recession::Prompt, false) => 0.09,
    };
    // Light windows dim toward their own ink, keeping their hue; dark ones
    // toward black.
    let shade = if dark {
        Color::BLACK
    } else {
        ink(appearance, ColorRole::Text)
    };
    container::Style::default().background(shade.scale_alpha(alpha))
}

/// The recessed area that presents an image or page with even margins.
pub(super) fn stage(appearance: &Appearance) -> container::Style {
    container::Style::default()
        .background(mix(
            ink(appearance, ColorRole::Surface),
            ink(appearance, ColorRole::Background),
            0.45,
        ))
        .border(iced::Border::default().rounded(12))
}

/// The Properties side panel: the sidebar's quieter surface, on the other side.
/// The Properties side panel: the folder's own surface, parted by its
/// divider, with the same cards as the Properties window.
pub(super) fn panel_surface(appearance: &Appearance) -> container::Style {
    container::Style::default().background(ink(appearance, ColorRole::Surface))
}

/// The track behind a group of segmented controls.
pub(super) fn segmented(appearance: &Appearance) -> container::Style {
    container::Style::default()
        .background(ink(appearance, ColorRole::Raised).scale_alpha(0.8))
        .border(iced::Border {
            width: 1.0,
            color: ink(appearance, ColorRole::Border).scale_alpha(if appearance.high_contrast() {
                1.0
            } else {
                0.35
            }),
            radius: 14.0.into(),
        })
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

/// The emphasis of a heading's symbol: the accent for ordinary windows, the
/// danger role for confirmations that cannot be undone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Accent,
    Danger,
}

pub(super) fn symbol_tile(appearance: &Appearance, tone: Tone) -> container::Style {
    match tone {
        Tone::Accent => highlight(appearance, ColorRole::AccentSurface),
        Tone::Danger => container::Style::default()
            .background(ink(appearance, ColorRole::Danger).scale_alpha(
                if appearance.high_contrast() {
                    0.0
                } else if appearance.color_scheme == canopa_theme::ColorScheme::Dark {
                    0.22
                } else {
                    0.16
                },
            ))
            .border(iced::Border {
                width: if appearance.high_contrast() { 1.0 } else { 0.0 },
                color: ink(appearance, ColorRole::Danger),
                radius: 10.0.into(),
            }),
    }
}

/// A dialog heading whose symbol carries `tone`.
pub(super) fn toned_heading<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    tone: Tone,
    title: impl Into<String>,
    subtitle: impl Into<String>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let mut bold = canopa_ui::font(appearance);
    bold.weight = Weight::Semibold;
    let title = text(title.into())
        .font(bold)
        .size(appearance.typography().title_size.px())
        .wrapping(text::Wrapping::WordOrGlyph);
    heading_toned(desktop, icon, tone, title.into(), subtitle.into(), None)
}

/// The filled button that confirms an action which cannot be undone. It keeps
/// the shared focus ring and corner radius; only its fill uses the danger role.
pub(super) fn danger_action<'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Option<Message>,
    focused: bool,
    appearance: &Appearance,
) -> iced::widget::Button<'a, Message> {
    use iced::widget::button;
    let radius = appearance.style(UiKind::Button).radius;
    let appearance = appearance.clone();
    canopa_ui::styled_action(
        content,
        on_press,
        canopa_ui::Action::Destructive,
        focused,
        &appearance.clone(),
    )
    .style(move |theme, status| {
        let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Primary, focused);
        let rgb = |c: Color| {
            let [r, g, b, _] = c.into_rgba8();
            canopa_theme::Rgb(r, g, b)
        };
        // The dark scheme's danger role is a light tint for text on dark
        // surfaces; a fill reads as danger when deepened to carry white text.
        let danger = ink(&appearance, ColorRole::Danger);
        let deep = mix(danger, Color::from_rgb8(150, 0, 30), 0.72);
        let danger = if appearance.color_scheme == canopa_theme::ColorScheme::Dark
            && !appearance.high_contrast()
            && rgb(deep).contrast(canopa_theme::Rgb(255, 255, 255)) >= 4.5
        {
            deep
        } else {
            danger
        };
        let fill = match status {
            button::Status::Hovered => mix(danger, Color::WHITE, 0.1),
            button::Status::Pressed => mix(danger, Color::BLACK, 0.12),
            _ => danger,
        };
        if status == button::Status::Disabled {
            style.background = None;
            style.text_color = ink(&appearance, ColorRole::Disabled);
        } else {
            style.background = Some(fill.into());
            let canopa_theme::Rgb(r, g, b) = rgb(danger).contrasting_text();
            style.text_color = Color::from_rgb8(r, g, b);
        }
        if !(focused && status != button::Status::Disabled) {
            style.border.width = 0.0;
        }
        style.border.radius = iced::border::Radius {
            top_left: radius[0].px(),
            top_right: radius[1].px(),
            bottom_right: radius[2].px(),
            bottom_left: radius[3].px(),
        };
        style
    })
}

pub(super) fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * amount,
        g: a.g + (b.g - a.g) * amount,
        b: a.b + (b.b - a.b) * amount,
        a: a.a,
    }
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

/// `heading_line` led by an item's own picture instead of a symbol tile.
pub(super) fn heading_line_picture<'a>(
    desktop: &'a Desktop,
    picture: Element<'a, Message>,
    title: String,
    subtitle: String,
    action: Element<'a, Message>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let mut bold = font;
    bold.weight = Weight::Semibold;
    let title = single_line(
        title,
        bold,
        appearance.typography().title_size.px(),
        None,
        true,
    );
    let subtitle = (!subtitle.is_empty()).then(|| {
        text(subtitle)
            .font(font)
            .size(appearance.typography().label_size.px() + 1.0)
            .color(ink(appearance, ColorRole::Muted))
            .wrapping(text::Wrapping::WordOrGlyph)
            .into()
    });
    container(heading_with_symbol(picture, title, subtitle, Some(action)))
        .padding([4, 0])
        .into()
}

fn heading<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    title: Element<'a, Message>,
    subtitle: String,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    heading_toned(desktop, icon, Tone::Accent, title, subtitle, action)
}

fn heading_toned<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    tone: Tone,
    title: Element<'a, Message>,
    subtitle: String,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let font = canopa_ui::font(appearance);
    let subtitle = (!subtitle.is_empty()).then(|| {
        text(subtitle)
            .font(font)
            .size(appearance.typography().label_size.px() + 1.0)
            .color(ink(appearance, ColorRole::Muted))
            .wrapping(text::Wrapping::WordOrGlyph)
            .into()
    });
    heading_parts(desktop, icon, tone, title, subtitle, action)
}

/// A heading whose subtitle is already laid out, such as a truncated path.
pub(super) fn heading_parts<'a>(
    desktop: &'a Desktop,
    icon: Icon,
    tone: Tone,
    title: Element<'a, Message>,
    subtitle: Option<Element<'a, Message>>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let symbol = container(desktop.icons.chrome(icon, 21.0, 1.0))
        .padding(9)
        .style(move |_| symbol_tile(appearance, tone));
    heading_with_symbol(symbol.into(), title, subtitle, action)
}

/// A heading led by `symbol`, such as an item's own picture.
pub(super) fn heading_with_symbol<'a>(
    symbol: Element<'a, Message>,
    title: Element<'a, Message>,
    subtitle: Option<Element<'a, Message>>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut labels = column![title].width(Length::Fill).spacing(2);
    if let Some(subtitle) = subtitle {
        labels = labels.push(subtitle);
    }
    let mut content: Vec<Element<'a, Message>> = vec![symbol, labels.into()];
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
        container(desktop.icons.chrome(Icon::Close, 16.0, 0.86))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        message.clone(),
        canopa_ui::Action::Quiet,
        false,
        appearance,
    )
    .width(30)
    .height(30)
    .padding(4)
    .style(close_style(appearance));
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
    .padding(6)
    .style(super::chrome::tooltip_style)
    .delay(std::time::Duration::from_millis(400))
    .into()
}

/// A round, filled close control so window dismissal reads as a button.
pub(super) fn close_style(
    appearance: &Appearance,
) -> impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static {
    let appearance = appearance.clone();
    move |theme, status| {
        use iced::widget::button::Status;
        let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
        let raised = ink(&appearance, ColorRole::Raised);
        style.background = Some(
            match status {
                Status::Hovered => mix(raised, ink(&appearance, ColorRole::Text), 0.1),
                Status::Pressed => mix(raised, Color::BLACK, 0.1),
                _ => raised,
            }
            .into(),
        );
        style.border = iced::Border {
            width: if appearance.high_contrast() { 1.0 } else { 0.0 },
            color: ink(&appearance, ColorRole::Border),
            radius: 15.0.into(),
        };
        style
    }
}

/// The selected segment in a segmented control: the accent surface with a
/// faint accent edge, as in the browser's view switcher.
pub(super) fn selected_segment(
    appearance: &Appearance,
) -> impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static {
    let appearance = appearance.clone();
    move |theme, status| {
        let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
        let selected = selected_segment_style(&appearance);
        style.text_color = ink(&appearance, ColorRole::OnAccent);
        style.background = selected.background;
        style.border = selected.border;
        style
    }
}

/// The selected segment of a segmented control: the accent, as checked boxes
/// and primary buttons carry it, with its own foreground.
pub(super) fn selected_segment_style(appearance: &Appearance) -> container::Style {
    container::Style::default()
        .background(ink(appearance, ColorRole::Accent))
        .color(ink(appearance, ColorRole::OnAccent))
        .border(iced::Border {
            width: if appearance.high_contrast() { 1.0 } else { 0.0 },
            color: ink(appearance, ColorRole::Focus),
            radius: 11.0.into(),
        })
}

/// The fill of a selected segment: a clear accent tint over the track.
pub(super) fn selected_fill(appearance: &Appearance) -> Color {
    if appearance.high_contrast() {
        ink(appearance, ColorRole::AccentSurface)
    } else {
        mix(
            ink(appearance, ColorRole::AccentSurface),
            ink(appearance, ColorRole::Accent),
            0.22,
        )
    }
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

/// A quiet grouped section: a tinted surface without its own outline, for
/// groups that already sit inside an outlined window.
pub(super) fn section_card<'a>(
    appearance: &'a Appearance,
    body: impl Into<Element<'a, Message>>,
) -> container::Container<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding(14)
        .style(move |_| {
            // Dark dialogs are already lifted to the raised tone; their cards
            // step a little further toward the text so they still separate.
            let background = match appearance.color_scheme {
                canopa_theme::ColorScheme::Dark => mix(
                    ink(appearance, ColorRole::Raised),
                    ink(appearance, ColorRole::Text),
                    0.06,
                ),
                canopa_theme::ColorScheme::Light => {
                    ink(appearance, ColorRole::Raised).scale_alpha(0.55)
                }
            };
            container::Style::default()
                .background(background)
                .border(iced::Border {
                    width: if appearance.high_contrast() { 1.0 } else { 0.0 },
                    color: ink(appearance, ColorRole::Border),
                    radius: 12.0.into(),
                })
        })
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
            .style(canopa_ui::scrollbar)
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

/// `content` with a blocking question over it: the window dims, nothing
/// beneath takes input, and the question sits raised in the middle. The layer
/// stays, empty, without a question, so the content keeps its widget state.
pub(super) fn modal<'a>(
    desktop: &'a Desktop,
    content: Element<'a, Message>,
    question: Option<(String, Element<'a, Message>)>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    let layer: Element<'a, Message> = match question {
        Some((title, buttons)) => {
            let mut bold = canopa_ui::font(appearance);
            bold.weight = Weight::Semibold;
            let card = container(
                column![
                    heading_parts(
                        desktop,
                        Icon::Warning,
                        Tone::Danger,
                        text(title)
                            .font(bold)
                            .size(appearance.typography().body_size.px() + 1.0)
                            .wrapping(text::Wrapping::WordOrGlyph)
                            .into(),
                        None,
                        None,
                    ),
                    container(buttons).align_right(Length::Fill),
                ]
                .spacing(16),
            )
            .padding(20)
            .max_width(440)
            .style(move |_| {
                let mut style = window_surface(appearance, true);
                style.border = iced::Border {
                    color: ink(appearance, ColorRole::Border),
                    width: 1.0,
                    radius: 14.0.into(),
                };
                style.shadow = iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 28.0,
                };
                style
            });
            iced::widget::opaque(
                container(card)
                    .padding(24)
                    .center(Length::Fill)
                    .style(move |_| scrim(appearance, Recession::Prompt)),
            )
        }
        None => space().width(0).height(0).into(),
    };
    iced::widget::stack![content, layer].into()
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

#[cfg(test)]
mod ellipsis_tests {
    use super::*;
    use iced::Font;

    #[test]
    fn file_names_lose_their_middle_and_keep_their_ending() {
        let font = Font::DEFAULT;
        let name = "Quarterly financial report for the board, final version.pdf";
        let full = text_width(name, font, 14.0);
        assert_eq!(ellipsize(name, font, 14.0, full + 1.0, true), name);
        let short = ellipsize(name, font, 14.0, full / 2.0, true);
        assert!(
            short.starts_with("Quarterly") && short.ends_with(".pdf"),
            "{short}"
        );
        assert!(short.contains('…') && text_width(&short, font, 14.0) <= full / 2.0);
        let end = ellipsize(name, font, 14.0, full / 2.0, false);
        assert!(end.ends_with('…') && end.starts_with("Quarterly"), "{end}");
    }

    #[test]
    fn grid_names_break_after_separators_and_keep_their_extension() {
        let font = Font::DEFAULT;
        let name = "IMG_20260104_1200.png";
        let full = text_width(name, font, 13.0);
        assert_eq!(two_lines(name, font, 13.0, full + 1.0), name);
        let broken = two_lines(name, font, 13.0, full - 4.0);
        assert_eq!(broken, "IMG_20260104_\n1200.png");
        let long = two_lines(&name.repeat(4), font, 13.0, full);
        let (_, second) = long.split_once('\n').unwrap();
        assert!(second.contains('…') && second.ends_with(".png"), "{long}");
    }
}
