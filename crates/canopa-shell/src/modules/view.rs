/// Shared view helpers for collapsed, panel, and horizontal module layouts.
///
/// These cover the layout patterns that repeat across battery, network, volume,
/// and focused-window. Keeping them here avoids duplicating widget construction code
/// in every module.
use std::path::PathBuf;

use iced::widget::{canvas, container, image, row, svg};
use iced::{Alignment, Element, Length, Pixels, Point, Rectangle, Size, Vector};

use crate::app::Message;
use crate::config::FormatMode;
use crate::theme::bar_metrics::ICON_SIZE;

mod rotated_text;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IconLabelLayout {
    Collapsed,
    Expanded,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum FormattedSegmentVisual {
    Text {
        text: String,
        style: crate::config::ResolvedStyle,
    },
    Image {
        path: PathBuf,
        fallback_text: String,
        style: crate::config::ResolvedStyle,
    },
}

impl From<IconLabelLayout> for FormatMode {
    fn from(layout: IconLabelLayout) -> Self {
        match layout {
            IconLabelLayout::Collapsed => FormatMode::Collapsed,
            IconLabelLayout::Expanded => FormatMode::Expanded,
            IconLabelLayout::Horizontal => FormatMode::Horizontal,
        }
    }
}

/// State-derived data for modules that render as an icon plus a short label.
///
/// Modules that need extra rows, overlay badges, focus handling, or custom
/// click targets keep their bespoke `Module` view methods instead of using this.
pub trait IconLabel: Send + Sync {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        crate::config::ContentRotation::None
    }
    fn accepts_state(&self, _state: &super::ModuleState) -> bool {
        true
    }
    fn formatted_color(&self, state: &super::ModuleState, layout: IconLabelLayout) -> iced::Color {
        if layout == IconLabelLayout::Collapsed {
            self.color(state)
        } else {
            crate::theme::palette().muted
        }
    }
    fn formatted_size(&self, _layout: IconLabelLayout) -> u32 {
        crate::theme::label_size() as u32
    }
    fn fallback_view(
        &self,
        state: &super::ModuleState,
        layout: IconLabelLayout,
        style: &crate::config::Style,
    ) -> Element<'_, Message> {
        default_icon_label_view(self, state, layout, style)
    }
    fn decorate<'a>(
        &'a self,
        _state: &super::ModuleState,
        _layout: IconLabelLayout,
        _rotation: crate::config::ResolvedContentRotation,
        content: Element<'a, Message>,
    ) -> Element<'a, Message> {
        content
    }

    fn icon(&self, state: &super::ModuleState) -> String;
    fn label(&self, state: &super::ModuleState) -> String;
    fn label_for_layout(&self, state: &super::ModuleState, _layout: IconLabelLayout) -> String {
        self.label(state)
    }
    /// Render styled segments for a given layout if a format template is configured.
    /// Returns `None` to fall back to the default icon+label rendering.
    fn styled_segments_for_layout(
        &self,
        _state: &super::ModuleState,
        _layout: IconLabelLayout,
        _default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        None
    }

    fn color(&self, _state: &super::ModuleState) -> iced::Color {
        crate::theme::palette().text
    }
}

/// Centred icon in a 20 px-tall fill-width container — the standard collapsed view.
///
/// ```text
/// ┌────────────────┐  height: 20px
/// │       ⚡       │  icon centred, size 16
/// └────────────────┘
/// ```
pub(super) fn collapsed_icon_with_style<'a>(
    icon: impl ToString,
    color: iced::Color,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(styled_segment_text(
        icon.to_string(),
        ICON_SIZE,
        color,
        fallback_resolved_style(default_style),
    ))
    .width(Length::Fill)
    .height(20)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

