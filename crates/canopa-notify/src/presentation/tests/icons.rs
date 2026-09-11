use super::*;
use crate::{Daemon, DaemonOptions, NotificationInput, NotificationTimeout};

fn icon_file(root: &Path, relative: &str, pixel: [u8; 4]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba(pixel))
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
}

fn theme(root: &Path, name: &str, metadata: &str) {
    let directory = root.join(name);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("index.theme"), metadata).unwrap();
}

async fn fixture_daemon(
    path: &Path,
    roots: Vec<PathBuf>,
    theme: &str,
    scale: u16,
) -> (Daemon, BannerUpdateReceiver) {
    let config = path.join("notify.kdl");
    std::fs::write(&config, format!("icons theme=\"{theme}\" scale={scale}\n")).unwrap();
    let (updates, mut receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    receiver.icon_roots = roots.into();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            path.join("history.db"),
            IcedLayerShellBannerPresenter { updates },
        )
        .with_configuration_file(config),
    )
    .await
    .unwrap();
    (daemon, receiver)
}

fn named_notification(name: &str) -> NotificationInput {
    NotificationInput {
        app_name: "Fixture".into(),
        summary: "Theme fixture".into(),
        app_icon: NotificationVisualReference::icon_name(name),
        expire_timeout: NotificationTimeout::Never,
        ..Default::default()
    }
}

async fn next_banner(
    stream: &mut (impl iced::futures::Stream<Item = BannerRuntimeInput> + Unpin),
) -> BannerView {
    let BannerRuntimeInput::Replace { presentation, .. } =
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("asset preparation stalled")
            .unwrap();
    presentation.banners.last().unwrap().clone()
}

fn rgba(pixel: [u8; 4]) -> Option<BannerVisual> {
    Some(BannerVisual::Rgba {
        width: 1,
        height: 1,
        pixels: pixel.into(),
    })
}

#[tokio::test]
async fn icon_index_symlinks_work_but_special_or_cyclic_indexes_stop_safely() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let index = "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories=d\n[d]\nSize=34\n";
    for kind in ["regular-link", "directory", "cycle", "fifo"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("icons");
        std::fs::create_dir_all(root.join("Birch")).unwrap();
        let path = root.join("Birch/index.theme");
        match kind {
            "regular-link" => {
                std::fs::write(temp.path().join("shared.theme"), index).unwrap();
                std::os::unix::fs::symlink(temp.path().join("shared.theme"), &path).unwrap();
            }
            "directory" => std::fs::create_dir(&path).unwrap(),
            "cycle" => std::os::unix::fs::symlink(&path, &path).unwrap(),
            _ => rustix::fs::mkfifoat(
                rustix::fs::CWD,
                &path,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap(),
        }
        icon_file(&root, "Birch/d/asset.png", [11, 0, 0, 255]);
        icon_file(&root, "asset.png", [99, 0, 0, 255]);
        let (daemon, receiver) = fixture_daemon(temp.path(), vec![root], "Birch", 1).await;
        daemon
            .receive_notification(named_notification("asset"))
            .await
            .unwrap();
        // A regressed blocking FIFO open must fail the test without hanging worker shutdown.
        let (cancel_rescue, cancelled) = mpsc::channel::<()>();
        let rescue = (kind == "fifo").then(|| {
            std::thread::spawn(move || {
                if matches!(
                    cancelled.recv_timeout(Duration::from_secs(2)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    let _writer = rustix::fs::open(
                        &path,
                        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
                        rustix::fs::Mode::empty(),
                    )
                    .unwrap();
                    std::thread::sleep(Duration::from_millis(100));
                }
            })
        });
        let mut stream = Box::pin(banner_updates(receiver));
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
        let elapsed = started.elapsed();
        drop(cancel_rescue);
        if let Some(rescue) = rescue {
            rescue.join().unwrap();
        }
        let BannerRuntimeInput::Replace { presentation, .. } = result.unwrap().unwrap();
        assert!(
            elapsed < Duration::from_secs(1),
            "{kind} blocked preparation"
        );
        assert_eq!(
            presentation.banners[0].app_icon,
            if kind == "regular-link" {
                rgba([11, 0, 0, 255])
            } else {
                None
            }
        );
    }
}

#[tokio::test]
async fn app_identity_fallback_never_overrides_explicit_icons_or_escapes_roots() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("icons");
    icon_file(&root, "org.example.Mail.desktop.png", [11, 0, 0, 255]);
    icon_file(&root, "org.example.Mail.png", [22, 0, 0, 255]);
    icon_file(temp.path(), "explicit.png", [33, 0, 0, 255]);
    let (daemon, receiver) = fixture_daemon(temp.path(), vec![root.clone()], "hicolor", 1).await;
    let input = NotificationInput {
        desktop_entry: Some("org.example.Mail.desktop".into()),
        app_icon: None,
        ..named_notification("unused")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    assert_eq!(
        next_banner(&mut stream).await.app_icon,
        rgba([11, 0, 0, 255])
    );
    std::fs::remove_file(root.join("org.example.Mail.desktop.png")).unwrap();
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..input.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        next_banner(&mut stream).await.app_icon,
        rgba([22, 0, 0, 255])
    );
    for (icon, expected) in [
        (
            NotificationVisualReference::local_path(
                temp.path().join("explicit.png").to_string_lossy(),
            )
            .unwrap(),
            rgba([33, 0, 0, 255]),
        ),
        (NotificationVisualReference::IconName("absent".into()), None),
        (
            NotificationVisualReference::IconName("../explicit".into()),
            None,
        ),
        (
            NotificationVisualReference::IconName("..\\explicit".into()),
            None,
        ),
        (
            NotificationVisualReference::IconName("nul\0name".into()),
            None,
        ),
    ] {
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(receipt.notification_id),
                app_icon: Some(icon),
                ..input.clone()
            })
            .await
            .unwrap();
        assert_eq!(next_banner(&mut stream).await.app_icon, expected);
    }
}

