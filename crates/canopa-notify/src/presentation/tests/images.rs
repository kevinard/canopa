use super::*;
use crate::NotificationBodyImage;

pub(super) static IMAGE_FIXTURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn body_image_banner(path: &Path, alt: Option<&str>) -> BannerView {
    let path = path.to_string_lossy().into_owned();
    let mut banner = banner_view(1, alt.unwrap_or_default());
    banner.body_content = NotificationBodyContent {
        segments: vec![NotificationBodySegment::body_image(
            NotificationBodyImage::local_path(path.clone(), alt.map(str::to_owned)),
        )],
    };
    banner.body_segments = vec![BannerBodySegment::image(BannerVisual::LocalPath { path })];
    banner
}

async fn render_banner(banner: BannerView) -> BannerView {
    let _fixture = IMAGE_FIXTURE.lock().await;
    let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    let presenter = IcedLayerShellBannerPresenter { updates };
    presenter.replace_visible_banners(vec![banner]).unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let BannerRuntimeInput::Replace {
        mut presentation, ..
    } = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("image preparation stalled the renderer")
        .unwrap();
    presentation.banners.remove(0)
}

#[tokio::test]
async fn renderer_displays_body_image_alternative_when_existing_file_is_corrupt() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("chart.png");
    std::fs::write(&path, b"not an image").unwrap();

    let banner = render_banner(body_image_banner(&path, Some("Chart unavailable"))).await;

    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::plain_text("Chart unavailable")]
    );
}

fn write_png(path: &Path, width: u32, height: u32, pixel: [u8; 4]) {
    image::RgbaImage::from_pixel(width, height, image::Rgba(pixel))
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
}

#[tokio::test]
async fn renderer_rejects_an_image_over_four_megapixels_before_publishing_pixels() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("large.png");
    write_png(&path, 2048, 2049, [11, 22, 33, 255]);

    let banner = render_banner(body_image_banner(&path, Some("Large chart"))).await;

    assert!(
        matches!(&banner.body_segments[..], [BannerBodySegment::Text { text, .. }] if text == "Large chart"),
        "an image over 4,194,304 pixels must fall back before reaching the renderer"
    );
}

#[tokio::test]
async fn renderer_bounds_total_decoded_pixels_across_all_banner_visual_slots() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("square.png");
    write_png(&path, 2048, 2048, [11, 22, 33, 255]);
    let mut banner = body_image_banner(&path, Some("Chart exceeds remaining budget"));
    banner.app_icon = Some(BannerVisual::LocalPath {
        path: path.to_string_lossy().into_owned(),
    });
    banner.image = banner.app_icon.clone();

    let banner = render_banner(banner).await;

    assert!(matches!(
        banner.app_icon,
        Some(BannerVisual::Rgba {
            width: 2048,
            height: 2048,
            ..
        })
    ));
    assert!(matches!(
        banner.image,
        Some(BannerVisual::Rgba {
            width: 2048,
            height: 2048,
            ..
        })
    ));
    assert!(
        matches!(&banner.body_segments[..], [BannerBodySegment::Text { text, .. }] if text == "Chart exceeds remaining budget"),
        "the 32 MiB pixel budget must be shared by header and body visuals"
    );
}

#[tokio::test]
async fn renderer_bounds_file_attempts_and_aggregate_encoded_bytes_per_update() {
    let temp = tempfile::tempdir().unwrap();
    for (name, file_size, count, expected_images) in [
        ("tiny.png", None, 33, 32),
        ("padded.png", Some(16 * 1024 * 1024), 3, 2),
    ] {
        let path = temp.path().join(name);
        write_png(&path, 1, 1, [11, 22, 33, 255]);
        if let Some(size) = file_size {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(size)
                .unwrap();
        }
        let mut banner = body_image_banner(&path, Some("Budget fallback"));
        banner.body_content.segments = vec![banner.body_content.segments[0].clone(); count];

        let banner = render_banner(banner).await;

        assert_eq!(
            banner
                .body_segments
                .iter()
                .filter(|segment| matches!(segment, BannerBodySegment::Image { .. }))
                .count(),
            expected_images,
            "{name}"
        );
        assert!(
            matches!(banner.body_segments.last(), Some(BannerBodySegment::Text { text, .. }) if text == "Budget fallback")
        );
    }
}

