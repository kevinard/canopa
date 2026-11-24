use super::{Kind, Message, Selection};
use crate::screenshot_image::{Canvas, Frame};
use iced::advanced::{
    Clipboard, Layout, Renderer as _, Shell, Widget,
    image::{self, Renderer as _},
    layout, renderer,
    widget::Tree,
};
use iced::{Color, Event, Length, Point, Rectangle, Size, Theme, mouse, widget::image::Handle};

pub(super) struct Scene<'a> {
    pub source: &'a crate::screenshot_image::Source,
    pub file: &'a std::fs::File,
    pub frames: &'a [Frame],
    pub images: &'a [Handle],
    pub canvas: &'a Canvas,
    pub selected: Option<&'a Selection>,
    pub cursor: Option<(u32, u32)>,
    pub dragging: bool,
    pub mode: Kind,
    pub accent: Color,
}
impl Scene<'_> {
    fn fit(&self, bounds: Rectangle) -> Rectangle {
        let scale = (bounds.width / self.canvas.width as f32)
            .min(bounds.height / self.canvas.height as f32);
        let width = self.canvas.width as f32 * scale;
        let height = self.canvas.height as f32 * scale;
        Rectangle {
            x: bounds.x + (bounds.width - width) / 2.0,
            y: bounds.y + (bounds.height - height) / 2.0,
            width,
            height,
        }
    }
    fn project(&self, bounds: Rectangle, point: Option<Point>, clamp: bool) -> Option<(u32, u32)> {
        let point = point?;
        let fit = self.fit(bounds);
        if fit.width <= 0.0 || fit.height <= 0.0 || (!clamp && !fit.contains(point)) {
            return None;
        }
        let x = ((point.x - fit.x) as f64 / fit.width as f64 * self.canvas.width as f64)
            .floor()
            .clamp(0.0, (self.canvas.width - 1) as f64) as u32;
        let y = ((point.y - fit.y) as f64 / fit.height as f64 * self.canvas.height as f64)
            .floor()
            .clamp(0.0, (self.canvas.height - 1) as f64) as u32;
        Some((x, y))
    }
    fn marker(&self) -> Option<(f64, f64)> {
        if let Some(Selection::Pixel { frame, x, y }) = self.selected {
            let f = &self.frames[*frame];
            let b = self.canvas.bounds(f);
            Some((
                b.x + (*x as f64 + 0.5) / f.width as f64 * b.width,
                b.y + (*y as f64 + 0.5) / f.height as f64 * b.height,
            ))
        } else if self.mode == Kind::Area {
            self.cursor.map(|p| (p.0 as f64 + 0.5, p.1 as f64 + 0.5))
        } else {
            None
        }
    }
    fn draw_loupe(&self, renderer: &mut iced::Renderer, bounds: Rectangle, clip: Rectangle) {
        let Some(Selection::Pixel { frame, x, y }) = self.selected else {
            return;
        };
        const CELL: f32 = 13.0;
        const SIDE: f32 = 7.0 * CELL;
        let fit = self.fit(bounds);
        let marker_on_right = self.marker().is_some_and(|(x, _)| {
            fit.x + (x / self.canvas.width as f64) as f32 * fit.width > bounds.center_x()
        });
        let panel = Rectangle {
            x: if marker_on_right {
                bounds.x + 10.0
            } else {
                bounds.x + bounds.width - SIDE - 18.0
            },
            y: bounds.y + 10.0,
            width: SIDE + 8.0,
            height: SIDE + 8.0,
        };
        renderer.with_layer(clip, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: panel,
                    border: iced::Border::default()
                        .rounded(4)
                        .width(1)
                        .color(Color::WHITE),
                    ..Default::default()
                },
                Color::from_rgb8(20, 24, 29),
            );
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let (Some(sx), Some(sy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                    else {
                        continue;
                    };
                    let Ok([r, g, b, _]) = self.source.pixel(self.file, *frame, sx, sy) else {
                        continue;
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: panel.x + 4.0 + (dx + 3) as f32 * CELL,
                                y: panel.y + 4.0 + (dy + 3) as f32 * CELL,
                                width: CELL,
                                height: CELL,
                            },
                            border: iced::Border::default()
                                .width(1)
                                .color(Color::BLACK.scale_alpha(0.35)),
                            ..Default::default()
                        },
                        Color::from_rgb8(r, g, b),
                    );
                }
            }
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: panel.x + 4.0 + 3.0 * CELL,
                        y: panel.y + 4.0 + 3.0 * CELL,
                        width: CELL,
                        height: CELL,
                    },
                    border: iced::Border::default().width(2).color(Color::WHITE),
                    ..Default::default()
                },
                Color::TRANSPARENT,
            );
        });
    }
}
impl Widget<Message, Theme, iced::Renderer> for Scene<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn layout(
        &mut self,
        _: &mut Tree,
        _: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fill, Length::Fill, Size::new(400.0, 240.0)))
    }
    fn draw(
        &self,
        _: &Tree,
        renderer: &mut iced::Renderer,
        _: &Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let fit = self.fit(bounds);
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..Default::default()
                },
                Color::from_rgb8(15, 18, 20),
            );
            for (frame, handle) in self.frames.iter().zip(self.images) {
                let b = self.canvas.bounds(frame);
                let target = Rectangle {
                    x: fit.x + (b.x / self.canvas.width as f64 * fit.width as f64) as f32,
                    y: fit.y + (b.y / self.canvas.height as f64 * fit.height as f64) as f32,
                    width: (b.width / self.canvas.width as f64 * fit.width as f64) as f32,
                    height: (b.height / self.canvas.height as f64 * fit.height as f64) as f32,
                };
                renderer.draw_image(
                    image::Image::new(handle.clone()).filter_method(image::FilterMethod::Nearest),
                    target,
                    clip,
                );
            }
        });
        // The renderer batches images and quads separately within a layer.
        // Put selection chrome on its own layer so it stays above the preview.
        renderer.with_layer(clip, |renderer| {
            if let Some(Selection::Image(Some(rect))) = self.selected {
                let selected = Rectangle {
                    x: fit.x + rect.x as f32 / self.canvas.width as f32 * fit.width,
                    y: fit.y + rect.y as f32 / self.canvas.height as f32 * fit.height,
                    width: rect.width as f32 / self.canvas.width as f32 * fit.width,
                    height: rect.height as f32 / self.canvas.height as f32 * fit.height,
                };
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: selected,
                        border: iced::Border {
                            color: self.accent,
                            width: 2.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Color {
                        a: 0.16,
                        ..self.accent
                    },
                );
            }
            if let Some((x, y)) = self.marker() {
                let x = fit.x + (x / self.canvas.width as f64 * fit.width as f64) as f32;
                let y = fit.y + (y / self.canvas.height as f64 * fit.height as f64) as f32;
                if self.mode == Kind::Color {
                    for (side, width, color) in
                        [(30.0, 4.0, Color::BLACK), (28.0, 2.0, Color::WHITE)]
                    {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle {
                                    x: x - side / 2.0,
                                    y: y - side / 2.0,
                                    width: side,
                                    height: side,
                                },
                                border: iced::Border::default()
                                    .rounded(side / 2.0)
                                    .width(width)
                                    .color(color),
                                ..Default::default()
                            },
                            Color::TRANSPARENT,
                        );
                    }
                }
                for (width, height) in [(18.0, 3.0), (3.0, 18.0), (16.0, 1.0), (1.0, 16.0)] {
                    let color = if width == 3.0 || height == 3.0 {
                        Color::BLACK
                    } else {
                        Color::WHITE
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: x - width / 2.0,
                                y: y - height / 2.0,
                                width,
                                height,
                            },
                            ..Default::default()
                        },
                        color,
                    );
                }
            }
        });
        if self.mode == Kind::Color {
            self.draw_loupe(renderer, bounds, clip);
        }
    }
    fn update(
        &mut self,
        _: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &iced::Renderer,
        _: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _: &Rectangle,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => shell.publish(Message::Hover(
                self.project(layout.bounds(), cursor.position(), self.dragging),
            )),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(point) = self.project(layout.bounds(), cursor.position(), false) {
                    shell.publish(Message::Press(point));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if self.dragging => {
                shell.publish(Message::Release(self.project(
                    layout.bounds(),
                    cursor.position(),
                    true,
                )));
                shell.capture_event();
            }
            _ => {}
        }
    }
    fn mouse_interaction(
        &self,
        _: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &Rectangle,
        _: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::None
        }
    }
}
