//! Rotated glyphs use the normal font rasterizer at the output's physical scale.
//! The GPU only copies those pixels; it never tessellates or resamples letters.

use std::sync::{Arc, Mutex};

use iced::advanced::{
    graphics::text as graphics_text,
    text::{self, Paragraph},
};
use iced::widget::shader;
use iced::{Color, Font, Pixels, Point, Rectangle, Size, wgpu};

use super::FormattedSegmentVisual;
use crate::config::{ResolvedContentRotation, ResolvedStyle, Weight};

pub(super) fn font(style: ResolvedStyle) -> Font {
    Font {
        weight: match style.weight.unwrap_or(Weight::Normal) {
            Weight::Light => iced::font::Weight::Light,
            Weight::Normal => iced::font::Weight::Normal,
            Weight::Bold => iced::font::Weight::Bold,
        },
        ..Font::DEFAULT
    }
}

fn paragraph(content: &str, font: Font, size: f32) -> graphics_text::Paragraph {
    graphics_text::Paragraph::with_text(text::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: text::LineHeight::Relative(1.2),
        font,
        align_x: text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
}

pub(super) fn measure(content: &str, style: ResolvedStyle, size: f32) -> Size {
    paragraph(content, font(style), size).min_bounds()
}

#[derive(Debug, Clone, PartialEq)]
struct Run {
    content: String,
    origin: Point,
    font: Font,
    size: f32,
    color: Color,
}

pub(super) struct Program {
    runs: Arc<[Run]>,
    content_size: Size,
    rotation: ResolvedContentRotation,
}

impl Program {
    pub(super) fn new(
        segments: &[FormattedSegmentVisual],
        fallback_size: u32,
        fallback_color: Color,
        content_size: Size,
        rotation: ResolvedContentRotation,
    ) -> Self {
        let mut x = 0.0;
        let mut runs = Vec::new();
        for segment in segments {
            let (width, height) = super::visual_segment_canvas_metrics(segment, fallback_size);
            match segment {
                FormattedSegmentVisual::Image { path, .. } if path.exists() => {}
                FormattedSegmentVisual::Text { text, style }
                | FormattedSegmentVisual::Image {
                    fallback_text: text,
                    style,
                    ..
                } => {
                    runs.push(Run {
                        content: text.clone(),
                        origin: Point::new(x, (content_size.height - height) / 2.0),
                        font: font(*style),
                        size: super::visual_segment_size(*style, fallback_size),
                        color: style.color.unwrap_or(fallback_color),
                    });
                }
            }
            x += width + super::CANVAS_SEGMENT_SPACING;
        }
        Self {
            runs: runs.into(),
            content_size,
            rotation,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct State(Arc<Mutex<Option<Upload>>>);

impl<Message> shader::Program<Message> for Program {
    type State = State;
    type Primitive = Primitive;

    fn draw(&self, state: &State, _cursor: iced::mouse::Cursor, _bounds: Rectangle) -> Primitive {
        Primitive {
            runs: self.runs.clone(),
            content_size: self.content_size,
            rotation: self.rotation,
            upload: state.0.clone(),
        }
    }
}

#[derive(Debug)]
pub(super) struct Primitive {
    runs: Arc<[Run]>,
    content_size: Size,
    rotation: ResolvedContentRotation,
    upload: Arc<Mutex<Option<Upload>>>,
}

#[derive(Debug, PartialEq)]
struct RasterKey {
    runs: Arc<[Run]>,
    content_size: Size,
    extent: Size<u32>,
    scale: f32,
    rotation: ResolvedContentRotation,
}

#[derive(Debug)]
struct Upload {
    key: RasterKey,
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    viewport: Size<u32>,
}

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(
        &self,
        pipeline: &mut Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let scale = viewport.scale_factor();
        let maximum = device.limits().max_texture_dimension_2d;
        let extent = Size::new(
            ((bounds.width * scale).ceil() as u32).clamp(1, maximum),
            ((bounds.height * scale).ceil() as u32).clamp(1, maximum),
        );
        let key = RasterKey {
            runs: self.runs.clone(),
            content_size: self.content_size,
            extent,
            scale,
            rotation: self.rotation,
        };
        let mut upload = self.upload.lock().expect("rotated text upload");
        if upload.as_ref().is_none_or(|upload| upload.key != key) {
            let pixels = rasterize(&key);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("niribar rotated glyphs"),
                size: wgpu::Extent3d {
                    width: extent.width,
                    height: extent.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: pipeline.texture_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                texture.as_image_copy(),
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(extent.width * 4),
                    rows_per_image: None,
                },
                texture.size(),
            );
            let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("niribar rotated glyph placement"),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("niribar rotated glyphs"),
                layout: &pipeline.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&Default::default()),
                        ),
                    },
                ],
            });
            *upload = Some(Upload {
                key,
                bind_group,
                uniforms,
                viewport: viewport.physical_size(),
            });
        }
        let upload = upload.as_mut().expect("prepared rotated text");
        upload.viewport = viewport.physical_size();
        let values = [
            (bounds.x * scale).round(),
            (bounds.y * scale).round(),
            extent.width as f32,
            extent.height as f32,
            upload.viewport.width as f32,
            upload.viewport.height as f32,
            0.0,
            0.0,
        ];
        let bytes = values
            .into_iter()
            .flat_map(f32::to_ne_bytes)
            .collect::<Vec<_>>();
        queue.write_buffer(&upload.uniforms, 0, &bytes);
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let upload = self.upload.lock().expect("rotated text upload");
        if let Some(upload) = upload.as_ref() {
            // Pixel-aligned placement in the full viewport avoids a second
            // filtering step after the font rasterizer. Iced owns the scissor.
            pass.set_viewport(
                0.0,
                0.0,
                upload.viewport.width as f32,
                upload.viewport.height as f32,
                0.0,
                1.0,
            );
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &upload.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        true
    }
}