#[tokio::test]
async fn renderer_prepares_images_when_polled_outside_the_daemon_executor() {
    let _fixture = IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pixel.png");
    write_png(&path, 1, 1, [11, 22, 33, 255]);
    let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    let presenter = IcedLayerShellBannerPresenter { updates };
    presenter
        .replace_visible_banners(vec![body_image_banner(&path, None)])
        .unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let (finished, result) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        // Iced's thread-pool executor does not enter the daemon's Tokio runtime.
        let input = iced::futures::executor::block_on(stream.next());
        let _ = finished.send(input);
    });

    let BannerRuntimeInput::Replace { presentation, .. } =
        tokio::time::timeout(Duration::from_secs(5), result)
            .await
            .unwrap()
            .expect("renderer stream panicked outside the daemon executor")
            .unwrap();
    assert_eq!(
        presentation.banners[0].body_segments,
        vec![BannerBodySegment::image(BannerVisual::Rgba {
            width: 1,
            height: 1,
            pixels: vec![11, 22, 33, 255]
        })]
    );
}

#[test]
fn superseded_image_preparation_cannot_display_or_acknowledge_a_closed_banner() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pixel.png");
    write_png(&path, 1, 1, [11, 22, 33, 255]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let _fixture = IMAGE_FIXTURE.lock().await;
        let (release, blocked) = mpsc::channel::<()>();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            let _ = blocked.recv_timeout(Duration::from_secs(10));
        });
        ready.await.unwrap();
        let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
        let presenter = IcedLayerShellBannerPresenter { updates };
        let mut stream = Box::pin(banner_updates(receiver));
        let old_acknowledged = Arc::new(AtomicBool::new(false));
        let old = old_acknowledged.clone();
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::visible_banners(vec![body_image_banner(&path, None)]),
                BannerPresentationAcknowledgement::new(move || {
                    old.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();
        // The external executor is occupied: preparation cannot complete on this poll.
        assert!(iced::futures::poll!(stream.next()).is_pending());
        let closed_acknowledged = Arc::new(AtomicBool::new(false));
        let closed = closed_acknowledged.clone();
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::default(),
                BannerPresentationAcknowledgement::new(move || {
                    closed.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();
        let BannerRuntimeInput::Replace {
            presentation,
            acknowledgement,
        } = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap();
        assert!(
            presentation.banners.is_empty(),
            "completed stale preparation resurrected a closed banner"
        );
        assert!(!old_acknowledged.load(Ordering::SeqCst));
        assert!(!closed_acknowledged.load(Ordering::SeqCst));
        acknowledgement.acknowledge();
        assert!(closed_acknowledged.load(Ordering::SeqCst));
        assert!(!old_acknowledged.load(Ordering::SeqCst));
        drop(release);
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                presenter
                    .replace_visible_banners(vec![body_image_banner(&path, None)])
                    .unwrap();
                let BannerRuntimeInput::Replace { presentation, .. } = stream.next().await.unwrap();
                if matches!(
                    &presentation.banners[0].body_segments[..],
                    [BannerBodySegment::Image {
                        image: BannerVisual::Rgba { .. }
                    }]
                ) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled preparation did not release decoder admission");
    });
}

#[tokio::test]
async fn renderer_omits_invalid_raw_pixels_without_losing_the_banner() {
    for (width, height, pixels) in [
        (u32::MAX, u32::MAX, vec![]),
        (1, 1, vec![11, 22]),
        (0, 1, vec![]),
        (1, 0, vec![]),
    ] {
        let mut banner = banner_view(1, "Text survives invalid pixels");
        banner.image = Some(BannerVisual::Rgba {
            width,
            height,
            pixels,
        });
        let banner = render_banner(banner).await;
        assert!(banner.image.is_none());
        assert_eq!(banner.body, "Text survives invalid pixels");
    }
}

#[tokio::test]
async fn prepared_pixels_keep_visual_roles_and_survive_source_edits_and_deletion() {
    let _fixture = IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let paths = ["app.png", "notification.jpg", "action.png", "body.png"]
        .map(|name| temp.path().join(name));
    let colors = [
        [200, 0, 0, 255],
        [0, 200, 0, 255],
        [0, 0, 200, 255],
        [11, 22, 33, 80],
    ];
    for (path, color) in paths.iter().zip(colors) {
        write_png(path, 1, 1, color);
    }
    let mut banner = body_image_banner(&paths[3], Some("Body alternative"));
    let local = |index: usize| BannerVisual::LocalPath {
        path: paths[index].to_string_lossy().into_owned(),
    };
    banner.app_icon = Some(local(0));
    banner.image = Some(local(1));
    banner.actions = vec![BannerAction {
        key: "open".into(),
        label: "Open".into(),
        icon: Some(local(2)),
    }];
    let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    let presenter = IcedLayerShellBannerPresenter { updates };
    let mut stream = Box::pin(banner_updates(receiver));
    presenter
        .replace_visible_banners(vec![banner.clone()])
        .unwrap();
    let BannerRuntimeInput::Replace {
        presentation: first,
        ..
    } = stream.next().await.unwrap();
    let rgba = |pixels: [u8; 4]| BannerVisual::Rgba {
        width: 1,
        height: 1,
        pixels: pixels.to_vec(),
    };
    assert_eq!(first.banners[0].app_icon, Some(rgba(colors[0])));
    assert_eq!(first.banners[0].image, Some(rgba(colors[1])));
    assert_eq!(first.banners[0].actions[0].icon, Some(rgba(colors[2])));
    assert_eq!(
        first.banners[0].body_segments,
        vec![BannerBodySegment::image(rgba(colors[3]))]
    );

    write_png(&paths[3], 1, 1, [44, 55, 66, 255]);
    std::fs::remove_file(&paths[0]).unwrap();
    presenter
        .replace_visible_banners(vec![banner.clone()])
        .unwrap();
    let BannerRuntimeInput::Replace {
        presentation: second,
        ..
    } = stream.next().await.unwrap();
    assert!(second.banners[0].app_icon.is_none());
    assert_eq!(
        second.banners[0].body_segments,
        vec![BannerBodySegment::image(rgba([44, 55, 66, 255]))]
    );
    std::fs::remove_file(&paths[3]).unwrap();
    assert_eq!(first.banners[0].app_icon, Some(rgba(colors[0])));
    assert_eq!(
        first.banners[0].body_segments,
        vec![BannerBodySegment::image(rgba(colors[3]))]
    );
    presenter.replace_visible_banners(vec![banner]).unwrap();
    let BannerRuntimeInput::Replace {
        presentation: third,
        ..
    } = stream.next().await.unwrap();
    assert_eq!(
        third.banners[0].body_segments,
        vec![BannerBodySegment::plain_text("Body alternative")]
    );
}

#[tokio::test]
async fn renderer_preserves_body_order_styles_and_optional_alternatives_on_decode_failure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing.png");
    let mut banner = body_image_banner(&path, Some("chart"));
    let bold = NotificationBodyStyle {
        bold: true,
        ..Default::default()
    };
    banner
        .body_content
        .segments
        .insert(0, NotificationBodySegment::styled_text("Before ", bold));
    banner
        .body_content
        .segments
        .push(NotificationBodySegment::plain_text(" after"));
    banner
        .body_content
        .segments
        .push(NotificationBodySegment::body_image(
            NotificationBodyImage::local_path(path.to_string_lossy(), None),
        ));
    let banner = render_banner(banner).await;
    assert_eq!(
        banner.body_segments,
        vec![
            BannerBodySegment::styled_text("Before ", bold),
            BannerBodySegment::plain_text("chart after")
        ]
    );
}