/// Icon + label row that fills available height — the standard horizontal bar view.
///
/// Unlike `expanded_icon_label`, width shrinks to content (no `Length::Fill`), making
/// it suitable for modules laid out side-by-side in a horizontal bar.
///
/// ```text
/// ┌──────────────┐  height: Fill, padding [0, 6]
/// │ ⚡  75%      │  icon size 16 + label size 12 (subtext colour)
/// └──────────────┘
/// ```
pub(super) fn horizontal_icon_label_with_style<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(
        row![
            styled_segment_text(
                icon.to_string(),
                ICON_SIZE,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                crate::theme::label_size() as u32,
                crate::theme::palette().muted,
                fallback_resolved_style(default_style)
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}

/// Icon + label row in a padded 20 px-tall container — the standard expanded view.
///
/// ```text
/// ┌───────────────────────────────┐  height: 20px, padding [0, 6]
/// │ ⚡  75%                       │  icon size 16 + label size 12 (subtext colour)
/// └───────────────────────────────┘
/// ```
pub(super) fn expanded_icon_label_with_style<'a>(
    icon: impl ToString,
    icon_color: iced::Color,
    label: impl ToString,
    default_style: &crate::config::Style,
) -> Element<'a, Message> {
    container(
        row![
            styled_segment_text(
                icon.to_string(),
                ICON_SIZE,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                crate::theme::label_size() as u32,
                crate::theme::palette().muted,
                fallback_resolved_style(default_style)
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(20)
    .align_y(Alignment::Center)
    .padding([0, 6])
    .into()
}

pub(super) fn icon_label_view_with_default<'a, M>(
    module: &'a M,
    state: &super::ModuleState,
    layout: IconLabelLayout,
    default_style: &crate::config::Style,
) -> Element<'a, Message>
where
    M: IconLabel + ?Sized,
{
    icon_label_view_with_rotation(
        module,
        state,
        layout,
        default_style,
        crate::config::ResolvedContentRotation::None,
    )
}

pub(super) fn icon_label_view_with_rotation<'a, M>(
    module: &'a M,
    state: &super::ModuleState,
    layout: IconLabelLayout,
    default_style: &crate::config::Style,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message>
where
    M: IconLabel + ?Sized,
{
    if !module.accepts_state(state) {
        return iced::widget::Space::new().into();
    }
    let mut applied_rotation = rotation;
    let content =
        if let Some(segments) = module.styled_segments_for_layout(state, layout, default_style) {
            let size = module.formatted_size(layout);
            let color = module.formatted_color(state, layout);
            match layout {
                IconLabelLayout::Collapsed => {
                    collapsed_formatted_segments(segments, size, color, rotation)
                }
                IconLabelLayout::Expanded => {
                    expanded_formatted_segments(segments, size, color, rotation)
                }
                IconLabelLayout::Horizontal => {
                    horizontal_formatted_segments(segments, size, color, rotation)
                }
            }
        } else {
            applied_rotation = crate::config::ResolvedContentRotation::None;
            module.fallback_view(state, layout, default_style)
        };
    module.decorate(state, layout, applied_rotation, content)
}

pub(super) fn default_icon_label_view<'a, M: IconLabel + ?Sized>(
    module: &'a M,
    state: &super::ModuleState,
    layout: IconLabelLayout,
    style: &crate::config::Style,
) -> Element<'a, Message> {
    match layout {
        IconLabelLayout::Collapsed => {
            collapsed_icon_with_style(module.icon(state), module.color(state), style)
        }
        IconLabelLayout::Expanded => expanded_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            style,
        ),
        IconLabelLayout::Horizontal => horizontal_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            style,
        ),
    }
}

pub(super) fn stacked_icon_label<'a>(
    icon: String,
    label: String,
    color: iced::Color,
    style: &crate::config::Style,
) -> Element<'a, Message> {
    iced::widget::column![
        styled_segment_text(icon, ICON_SIZE, color, fallback_resolved_style(style)),
        styled_segment_text(
            label,
            crate::theme::label_size() as u32,
            crate::theme::palette().text,
            fallback_resolved_style(style)
        ),
    ]
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into()
}

