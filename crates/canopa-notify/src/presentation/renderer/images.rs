//! Turn local references into stable pixels before widgets see them.
use super::{
    BannerBodySegment, BannerPresentation, BannerVisual, NotificationBodyImageSource,
    NotificationBodySegment, NotificationBodyStyle, push_banner_body_text_segment,
};
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const MAX_ENCODED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VISUAL_PIXELS: u64 = 4 * 1024 * 1024;

static DECODER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

struct ConsumerLifetime(Arc<AtomicBool>);

impl Drop for ConsumerLifetime {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct ImageBudget<'a> {
    remaining_pixels_bytes: usize,
    remaining_file_attempts: usize,
    remaining_encoded_bytes: u64,
    cancelled: &'a dyn Fn() -> bool,
}

pub(super) async fn prepare(
    presentation: BannerPresentation,
    executor: &tokio::runtime::Handle,
    icon_roots: Arc<[std::path::PathBuf]>,
    superseded: impl Fn() -> bool + Send + 'static,
) -> Option<BannerPresentation> {
    if !has_local_files(&presentation) {
        return prepare_pixels(presentation, &superseded, false, &icon_roots);
    }
    let Ok(permit) = DECODER.try_acquire() else {
        return prepare_pixels(presentation, &superseded, false, &icon_roots);
    };
    let lifetime = ConsumerLifetime(Arc::new(AtomicBool::new(false)));
    let cancelled = lifetime.0.clone();
    let prepared = executor
        .spawn_blocking(move || {
            // Cancelling the consumer cannot release admission while this job still exists.
            let _permit = permit;
            prepare_pixels(
                presentation,
                &|| cancelled.load(Ordering::Relaxed) || superseded(),
                true,
                &icon_roots,
            )
        })
        .await;
    drop(lifetime);
    match prepared {
        Ok(prepared) => prepared,
        Err(_) => {
            eprintln!("level=warn component=presentation event=image_preparation_failed");
            None
        }
    }
}

fn has_local_files(presentation: &BannerPresentation) -> bool {
    presentation.banners.iter().any(|banner| {
        banner
            .app_icon
            .iter()
            .chain(&banner.image)
            .chain(
                banner
                    .actions
                    .iter()
                    .filter_map(|action| action.icon.as_ref()),
            )
            .any(|visual| {
                matches!(
                    visual,
                    BannerVisual::LocalPath { .. } | BannerVisual::IconNames { .. }
                )
            })
            || banner
                .body_content
                .segments
                .iter()
                .any(|segment| matches!(segment, NotificationBodySegment::Image { .. }))
    })
}

fn prepare_pixels(
    mut presentation: BannerPresentation,
    cancelled: &dyn Fn() -> bool,
    allow_files: bool,
    icon_roots: &[std::path::PathBuf],
) -> Option<BannerPresentation> {
    let mut budget = ImageBudget {
        remaining_pixels_bytes: 32 * 1024 * 1024,
        remaining_file_attempts: if allow_files { 32 } else { 0 },
        remaining_encoded_bytes: 32 * 1024 * 1024,
        cancelled,
    };
    let mut icons = super::icons::IconLookup::new(
        icon_roots,
        &presentation.icon_settings,
        cancelled,
        allow_files,
    );
    for banner in &mut presentation.banners {
        if cancelled() {
            return None;
        }
        banner.app_icon = banner
            .app_icon
            .take()
            .and_then(|visual| budget.resolve(visual, &mut icons, super::APP_ICON_VISUAL_SIZE));
        let image_size = if banner.app_icon.is_some() {
            super::NOTIFICATION_IMAGE_VISUAL_SIZE
        } else {
            super::APP_ICON_VISUAL_SIZE
        };
        banner.image = banner
            .image
            .take()
            .and_then(|visual| budget.resolve(visual, &mut icons, image_size));
        for action in &mut banner.actions {
            action.icon = action
                .icon
                .take()
                .and_then(|visual| budget.resolve(visual, &mut icons, 16));
        }
        let mut segments = Vec::new();
        for (segment_index, segment) in banner.body_content.segments.iter().enumerate() {
            if cancelled() {
                return None;
            }
            match segment {
                NotificationBodySegment::Link { text, style, .. } => {
                    segments.push(BannerBodySegment::Link {
                        text: text.clone(),
                        style: *style,
                        segment_index,
                    });
                }
                NotificationBodySegment::Text { text, style } => {
                    push_banner_body_text_segment(&mut segments, text, *style);
                }
                NotificationBodySegment::Image { image } => {
                    let NotificationBodyImageSource::LocalPath { path } = &image.source;
                    if let Some(visual) = budget.decode_file(Path::new(path)) {
                        segments.push(BannerBodySegment::image(visual));
                    } else if let Some(alt) = &image.alt {
                        push_banner_body_text_segment(
                            &mut segments,
                            alt,
                            NotificationBodyStyle::default(),
                        );
                    }
                }
            }
        }
        banner.body_segments = segments;
    }
    (!cancelled()).then_some(presentation)
}