#[tokio::test]
async fn renderer_decodes_png_color_layouts_jpeg_and_regular_file_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pixel.png");
    for (source, expected) in [
        (
            image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                1,
                1,
                image::Rgb([11, 22, 33]),
            )),
            [11, 22, 33, 255],
        ),
        (
            image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(1, 1, image::Luma([42]))),
            [42, 42, 42, 255],
        ),
        (
            image::DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_pixel(
                1,
                1,
                image::LumaA([42, 80]),
            )),
            [42, 42, 42, 80],
        ),
        (
            image::DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
                1,
                1,
                image::Rgba([65535, 0, 65535, 65535]),
            )),
            [255, 0, 255, 255],
        ),
    ] {
        source
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();
        let banner = render_banner(body_image_banner(&path, None)).await;
        assert_eq!(
            banner.body_segments,
            vec![BannerBodySegment::image(BannerVisual::Rgba {
                width: 1,
                height: 1,
                pixels: expected.to_vec()
            })]
        );
    }
    let jpeg = temp.path().join("photo.jpeg");
    image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0]))
        .save(&jpeg)
        .unwrap();
    let link = temp.path().join("photo-link.png");
    std::os::unix::fs::symlink(&jpeg, &link).unwrap();
    let banner = render_banner(body_image_banner(&link, None)).await;
    let [
        BannerBodySegment::Image {
            image:
                BannerVisual::Rgba {
                    width,
                    height,
                    pixels,
                },
        },
    ] = &banner.body_segments[..]
    else {
        panic!("JPEG symlink was not decoded");
    };
    assert_eq!((*width, *height), (2, 2));
    assert!(
        pixels
            .chunks_exact(4)
            .all(|pixel| pixel[0] >= 253 && pixel[1] <= 2 && pixel[2] <= 2 && pixel[3] == 255)
    );
}

