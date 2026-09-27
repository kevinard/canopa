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
