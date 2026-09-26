//! PROTOTYPE - throwaway focused-window rotation experiment.
//!
//! Three focused-window render variants, switchable in-app:
//! A generated SVG rotated as a snapshot, B iced Canvas glyph drawing, and
//! C a real iced widget subtree rotated through the renderer transform stack.

use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::{button, canvas, column, container, row, scrollable, svg, text};
use iced::{
    Alignment, Border, Color, Element, Event, Length, Pixels, Point, Radians, Rectangle, Size,
    Subscription, Task, Theme, Transformation, Vector,
};

const BAR_WIDTH: f32 = 48.0;
const BAR_HEIGHT: f32 = 360.0;
const ICON_SIZE: f32 = 18.0;
const SEGMENT_SPACING: f32 = 6.0;

fn main() -> iced::Result {
    iced::application(Prototype::default, update, view)
        .title("niribar focused-window rotation prototype")
        .subscription(subscription)
        .theme(theme)
        .window_size(Size::new(980.0, 620.0))
        .centered()
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    SelectVariant(Variant),
    SelectSample(usize),
    ToggleDirection,
    FontBigger,
    FontSmaller,
    Event(Event),
}

#[derive(Debug, Clone)]
struct Prototype {
    variant: Variant,
    direction: Direction,
    sample_index: usize,
    font_size: f32,
}

impl Default for Prototype {
    fn default() -> Self {
        Self {
            variant: Variant::CanvasGlyphs,
            direction: Direction::CounterClockwise,
            sample_index: 1,
            font_size: 13.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    SvgSnapshot,
    CanvasGlyphs,
    SubtreeTransform,
}

impl Variant {
    const ALL: [Self; 3] = [
        Self::SvgSnapshot,
        Self::CanvasGlyphs,
        Self::SubtreeTransform,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::SvgSnapshot => "A",
            Self::CanvasGlyphs => "B",
            Self::SubtreeTransform => "C",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::SvgSnapshot => "SVG snapshot",
            Self::CanvasGlyphs => "Canvas glyphs",
            Self::SubtreeTransform => "Patched subtree transform",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|variant| *variant == self)
            .unwrap();
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|variant| *variant == self)
            .unwrap();
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Clockwise,
    CounterClockwise,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Self::Clockwise => "clockwise",
            Self::CounterClockwise => "counter-clockwise",
        }
    }

    fn radians(self) -> f32 {
        match self {
            Self::Clockwise => std::f32::consts::FRAC_PI_2,
            Self::CounterClockwise => -std::f32::consts::FRAC_PI_2,
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Clockwise => Self::CounterClockwise,
            Self::CounterClockwise => Self::Clockwise,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    title: &'static str,
    app_id: &'static str,
}

const SAMPLES: [Sample; 4] = [
    Sample {
        title: "Firefox",
        app_id: "firefox.desktop",
    },
    Sample {
        title: "Quarterly planning - Module Content Rotation and Focused Window Icons",
        app_id: "org.mozilla.firefox",
    },
    Sample {
        title: "src/modules/focused_window.rs - niribar",
        app_id: "com.visualstudio.code",
    },
    Sample {
        title: "Terminal: rtk cargo test config::template_tests --lib",
        app_id: "Alacritty",
    },
];

fn update(state: &mut Prototype, message: Message) -> Task<Message> {
    match message {
        Message::SelectVariant(variant) => state.variant = variant,
        Message::SelectSample(index) => state.sample_index = index.min(SAMPLES.len() - 1),
        Message::ToggleDirection => state.direction = state.direction.toggle(),
        Message::FontBigger => state.font_size = (state.font_size + 1.0).min(22.0),
        Message::FontSmaller => state.font_size = (state.font_size - 1.0).max(9.0),
        Message::Event(Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })) => {
            use iced::keyboard::key::Named;
            use iced::keyboard::{Key, key};

            match key {
                Key::Named(Named::ArrowRight) => state.variant = state.variant.next(),
                Key::Named(Named::ArrowLeft) => state.variant = state.variant.previous(),
                Key::Character(value) if value.as_str() == "r" => {
                    state.direction = state.direction.toggle();
                }
                Key::Character(value) if value.as_str() == "+" || value.as_str() == "=" => {
                    state.font_size = (state.font_size + 1.0).min(22.0);
                }
                Key::Character(value) if value.as_str() == "-" => {
                    state.font_size = (state.font_size - 1.0).max(9.0);
                }
                Key::Named(key::Named::Space) => state.variant = state.variant.next(),
                _ => {}
            }
        }
        Message::Event(_) => {}
    }
    Task::none()
}

