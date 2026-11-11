use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use std::path::Path;

fn policy() -> RasterPolicy {
    RasterPolicy {
        max_encoded_bytes: 4096,
        max_dimension: 64,
        max_pixels: 64 * 64,
        decoder_allocation: DecoderAllocation::DecoderOnly(16 * 1024 * 1024),
        formats: Formats::StaticPngJpeg,
        file_access: FileAccess::Follow,
        apply_orientation: true,
        thumbnail: None,
    }
}

fn budget() -> RasterBudget {
    RasterBudget {
        file_attempts: 16,
        encoded_bytes: 16 * 4096,
        pixel_bytes: 16 * 64 * 64 * 4,
    }
}

fn png(path: &Path, width: u32, height: u32) {
    image::RgbaImage::from_pixel(width, height, image::Rgba([11, 22, 33, 255]))
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
}

#[test]
fn caller_format_and_thumbnail_policies_survive_feature_unification() {
    let temp = tempfile::tempdir().unwrap();
    let gif = temp.path().join("disguised.png");
    image::RgbaImage::from_pixel(8, 4, image::Rgba([11, 22, 33, 255]))
        .save_with_format(&gif, image::ImageFormat::Gif)
        .unwrap();
    let mut strict = RasterBatch::new(policy(), budget(), &|| false);
    assert!(strict.load(&gif).is_none());
    let mut panel = RasterBatch::new(
        RasterPolicy {
            formats: Formats::EnabledRasterCodecs,
            thumbnail: Some(4),
            ..policy()
        },
        budget(),
        &|| false,
    );
    let decoded = panel.load(&gif).unwrap();
    assert_eq!((decoded.width, decoded.height), (4, 2));
    assert_eq!(decoded.pixels.len(), 32);
    let renamed = temp.path().join("image.bin");
    png(&renamed, 2, 2);
    assert!(strict.load(&renamed).is_none());
    assert!(panel.load(&renamed).is_some());
}

#[test]
fn shared_batch_accounts_failed_reads_raw_pixels_and_source_size_before_thumbnail() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("square.png");
    png(&path, 8, 8);
    let encoded = std::fs::metadata(&path).unwrap().len() as usize;
    let mut images = RasterBatch::new(
        RasterPolicy {
            thumbnail: Some(2),
            ..policy()
        },
        RasterBudget {
            file_attempts: 4,
            encoded_bytes: 4 * encoded,
            pixel_bytes: 256 + 4,
        },
        &|| false,
    );
    assert!(!images.admit_rgba(1, 1, 3));
    assert!(images.admit_rgba(1, 1, 4));
    let decoded = images.load(&path).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 2));
    assert!(!images.admit_rgba(1, 1, 4));
    assert!(images.load(&path).is_none());

    let corrupt = temp.path().join("corrupt.png");
    std::fs::write(&corrupt, vec![0; encoded]).unwrap();
    let mut images = RasterBatch::new(
        policy(),
        RasterBudget {
            encoded_bytes: encoded,
            ..budget()
        },
        &|| false,
    );
    assert!(images.load(&corrupt).is_none());
    assert!(
        images.load(&path).is_none(),
        "failed decoding still consumes encoded bytes"
    );
    let mut images = RasterBatch::new(
        policy(),
        RasterBudget {
            file_attempts: 1,
            ..budget()
        },
        &|| false,
    );
    assert!(images.load(Path::new("relative.png")).is_none());
    assert!(
        images.load(&path).is_none(),
        "invalid sources still consume attempts"
    );
}