pub(super) fn spaced_icon_label<'a>(
    icon: String,
    label: String,
    style: &crate::config::Style,
) -> Element<'a, Message> {
    row![
        styled_segment_text(
            icon,
            16,
            crate::theme::palette().text,
            fallback_resolved_style(style)
        ),
        iced::widget::Space::new().width(Length::Fill),
        styled_segment_text(
            label,
            13,
            crate::theme::palette().text,
            fallback_resolved_style(style)
        ),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Build a centred, 4px-spaced row of styled segments. Container wrapping
/// (width/height/padding) is the caller's responsibility — this helper exists
/// only to eliminate the per-module map+spacing+align boilerplate.
pub(super) fn styled_segments_row<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    size: u32,
    color: iced::Color,
) -> iced::widget::Row<'a, Message> {
    row(segments
        .into_iter()
        .map(|(segment, style)| styled_segment_text(segment, size, color, style))
        .collect::<Vec<_>>())
    .spacing(4)
    .align_y(Alignment::Center)
}

pub(super) fn formatted_segments_content<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    fallback_size: u32,
    fallback_color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    let segments = segments
        .into_iter()
        .map(|(text, style)| FormattedSegmentVisual::Text { text, style })
        .collect();
    formatted_segment_visuals_content(segments, fallback_size, fallback_color, rotation)
}

pub(super) fn formatted_segment_visuals_content<'a>(
    segments: Vec<FormattedSegmentVisual>,
    fallback_size: u32,
    fallback_color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    match rotation {
        crate::config::ResolvedContentRotation::None => {
            visual_segments_row(segments, fallback_size, fallback_color).into()
        }
        crate::config::ResolvedContentRotation::Clockwise
        | crate::config::ResolvedContentRotation::CounterClockwise => {
            rotated_visual_segments_canvas(segments, fallback_size, fallback_color, rotation)
        }
    }
}

fn visual_segments_row<'a>(
    segments: Vec<FormattedSegmentVisual>,
    fallback_size: u32,
    fallback_color: iced::Color,
) -> iced::widget::Row<'a, Message> {
    row(segments
        .into_iter()
        .map(|segment| match segment {
            FormattedSegmentVisual::Text { text, style } => {
                styled_segment_text(text, fallback_size, fallback_color, style)
            }
            FormattedSegmentVisual::Image {
                path,
                fallback_text,
                style,
            } => image_segment_element(path, fallback_text, style, fallback_size, fallback_color),
        })
        .collect::<Vec<_>>())
    .spacing(4)
    .align_y(Alignment::Center)
}

fn image_segment_element<'a>(
    path: PathBuf,
    fallback_text: String,
    style: crate::config::ResolvedStyle,
    fallback_size: u32,
    fallback_color: iced::Color,
) -> Element<'a, Message> {
    let size = visual_segment_size(style, fallback_size).round().max(1.0) as u32;
    if !path.exists() {
        return styled_segment_text(fallback_text, fallback_size, fallback_color, style);
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("svg") {
        svg(svg::Handle::from_path(path))
            .width(size)
            .height(size)
            .into()
    } else {
        image(image::Handle::from_path(path))
            .width(size)
            .height(size)
            .into()
    }
}

pub(super) fn styled_segment_text<'a>(
    segment: String,
    fallback_size: u32,
    fallback_color: iced::Color,
    style: crate::config::ResolvedStyle,
) -> Element<'a, Message> {
    // Embedded, monochrome shell marks use the same configured size/color as
    // glyph icons. No filesystem lookup or extra module kind is needed.
    let icon = match segment.as_str() {
        "canopa:mark" => Some(include_bytes!("../../assets/canopa-mark.svg").as_slice()),
        "canopa:overview" => Some(include_bytes!("../../assets/overview.svg").as_slice()),
        "canopa:controls" => Some(include_bytes!("../../assets/controls.svg").as_slice()),
        "canopa:bluetooth" => Some(include_bytes!("../../assets/bluetooth.svg").as_slice()),
        "canopa:brightness" => Some(include_bytes!("../../assets/brightness.svg").as_slice()),
        "canopa:lock" => Some(include_bytes!("../../assets/lock.svg").as_slice()),
        "canopa:logout" => Some(include_bytes!("../../assets/logout.svg").as_slice()),
        "canopa:reboot" => Some(include_bytes!("../../assets/reboot.svg").as_slice()),
        "canopa:poweroff" => Some(include_bytes!("../../assets/poweroff.svg").as_slice()),
        _ => None,
    };
    if let Some(bytes) = icon {
        let size = style.size.unwrap_or(fallback_size as f32);
        let color = style.color.unwrap_or(fallback_color);
        return svg(svg::Handle::from_memory(bytes))
            .width(size)
            .height(size)
            .style(move |_, _| svg::Style { color: Some(color) })
            .into();
    }
    let font = segment_font(&segment, style);
    crate::theme::text(segment)
        .size(style.size.unwrap_or(fallback_size as f32))
        .color(style.color.unwrap_or(fallback_color))
        .font(font)
        .into()
}

