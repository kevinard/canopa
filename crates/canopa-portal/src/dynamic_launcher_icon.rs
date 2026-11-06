//! A small, owned preview. Original icon bytes stay in the backend.
use crate::dynamic_launcher::{ICON_BYTES, LauncherDraft, LauncherIcon};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Semaphore, watch};

const SIDE: u32 = 128;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogInput {
    pub draft: LauncherDraft,
    pub preview: Preview,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Preview {
    pub(crate) fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= SIDE
            && self.height <= SIDE
            && self.pixels.len() == self.width as usize * self.height as usize * 4
    }
}

pub(crate) async fn prepare(
    icon: LauncherIcon,
    cancelled: &watch::Receiver<bool>,
) -> anyhow::Result<Preview> {
    // Hold the permit in the worker, even when its request is cancelled. A codec
    // already in progress cannot be preempted or spawn an unbounded replacement.
    static WORKERS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = WORKERS
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .try_acquire_owned()?;
    let cancelled = cancelled.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let check = || *cancelled.borrow() || cancelled.has_changed().is_err();
        anyhow::ensure!(!check(), "Launcher request cancelled");
        let preview = decode(icon.bytes(), &check);
        anyhow::ensure!(!check(), "Launcher request cancelled");
        preview.ok_or_else(|| anyhow::anyhow!("Invalid launcher icon"))
    })
    .await?
}

fn decode(bytes: &[u8], cancelled: &dyn Fn() -> bool) -> Option<Preview> {
    if bytes.len() > ICON_BYTES || cancelled() {
        return None;
    }
    let raster = match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png | image::ImageFormat::Jpeg) => RasterBatch::new(
            RasterPolicy {
                max_encoded_bytes: ICON_BYTES,
                max_dimension: 512,
                max_pixels: 512 * 512,
                decoder_allocation: DecoderAllocation::IncludingOutput(8 * 1024 * 1024),
                formats: Formats::StaticPngJpeg,
                file_access: FileAccess::Follow,
                apply_orientation: true,
                thumbnail: Some(SIDE),
            },
            RasterBudget {
                file_attempts: 1,
                encoded_bytes: ICON_BYTES,
                pixel_bytes: 512 * 512 * 4,
            },
            cancelled,
        )
        .decode(bytes.to_vec())?,
        Ok(_) => return None,
        Err(_) => canopa_assets::svg::rasterize(
            bytes,
            canopa_assets::svg::Policy {
                max_encoded_bytes: ICON_BYTES,
                max_dimension: 4096,
                max_pixels: 4096 * 4096,
                output_side: SIDE,
                // Show a neutral icon on a white tile in either appearance.
                foreground: [0, 0, 0],
            },
        )?,
    };
    (!cancelled()).then_some(Preview {
        width: raster.width,
        height: raster.height,
        pixels: raster.pixels,
    })
}

#[cfg(test)]
mod tests;