#[tokio::test]
async fn renderer_rejects_unsupported_truncated_oversized_and_nonlocal_sources() {
    let temp = tempfile::tempdir().unwrap();
    let oversized = temp.path().join("oversized.png");
    write_png(&oversized, 1, 1, [11, 22, 33, 255]);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&oversized)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let wide = temp.path().join("wide.png");
    write_png(&wide, 4097, 1, [11, 22, 33, 255]);
    let truncated = temp.path().join("truncated.png");
    write_png(&truncated, 1, 1, [11, 22, 33, 255]);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&truncated)
        .unwrap()
        .set_len(40)
        .unwrap();
    let disguised_gif = temp.path().join("disguised.png");
    std::fs::write(
        &disguised_gif,
        [
            71, 73, 70, 56, 57, 97, 1, 0, 1, 0, 128, 0, 0, 0, 0, 0, 255, 255, 255, 33, 249, 4, 1,
            0, 0, 0, 0, 44, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 68, 1, 0, 59,
        ],
    )
    .unwrap();
    let directory = temp.path().join("directory.png");
    std::fs::create_dir(&directory).unwrap();
    let cycle = temp.path().join("cycle.png");
    std::os::unix::fs::symlink(&cycle, &cycle).unwrap();
    for path in [
        oversized,
        wide,
        truncated,
        disguised_gif,
        directory,
        cycle,
        PathBuf::from("relative.png"),
        PathBuf::from("https://example.invalid/image.png"),
    ] {
        let banner = render_banner(body_image_banner(&path, Some("Unavailable"))).await;
        assert_eq!(
            banner.body_segments,
            vec![BannerBodySegment::plain_text("Unavailable")],
            "{}",
            path.display()
        );
    }
}

