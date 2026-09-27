//! Bounded local-image decoding outside the GUI and async executor threads.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use canopa_panel_protocol::{PanelContent, RichBodyImageSource, RichBodySegment};
use iced::widget::image::Handle;

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMAGES: usize = 16;

pub(super) async fn prepare(content: &PanelContent) -> HashMap<String, Handle> {
    let mut paths = Vec::new();
    for section in &content.sections {
        for item in &section.items {
            if let Some(body) = &item.rich_body {
                for segment in &body.segments {
                    if let RichBodySegment::Image { image } = segment {
                        let RichBodyImageSource::LocalPath { path } = &image.source;
                        if paths.len() < MAX_IMAGES && !paths.contains(path) {
                            paths.push(path.clone());
                        }
                    }
                }
            }
        }
    }
    if paths.is_empty() {
        return HashMap::new();
    }
    static WORKERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let workers = WORKERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone();
    let Ok(Ok(permit)) = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        workers.acquire_owned(),
    )
    .await
    else {
        return HashMap::new();
    };
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut images = RasterBatch::new(
            RasterPolicy {
                max_encoded_bytes: MAX_FILE_BYTES,
                max_dimension: 2048,
                max_pixels: 2048 * 2048,
                decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
                formats: Formats::EnabledRasterCodecs,
                file_access: FileAccess::ResolveFirst,
                apply_orientation: false,
                thumbnail: Some(512),
            },
            RasterBudget {
                file_attempts: MAX_IMAGES,
                encoded_bytes: MAX_IMAGES * MAX_FILE_BYTES,
                pixel_bytes: MAX_IMAGES * 2048 * 2048 * 4,
            },
            &|| false,
        );
        paths
            .into_iter()
            .filter_map(|path| {
                let image = images.load(Path::new(&path))?;
                Some((
                    path,
                    Handle::from_rgba(image.width, image.height, image.pixels),
                ))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