/// Give standalone Nerd Font icons a full icon cell instead of a code font's
/// narrower character cell. Ordinary text keeps the toolkit's font selection;
/// systems without the symbols font retain normal glyph fallback.
pub(super) fn segment_font(segment: &str, style: crate::config::ResolvedStyle) -> iced::Font {
    let mut chars = segment.chars();
    let is_icon = matches!(
        chars.next(),
        Some('\u{e000}'..='\u{f8ff}' | '\u{f0000}'..='\u{ffffd}' | '\u{100000}'..='\u{10fffd}')
    ) && chars.next().is_none();
    iced::Font {
        family: if is_icon {
            iced::font::Family::Name("Symbols Nerd Font Mono")
        } else {
            crate::theme::font().family
        },
        weight: match style.weight.unwrap_or(crate::config::Weight::Normal) {
            crate::config::Weight::Light => iced::font::Weight::Light,
            crate::config::Weight::Normal => iced::font::Weight::Normal,
            crate::config::Weight::Bold => iced::font::Weight::Bold,
        },
        ..crate::theme::font()
    }
}

pub(super) fn fallback_resolved_style(
    default_style: &crate::config::Style,
) -> crate::config::ResolvedStyle {
    default_style.resolve()
}

pub(super) fn render_format_segments<'a>(
    format: &crate::config::FormatConfig,
    mode: crate::config::FormatMode,
    values: &[crate::config::FormatValue<'a>],
    state: Option<&str>,
    default_style: &crate::config::Style,
) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
    render_format_segments_with_text_size(
        format,
        mode,
        values,
        state,
        default_style,
        crate::theme::label_size() as u32,
    )
}

pub(super) fn render_format_segments_with_text_size<'a>(
    format: &crate::config::FormatConfig,
    mode: crate::config::FormatMode,
    values: &[crate::config::FormatValue<'a>],
    state: Option<&str>,
    default_style: &crate::config::Style,
    text_size: u32,
) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
    format.template_for(mode).map(|template| {
        let mut segments =
            template.render_styled_cow_segments_with_default(values, state, default_style);
        if mode != FormatMode::Expanded {
            for (index, (_, style)) in segments.iter_mut().enumerate() {
                // Keep explicit bar/segment sizes authoritative. A mixed segment
                // such as "{icon} {value}" remains one run at the text size.
                style
                    .size
                    .get_or_insert(if template.segment_is_exact_placeholder(index, "icon") {
                        ICON_SIZE as f32
                    } else {
                        text_size as f32
                    });
            }
        }
        segments
    })
}

fn collapsed_formatted_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    size: u32,
    color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    container(formatted_segments_content(segments, size, color, rotation))
        .width(Length::Fill)
        .height(
            if rotation == crate::config::ResolvedContentRotation::None {
                Length::Fixed(20.0)
            } else {
                Length::Shrink
            },
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn expanded_formatted_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    size: u32,
    color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    container(formatted_segments_content(segments, size, color, rotation))
        .width(Length::Fill)
        .height(
            if rotation == crate::config::ResolvedContentRotation::None {
                Length::Fixed(20.0)
            } else {
                Length::Shrink
            },
        )
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
}

