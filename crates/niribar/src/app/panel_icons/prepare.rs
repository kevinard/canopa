//! File reads, raster decoding and SVG parsing/rasterization run on the worker.
use std::path::Path;
use std::sync::{Arc, OnceLock};

use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use iced::widget::image::Handle;
use resvg::{tiny_skia, usvg};

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
// Menu icons occupy 18 logical pixels. Prepare at 4x so fractional/HiDPI outputs
// need no SVG parsing or raster decoding during layout or drawing.
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
    Some(Handle::from_rgba(image.width, image.height, image.pixels))
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
    Some(Handle::from_rgba(width, height, pixels))
}
