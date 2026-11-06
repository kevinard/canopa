//! Bounded source ownership. Nothing in the dialog can reread a caller's URI.
use std::{
    fs::File,
    io::{self, Seek, Write},
    sync::Arc,
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

const INPUT_BYTES: usize = 32 * 1024 * 1024;
const PNG_BYTES: usize = 128 * 1024 * 1024;
const PIXELS: usize = 64 * 1024 * 1024;
const SIDE: u32 = 384;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    width: u32,
    height: u32,
    rgba: String,
}

impl Preview {
    pub(crate) fn pixels(self) -> anyhow::Result<(u32, u32, Vec<u8>)> {
        anyhow::ensure!(
            self.width > 0
                && self.height > 0
                && self.width <= SIDE
                && self.height <= SIDE
                && self.rgba.len() <= (SIDE * SIDE * 4 / 3 * 4) as usize,
            "Invalid wallpaper preview"
        );
        let pixels = STANDARD.decode(self.rgba)?;
        anyhow::ensure!(
            pixels.len() == self.width as usize * self.height as usize * 4,
            "Invalid wallpaper preview"
        );
        Ok((self.width, self.height, pixels))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogInput {
    pub app_id: String,
    pub parent_window: String,
    pub destination: crate::wallpaper::Destination,
    pub width: u32,
    pub height: u32,
    pub preview: Preview,
}

pub(crate) struct Frozen {
    file: File,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) preview: Preview,
    _permit: OwnedSemaphorePermit,
}
impl Frozen {
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
}

pub(crate) struct Images {
    slots: Arc<Semaphore>,
    client: Option<reqwest::Client>,
}

fn remote(url: &url::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
        && url.as_str().len() <= 16 * 1024
}

impl Images {
    pub(crate) fn new() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(2)),
            client: client_builder().build().ok(),
        }
    }

    pub(crate) async fn prepare(
        &self,
        uri: &str,
        cancelled: &watch::Receiver<bool>,
    ) -> anyhow::Result<Frozen> {
        let permit = self.slots.clone().try_acquire_owned()?;
        let url = url::Url::parse(uri).map_err(|_| anyhow::anyhow!("Invalid wallpaper URI"))?;
        let check = || *cancelled.borrow() || cancelled.has_changed().is_err();
        anyhow::ensure!(!check(), "Wallpaper cancelled");
        if url.scheme() == "file" {
            anyhow::ensure!(
                url.query().is_none()
                    && url.fragment().is_none()
                    && url.username().is_empty()
                    && url.password().is_none(),
                "Invalid wallpaper file URI"
            );
            let path = url
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Invalid wallpaper file URI"))?;
            let cancelled = cancelled.clone();
            return tokio::task::spawn_blocking(move || {
                let check = || *cancelled.borrow() || cancelled.has_changed().is_err();
                anyhow::ensure!(!check(), "Wallpaper cancelled");
                let bytes = canopa_file::read_bounded(&path, INPUT_BYTES)
                    .map_err(|_| anyhow::anyhow!("Wallpaper file unavailable or too large"))?;
                freeze(bytes, permit, &check)
            })
            .await?;
        }
        anyhow::ensure!(remote(&url), "Unsupported wallpaper URI");
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Wallpaper download unavailable"))?;
        let download = async {
            let mut response = client
                .get(url)
                .send()
                .await
                .map_err(|_| anyhow::anyhow!("Wallpaper download failed"))?;
            anyhow::ensure!(
                response.status().is_success(),
                "Wallpaper download rejected"
            );
            anyhow::ensure!(
                response
                    .content_length()
                    .is_none_or(|n| n <= INPUT_BYTES as u64),
                "Wallpaper download too large"
            );
            anyhow::ensure!(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_ENCODING)
                    .is_none_or(|v| v.to_str().is_ok_and(|s| s.eq_ignore_ascii_case("identity"))),
                "Unsupported wallpaper content encoding"
            );
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow::anyhow!("Wallpaper download failed"))?
            {
                anyhow::ensure!(
                    bytes.len().saturating_add(chunk.len()) <= INPUT_BYTES,
                    "Wallpaper download too large"
                );
                bytes.extend_from_slice(&chunk);
            }
            Ok::<_, anyhow::Error>(bytes)
        };
        let mut cancellation = cancelled.clone();
        let bytes = tokio::select! {
            biased;
            _ = async { if !*cancellation.borrow() { let _ = cancellation.changed().await; } } => anyhow::bail!("Wallpaper cancelled"),
            bytes = download => bytes?,
        };
        let cancelled = cancelled.clone();
        tokio::task::spawn_blocking(move || {
            freeze(bytes, permit, &|| {
                *cancelled.borrow() || cancelled.has_changed().is_err()
            })
        })
        .await?
    }
}

fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(BoundedDns {
            slots: Arc::new(Semaphore::new(2)),
        }))
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(0)
        .referer(false)
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 || !remote(attempt.url()) {
                attempt.error("Unsupported wallpaper redirect")
            } else {
                attempt.follow()
            }
        }))
}

// System DNS can outlive a cancelled HTTP future. Retain a separate permit in
// the actual blocking lookup, so cancellation cannot accumulate resolver jobs.
struct BoundedDns {
    slots: Arc<Semaphore>,
}

impl reqwest::dns::Resolve for BoundedDns {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        use std::net::ToSocketAddrs;
        let permit = self.slots.clone().try_acquire_owned();
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let permit = permit.map_err(|_| io::Error::other("Wallpaper resolver busy"))?;
            if host.len() > 255 {
                return Err(io::Error::other("Invalid wallpaper host").into());
            }
            let addresses = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let addresses: Vec<_> = (host.as_str(), 0).to_socket_addrs()?.take(32).collect();
                Ok::<reqwest::dns::Addrs, io::Error>(Box::new(addresses.into_iter()))
            })
            .await??;
            Ok(addresses)
        })
    }
}

fn freeze(
    bytes: Vec<u8>,
    permit: OwnedSemaphorePermit,
    cancelled: &dyn Fn() -> bool,
) -> anyhow::Result<Frozen> {
    let raster = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: INPUT_BYTES,
            max_dimension: 16_384,
            max_pixels: PIXELS as u64,
            decoder_allocation: DecoderAllocation::IncludingOutput(512 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: None,
        },
        RasterBudget {
            file_attempts: 1,
            encoded_bytes: INPUT_BYTES,
            pixel_bytes: PIXELS * 4,
        },
        cancelled,
    )
    .decode(bytes)
    .ok_or_else(|| anyhow::anyhow!("Unsupported or oversized wallpaper image"))?;
    anyhow::ensure!(!cancelled(), "Wallpaper cancelled");
    let (width, height) = (raster.width, raster.height);
    let rgba = image::RgbaImage::from_raw(width, height, raster.pixels).expect("prepared RGBA");
    let thumbnail = if width <= SIDE && height <= SIDE {
        rgba.clone()
    } else {
        let (preview_width, preview_height) = if width >= height {
            (
                SIDE,
                ((u64::from(height) * u64::from(SIDE) / u64::from(width)) as u32).max(1),
            )
        } else {
            (
                ((u64::from(width) * u64::from(SIDE) / u64::from(height)) as u32).max(1),
                SIDE,
            )
        };
        image::imageops::thumbnail(&rgba, preview_width, preview_height)
    };
    let preview = Preview {
        width: thumbnail.width(),
        height: thumbnail.height(),
        rgba: STANDARD.encode(thumbnail.as_raw()),
    };
    let mut file = File::from(rustix::fs::memfd_create(
        c"canopa-wallpaper",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?);
    image::codecs::png::PngEncoder::new(LimitedWriter {
        file: &mut file,
        remaining: PNG_BYTES,
    })
    .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)?;
    anyhow::ensure!(!cancelled(), "Wallpaper cancelled");
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::SEAL,
    )?;
    file.rewind()?;
    Ok(Frozen {
        file,
        width,
        height,
        preview,
        _permit: permit,
    })
}

struct LimitedWriter<'a> {
    file: &'a mut File,
    remaining: usize,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other("Wallpaper PNG too large"));
        }
        let count = self.file.write(bytes)?;
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests;