fn subscription(_state: &Prototype) -> Subscription<Message> {
    iced::event::listen().map(Message::Event)
}

fn theme(_state: &Prototype) -> Theme {
    Theme::Dark
}

fn view(state: &Prototype) -> Element<'_, Message> {
    let sample = SAMPLES[state.sample_index];
    let stage = container(stage_view(state, sample))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .style(panel_style);

    let content = row![controls(state, sample), stage]
        .spacing(20)
        .padding(20)
        .height(Length::Fill)
        .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(app_style)
        .into()
}

fn controls(state: &Prototype, sample: Sample) -> Element<'_, Message> {
    let variants = Variant::ALL.into_iter().fold(
        column![text("Variants").size(13)].spacing(8),
        |column, variant| {
            column.push(
                button(text(format!("{}  {}", variant.key(), variant.label())).size(13))
                    .width(Length::Fill)
                    .on_press(Message::SelectVariant(variant))
                    .style(move |theme, status| {
                        if state.variant == variant {
                            selected_button_style(theme, status)
                        } else {
                            iced::widget::button::secondary(theme, status)
                        }
                    }),
            )
        },
    );

    let samples = SAMPLES.iter().enumerate().fold(
        column![text("Samples").size(13)].spacing(8),
        |column, (index, entry)| {
            column.push(
                button(text(entry.title).size(12))
                    .width(Length::Fill)
                    .on_press(Message::SelectSample(index))
                    .style(move |theme, status| {
                        if state.sample_index == index {
                            selected_button_style(theme, status)
                        } else {
                            iced::widget::button::secondary(theme, status)
                        }
                    }),
            )
        },
    );

    let state_text = column![
        text("State").size(13),
        text(format!(
            "variant: {} - {}",
            state.variant.key(),
            state.variant.label()
        ))
        .size(12),
        text(format!("direction: {}", state.direction.label())).size(12),
        text(format!("font-size: {:.0}px", state.font_size)).size(12),
        text(format!("app-id: {}", sample.app_id)).size(12),
    ]
    .spacing(6);

    let body = column![
        variants,
        samples,
        column![
            text("Adjust").size(13),
            button(text("Rotate direction").size(12))
                .width(Length::Fill)
                .on_press(Message::ToggleDirection),
            row![
                button(text("-").size(14)).on_press(Message::FontSmaller),
                button(text("+").size(14)).on_press(Message::FontBigger),
            ]
            .spacing(8),
        ]
        .spacing(8),
        state_text,
    ]
    .spacing(18)
    .width(Length::Fill);

    container(scrollable(body))
        .width(270)
        .height(Length::Fill)
        .padding(16)
        .style(panel_style)
        .into()
}

fn stage_view(state: &Prototype, sample: Sample) -> Element<'_, Message> {
    let active = focused_window_variant(sample, state.variant, state.direction, state.font_size);
    let reference = horizontal_reference(sample, state.font_size);

    let module = container(active)
        .width(BAR_WIDTH)
        .height(BAR_HEIGHT)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(bar_style);

    let preview = row![
        column![
            text(format!(
                "{} - {}",
                state.variant.key(),
                state.variant.label()
            ))
            .size(18),
            text(sample.title).size(14),
            text(format!("direction: {}", state.direction.label())).size(12),
            reference,
        ]
        .spacing(12)
        .width(Length::Fill),
        module,
    ]
    .spacing(42)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .height(Length::Fill);

    column![
        preview,
        container(
            row![
                button(text("<").size(16))
                    .on_press(Message::SelectVariant(state.variant.previous())),
                text(format!(
                    "{} - {}",
                    state.variant.key(),
                    state.variant.label()
                ))
                .size(13),
                button(text(">").size(16)).on_press(Message::SelectVariant(state.variant.next())),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
        )
        .padding([10, 14])
        .style(switcher_style)
    ]
    .spacing(16)
    .align_x(Alignment::Center)
    .into()
}

fn horizontal_reference(sample: Sample, font_size: f32) -> Element<'static, Message> {
    container(
        row![
            icon_widget(),
            text(sample.title.to_string())
                .size(font_size)
                .color(Color::from_rgb8(231, 236, 242)),
        ]
        .spacing(SEGMENT_SPACING)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .style(reference_style)
    .into()
}

