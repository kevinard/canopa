//! Admit bounded raster offers and prepare a preview before exposing them to the UI.
use image::{ImageFormat, ImageReader, Limits};
use std::{io::Cursor, sync::Arc};

pub(super) const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;

#[derive(Clone)]
pub struct Image(Arc<Data>);

struct Data {
    bytes: Vec<u8>,
    mime: &'static str,
    width: u32,
    height: u32,
    thumbnail: iced::widget::image::Handle,
    memory: usize,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClipboardImage")
    }
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.bytes == other.0.bytes
    }
}

impl Eq for Image {}

impl Image {
    pub(super) fn decode(bytes: Vec<u8>) -> Option<Self> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        let format = image::guess_format(&bytes).ok()?;
        let mime = match format {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            _ => return None,
        };
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(80 * 1024 * 1024);
        let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
        reader.limits(limits);
        let image = reader.decode().ok()?;
        let width = image.width();
        let height = image.height();
        let thumb = image.thumbnail(224, 160).into_rgba8();
        let (thumb_width, thumb_height) = thumb.dimensions();
        let rgba = thumb.into_raw();
        let memory = bytes.len() + rgba.len();
        Some(Self(Arc::new(Data {
            bytes,
            mime,
            width,
            height,
            memory,
            thumbnail: iced::widget::image::Handle::from_rgba(thumb_width, thumb_height, rgba),
        })))
    }

    pub(super) fn memory(&self) -> usize {
        self.0.memory
    }

    pub fn thumbnail(&self) -> &iced::widget::image::Handle {
        &self.0.thumbnail
    }

    pub fn description(&self) -> String {
        let format = match self.0.mime {
            "image/png" => "PNG",
            "image/jpeg" => "JPEG",
            "image/gif" => "GIF",
            "image/webp" => "WebP",
            _ => unreachable!(),
        };
        fl!(
            "shell-clipboard-image-description",
            format = format,
            width = self.0.width,
            height = self.0.height
        )
    }

    pub async fn copy(&self) -> Result<(), String> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;

        // wl-copy acknowledges the selection before leaving its clipboard owner behind.
        let mut child = tokio::process::Command::new("wl-copy")
            .args(["--type", self.0.mime])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| fl!("shell-clipboard-image-copy-failed"))?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| fl!("shell-clipboard-image-copy-failed"))?;
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            input.write_all(&self.0.bytes).await?;
            drop(input);
            child.wait().await
        })
        .await;
        match result {
            Ok(Ok(status)) if status.success() => Ok(()),
            _ => Err(fl!("shell-clipboard-image-copy-failed")),
        }
    }
}
