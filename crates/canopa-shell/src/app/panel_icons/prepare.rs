//! File reads, raster decoding and SVG parsing/rasterization run on the worker.
use std::path::Path;
use std::sync::{Arc, OnceLock};

use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use iced::widget::image::Handle;
use resvg::{tiny_skia, usvg};

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
// Keep enough source pixels for menu and launcher icons on scaled outputs.
// All decoding and transparent padding removal stay on the worker.
const PIXEL_SIZE: u32 = 72;

pub(super) fn from_path(path: &Path) -> Option<Handle> {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        return svg(path);
    }
    let image = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: MAX_FILE_BYTES,
            max_dimension: 2048,
            max_pixels: 2048 * 2048,
            decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: false,
            thumbnail: Some(PIXEL_SIZE),
        },
        RasterBudget {
            file_attempts: 1,
            encoded_bytes: MAX_FILE_BYTES,
            pixel_bytes: 2048 * 2048 * 4,
        },
        &|| false,
    )
    .load(path)?;
    Some(icon_handle(image.width, image.height, image.pixels))
}

fn svg(path: &Path) -> Option<Handle> {
    let bytes = canopa_file::read_bounded(path, MAX_FILE_BYTES).ok()?;
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    let options = usvg::Options {
        fontdb: FONTS
            .get_or_init(|| {
                let mut fonts = usvg::fontdb::Database::new();
                fonts.load_system_fonts();
                Arc::new(fonts)
            })
            .clone(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(&bytes, &options).ok()?;
    let size = tree.size();
    let scale = PIXEL_SIZE as f32 / size.width().max(size.height());
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // Iced expects straight RGBA; tiny-skia produces premultiplied channels.
    let pixels: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let pixel = pixel.demultiply();
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
        })
        .collect();
    Some(icon_handle(width, height, pixels))
}

fn icon_handle(width: u32, height: u32, pixels: Vec<u8>) -> Handle {
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        let x = index as u32 % width;
        let y = index as u32 / width;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    if right <= left || bottom <= top || (left, top, right, bottom) == (0, 0, width, height) {
        return Handle::from_rgba(width, height, pixels);
    }

    let mut cropped = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
    for y in top..bottom {
        let start = ((y * width + left) * 4) as usize;
        let end = ((y * width + right) * 4) as usize;
        cropped.extend_from_slice(&pixels[start..end]);
    }
    Handle::from_rgba(right - left, bottom - top, cropped)
}
