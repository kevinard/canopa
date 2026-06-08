/// Shared view helpers for collapsed, panel, and horizontal module layouts.
///
/// These cover the layout patterns that repeat across battery, network, volume,
/// and focused-window. Keeping them here avoids duplicating widget construction code
/// in every module.
use std::path::PathBuf;

use iced::widget::{canvas, container, image, row, svg, text};
use iced::{Alignment, Element, Length, Pixels, Point, Rectangle, Size, Vector};

use crate::app::Message;
use crate::config::FormatMode;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum IconLabelLayout {
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
pub(super) trait IconLabel {
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
        crate::theme::TEXT
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
        16,
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
                16,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                12,
                crate::theme::SUBTEXT,
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
                16,
                icon_color,
                fallback_resolved_style(default_style)
            ),
            styled_segment_text(
                label.to_string(),
                12,
                crate::theme::SUBTEXT,
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
    M: IconLabel,
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
    M: IconLabel,
{
    if let Some(segments) = module.styled_segments_for_layout(state, layout, default_style) {
        return match layout {
            IconLabelLayout::Collapsed => {
                collapsed_formatted_segments(segments, 16, module.color(state), rotation)
            }
            IconLabelLayout::Expanded => {
                expanded_formatted_segments(segments, 12, crate::theme::SUBTEXT, rotation)
            }
            IconLabelLayout::Horizontal => {
                horizontal_formatted_segments(segments, 12, crate::theme::SUBTEXT, rotation)
            }
        };
    }

    match layout {
        IconLabelLayout::Collapsed => {
            collapsed_icon_with_style(module.icon(state), module.color(state), default_style)
        }
        IconLabelLayout::Expanded => expanded_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            default_style,
        ),
        IconLabelLayout::Horizontal => horizontal_icon_label_with_style(
            module.icon(state),
            module.color(state),
            module.label_for_layout(state, layout),
            default_style,
        ),
    }
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
    let mut widget = text(segment)
        .size(style.size.unwrap_or(fallback_size as f32))
        .color(style.color.unwrap_or(fallback_color));
    if let Some(weight) = style.weight {
        let mut font = iced::Font::DEFAULT;
        font.weight = match weight {
            crate::config::Weight::Light => iced::font::Weight::Light,
            crate::config::Weight::Normal => iced::font::Weight::Normal,
            crate::config::Weight::Bold => iced::font::Weight::Bold,
        };
        widget = widget.font(font);
    }
    widget.into()
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
    format.template_for(mode).map(|template| {
        template.render_styled_cow_segments_with_default(values, state, default_style)
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

    canvas::Canvas::new(RotatedVisualSegments {
        segments,
        fallback_size,
        fallback_color,
        rotation,
        content_width,
        content_height,
    })
    .width(Length::Fixed(content_height))
    .height(Length::Fixed(content_width))
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
                draw_canvas_visual_segment(
                    frame,
                    segment,
                    Point::new(x, origin.y + (self.content_height - segment_height) / 2.0),
                    segment_width,
                    segment_height,
                    self.fallback_size,
                    self.fallback_color,
                    text_alignment,
                );
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
    width: f32,
    height: f32,
    fallback_size: u32,
    fallback_color: iced::Color,
    text_alignment: CanvasTextAlignment,
) {
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
            let mut font = iced::Font::DEFAULT;
            if let Some(weight) = style.weight {
                font.weight = match weight {
                    crate::config::Weight::Light => iced::font::Weight::Light,
                    crate::config::Weight::Normal => iced::font::Weight::Normal,
                    crate::config::Weight::Bold => iced::font::Weight::Bold,
                };
            }
            let layout = visual_segment_canvas_text_layout(origin, width, height, text_alignment);

            frame.fill_text(canvas::Text {
                content: text.clone(),
                position: layout.position,
                max_width: layout.max_width,
                color: style.color.unwrap_or(fallback_color),
                size: Pixels(size),
                font,
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
            let line_count = text.lines().count().max(1) as f32;
            let width = text
                .lines()
                .map(|line| estimated_canvas_text_line_width(line, size))
                .fold(size, f32::max)
                .ceil();
            (width, (line_count * size * 1.2).ceil())
        }
    }
}

const CANVAS_SEGMENT_SPACING: f32 = 4.0;

fn estimated_canvas_text_line_width(line: &str, size: f32) -> f32 {
    let width_em = line
        .chars()
        .map(estimated_canvas_char_width_em)
        .sum::<f32>();
    (width_em * size).max(size)
}

fn estimated_canvas_char_width_em(ch: char) -> f32 {
    match ch {
        ' ' | '\t' => 0.35,
        'i' | 'j' | 'l' | 'I' | '!' | '.' | ',' | ':' | ';' | '\'' | '`' | '|' => 0.35,
        'm' | 'w' | 'M' | 'W' | '@' | '#' | '%' | '&' | '<' | '>' => 1.0,
        _ if ch.is_ascii_alphanumeric() => 0.56,
        _ if ch.is_ascii_punctuation() => 0.65,
        _ => 1.0,
    }
}

fn visual_segment_size(style: crate::config::ResolvedStyle, fallback_size: u32) -> f32 {
    style.size.unwrap_or(fallback_size as f32).max(1.0)
}

#[cfg(test)]
mod tests {
    use crate::config::{ColorValue, Style, ThemeToken, Weight};

    #[test]
    fn bar_style_resolves_as_fallback_text_style() {
        let bar_style = Style {
            color: Some(ColorValue::Token(ThemeToken::Accent)),
            size: Some(14.0),
            weight: Some(Weight::Bold),
        };

        let resolved = super::fallback_resolved_style(&bar_style);

        assert_eq!(resolved.color, Some(crate::theme::ACCENT));
        assert_eq!(resolved.size, Some(14.0));
        assert_eq!(resolved.weight, Some(Weight::Bold));
    }

    #[test]
    fn formatted_segments_helper_constructs_unrotated_and_rotated_content() {
        let segments = vec![("Hello".to_string(), crate::config::ResolvedStyle::default())];

        let _ = super::formatted_segments_content(
            segments.clone(),
            12,
            crate::theme::TEXT,
            crate::config::ResolvedContentRotation::None,
        );
        let _ = super::formatted_segments_content(
            segments.clone(),
            12,
            crate::theme::TEXT,
            crate::config::ResolvedContentRotation::Clockwise,
        );
        let _ = super::formatted_segments_content(
            segments,
            12,
            crate::theme::TEXT,
            crate::config::ResolvedContentRotation::CounterClockwise,
        );
    }

    #[test]
    fn rotated_canvas_metrics_reserve_segment_space() {
        let segments = vec![super::FormattedSegmentVisual::Text {
            text: "W<&".to_string(),
            style: crate::config::ResolvedStyle {
                size: Some(16.0),
                ..Default::default()
            },
        }];

        let (width, height) = super::visual_segments_canvas_size(&segments, 12);

        assert!(width >= 48.0);
        assert!(height >= 19.0);
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
            "niribar-focused-window-icon-{}.svg",
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
            crate::theme::TEXT,
            crate::config::ResolvedContentRotation::None,
        );
        let _ = super::formatted_segment_visuals_content(
            segments,
            12,
            crate::theme::TEXT,
            crate::config::ResolvedContentRotation::Clockwise,
        );

        let _ = std::fs::remove_file(path);
    }
}