impl ImageBudget<'_> {
    fn resolve(
        &mut self,
        visual: BannerVisual,
        icons: &mut super::icons::IconLookup<'_>,
        size: u16,
    ) -> Option<BannerVisual> {
        if (self.cancelled)() {
            return None;
        }
        match &visual {
            BannerVisual::IconNames { names } => {
                let path = icons.resolve(names, size)?;
                self.decode_file(&path)
            }
            BannerVisual::LocalPath { path } => self.decode_file(Path::new(path)),
            BannerVisual::Rgba {
                width,
                height,
                pixels,
            } => {
                let bytes = pixel_bytes(*width, *height)?;
                if pixels.len() != bytes {
                    return None;
                }
                self.remaining_pixels_bytes = self.remaining_pixels_bytes.checked_sub(bytes)?;
                Some(visual)
            }
        }
    }

    fn decode_file(&mut self, path: &Path) -> Option<BannerVisual> {
        if (self.cancelled)() {
            return None;
        }
        self.remaining_file_attempts = self.remaining_file_attempts.checked_sub(1)?;
        if !path.is_absolute() || !crate::domain::has_static_raster_extension(path.to_str()?) {
            return None;
        }
        let bytes = read_file(path, &mut self.remaining_encoded_bytes)?;
        if (self.cancelled)() {
            return None;
        }
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
            return None;
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().ok()?;
        let (width, height) = decoder.dimensions();
        let bytes = pixel_bytes(width, height)?;
        if bytes > self.remaining_pixels_bytes || (self.cancelled)() {
            return None;
        }
        let orientation = decoder
            .exif_metadata()
            .ok()
            .flatten()
            .as_deref()
            .and_then(image::metadata::Orientation::from_exif_chunk)
            .unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut decoded = image::DynamicImage::from_decoder(decoder).ok()?;
        decoded.apply_orientation(orientation);
        let (width, height) = (decoded.width(), decoded.height());
        let pixels = decoded.into_rgba8().into_raw();
        self.remaining_pixels_bytes -= bytes;
        Some(BannerVisual::Rgba {
            width,
            height,
            pixels,
        })
    }
}

fn pixel_bytes(width: u32, height: u32) -> Option<usize> {
    let pixels = u64::from(width) * u64::from(height);
    if width > 4096 || height > 4096 || pixels == 0 || pixels > MAX_VISUAL_PIXELS {
        return None;
    }
    Some((pixels * 4) as usize)
}

fn read_file(path: &Path, remaining_bytes: &mut u64) -> Option<Vec<u8>> {
    if *remaining_bytes == 0 {
        return None;
    }
    let maximum_bytes = MAX_ENCODED_BYTES.min(*remaining_bytes);
    let result = crate::local_file::read_bounded(path, maximum_bytes as usize);
    let bytes_read = result
        .as_ref()
        .map(Vec::len)
        .unwrap_or_else(|error| error.bytes_read);
    *remaining_bytes = remaining_bytes.saturating_sub(bytes_read as u64);
    result.ok()
}
