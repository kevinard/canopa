//! Bounded local-image decoding outside the GUI and async executor threads.
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use canopa_panel_protocol::{PanelContent, RichBodyImageSource, RichBodySegment};
use iced::widget::image::Handle;

const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
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
        paths
            .into_iter()
            .filter_map(|path| decode(&path).map(|image| (path, image)))
            .collect()
    })
    .await
    .unwrap_or_default()
}

fn decode(path: &str) -> Option<Handle> {
    if !Path::new(path).is_absolute() {
        return None;
    }
    // Allow ordinary symlinked assets, but do not follow a replaced final symlink or block on FIFOs.
    let path = std::fs::canonicalize(path).ok()?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW).bits() as i32)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    let image = if image.width() > 512 || image.height() > 512 {
        image.thumbnail(512, 512)
    } else {
        image
    }
    .into_rgba8();
    Some(Handle::from_rgba(
        image.width(),
        image.height(),
        image.into_raw(),
    ))
}