fn focused_window_variant(
    sample: Sample,
    variant: Variant,
    direction: Direction,
    font_size: f32,
) -> Element<'static, Message> {
    match variant {
        Variant::SvgSnapshot => svg_snapshot(sample, direction, font_size),
        Variant::CanvasGlyphs => canvas::Canvas::new(CanvasGlyphs {
            title: sample.title.to_string(),
            app_id: sample.app_id.to_string(),
            direction,
            font_size,
        })
        .width(BAR_WIDTH)
        .height(BAR_HEIGHT)
        .into(),
        Variant::SubtreeTransform => {
            let content = container(
                row![
                    icon_widget(),
                    text(sample.title.to_string())
                        .size(font_size)
                        .color(Color::from_rgb8(231, 236, 242)),
                ]
                .spacing(SEGMENT_SPACING)
                .align_y(Alignment::Center),
            )
            .width(Length::Shrink)
            .height(28)
            .padding([4, 2]);

            rotated(content, direction)
                .width(BAR_WIDTH)
                .height(BAR_HEIGHT)
                .into()
        }
    }
}

fn icon_widget() -> Element<'static, Message> {
    container(text("F").size(11).color(Color::from_rgb8(11, 18, 32)))
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(icon_style)
        .into()
}

fn svg_snapshot(sample: Sample, direction: Direction, font_size: f32) -> Element<'static, Message> {
    let width = estimated_text_width(sample.title, font_size) + ICON_SIZE + SEGMENT_SPACING + 10.0;
    let height = font_size.max(ICON_SIZE) + 8.0;
    let text_y = (height + font_size * 0.72) / 2.0;
    let icon_y = (height - ICON_SIZE) / 2.0;
    let title = escape_xml(sample.title);
    let svg_markup = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<rect x="1" y="{icon_y}" width="{ICON_SIZE}" height="{ICON_SIZE}" rx="4" fill="#72d6a3"/>
<text x="{icon_text_x}" y="{icon_text_y}" text-anchor="middle" fill="#0b1220" font-family="sans-serif" font-size="11" font-weight="700">F</text>
<text x="{text_x}" y="{text_y}" fill="#e7ecf2" font-family="sans-serif" font-size="{font_size}">{title}</text>
</svg>"##,
        icon_text_x = 1.0 + ICON_SIZE / 2.0,
        icon_text_y = icon_y + ICON_SIZE * 0.68,
        text_x = ICON_SIZE + SEGMENT_SPACING + 1.0,
    );

    svg(svg::Handle::from_memory(svg_markup.into_bytes()))
        .width(Length::Shrink)
        .height(Length::Shrink)
        .rotation(iced::Rotation::Solid(Radians(direction.radians())))
        .into()
}

#[derive(Debug, Clone)]
struct CanvasGlyphs {
    title: String,
    app_id: String,
    direction: Direction,
    font_size: f32,
}

impl<Message> canvas::Program<Message> for CanvasGlyphs {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::alignment::{Horizontal, Vertical};

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::TRANSPARENT);

        let content_width =
            estimated_text_width(&self.title, self.font_size) + ICON_SIZE + SEGMENT_SPACING + 2.0;
        let content_height = self.font_size.max(ICON_SIZE) + 8.0;
        let origin = Point::new(-content_width / 2.0, -content_height / 2.0);

        frame.with_save(|frame| {
            frame.translate(Vector::new(bounds.width / 2.0, bounds.height / 2.0));
            frame.rotate(Radians(self.direction.radians()));

            let icon_y = origin.y + (content_height - ICON_SIZE) / 2.0;
            let icon_path = canvas::Path::rounded_rectangle(
                Point::new(origin.x, icon_y),
                Size::new(ICON_SIZE, ICON_SIZE),
                4.0.into(),
            );
            frame.fill(&icon_path, Color::from_rgb8(114, 214, 163));

            frame.fill_text(canvas::Text {
                content: "F".to_string(),
                position: Point::new(origin.x + ICON_SIZE / 2.0, icon_y + ICON_SIZE / 2.0),
                color: Color::from_rgb8(11, 18, 32),
                size: Pixels(11.0),
                align_x: Horizontal::Center.into(),
                align_y: Vertical::Center,
                ..Default::default()
            });

            frame.fill_text(canvas::Text {
                content: self.title.clone(),
                position: Point::new(
                    origin.x + ICON_SIZE + SEGMENT_SPACING,
                    origin.y + content_height / 2.0,
                ),
                max_width: content_width,
                color: Color::from_rgb8(231, 236, 242),
                size: Pixels(self.font_size),
                align_y: Vertical::Center,
                ..Default::default()
            });

            frame.fill_text(canvas::Text {
                content: self.app_id.clone(),
                position: Point::new(
                    origin.x + ICON_SIZE + SEGMENT_SPACING,
                    origin.y + content_height + 8.0,
                ),
                max_width: content_width,
                color: Color::from_rgb8(148, 163, 184),
                size: Pixels(9.0),
                align_y: Vertical::Center,
                ..Default::default()
            });
        });

        vec![frame.into_geometry()]
    }
}

