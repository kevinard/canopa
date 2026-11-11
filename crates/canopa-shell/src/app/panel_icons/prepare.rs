//! File reads, raster decoding and SVG parsing/rasterization run on the worker,
//! through the shared content-sniffing loader in `canopa-assets`.
use std::path::Path;

use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy, svg,
};
use iced::widget::image::Handle;

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
// Keep enough source pixels for menu and launcher icons on scaled outputs.
// All decoding and transparent padding removal stay on the worker.
const PIXEL_SIZE: u32 = 72;

/// SVG or raster by content, so absolute `Icon=` paths need no known extension.
pub(super) fn from_path(path: &Path) -> Option<Handle> {
    let raster = RasterBatch::new(
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
    .load_image_or_svg(
        path,
        &svg::Policy {
            max_encoded_bytes: MAX_FILE_BYTES,
            max_dimension: 2048,
            max_pixels: 2048 * 2048,
            output_side: PIXEL_SIZE,
            // usvg's default currentColor, as before the shared loader.
            foreground: [0, 0, 0],
        },
    )?;
    Some(canopa_ui::icons::trimmed_handle(raster))
}