#[tokio::test]
async fn icon_metadata_and_candidate_work_limits_are_shared_by_the_frame() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    for (count, padded, available) in [
        (32, false, true),
        (33, false, false),
        (8, true, true),
        (9, true, false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("icons");
        for i in 0..count {
            let parent = if i + 1 < count {
                format!("Inherits=Theme{}\n", i + 1)
            } else {
                String::new()
            };
            let mut index = format!(
                "[Icon Theme]\nName=Fixture\nComment=Fixture\n{parent}Directories=d\n[d]\nSize=34\nType=Fixed\n"
            );
            if padded {
                index.push('#');
                index.extend(std::iter::repeat_n('x', 256 * 1024 - index.len()));
            }
            theme(&root, &format!("Theme{i}"), &index);
        }
        icon_file(
            &root,
            &format!("Theme{}/d/asset.png", count - 1),
            [11, 0, 0, 255],
        );
        icon_file(&root, "asset.png", [99, 0, 0, 255]);
        let (daemon, receiver) = fixture_daemon(temp.path(), vec![root], "Theme0", 1).await;
        daemon
            .receive_notification(named_notification("asset"))
            .await
            .unwrap();
        let mut stream = Box::pin(banner_updates(receiver));
        assert_eq!(
            next_banner(&mut stream).await.app_icon,
            if available {
                rgba([11, 0, 0, 255])
            } else {
                None
            },
            "themes={count}, padded={padded}"
        );
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("icons");
    let index = format!(
        "[Icon Theme]\nName=Fixture\nComment=Fixture\nDirectories={}\n{}",
        (0..1024)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join(","),
        (0..1024)
            .map(|i| format!("[d{i}]\nSize=34\nType=Fixed\n"))
            .collect::<String>()
    );
    theme(&root, "Birch", &index);
    icon_file(&root, "Birch/d0/action.png", [99, 0, 0, 255]);
    icon_file(&root, "body.png", [22, 0, 0, 255]);
    let roots = std::iter::once(root.clone())
        .chain((1..32).map(|i| temp.path().join(format!("root{i}"))))
        .collect();
    let (daemon, receiver) = fixture_daemon(temp.path(), roots, "Birch", 1).await;
    daemon
        .receive_notification(NotificationInput {
            actions: vec![
                NotificationAction::new("action", "Action")
                    .with_icon(NotificationVisualReference::icon_name("action").unwrap()),
            ],
            body: format!(
                "<img src=\"{}\" alt=\"body\"/>",
                root.join("body.png").display()
            ),
            ..named_notification("absent")
        })
        .await
        .unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let banner = next_banner(&mut stream).await;
    assert!(banner.app_icon.is_none());
    assert!(
        banner.actions[0].icon.is_none(),
        "a later slot cannot reset the exhausted 65,536-check budget"
    );
    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::image(rgba([22, 0, 0, 255]).unwrap())]
    );
}