struct Rotated<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: renderer::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    direction: Direction,
    width: Length,
    height: Length,
}

fn rotated<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    direction: Direction,
) -> Rotated<'a, Message> {
    Rotated {
        content: content.into(),
        direction,
        width: Length::Shrink,
        height: Length::Shrink,
    }
}

impl<Message, Theme, Renderer> Rotated<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Rotated<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let wrapper_size =
            limits.resolve(self.width, self.height, Size::new(BAR_WIDTH, BAR_HEIGHT));
        let child_limits = layout::Limits::new(
            Size::ZERO,
            Size::new(wrapper_size.height, wrapper_size.width),
        )
        .loose();
        let child = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &child_limits);

        layout::Node::with_children(wrapper_size, vec![child])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(child) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, child, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(child) = layout.children().next() else {
            return;
        };
        let transform = rotate_around(
            child.bounds().center(),
            layout.bounds().center(),
            self.direction.radians(),
        );
        let inverse = transform.inverse();

        self.content.as_widget_mut().update(
            tree,
            event,
            child,
            cursor * inverse,
            renderer,
            clipboard,
            shell,
            &(*viewport * inverse),
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(child) = layout.children().next() else {
            return;
        };
        let transform = rotate_around(
            child.bounds().center(),
            bounds.center(),
            self.direction.radians(),
        );
        let inverse = transform.inverse();

        renderer.with_layer(bounds, |renderer| {
            renderer.with_transformation(transform, |renderer| {
                self.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    style,
                    child,
                    cursor * inverse,
                    &(*viewport * inverse),
                );
            });
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Some(child) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        let transform = rotate_around(
            child.bounds().center(),
            layout.bounds().center(),
            self.direction.radians(),
        );
        let inverse = transform.inverse();

        self.content.as_widget().mouse_interaction(
            tree,
            child,
            cursor * inverse,
            &(*viewport * inverse),
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            tree,
            layout.children().next()?,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Rotated<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(value: Rotated<'a, Message, Theme, Renderer>) -> Self {
        Element::new(value)
    }
}

fn rotate_around(source_center: Point, target_center: Point, radians: f32) -> Transformation {
    Transformation::translate(target_center.x, target_center.y)
        * Transformation::rotate(Radians(radians))
        * Transformation::translate(-source_center.x, -source_center.y)
}

fn estimated_text_width(value: &str, font_size: f32) -> f32 {
    value.chars().count() as f32 * font_size * 0.56
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn app_style(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb8(10, 15, 24))
        .color(Color::from_rgb8(226, 232, 240))
}

fn panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(17, 24, 39).into()),
        border: Border {
            color: Color::from_rgb8(51, 65, 85),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn bar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(28, 37, 52).into()),
        border: Border {
            color: Color::from_rgb8(71, 85, 105),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..Default::default()
    }
}

fn reference_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(30, 41, 59).into()),
        border: Border {
            color: Color::from_rgb8(71, 85, 105),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn switcher_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(2, 6, 23).into()),
        border: Border {
            color: Color::from_rgb8(100, 116, 139),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

fn icon_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::from_rgb8(114, 214, 163).into()),
        border: Border {
            color: Color::from_rgba8(255, 255, 255, 0.28),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn selected_button_style(
    _theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let base = match status {
        iced::widget::button::Status::Hovered => Color::from_rgb8(40, 116, 84),
        _ => Color::from_rgb8(25, 92, 67),
    };
    iced::widget::button::Style {
        background: Some(base.into()),
        text_color: Color::WHITE,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