pub(super) struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    texture_format: wgpu::TextureFormat,
}

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("niribar rotated glyphs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("niribar rotated glyphs"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("niribar rotated glyphs"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rotated_text.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("niribar rotated glyphs"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            texture_format: if format.is_srgb() {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
        }
    }
}

fn rasterize(key: &RasterKey) -> Vec<u8> {
    use graphics_text::cosmic_text::{Color as GlyphColor, SwashCache};
    use image::Pixel;

    let mut pixels = vec![0; key.extent.width as usize * key.extent.height as usize * 4];
    let source_width = (key.content_size.width * key.scale).ceil() as i32;
    let source_height = (key.content_size.height * key.scale).ceil() as i32;
    let mut cache = SwashCache::new();
    for segment in key.runs.iter() {
        let paragraph = paragraph(&segment.content, segment.font, segment.size);
        let mut fonts = graphics_text::font_system().write().expect("font system");
        let [r, g, b, alpha] = segment.color.into_rgba8();
        for line in paragraph.buffer().layout_runs() {
            for glyph in line.glyphs {
                let physical = glyph.physical(
                    (segment.origin.x * key.scale, segment.origin.y * key.scale),
                    key.scale,
                );
                cache.with_pixels(
                    fonts.raw(),
                    physical.cache_key,
                    GlyphColor::rgb(r, g, b),
                    |x, y, color| {
                        let x = x + physical.x;
                        let y = y + physical.y + (line.line_y * key.scale).round() as i32;
                        let (x, y) = match key.rotation {
                            ResolvedContentRotation::Clockwise => (source_height - 1 - y, x),
                            ResolvedContentRotation::CounterClockwise => (y, source_width - 1 - x),
                            ResolvedContentRotation::None => (x, y),
                        };
                        let (width, height) = if key.rotation == ResolvedContentRotation::None {
                            (source_width, source_height)
                        } else {
                            (source_height, source_width)
                        };
                        let x = x + (key.extent.width as i32 - width).div_euclid(2);
                        let y = y + (key.extent.height as i32 - height).div_euclid(2);
                        if x >= 0
                            && y >= 0
                            && x < key.extent.width as i32
                            && y < key.extent.height as i32
                        {
                            let offset = (y as usize * key.extent.width as usize + x as usize) * 4;
                            image::Rgba::from_slice_mut(&mut pixels[offset..offset + 4]).blend(
                                &image::Rgba([
                                    color.r(),
                                    color.g(),
                                    color.b(),
                                    (u16::from(color.a()) * u16::from(alpha) / 255) as u8,
                                ]),
                            );
                        }
                    },
                );
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turns_preserve_native_glyph_pixels_at_each_output_scale() {
        let content = "Focused window: Àgjpq ffi\nSecond line";
        let size = measure(content, ResolvedStyle::default(), 16.0);
        let size = Size::new(size.width.ceil(), size.height.ceil());
        let runs: Arc<[Run]> = vec![Run {
            content: content.into(),
            origin: Point::ORIGIN,
            font: Font::DEFAULT,
            size: 16.0,
            color: Color::WHITE,
        }]
        .into();
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let extent = Size::new(
                (size.width * scale).ceil() as u32,
                (size.height * scale).ceil() as u32,
            );
            let mut key = RasterKey {
                runs: runs.clone(),
                content_size: size,
                extent,
                scale,
                rotation: ResolvedContentRotation::None,
            };
            let horizontal = rasterize(&key);
            let coverage = horizontal
                .chunks_exact(4)
                .map(|pixel| pixel[3])
                .collect::<std::collections::HashSet<_>>();
            assert!(
                coverage.len() > 32,
                "native grayscale glyph coverage at {scale}"
            );
            key.extent = Size::new(extent.height, extent.width);
            for rotation in [
                ResolvedContentRotation::Clockwise,
                ResolvedContentRotation::CounterClockwise,
            ] {
                key.rotation = rotation;
                let rotated = rasterize(&key);
                for y in 0..extent.height {
                    for x in 0..extent.width {
                        let (rx, ry) = match rotation {
                            ResolvedContentRotation::Clockwise => (extent.height - 1 - y, x),
                            _ => (y, extent.width - 1 - x),
                        };
                        let source = ((y * extent.width + x) * 4) as usize;
                        let target = ((ry * key.extent.width + rx) * 4) as usize;
                        assert_eq!(
                            &horizontal[source..source + 4],
                            &rotated[target..target + 4]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn constrained_titles_crop_glyphs_without_rescaling_or_wrapping() {
        let content = "An intentionally long focused-window title";
        let size = measure(content, ResolvedStyle::default(), 16.0);
        let size = Size::new(size.width.ceil(), size.height.ceil());
        let mut key = RasterKey {
            runs: vec![Run {
                content: content.into(),
                origin: Point::ORIGIN,
                font: Font::DEFAULT,
                size: 16.0,
                color: Color::WHITE,
            }]
            .into(),
            content_size: size,
            extent: Size::new(size.height as u32, size.width as u32),
            scale: 1.0,
            rotation: ResolvedContentRotation::CounterClockwise,
        };
        let full = rasterize(&key);
        let full_height = key.extent.height;
        key.extent.height = 60;
        let cropped = rasterize(&key);
        let first_row = (full_height - key.extent.height).div_ceil(2) as usize;
        let start = first_row * key.extent.width as usize * 4;
        assert_eq!(cropped, full[start..start + cropped.len()]);
    }
}