#[tokio::test]
async fn unsafe_or_over_budget_icon_indexes_do_not_bypass_overrides_or_drop_notifications() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let base =
        "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories=d0\n[d0]\nSize=34\nType=Fixed\n";
    let mut exact_bytes = base.to_string();
    exact_bytes.push('#');
    exact_bytes.extend(std::iter::repeat_n('x', 256 * 1024 - exact_bytes.len()));
    let mut over_bytes = exact_bytes.clone();
    over_bytes.push('x');
    let directories = |count| {
        format!(
            "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories={}\n{}",
            (0..count)
                .map(|i| format!("d{i}"))
                .collect::<Vec<_>>()
                .join(","),
            (0..count)
                .map(|i| format!("[d{i}]\nSize=34\nType=Fixed\n"))
                .collect::<String>()
        )
    };
    let parents = |count| {
        base.replace(
            "Directories=d0",
            &format!(
                "Inherits={}\nDirectories=d0",
                (0..count)
                    .map(|i| format!("Parent{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
    };
    let mut cases = vec![
        ("exact bytes", exact_bytes.into_bytes(), 1, true),
        ("over bytes", over_bytes.into_bytes(), 1, false),
        ("exact directories", directories(1024).into_bytes(), 1, true),
        ("over directories", directories(1025).into_bytes(), 1, false),
        ("exact parents", parents(32).into_bytes(), 1, true),
        ("over parents", parents(33).into_bytes(), 1, false),
        ("exact roots", base.as_bytes().to_vec(), 32, true),
        ("over roots", base.as_bytes().to_vec(), 33, false),
        ("invalid UTF-8", vec![0xff], 1, false),
    ];
    for invalid in [
        "Size=0",
        "Size=-1",
        "Size=4294967295",
        "Size=34\nScale=0",
        "Size=34\nSize=34",
        "Size=34\nType=Unknown",
    ] {
        cases.push((
            "invalid directory",
            base.replace("Size=34\nType=Fixed", invalid).into_bytes(),
            1,
            false,
        ));
    }
    for invalid in ["../escape", "/absolute", "d0/../../escape", "d0\\escape"] {
        cases.push((
            "unsafe directory",
            base.replace("d0", invalid).into_bytes(),
            1,
            false,
        ));
    }
    for (label, bytes, count, available) in cases {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("icons");
        theme(&root, "Birch", base);
        std::fs::write(root.join("Birch/index.theme"), bytes).unwrap();
        icon_file(&root, "Birch/d0/asset.png", [11, 0, 0, 255]);
        icon_file(&root, "asset.png", [99, 0, 0, 255]);
        let roots = std::iter::once(root)
            .chain((1..count).map(|i| temp.path().join(format!("root{i}"))))
            .collect();
        let (daemon, receiver) = fixture_daemon(temp.path(), roots, "Birch", 1).await;
        daemon
            .receive_notification(named_notification("asset"))
            .await
            .unwrap();
        let mut stream = Box::pin(banner_updates(receiver));
        let banner = next_banner(&mut stream).await;
        assert_eq!(
            banner.app_icon,
            if available {
                rgba([11, 0, 0, 255])
            } else {
                None
            },
            "{label}"
        );
        assert_eq!(banner.summary, "Theme fixture");
        assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
        assert!(daemon.sender_events().await.is_empty());
    }
}

#[tokio::test]
async fn icon_sizes_follow_fixed_scalable_threshold_and_stable_tie_rules() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    for (scale, first, second, expected) in [
        (1, "Size=36\n", "Size=34\nType=Fixed\n", [11, 0, 0, 255]),
        (
            2,
            "Size=68\nType=Fixed\n",
            "Size=34\nScale=2\nType=Scalable\nMinSize=32\nMaxSize=36\n",
            [22, 0, 0, 255],
        ),
        (
            2,
            "Size=32\nScale=2\nType=Fixed\n",
            "Size=68\nType=Fixed\n",
            [22, 0, 0, 255],
        ),
        (
            2,
            "Size=20\nScale=2\nThreshold=2\n",
            "Size=40\nScale=2\nType=Fixed\n",
            [22, 0, 0, 255],
        ),
        (
            1,
            "Size=32\nType=Fixed\n",
            "Size=36\nType=Fixed\n",
            [11, 0, 0, 255],
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("icons");
        theme(
            &root,
            "Birch",
            &format!(
                "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories=first,second\n[first]\n{first}[second]\n{second}"
            ),
        );
        icon_file(&root, "Birch/first/asset.png", [11, 0, 0, 255]);
        icon_file(&root, "Birch/second/asset.png", [22, 0, 0, 255]);
        let (daemon, receiver) = fixture_daemon(temp.path(), vec![root], "Birch", scale).await;
        daemon
            .receive_notification(named_notification("asset"))
            .await
            .unwrap();
        let mut stream = Box::pin(banner_updates(receiver));
        assert_eq!(
            next_banner(&mut stream).await.app_icon,
            rgba(expected),
            "{first} / {second}"
        );
    }
}

#[tokio::test]
async fn icon_updates_refresh_first_index_roots_parents_and_previously_missing_files() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user");
    let system = temp.path().join("system");
    let index = "[Icon Theme]\nName=Fixture\nComment=Fixture\nDirectories=34/apps\n[34/apps]\nSize=34\nType=Fixed\n";
    theme(
        &user,
        "Birch",
        &format!("{index}Inherits=ignored-in-directory\n"),
    );
    theme(
        &system,
        "Birch",
        "[Icon Theme]\nName=Ignored\nComment=Fixture\nDirectories=wrong\n[wrong]\nSize=34\n",
    );
    icon_file(&system, "Birch/wrong/asset.png", [99, 0, 0, 255]);
    icon_file(&system, "Birch/34/apps/asset.png", [11, 0, 0, 255]);
    theme(
        &system,
        "Parent",
        "[Icon Theme]\nName=Parent\nComment=Fixture\nDirectories=34/apps\nInherits=Birch\n[34/apps]\nSize=34\n",
    );
    theme(&system, "hicolor", index);
    icon_file(&system, "Parent/34/apps/asset.png", [33, 0, 0, 255]);
    icon_file(&system, "hicolor/34/apps/asset.png", [44, 0, 0, 255]);
    icon_file(&system, "asset.png", [55, 0, 0, 255]);
    let (daemon, receiver) =
        fixture_daemon(temp.path(), vec![user.clone(), system.clone()], "Birch", 1).await;
    let input = named_notification("asset");
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let original = next_banner(&mut stream).await;
    assert_eq!(original.app_icon, rgba([11, 0, 0, 255]));
    // Higher-priority root adds the same declared file after the first frame.
    icon_file(&user, "Birch/34/apps/asset.png", [22, 0, 0, 255]);
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..input.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        next_banner(&mut stream).await.app_icon,
        rgba([22, 0, 0, 255])
    );
    // First index changes; the lower root's conflicting index must still be ignored.
    theme(
        &user,
        "Birch",
        "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories=\nInherits=hicolor,Parent\n",
    );
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..input.clone()
        })
        .await
        .unwrap();
    assert_eq!(
        next_banner(&mut stream).await.app_icon,
        rgba([33, 0, 0, 255])
    );
    for (remove, expected) in [
        ("Parent/34/apps/asset.png", Some([44, 0, 0, 255])),
        ("hicolor/34/apps/asset.png", Some([55, 0, 0, 255])),
        ("asset.png", None),
    ] {
        std::fs::remove_file(system.join(remove)).unwrap();
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(receipt.notification_id),
                ..input.clone()
            })
            .await
            .unwrap();
        assert_eq!(
            next_banner(&mut stream).await.app_icon,
            expected.and_then(rgba)
        );
    }
    icon_file(&system, "asset.png", [66, 0, 0, 255]);
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..input
        })
        .await
        .unwrap();
    assert_eq!(
        next_banner(&mut stream).await.app_icon,
        rgba([66, 0, 0, 255])
    );
    assert_eq!(
        original.app_icon,
        rgba([11, 0, 0, 255]),
        "old pixels remain stable"
    );
}

