//! Bounded local-image decoding outside the GUI and async executor threads.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use canopa_panel_protocol::{PanelContent, PanelIcon, RichBodyImageSource, RichBodySegment};
use iced::widget::image::Handle;

const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMAGES: usize = 16;
const MAX_ICONS: usize = 16;

#[derive(Default)]
pub(super) struct PreparedAssets {
    pub images: HashMap<String, Handle>,
    pub icons: HashMap<PanelIcon, Handle>,
}

pub(super) async fn prepare(content: &PanelContent) -> PreparedAssets {
    let mut paths = Vec::new();
    let mut icons = Vec::new();
    for section in &content.sections {
        if let Some(icon) = &section.icon
            && icons.len() < MAX_ICONS
            && !icons.contains(icon)
        {
            icons.push(icon.clone());
        }
        for item in &section.items {
            if let Some(icon) = item
                .attribution
                .as_ref()
                .and_then(|source| source.icon.as_ref())
                && icons.len() < MAX_ICONS
                && !icons.contains(icon)
            {
                icons.push(icon.clone());
            }
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
    if paths.is_empty() && icons.is_empty() {
        return PreparedAssets::default();
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
        return PreparedAssets::default();
    };
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        prepare_assets(paths, icons, canopa_assets::icons::IconResolver::system())
    })
    .await
    .unwrap_or_default()
}

