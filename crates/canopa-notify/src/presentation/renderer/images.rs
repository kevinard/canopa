//! Turn local references into stable pixels before widgets see them.
use super::{
    BannerBodySegment, BannerPresentation, BannerVisual, NotificationBodyImageSource,
    NotificationBodySegment, NotificationBodyStyle, push_banner_body_text_segment,
};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, Raster, RasterBatch, RasterBudget, RasterPolicy,
};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

static DECODER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

struct ConsumerLifetime(Arc<AtomicBool>);

impl Drop for ConsumerLifetime {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
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
    let mut budget = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 16 * 1024 * 1024,
            max_dimension: 4096,
            max_pixels: 4 * 1024 * 1024,
            decoder_allocation: DecoderAllocation::DecoderOnly(64 * 1024 * 1024),
            formats: Formats::StaticPngJpeg,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: None,
        },
        RasterBudget {
            pixel_bytes: 32 * 1024 * 1024,
            file_attempts: if allow_files { 32 } else { 0 },
            encoded_bytes: 32 * 1024 * 1024,
        },
        cancelled,
    );
    let icon_cancelled = || !allow_files || cancelled();
    let mut icons = super::icons::IconLookup::new(
        icon_roots,
        super::icons::LookupOptions {
            theme: &presentation.icon_settings.theme,
            scale: presentation.icon_settings.scale,
            formats: super::icons::IconFormats::PngJpeg,
            allow_unindexed_hicolor: false,
        },
        &icon_cancelled,
    );
    for banner in &mut presentation.banners {
        if cancelled() {
            return None;
        }
        banner.app_icon = banner.app_icon.take().and_then(|visual| {
            resolve(visual, &mut icons, super::APP_ICON_VISUAL_SIZE, &mut budget)
        });
        let image_size = if banner.app_icon.is_some() {
            super::NOTIFICATION_IMAGE_VISUAL_SIZE
        } else {
            super::APP_ICON_VISUAL_SIZE
        };
        banner.image = banner
            .image
            .take()
            .and_then(|visual| resolve(visual, &mut icons, image_size, &mut budget));
        for action in &mut banner.actions {
            action.icon = action
                .icon
                .take()
                .and_then(|visual| resolve(visual, &mut icons, 16, &mut budget));
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
                    if let Some(visual) = budget.load(Path::new(path)).map(prepared_visual) {
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

fn resolve(
    visual: BannerVisual,
    icons: &mut super::icons::IconLookup<'_>,
    size: u16,
    budget: &mut RasterBatch<'_>,
) -> Option<BannerVisual> {
    match &visual {
        BannerVisual::IconNames { names } => {
            let path = icons.resolve(names, size)?;
            budget.load(&path).map(prepared_visual)
        }
        BannerVisual::LocalPath { path } => budget.load(Path::new(path)).map(prepared_visual),
        BannerVisual::Rgba {
            width,
            height,
            pixels,
        } => budget
            .admit_rgba(*width, *height, pixels.len())
            .then_some(visual),
    }
}

fn prepared_visual(raster: Raster) -> BannerVisual {
    BannerVisual::Rgba {
        width: raster.width,
        height: raster.height,
        pixels: raster.pixels,
    }
}