#[tokio::test]
async fn renderer_selects_nominal_size_scale_and_primary_image_role() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("icons");
    theme(
        &root,
        "Birch",
        "[Icon Theme]\nName=Birch\nComment=Fixture\nDirectories=68/apps\nScaledDirectories=34/apps,58/images,16/actions\n[68/apps]\nSize=68\nType=Fixed\n[34/apps]\nSize=34\nScale=2\nType=Fixed\n[58/images]\nSize=58\nScale=2\nType=Fixed\n[16/actions]\nSize=16\nScale=2\nType=Fixed\n",
    );
    for (directory, color) in [
        ("68/apps", [90, 0, 0, 255]),
        ("34/apps", [11, 22, 33, 255]),
        ("58/images", [44, 55, 66, 255]),
        ("16/actions", [77, 88, 99, 255]),
    ] {
        icon_file(&root, &format!("Birch/{directory}/asset.png"), color);
    }
    let (daemon, receiver) = fixture_daemon(temp.path(), vec![root], "Birch", 2).await;
    let input = NotificationInput {
        image_reference: NotificationVisualReference::icon_name("asset"),
        actions: vec![
            NotificationAction::new("asset", "Action")
                .with_icon(NotificationVisualReference::icon_name("asset").unwrap()),
        ],
        ..named_notification("asset")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let banner = next_banner(&mut stream).await;
    assert_eq!(banner.app_icon, rgba([11, 22, 33, 255]));
    assert_eq!(banner.image, rgba([44, 55, 66, 255]));
    assert_eq!(banner.actions[0].icon, rgba([77, 88, 99, 255]));
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            app_icon: NotificationVisualReference::icon_name("absent"),
            ..input
        })
        .await
        .unwrap();
    let banner = next_banner(&mut stream).await;
    assert!(banner.app_icon.is_none());
    assert_eq!(
        banner.image,
        rgba([11, 22, 33, 255]),
        "an image replacing the app icon uses the 34-pixel primary slot"
    );
}