fn horizontal_formatted_segments<'a>(
    segments: Vec<(String, crate::config::ResolvedStyle)>,
    size: u32,
    color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    container(formatted_segments_content(segments, size, color, rotation))
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
}

fn rotated_visual_segments_canvas<'a>(
    segments: Vec<FormattedSegmentVisual>,
    fallback_size: u32,
    fallback_color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
) -> Element<'a, Message> {
    let (content_width, content_height) = visual_segments_canvas_size(&segments, fallback_size);
    let text = rotated_text::Program::new(
        &segments,
        fallback_size,
        fallback_color,
        Size::new(content_width, content_height),
        rotation,
    );

    let icons = canvas::Canvas::new(RotatedVisualSegments {
        segments,
        fallback_size,
        fallback_color,
        rotation,
        content_width,
        content_height,
    })
    .width(Length::Fixed(content_height))
    .height(Length::Fixed(content_width));
    iced::widget::stack![
        icons,
        iced::widget::shader::Shader::new(text)
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .into()
}

#[derive(Debug, Clone)]
struct RotatedVisualSegments {
    segments: Vec<FormattedSegmentVisual>,
    fallback_size: u32,
    fallback_color: iced::Color,
    rotation: crate::config::ResolvedContentRotation,
    content_width: f32,
    content_height: f32,
}

impl<Message> canvas::Program<Message> for RotatedVisualSegments {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), iced::Color::TRANSPARENT);

        let radians = match self.rotation {
            crate::config::ResolvedContentRotation::Clockwise => std::f32::consts::FRAC_PI_2,
            crate::config::ResolvedContentRotation::CounterClockwise => {
                -std::f32::consts::FRAC_PI_2
            }
            crate::config::ResolvedContentRotation::None => 0.0,
        };

        frame.with_save(|frame| {
            frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
            frame.rotate(iced::Radians(radians));

            let origin = Point::new(-self.content_width / 2.0, -self.content_height / 2.0);
            let mut x = origin.x;
            let text_alignment = if self.segments.len() == 1 {
                CanvasTextAlignment::Center
            } else {
                CanvasTextAlignment::Start
            };

            for segment in &self.segments {
                let (segment_width, segment_height) =
                    visual_segment_canvas_metrics(segment, self.fallback_size);
                // WGPU text is rendered by the physical-pixel glyph layer.
                // Keep canvas text as a fallback for the test-only tiny-skia renderer.
                let is_image =
                    matches!(segment, FormattedSegmentVisual::Image { path, .. } if path.exists());
                if is_image || iced::advanced::renderer::Headless::name(renderer) != "wgpu" {
                    draw_canvas_visual_segment(
                        frame,
                        segment,
                        Point::new(x, origin.y + (self.content_height - segment_height) / 2.0),
                        Size::new(segment_width, segment_height),
                        self.fallback_size,
                        self.fallback_color,
                        text_alignment,
                    );
                }
                x += segment_width + CANVAS_SEGMENT_SPACING;
            }
        });

        vec![frame.into_geometry()]
    }
}

