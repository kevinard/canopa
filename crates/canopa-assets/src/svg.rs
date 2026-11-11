//! Rasterize a bounded SVG without external images, fonts or filesystem access.
use crate::Raster;
use resvg::{tiny_skia, usvg};

#[derive(Clone)]
pub struct Policy {
    pub max_encoded_bytes: usize,
    pub max_dimension: u32,
    pub max_pixels: u64,
    pub output_side: u32,
    pub foreground: [u8; 3],
}

/// Call on an admitted blocking worker. Resource limits bound input and output;
/// XML parsing and vector rendering are synchronous and cannot be preempted.
pub fn rasterize(bytes: &[u8], policy: Policy) -> Option<Raster> {
    if bytes.len() > policy.max_encoded_bytes || policy.output_side == 0 {
        return None;
    }
    let [r, g, b] = policy.foreground;
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        style_sheet: Some(format!("svg {{ color: #{r:02x}{g:02x}{b:02x}; }}")),
        ..Default::default()
    };
    let source = std::str::from_utf8(bytes).ok()?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let tree = usvg::Tree::from_str(source, &options).ok()?;
    let size = tree.size();
    if size.width() > policy.max_dimension as f32
        || size.height() > policy.max_dimension as f32
        || f64::from(size.width()) * f64::from(size.height()) > policy.max_pixels as f64
    {
        return None;
    }
    let scale = policy.output_side as f32 / size.width().max(size.height());
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    if u64::from(width) * u64::from(height) > policy.max_pixels {
        return None;
    }
    let mut pixels = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    Some(Raster {
        width,
        height,
        pixels: pixels
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let pixel = pixel.demultiply();
                [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
            })
            .collect(),
    })
}

/// SVG by content: optional BOM and whitespace, then an `<svg` root, or an XML
/// declaration, comment or doctype followed by `<svg` within the first 4 KiB.
/// Compressed SVG (svgz) is not SVG here.
pub fn is_svg(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let head = &bytes[start..bytes.len().min(start + 4096)];
    if head.starts_with(b"<svg") {
        return true;
    }
    (head.starts_with(b"<?xml") || head.starts_with(b"<!--") || head.starts_with(b"<!DOCTYPE"))
        && head.windows(4).any(|window| window == b"<svg")
}