#[test]
fn dropped_consumer_releases_callbacks_but_keeps_decoder_admission_until_job_exit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pixel.png");
    write_png(&path, 1, 1, [11, 22, 33, 255]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let _fixture = IMAGE_FIXTURE.lock().await;
        let (release, blocked) = mpsc::channel::<()>();
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            let _ = blocked.recv_timeout(Duration::from_secs(10));
        });
        ready.await.unwrap();
        let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
        let presenter = IcedLayerShellBannerPresenter { updates };
        let mut stream = Box::pin(banner_updates(receiver));
        let marker = Arc::new(());
        let lifetime = Arc::downgrade(&marker);
        let acknowledged = Arc::new(AtomicBool::new(false));
        let observed = acknowledged.clone();
        presenter
            .replace_banner_presentation_with_acknowledgement(
                BannerPresentation::visible_banners(vec![body_image_banner(&path, None)]),
                BannerPresentationAcknowledgement::new(move || {
                    drop(marker);
                    observed.store(true, Ordering::SeqCst);
                }),
            )
            .unwrap();
        assert!(iced::futures::poll!(stream.next()).is_pending());
        presenter.replace_visible_banners(vec![]).unwrap();
        drop(stream);
        drop(presenter);
        assert!(
            lifetime.upgrade().is_none(),
            "abandoned decoding retained its display callback"
        );
        assert!(!acknowledged.load(Ordering::SeqCst));

        let (updates, mut receiver) =
            BannerUpdateSender::channel(tokio::runtime::Handle::current());
        receiver.icon_roots = vec![temp.path().to_path_buf()].into();
        let presenter = IcedLayerShellBannerPresenter { updates };
        let mut stream = Box::pin(banner_updates(receiver));
        let mut replacement = body_image_banner(&path, Some("Decoder busy"));
        replacement.app_icon = Some(BannerVisual::Rgba {
            width: 1,
            height: 1,
            pixels: vec![44, 55, 66, 255],
        });
        replacement.actions = vec![BannerAction {
            key: "named".into(),
            label: "Named icon".into(),
            icon: Some(BannerVisual::IconNames {
                names: vec!["pixel".into()],
            }),
        }];
        // Reopening cannot queue a second decoder behind the occupied executor.
        for _ in 0..32 {
            presenter
                .replace_visible_banners(vec![replacement.clone()])
                .unwrap();
            let BannerRuntimeInput::Replace { presentation, .. } =
                tokio::time::timeout(Duration::from_secs(1), stream.next())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                presentation.banners[0].body_segments,
                vec![BannerBodySegment::plain_text("Decoder busy")]
            );
            assert_eq!(presentation.banners[0].app_icon, replacement.app_icon);
            assert!(presentation.banners[0].actions[0].icon.is_none());
        }
        drop(release);
        blocker.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                presenter
                    .replace_visible_banners(vec![replacement.clone()])
                    .unwrap();
                let BannerRuntimeInput::Replace { presentation, .. } = stream.next().await.unwrap();
                if matches!(
                    &presentation.banners[0].body_segments[..],
                    [BannerBodySegment::Image {
                        image: BannerVisual::Rgba { .. }
                    }]
                ) {
                    assert_eq!(
                        presentation.banners[0].actions[0].icon,
                        Some(BannerVisual::Rgba {
                            width: 1,
                            height: 1,
                            pixels: vec![11, 22, 33, 255],
                        })
                    );
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("decoder admission did not recover after the abandoned job exited");
        assert!(!acknowledged.load(Ordering::SeqCst));
    });
}