fn draw_canvas_visual_segment(
    frame: &mut canvas::Frame,
    segment: &FormattedSegmentVisual,
    origin: Point,
    size: Size,
    fallback_size: u32,
    fallback_color: iced::Color,
    text_alignment: CanvasTextAlignment,
) {
    let Size { width, height } = size;
    match segment {
        FormattedSegmentVisual::Image { path, style, .. } if path.exists() => {
            let size = visual_segment_size(*style, fallback_size).ceil().max(1.0);
            let bounds = Rectangle::new(
                Point::new(origin.x, origin.y + (height - size) / 2.0),
                Size::new(size, size),
            );
            let extension = path.extension().and_then(|extension| extension.to_str());

            if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
                frame.draw_svg(
                    bounds,
                    iced::advanced::svg::Svg::new(iced::advanced::svg::Handle::from_path(
                        path.clone(),
                    )),
                );
            } else {
                frame.draw_image(
                    bounds,
                    iced::advanced::image::Image::new(iced::advanced::image::Handle::from_path(
                        path.clone(),
                    )),
                );
            }
        }
        FormattedSegmentVisual::Text { text, style }
        | FormattedSegmentVisual::Image {
            fallback_text: text,
            style,
            ..
        } => {
            let size = visual_segment_size(*style, fallback_size);
            let font = segment_font(text, *style);
            let layout = visual_segment_canvas_text_layout(origin, width, height, text_alignment);

            frame.fill_text(canvas::Text {
                content: text.clone(),
                position: layout.position,
                max_width: layout.max_width,
                color: style.color.unwrap_or(fallback_color),
                size: Pixels(size),
                font,
                shaping: iced::advanced::text::Shaping::Advanced,
                align_x: layout.align_x.into(),
                align_y: layout.align_y,
                ..Default::default()
            });
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct CanvasTextLayout {
    position: Point,
    max_width: f32,
    align_x: iced::alignment::Horizontal,
    align_y: iced::alignment::Vertical,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum CanvasTextAlignment {
    Start,
    Center,
}

fn visual_segment_canvas_text_layout(
    origin: Point,
    width: f32,
    height: f32,
    alignment: CanvasTextAlignment,
) -> CanvasTextLayout {
    match alignment {
        CanvasTextAlignment::Start => CanvasTextLayout {
            position: Point::new(origin.x, origin.y + height / 2.0),
            max_width: width,
            align_x: iced::alignment::Horizontal::Left,
            align_y: iced::alignment::Vertical::Center,
        },
        CanvasTextAlignment::Center => CanvasTextLayout {
            position: Point::new(origin.x + width / 2.0, origin.y + height / 2.0),
            max_width: width,
            align_x: iced::alignment::Horizontal::Center,
            align_y: iced::alignment::Vertical::Center,
        },
    }
}

fn visual_segments_canvas_size(
    segments: &[FormattedSegmentVisual],
    fallback_size: u32,
) -> (f32, f32) {
    let metrics = segments
        .iter()
        .map(|segment| visual_segment_canvas_metrics(segment, fallback_size))
        .collect::<Vec<_>>();

    let width = (metrics.iter().map(|metrics| metrics.0).sum::<f32>()
        + CANVAS_SEGMENT_SPACING * segments.len().saturating_sub(1) as f32)
        .ceil()
        .max(1.0);
    let height = metrics
        .iter()
        .map(|metrics| metrics.1)
        .fold(fallback_size as f32, f32::max)
        .ceil()
        .max(1.0);

    (width, height)
}

fn visual_segment_canvas_metrics(
    segment: &FormattedSegmentVisual,
    fallback_size: u32,
) -> (f32, f32) {
    match segment {
        FormattedSegmentVisual::Image { path, style, .. } if path.exists() => {
            let size = visual_segment_size(*style, fallback_size).ceil().max(1.0);
            (size, size)
        }
        FormattedSegmentVisual::Text { text, style }
        | FormattedSegmentVisual::Image {
            fallback_text: text,
            style,
            ..
        } => {
            let size = visual_segment_size(*style, fallback_size);
            let measured = rotated_text::measure(text, *style, size);
            (
                measured.width.ceil(),
                measured.height.ceil().max(size * 1.2),
            )
        }
    }
}

const CANVAS_SEGMENT_SPACING: f32 = 4.0;

fn visual_segment_size(style: crate::config::ResolvedStyle, fallback_size: u32) -> f32 {
    style.size.unwrap_or(fallback_size as f32).max(1.0)
}

/// A compact, full-width control within the host's Bar Panel card frame.
pub(super) fn panel_control<'a>(
    title: impl Into<String>,
    description: impl Into<String>,
    trailing: impl Into<String>,
    on_press: Option<crate::app::Message>,
    focused: bool,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::{column, row};
    crate::theme::action(
        row![
            column![
                crate::theme::text(title.into()).size(16).font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..crate::theme::font()
                }),
                crate::theme::text(description.into()).style(canopa_ui::muted),
            ]
            .spacing(4)
            .width(iced::Length::Fill),
            crate::theme::text(trailing.into()).color(crate::theme::palette().accent),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        on_press,
        canopa_ui::Action::Quiet,
        focused,
    )
    .padding([8, 6])
    .width(iced::Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use crate::config::{ColorValue, Style, ThemeToken, Weight};

    #[test]
    fn configured_nerd_icons_use_symbol_metrics_without_changing_text_fonts() {
        let style = crate::config::ResolvedStyle {
            weight: Some(Weight::Bold),
            ..Default::default()
        };
        for icon in ["", "󰁾", "󱐋"] {
            let font = super::segment_font(icon, style);
            assert_eq!(
                font.family,
                iced::font::Family::Name("Symbols Nerd Font Mono")
            );
            assert_eq!(font.weight, iced::font::Weight::Bold);
        }
        for text in ["Quarterly planning", "42%", "◆", " Wi-Fi", "", "󰁾󱐋"] {
            let font = super::segment_font(text, style);
            assert_eq!(font.family, crate::theme::font().family);
            assert_eq!(font.weight, iced::font::Weight::Bold);
        }
    }

    #[test]
    fn bar_formats_keep_icon_and_text_sizes_across_layouts_and_fallbacks() {
        use crate::config::{FormatConfig, FormatMode, FormatTemplate, SegmentStyles};
        let mut template = FormatTemplate::parse(
            &["{icon}", "{value}", "{icon} {value}"],
            &["icon", "value"],
            "test.format",
        )
        .unwrap();
        let values = [("icon", "⚡".into()), ("value", "75%".into())];
        for mode in [FormatMode::Collapsed, FormatMode::Horizontal] {
            let format = FormatConfig {
                collapsed: Some(template.clone()),
                ..Default::default()
            };
            let segments =
                super::render_format_segments(&format, mode, &values, None, &Style::default())
                    .unwrap();
            assert_eq!(
                segments
                    .iter()
                    .map(|(_, style)| style.size)
                    .collect::<Vec<_>>(),
                vec![Some(20.0), Some(12.0), Some(12.0)]
            );
            assert_eq!(segments[2].0, "⚡ 75%");

            let segments = super::render_format_segments(
                &format,
                mode,
                &values,
                None,
                &Style {
                    size: Some(19.0),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(segments.iter().all(|(_, style)| style.size == Some(19.0)));
        }
        template.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: Some(Style {
                    size: Some(18.0),
                    ..Default::default()
                }),
                by_state: vec![(
                    "critical".into(),
                    Style {
                        size: Some(22.0),
                        ..Default::default()
                    },
                )],
            },
        );
        let format = FormatConfig {
            collapsed: Some(template),
            ..Default::default()
        };
        for (state, expected) in [(None, 18.0), (Some("critical"), 22.0)] {
            let segments = super::render_format_segments(
                &format,
                FormatMode::Horizontal,
                &values,
                state,
                &Style {
                    size: Some(14.0),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(segments[0].1.size, Some(expected));
            assert_eq!(segments[1].1.size, Some(14.0));
        }
        let panel = super::render_format_segments(
            &format,
            FormatMode::Expanded,
            &values,
            None,
            &Style::default(),
        )
        .unwrap();
        assert_eq!(
            panel[1].1.size, None,
            "expanded content keeps its existing fallback"
        );
    }

    #[test]
    fn bar_style_resolves_as_fallback_text_style() {
        let bar_style = Style {
            color: Some(ColorValue::Token(ThemeToken::Accent)),
            size: Some(14.0),
            weight: Some(Weight::Bold),
        };

        let resolved = super::fallback_resolved_style(&bar_style);

        assert_eq!(resolved.color, Some(crate::theme::palette().accent));
        assert_eq!(resolved.size, Some(14.0));
        assert_eq!(resolved.weight, Some(Weight::Bold));
    }

    #[test]
    fn formatted_segments_helper_constructs_unrotated_and_rotated_content() {
        let segments = vec![("Hello".to_string(), crate::config::ResolvedStyle::default())];

        let _ = super::formatted_segments_content(
            segments.clone(),
            12,
            crate::theme::palette().text,
            crate::config::ResolvedContentRotation::None,
        );
        let _ = super::formatted_segments_content(
            segments.clone(),
            12,
            crate::theme::palette().text,
            crate::config::ResolvedContentRotation::Clockwise,
        );
        let _ = super::formatted_segments_content(
            segments,
            12,
            crate::theme::palette().text,
            crate::config::ResolvedContentRotation::CounterClockwise,
        );
    }

    #[test]
    fn rotated_canvas_metrics_do_not_force_extra_lines_in_styled_titles() {
        use iced::advanced::text::{self, Paragraph};
        for title in [
            "AAAAAAAAAAAAAAAAAAAAAA",
            "Bold focused-window title",
            "Àgjpq — Résumé\nSecond line",
        ] {
            let style = crate::config::ResolvedStyle {
                size: Some(16.0),
                weight: Some(Weight::Bold),
                ..Default::default()
            };
            let segments = vec![super::FormattedSegmentVisual::Text {
                text: title.into(),
                style,
            }];
            let (width, height) = super::visual_segments_canvas_size(&segments, 12);
            let rendered = iced::advanced::graphics::text::Paragraph::with_text(text::Text {
                content: title,
                bounds: iced::Size::new(width, f32::INFINITY),
                size: iced::Pixels(16.0),
                line_height: text::LineHeight::Relative(1.2),
                font: super::segment_font(title, style),
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::default(),
            });
            assert_eq!(
                rendered.buffer().layout_runs().count(),
                title.lines().count()
            );
            assert!(rendered.min_height() <= height);
        }
    }

    #[test]
    fn rotated_canvas_text_metrics_are_not_full_em_per_character() {
        let title = "Quarterly planning";
        let segments = vec![super::FormattedSegmentVisual::Text {
            text: title.to_string(),
            style: crate::config::ResolvedStyle {
                size: Some(12.0),
                ..Default::default()
            },
        }];

        let (width, _) = super::visual_segments_canvas_size(&segments, 12);

        assert!(width < title.chars().count() as f32 * 12.0 * 0.75);
    }

    #[test]
    fn rotated_canvas_centers_text_within_reserved_segment_space() {
        let layout = super::visual_segment_canvas_text_layout(
            iced::Point::new(-120.0, -10.0),
            240.0,
            20.0,
            super::CanvasTextAlignment::Center,
        );

        assert_eq!(layout.position.x, 0.0);
        assert_eq!(layout.align_x, iced::alignment::Horizontal::Center);
    }

    #[test]
    fn rotated_canvas_anchors_multi_segment_text_at_segment_start() {
        let layout = super::visual_segment_canvas_text_layout(
            iced::Point::new(20.0, -10.0),
            240.0,
            20.0,
            super::CanvasTextAlignment::Start,
        );

        assert_eq!(layout.position.x, 20.0);
        assert_eq!(layout.align_x, iced::alignment::Horizontal::Left);
    }

    #[test]
    fn formatted_visual_segments_helper_constructs_image_content() {
        let path = std::env::temp_dir().join(format!(
            "canopa-shell-focused-window-icon-{}.svg",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"></svg>"#,
        )
        .unwrap();
        let segments = vec![super::FormattedSegmentVisual::Image {
            path: path.clone(),
            fallback_text: "□".to_string(),
            style: crate::config::ResolvedStyle {
                size: Some(14.0),
                ..Default::default()
            },
        }];

        let _ = super::formatted_segment_visuals_content(
            segments.clone(),
            12,
            crate::theme::palette().text,
            crate::config::ResolvedContentRotation::None,
        );
        let _ = super::formatted_segment_visuals_content(
            segments,
            12,
            crate::theme::palette().text,
            crate::config::ResolvedContentRotation::Clockwise,
        );

        let _ = std::fs::remove_file(path);
    }
}
