//! Turn local references into stable pixels before widgets see them.
use super::{
    BannerBodySegment, BannerPresentation, BannerVisual, NotificationBodyImageSource,
    NotificationBodySegment, NotificationBodyStyle, push_banner_body_text_segment,
};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, Raster, RasterBatch, RasterBudget, RasterPolicy,
    icons::IconSource, svg,
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
                    BannerVisual::LocalPath { .. }
                        | BannerVisual::IconNames { .. }
                        | BannerVisual::DesktopEntry { .. }
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
            // PNG/JPEG by content, whatever the name; SVG is detected separately.
            formats: Formats::SniffedPngJpeg,
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
    // Fresh per frame: files are observed at preparation, never cached across frames.
    let theme = if allow_files {
        presentation.icon_settings.effective_theme()
    } else {
        presentation.icon_settings.theme.clone()
    };
    let scale = presentation.icon_settings.scale;
    let mut icons = super::icons::IconLookup::new(
        icon_roots,
        super::icons::LookupOptions {
            theme: &theme,
            scale,
            formats: super::icons::IconFormats::PngSvgXpm,
            allow_unindexed_hicolor: false,
        },
        &icon_cancelled,
    );
    let canopa_theme::Rgb(r, g, b) = presentation.appearance.palette().text;
    let mut sources = Sources {
        icons: &mut icons,
        budget: &mut budget,
        allow_files,
        scale,
        foreground: [r, g, b],
    };
    for banner in &mut presentation.banners {
        if cancelled() {
            return None;
        }
        banner.app_icon = banner
            .app_icon
            .take()
            .and_then(|visual| sources.resolve(visual, super::APP_ICON_VISUAL_SIZE));
        let image_size = if banner.app_icon.is_some() {
            super::NOTIFICATION_IMAGE_VISUAL_SIZE
        } else {
            super::APP_ICON_VISUAL_SIZE
        };
        banner.image = banner
            .image
            .take()
            .and_then(|visual| sources.resolve(visual, image_size));
        for action in &mut banner.actions {
            action.icon = action
                .icon
                .take()
                .and_then(|visual| sources.resolve(visual, 16));
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
                    // Body images stay PNG/JPEG rasters with an image extension.
                    if let Some(visual) = sources.budget.load(Path::new(path)).map(prepared_visual)
                    {
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

/// One frame's icon lookup and raster budget, shared by every visual role.
struct Sources<'s, 'a> {
    icons: &'s mut super::icons::IconLookup<'a>,
    budget: &'s mut RasterBatch<'a>,
    allow_files: bool,
    scale: u16,
    foreground: [u8; 3],
}

impl Sources<'_, '_> {
    fn resolve(&mut self, visual: BannerVisual, size: u16) -> Option<BannerVisual> {
        match &visual {
            BannerVisual::IconNames { names } => {
                // Sender icon names may be specific: try their generic forms too.
                self.icons.set_generic_fallback(true);
                let path = self.icons.resolve(names, size)?;
                self.load(&path, size).map(prepared_visual)
            }
            BannerVisual::DesktopEntry { ids } => {
                self.desktop_entry(ids, size).map(prepared_visual)
            }
            BannerVisual::LocalPath { path } => {
                self.load(Path::new(path), size).map(prepared_visual)
            }
            BannerVisual::Rgba {
                width,
                height,
                pixels,
            } => self
                .budget
                .admit_rgba(*width, *height, pixels.len())
                .then_some(visual),
        }
    }

    /// The application's installed `Icon=`, then its identities as icon names.
    fn desktop_entry(&mut self, ids: &[String], size: u16) -> Option<Raster> {
        if !self.allow_files {
            return None;
        }
        self.icons.set_generic_fallback(false);
        for id in ids.iter().take(2) {
            let Some(source) = canopa_apps::desktop_entry_icon(id)
                .as_deref()
                .and_then(IconSource::parse)
            else {
                continue;
            };
            let path = match source {
                IconSource::Path(path) => Some(path),
                IconSource::Name(name) => self.icons.resolve(&[name], size),
            };
            if let Some(raster) = path.and_then(|path| self.load(&path, size)) {
                return Some(raster);
            }
        }
        let path = self.icons.resolve(ids, size)?;
        self.load(&path, size)
    }

    fn load(&mut self, path: &Path, size: u16) -> Option<Raster> {
        // Rasterize vectors at twice the nominal size, or the source scale if larger.
        let side = u32::from(size) * u32::from(self.scale.max(2));
        self.budget.load_image_or_svg(
            path,
            &svg::Policy {
                max_encoded_bytes: 1024 * 1024,
                max_dimension: 4096,
                max_pixels: 4 * 1024 * 1024,
                output_side: side.min(1024),
                foreground: self.foreground,
            },
        )
    }
}

fn prepared_visual(raster: Raster) -> BannerVisual {
    BannerVisual::Rgba {
        width: raster.width,
        height: raster.height,
        pixels: raster.pixels,
    }
}
