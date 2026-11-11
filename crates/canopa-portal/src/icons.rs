use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
    icons::{IconFormats, IconLookup, LookupOptions},
};
use iced::widget::image::Handle;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// One bounded, off-thread batch. Missing/invalid assets keep textual app labels.
pub(crate) fn prepare(names: &[String], foreground: canopa_theme::Rgb) -> HashMap<String, Handle> {
    let roots = canopa_assets::icons::search_roots();
    // The desktop icon theme (GTK settings), its parents, then hicolor.
    let theme = canopa_assets::icons::IconResolver::system().desktop_theme();
    let mut lookup = IconLookup::new(
        &roots,
        LookupOptions {
            theme: &theme,
            scale: 1,
            formats: IconFormats::PngSvgXpm,
            allow_unindexed_hicolor: true,
        },
        &|| false,
    );
    let mut rasters = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 256 * 1024,
            max_dimension: 1024,
            max_pixels: 1024 * 1024,
            decoder_allocation: DecoderAllocation::IncludingOutput(8 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: false,
            thumbnail: Some(64),
        },
        RasterBudget {
            file_attempts: 256,
            encoded_bytes: 8 * 1024 * 1024,
            pixel_bytes: 32 * 1024 * 1024,
        },
        &|| false,
    );
    let mut handles = HashMap::new();
    let mut svg_budget = 8 * 1024 * 1024;
    for name in names.iter().take(256) {
        if handles.contains_key(name) {
            continue;
        }
        let path = if Path::new(name).is_absolute() {
            Some(PathBuf::from(name))
        } else {
            lookup.resolve(std::slice::from_ref(name), 32)
        };
        let Some(path) = path else {
            continue;
        };
        let handle = if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            if svg_budget == 0 {
                continue;
            }
            let read = canopa_file::read_bounded(&path, (256 * 1024).min(svg_budget));
            svg_budget = svg_budget.saturating_sub(
                read.as_ref()
                    .map_or_else(|error| error.bytes_read, Vec::len),
            );
            read.ok().and_then(|bytes| svg(&bytes, foreground))
        } else {
            rasters
                .load(&path)
                .map(|image| Handle::from_rgba(image.width, image.height, image.pixels))
        };
        if let Some(handle) = handle {
            handles.insert(name.clone(), handle);
        }
    }
    handles
}

fn svg(bytes: &[u8], canopa_theme::Rgb(r, g, b): canopa_theme::Rgb) -> Option<Handle> {
    let image = canopa_assets::svg::rasterize(
        bytes,
        canopa_assets::svg::Policy {
            max_encoded_bytes: 256 * 1024,
            max_dimension: 1024,
            max_pixels: 1024 * 1024,
            output_side: 64,
            foreground: [r, g, b],
        },
    )?;
    Some(Handle::from_rgba(image.width, image.height, image.pixels))
}