#[test]
fn local_reads_reject_special_files_and_keep_prepared_symlink_pixels_stable() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("square.png");
    let link = temp.path().join("link.png");
    let fifo = temp.path().join("fifo.png");
    png(&path, 2, 2);
    std::os::unix::fs::symlink(&path, &link).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    for file_access in [FileAccess::Follow, FileAccess::ResolveFirst] {
        let mut images = RasterBatch::new(
            RasterPolicy {
                file_access,
                ..policy()
            },
            budget(),
            &|| false,
        );
        let decoded = images.load(&link).unwrap();
        assert_eq!(decoded.pixels, [11, 22, 33, 255].repeat(4));
        // A missing NONBLOCK flag must fail this check, not hang the test process.
        let rescue_path = fifo.clone();
        let (done, wait) = std::sync::mpsc::channel();
        let rescue = std::thread::spawn(move || {
            if wait
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_err()
            {
                let _writer = rustix::fs::open(
                    &rescue_path,
                    rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
                    rustix::fs::Mode::empty(),
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        let started = std::time::Instant::now();
        assert!(images.load(&fifo).is_none());
        let elapsed = started.elapsed();
        done.send(()).unwrap();
        rescue.join().unwrap();
        assert!(elapsed < std::time::Duration::from_secs(1));
        assert!(images.load(temp.path()).is_none());
    }
    let mut images = RasterBatch::new(policy(), budget(), &|| false);
    let decoded = images.load(&link).unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert!(
        images.load(&link).is_none(),
        "later loads must observe source changes"
    );
    assert_eq!(decoded.pixels, [11, 22, 33, 255].repeat(4));
}

#[test]
fn dimensions_encoding_pixels_and_cancellation_have_independent_limits() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("square.png");
    png(&path, 8, 4);
    for limited in [
        RasterPolicy {
            max_encoded_bytes: 1,
            ..policy()
        },
        RasterPolicy {
            max_dimension: 7,
            ..policy()
        },
        RasterPolicy {
            max_pixels: 31,
            ..policy()
        },
    ] {
        assert!(
            RasterBatch::new(limited, budget(), &|| false)
                .load(&path)
                .is_none()
        );
    }
    let cancelled = std::cell::Cell::new(true);
    let is_cancelled = || cancelled.get();
    let mut images = RasterBatch::new(policy(), budget(), &is_cancelled);
    assert!(images.load(&path).is_none());
    assert!(!images.admit_rgba(1, 1, 4));
    cancelled.set(false);
    assert!(images.load(&path).is_some());
}

#[test]
fn owned_payloads_share_format_attempt_encoded_and_pixel_limits_with_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pixels.png");
    png(&path, 8, 8);
    let bytes = std::fs::read(&path).unwrap();
    let mut batch = RasterBatch::new(
        policy(),
        RasterBudget {
            file_attempts: 2,
            encoded_bytes: bytes.len() * 2,
            pixel_bytes: 256,
        },
        &|| false,
    );
    let image = batch.decode(bytes.clone()).unwrap();
    assert_eq!(image.pixels, [11, 22, 33, 255].repeat(64));
    assert!(
        batch.load(&path).is_none(),
        "owned output consumes the shared pixel budget"
    );
    let mut batch = RasterBatch::new(
        policy(),
        RasterBudget {
            file_attempts: 1,
            ..budget()
        },
        &|| false,
    );
    assert!(batch.decode(vec![0; 32]).is_none());
    assert!(
        batch.decode(bytes).is_none(),
        "failed owned payload consumes an attempt"
    );
}

#[cfg(feature = "svg")]
#[test]
fn svg_output_is_bounded_and_external_images_are_not_resolved() {
    let policy = || canopa_assets::svg::Policy {
        max_encoded_bytes: 4096,
        max_dimension: 64,
        max_pixels: 4096,
        output_side: 16,
        foreground: [10, 20, 30],
    };
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='32' height='16'><rect width='32' height='16' fill='currentColor'/><image href='file:///definitely-not-an-image.png' width='32' height='16'/></svg>";
    let image = canopa_assets::svg::rasterize(svg, policy()).unwrap();
    assert_eq!((image.width, image.height), (16, 8));
    assert_eq!(&image.pixels[..4], &[10, 20, 30, 255]);
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='65' height='1'/>";
    assert!(canopa_assets::svg::rasterize(svg, policy()).is_none());
}

#[test]
fn sniffed_formats_accept_extensionless_png_and_jpeg_but_not_other_content() {
    let temp = tempfile::tempdir().unwrap();
    let extensionless = temp.path().join("chromium-notification-XXXXXX");
    png(&extensionless, 2, 2);
    let gif = temp.path().join("other");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
        .save_with_format(&gif, image::ImageFormat::Gif)
        .unwrap();
    let mut sniffed = RasterBatch::new(
        RasterPolicy {
            formats: Formats::SniffedPngJpeg,
            ..policy()
        },
        budget(),
        &|| false,
    );
    assert!(sniffed.load(&extensionless).is_some());
    assert!(sniffed.load(&gif).is_none());
    assert!(
        RasterBatch::new(policy(), budget(), &|| false)
            .load(&extensionless)
            .is_none()
    );
}

#[cfg(feature = "svg")]
#[test]
fn svg_content_is_detected_without_trusting_the_file_name() {
    use canopa_assets::svg;
    let temp = tempfile::tempdir().unwrap();
    let disguised = temp.path().join("icon.png");
    std::fs::write(
        &disguised,
        "\u{feff}<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\" fill=\"currentColor\"/></svg>",
    )
    .unwrap();
    let raster_named_svg = temp.path().join("raster.svg");
    png(&raster_named_svg, 3, 3);
    let policy = svg::Policy {
        max_encoded_bytes: 4096,
        max_dimension: 64,
        max_pixels: 64 * 64,
        output_side: 8,
        foreground: [200, 100, 50],
    };
    let mut batch = RasterBatch::new(
        RasterPolicy {
            formats: Formats::SniffedPngJpeg,
            ..self::policy()
        },
        budget(),
        &|| false,
    );
    let vector = batch.load_image_or_svg(&disguised, &policy).unwrap();
    assert_eq!((vector.width, vector.height), (8, 8));
    assert_eq!(&vector.pixels[..4], &[200, 100, 50, 255]);
    let raster = batch.load_image_or_svg(&raster_named_svg, &policy).unwrap();
    assert_eq!((raster.width, raster.height), (3, 3));
    assert!(!svg::is_svg(b"\x1f\x8b compressed"));
    assert!(svg::is_svg(b"<!-- c --><svg/>"));
    assert!(!svg::is_svg(b"<?xml version=\"1.0\"?><html/>"));
}

#[test]
fn recolor_and_trim_keep_coverage() {
    let mut raster = canopa_assets::Raster {
        width: 3,
        height: 2,
        pixels: vec![
            0, 0, 0, 0, 9, 9, 9, 255, 0, 0, 0, 0, //
            0, 0, 0, 0, 9, 9, 9, 128, 0, 0, 0, 0,
        ],
    };
    raster.recolor([10, 20, 30, 255]);
    assert_eq!(&raster.pixels[4..8], &[10, 20, 30, 255]);
    let trimmed = raster.trimmed();
    assert_eq!((trimmed.width, trimmed.height), (1, 2));
    assert_eq!(trimmed.pixels, [10, 20, 30, 255, 10, 20, 30, 128]);
}
