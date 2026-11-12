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
/// The app mark before the toolbar when the sidebar is shown.
pub(super) const BRAND_WIDTH: f32 = 62.0;
/// Dialog and footer buttons match the height of a text field beside them.
pub(super) const DIALOG_BUTTON_PADDING: [f32; 2] = [9.0, 18.0];

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
    let mut frame = window_surface(appearance, false);
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

/// The surface that fills a Files window. The compositor owns the window's
/// shape, corner radius, outline and shadow, so the default surface runs edge
/// to edge instead of drawing rounded corners over an opaque rectangle. In
/// high contrast, floating dialogs keep their own full-strength outline.
/// Customized radii and borders still apply.
pub(super) fn window_surface(appearance: &Appearance, floating: bool) -> container::Style {
    let style = appearance.style(UiKind::Panel);
    let mut frame = canopa_ui::frame(appearance, style);
    if !style.custom_radius {
        frame.border.radius = 0.0.into();
    }
    if !style.custom_border {
        frame.border.width = if floating && appearance.high_contrast() {
            1.0
        } else {
            0.0
        };
    }
    // Floating dialogs sit one step above the browser surface they cover.
    let custom_background = appearance.customization.ui[UiKind::Panel as usize]
        .background
        .is_some();
    if floating && !custom_background && !appearance.high_contrast() {
        let lift = match appearance.color_scheme {
            canopa_theme::ColorScheme::Dark => 0.55,
            canopa_theme::ColorScheme::Light => 0.0,
        };
        frame.background = Some(
            mix(
                ink(appearance, ColorRole::Surface),
                ink(appearance, ColorRole::Raised),
                lift,
            )
            .into(),
        );
    }
    frame
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
        (Recession::Inactive, true) => 0.25,
        (Recession::Inactive, false) => 0.1,
        (Recession::Prompt, true) => 0.55,
        (Recession::Prompt, false) => 0.28,
    };
    container::Style::default().background(Color::BLACK.scale_alpha(alpha))
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
pub(super) fn panel_surface(appearance: &Appearance) -> container::Style {
    container::Style::default().background(mix(
        ink(appearance, ColorRole::Surface),
        ink(appearance, ColorRole::Background),
        0.35,
    ))
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
        let deep = mix(danger, Color::from_rgb8(150, 0, 30), 0.55);
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
    let mut labels = column![title].width(Length::Fill).spacing(2);
    if let Some(subtitle) = subtitle {
        labels = labels.push(subtitle);
    }
    let symbol = container(desktop.icons.chrome(icon, 21.0, 1.0))
        .padding(9)
        .style(move |_| symbol_tile(appearance, tone));
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
        style.text_color = ink(&appearance, ColorRole::Text);
        style.background = Some(selected_fill(&appearance).into());
        style.border = iced::Border {
            width: 1.0,
            color: ink(&appearance, ColorRole::Accent).scale_alpha(0.55),
            radius: 11.0.into(),
        };
        style
    }
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
            container::Style::default()
                .background(ink(appearance, ColorRole::Raised).scale_alpha(0.55))
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
    DangerTrash,
    Warning,
    Plus,
    Minus,
    Close,
    Fit,
    Image,
    NewFolder,
    Computer,
    Down,
    Disk,
    Toolbar,
    Check,
}

impl Icon {
    pub(super) const ALL: [Self; 45] = [
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
        Self::DangerTrash,
        Self::Warning,
        Self::Plus,
        Self::Minus,
        Self::Close,
        Self::Fit,
        Self::Image,
        Self::NewFolder,
        Self::Computer,
        Self::Down,
        Self::Disk,
        Self::Toolbar,
        Self::Check,
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
            Self::Up => "m6 15 6-6 6 6",
            Self::Down => "m6 9 6 6 6-6",
            Self::Check => "m5 12.5 4.5 4.5L19 7.5",
            Self::Disk => {
                "M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3v12c0 1.7-3.6 3-8 3s-8-1.3-8-3Zm0 0c0 1.7 3.6 3 8 3s8-1.3 8-3m-16 6c0 1.7 3.6 3 8 3s8-1.3 8-3"
            }
            Self::Toolbar => "M3 5h18v14H3Zm0 5h18M6.5 7.5h.01m3 0h.01m3 0h.01",
            Self::Search => "M16.2 16.2 21 21M18 11a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z",
            Self::Eye => {
                "M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6S2 12 2 12Zm12.5 0a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z"
            }
            Self::More => "M5 12h.01m6.99 0h.01m6.99 0h.01",
            Self::NewFolder => {
                "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Zm9 3.5v6m-3-3h6"
            }
            Self::Folder | Self::ActiveFolder => {
                "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"
            }
            Self::Home => "m3 10 9-7 9 7m-2-1v11H5V9m5 11v-6h4v6",
            Self::Desktop => "M3 5h18v12H3Zm9 12v4m-4 0h8",
            Self::Computer => "M4 4h16v10H4Zm-2 14h20l-2-4H4Zm8-2h4",
            Self::Documents => "M6 3h8l4 4v14H6Zm8 0v5h4m-9 4h6m-6 4h6",
            Self::Downloads => "M12 3v12m-4-4 4 4 4-4M5 16v4h14v-4",
            Self::Pictures => "M3 5h18v14H3Zm1 12 5-5 3 3 3-3 5 5M15 9h.01",
            Self::Drive => "M5 5h14l2 10v4H3v-4Zm-2 10h18m-4 2h.01",
            Self::Trash | Self::DangerTrash => "M4 6h16M9 6V3h6v3m3 0-1 15H7L6 6m4 4v7m4-7v7",
            Self::Warning => "M12 3 2 20h20Zm0 6v5m0 3h.01",
            Self::Plus => "M12 5v14M5 12h14",
            Self::Minus => "M5 12h14",
            Self::Close => "M6 6l12 12M18 6 6 18",
            Self::Fit => "M4 9V4h5m6 0h5v5m0 6v5h-5m-6 0H4v-5",
            Self::Image => "M4 5h16v14H4Zm0 11 4.5-4.5 3.5 3.5 2.5-2.5L20 17M15.5 9h.01",
            Self::Recovery => "M3 10a9 9 0 1 1 1 7m-1-13v6h6m3-4v6l3 2",
            Self::Eject => "m6 14 6-8 6 8Zm0 4h12",
            Self::Columns => "M3 5h4.5v14H3Zm6.75 0h4.5v14h-4.5ZM16.5 5H21v14h-4.5Z",
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