#[tokio::test]
async fn renderer_rejects_a_fifo_without_waiting_for_a_writer() {
    let _fixture = IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let fifo = temp.path().join("image.png");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    // A regression must fail promptly, not leave Tokio shutdown waiting on a stuck open.
    let rescue_path = fifo.clone();
    let (cancel_rescue, cancelled) = mpsc::channel::<()>();
    let rescue = std::thread::spawn(move || {
        if matches!(
            cancelled.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            let _writer = rustix::fs::open(
                &rescue_path,
                rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let (updates, receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    let presenter = IcedLayerShellBannerPresenter { updates };
    let mut stream = Box::pin(banner_updates(receiver));
    let started = std::time::Instant::now();
    presenter
        .replace_visible_banners(vec![body_image_banner(&fifo, Some("Not an image"))])
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
    let elapsed = started.elapsed();
    drop(cancel_rescue);
    rescue.join().unwrap();
    let BannerRuntimeInput::Replace { presentation, .. } = result.unwrap().unwrap();
    assert!(
        elapsed < Duration::from_secs(1),
        "FIFO lookup waited for its rescue writer"
    );
    assert_eq!(
        presentation.banners[0].body_segments,
        vec![BannerBodySegment::plain_text("Not an image")]
    );
}

#[tokio::test]
async fn renderer_preserves_exif_rotations_and_mirroring_for_jpeg_and_png() {
    use image::ImageEncoder;
    let temp = tempfile::tempdir().unwrap();
    let red = [240, 0, 0, 255];
    let green = [0, 240, 0, 255];
    let blue = [0, 0, 240, 255];
    let yellow = [240, 240, 0, 255];
    let source = image::RgbImage::from_fn(32, 16, |x, y| {
        image::Rgb(match (x < 16, y < 8) {
            (true, true) => [240, 0, 0],
            (false, true) => [0, 240, 0],
            (true, false) => [0, 0, 240],
            (false, false) => [240, 240, 0],
        })
    });
    // Literal TIFF orientation tags and independently worked corner mappings.
    for extension in ["jpeg", "png"] {
        for (orientation, width, height, top_left, bottom_right) in [
            (1, 32, 16, red, yellow),
            (2, 32, 16, green, blue),
            (3, 32, 16, yellow, red),
            (4, 32, 16, blue, green),
            (5, 16, 32, red, yellow),
            (6, 16, 32, blue, green),
            (7, 16, 32, yellow, red),
            (8, 16, 32, green, blue),
        ] {
            let exif = vec![
                b'I',
                b'I',
                42,
                0,
                8,
                0,
                0,
                0,
                1,
                0,
                0x12,
                1,
                3,
                0,
                1,
                0,
                0,
                0,
                orientation,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ];
            let path = temp
                .path()
                .join(format!("oriented-{orientation}.{extension}"));
            let file = std::fs::File::create(&path).unwrap();
            if extension == "jpeg" {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 100);
                encoder.set_exif_metadata(exif).unwrap();
                encoder.encode_image(&source).unwrap();
            } else {
                let mut encoder = image::codecs::png::PngEncoder::new(file);
                encoder.set_exif_metadata(exif).unwrap();
                encoder
                    .write_image(source.as_raw(), 32, 16, image::ExtendedColorType::Rgb8)
                    .unwrap();
            }
            let banner = render_banner(body_image_banner(&path, None)).await;
            let [
                BannerBodySegment::Image {
                    image:
                        BannerVisual::Rgba {
                            width: actual_width,
                            height: actual_height,
                            pixels,
                        },
                },
            ] = &banner.body_segments[..]
            else {
                panic!("EXIF {extension} did not render");
            };
            assert_eq!(
                (*actual_width, *actual_height),
                (width, height),
                "{extension} orientation {orientation}"
            );
            for (x, y, expected) in [(3, 3, top_left), (width - 4, height - 4, bottom_right)] {
                let start = ((y * width + x) * 4) as usize;
                assert!(
                    pixels[start..start + 4]
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 8),
                    "{extension} orientation {orientation} corner ({x}, {y})"
                );
            }
        }
    }
}