#[tokio::test]
async fn local_visual_files_are_observed_at_preparation_not_notification_mutation() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let (daemon, receiver) = fixture_daemon(temp.path(), vec![], "hicolor", 1).await;
    let path = temp.path().join("late.png");
    let local = NotificationVisualReference::local_path(path.to_string_lossy()).unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_icon: Some(local.clone()),
            image_reference: Some(local.clone()),
            actions: vec![NotificationAction::new("reply", "Reply").with_icon(local)],
            body: format!(
                "Before <img src=\"{}\" alt=\"chart\"/> after",
                path.display()
            ),
            ..named_notification("unused")
        })
        .await
        .unwrap();
    icon_file(temp.path(), "late.png", [11, 22, 33, 255]);
    let mut stream = Box::pin(banner_updates(receiver));
    let banner = next_banner(&mut stream).await;
    assert_eq!(banner.app_icon, rgba([11, 22, 33, 255]));
    assert_eq!(banner.image, rgba([11, 22, 33, 255]));
    assert_eq!(banner.actions[0].icon, rgba([11, 22, 33, 255]));
    assert_eq!(
        banner.body_segments,
        vec![
            BannerBodySegment::plain_text("Before "),
            BannerBodySegment::image(rgba([11, 22, 33, 255]).unwrap()),
            BannerBodySegment::plain_text(" after")
        ]
    );
}

#[tokio::test]
async fn renderer_resolves_selected_icon_theme_before_parent_or_undeclared_directories() {
    let _fixture = images::IMAGE_FIXTURE.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("icons");
    theme(
        &root,
        "Birch",
        "[Icon Theme]\nName=Birch\nComment=Fixture\nInherits=Parent\nDirectories=48/apps\n[48/apps]\nSize=48\nType=Fixed\n",
    );
    theme(
        &root,
        "Parent",
        "[Icon Theme]\nName=Parent\nComment=Fixture\nDirectories=34/apps\n[34/apps]\nSize=34\nType=Fixed\n",
    );
    icon_file(
        &root,
        "Birch/48/apps/org.example.IconFixture.png",
        [11, 22, 33, 255],
    );
    icon_file(
        &root,
        "Birch/00/rogue/org.example.IconFixture.png",
        [90, 0, 0, 255],
    );
    icon_file(
        &root,
        "Parent/34/apps/org.example.IconFixture.png",
        [0, 90, 0, 255],
    );
    let config = temp.path().join("notify.kdl");
    std::fs::write(&config, "icons theme=\"Birch\"\n").unwrap();
    let (updates, mut receiver) = BannerUpdateSender::channel(tokio::runtime::Handle::current());
    receiver.icon_roots = vec![root].into();
    let presenter = IcedLayerShellBannerPresenter { updates };
    let daemon = Daemon::open(
        DaemonOptions::graphical(temp.path().join("history.db"), presenter)
            .with_configuration_file(config),
    )
    .await
    .unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_name: "Fixture".into(),
            summary: "Named icon".into(),
            app_icon: NotificationVisualReference::icon_name("org.example.IconFixture"),
            expire_timeout: NotificationTimeout::Never,
            ..Default::default()
        })
        .await
        .unwrap();
    let mut stream = Box::pin(banner_updates(receiver));
    let BannerRuntimeInput::Replace { presentation, .. } =
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        presentation.banners[0].app_icon,
        Some(BannerVisual::Rgba {
            width: 1,
            height: 1,
            pixels: vec![11, 22, 33, 255],
        })
    );
}