fn prepare_assets(
    paths: Vec<String>,
    icons: Vec<PanelIcon>,
    resolver: &canopa_assets::icons::IconResolver,
) -> PreparedAssets {
    let policy = RasterPolicy {
        max_encoded_bytes: MAX_FILE_BYTES,
        max_dimension: 2048,
        max_pixels: 2048 * 2048,
        decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
        formats: Formats::EnabledRasterCodecs,
        file_access: FileAccess::ResolveFirst,
        apply_orientation: false,
        thumbnail: Some(512),
    };
    let budget = |count| RasterBudget {
        file_attempts: count,
        encoded_bytes: count * MAX_FILE_BYTES,
        pixel_bytes: count * 2048 * 2048 * 4,
    };
    let mut images = RasterBatch::new(policy, budget(MAX_IMAGES), &|| false);
    let prepared_images = paths
        .into_iter()
        .filter_map(|path| {
            let image = images.load(Path::new(&path))?;
            Some((path, canopa_ui::icons::handle(image)))
        })
        .collect();
    // Sender icons: PNG/JPEG or SVG by content, so extensionless files work.
    let mut pixels = RasterBatch::new(
        RasterPolicy {
            formats: Formats::SniffedPngJpeg,
            thumbnail: Some(128),
            ..policy
        },
        budget(MAX_ICONS),
        &|| false,
    );
    let vector = canopa_assets::svg::Policy {
        max_encoded_bytes: MAX_FILE_BYTES,
        max_dimension: 2048,
        max_pixels: 2048 * 2048,
        output_side: 128,
        // Symbolic SVGs: a neutral ink readable on light and dark panels.
        foreground: [0x88, 0x88, 0x88],
    };
    let mut prepared_icons = HashMap::new();
    for icon in &icons {
        use canopa_assets::icons::{IconQuery, IconSource};
        let path = match icon {
            PanelIcon::LocalPath { path } => IconSource::parse(path)
                .filter(|source| matches!(source, IconSource::Path(_)))
                .and_then(|source| {
                    resolver.resolve_source(&source, &IconQuery::new(&[], 34), &|| false)
                }),
            PanelIcon::Theme {
                names,
                theme,
                scale,
            } => resolver.resolve(
                &IconQuery {
                    scale: *scale,
                    theme: Some(theme),
                    generic_fallback: true,
                    ..IconQuery::new(names, 34)
                },
                &|| false,
            ),
        };
        if let Some(image) = path.and_then(|path| pixels.load_image_or_svg(&path, &vector)) {
            prepared_icons.insert(icon.clone(), canopa_ui::icons::handle(image));
        }
    }
    PreparedAssets {
        images: prepared_images,
        icons: prepared_icons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_icon(path: &Path, color: [u8; 4]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(34, 34, image::Rgba(color))
            .save(path)
            .unwrap();
    }

    fn color(handle: &Handle) -> &[u8] {
        let Handle::Rgba { pixels, .. } = handle else {
            panic!("icon must be decoded off the UI thread")
        };
        &pixels.as_ref()[..4]
    }

    #[test]
    fn center_icons_use_configured_theme_candidates_and_keep_body_images_separate() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("Custom")).unwrap();
        std::fs::write(
            root.join("Custom/index.theme"),
            "[Icon Theme]\nDirectories=34x34/apps\n[34x34/apps]\nSize=34\nType=Fixed\n",
        )
        .unwrap();
        write_icon(
            &root.join("Custom/34x34/apps/org.example.Mail.png"),
            [11, 22, 33, 255],
        );
        let explicit = root.join("explicit.png");
        write_icon(&explicit, [44, 55, 66, 255]);
        let named = PanelIcon::Theme {
            names: vec!["org.example.Mail.desktop".into(), "org.example.Mail".into()],
            theme: "Custom".into(),
            scale: 1,
        };
        let local = PanelIcon::LocalPath {
            path: explicit.to_str().unwrap().into(),
        };
        let mut missing = named.clone();
        if let PanelIcon::Theme { names, .. } = &mut missing {
            *names = vec!["missing".into()];
        }
        let prepared = prepare_assets(
            vec![explicit.to_str().unwrap().into()],
            vec![named.clone(), local.clone(), missing.clone()],
            &canopa_assets::icons::IconResolver::new(vec![root.to_path_buf()], Vec::new()),
        );
        assert_eq!(color(&prepared.icons[&named]), &[11, 22, 33, 255]);
        assert_eq!(color(&prepared.icons[&local]), &[44, 55, 66, 255]);
        assert_eq!(prepared.images.len(), 1);
        assert!(!prepared.icons.contains_key(&missing));
        write_icon(&explicit, [77, 88, 99, 255]);
        let next = prepare_assets(
            vec![],
            vec![local.clone()],
            &canopa_assets::icons::IconResolver::new(vec![root.to_path_buf()], Vec::new()),
        );
        assert_eq!(color(&next.icons[&local]), &[77, 88, 99, 255]);
        assert_eq!(
            color(&prepared.icons[&local]),
            &[44, 55, 66, 255],
            "published pixels remain immutable"
        );
        assert!(
            !next.icons.contains_key(&named),
            "retired icons do not leak into replacements"
        );
    }

    #[tokio::test]
    async fn icon_admission_deduplicates_and_bounds_assets_without_hiding_items() {
        let temp = tempfile::tempdir().unwrap();
        let mut items = Vec::new();
        for index in 0..MAX_ICONS + 2 {
            let path = temp.path().join(format!("{index}.png"));
            write_icon(&path, [index as u8, 22, 33, 255]);
            items.push(
                serde_json::json!({"id":format!("{index}"),"title":"Message",
                "attribution":{"label":"Mail", "icon":{"kind":"local_path","path":path}},
                "metadata":[], "actions":[]}),
            );
        }
        let mut duplicate = items[0].clone();
        duplicate["id"] = "duplicate".into();
        items.insert(1, duplicate);
        let icon = items[0]["attribution"]["icon"].clone();
        let mut content: PanelContent = serde_json::from_value(serde_json::json!({
            "title":"Messages", "sections":[{"id":"mail","title":"Mail","icon":icon,"controls":[],"items":items}], "actions":[]})).unwrap();
        let prepared = prepare(&content).await;
        assert_eq!(prepared.icons.len(), MAX_ICONS);
        assert_eq!(content.sections[0].items.len(), MAX_ICONS + 3);
        assert!(prepared.images.is_empty());
        content.sections[0].items.clear();
        let settings = prepare(&content).await;
        assert_eq!(
            settings.icons.len(),
            1,
            "section icons survive without message cards"
        );
        assert_eq!(
            color(&settings.icons[content.sections[0].icon.as_ref().unwrap()]),
            &[0, 22, 33, 255]
        );
    }
}
