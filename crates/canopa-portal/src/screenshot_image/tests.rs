use super::*;

fn frame(name: &str, x: i32, width: u32, logical_width: u32, height: u32, offset: u64) -> Frame {
    Frame {
        name: name.into(),
        logical: LogicalRect {
            x,
            y: 0,
            width: logical_width,
            height,
        },
        width,
        height,
        offset,
    }
}
fn control() -> Control {
    Control {
        stop: Arc::new(AtomicBool::new(false)),
        end: Instant::now() + Duration::from_secs(30),
    }
}

#[test]
fn accepted_pngs_are_private_unique_and_retained_when_recent_cache_is_full() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let cache = Cache::new(root.path().join("cache"));
    let png = cache.publish(&[12, 34, 56, 255], 1, 1, &control()).unwrap();
    let path = url::Url::parse(&png).unwrap().to_file_path().unwrap();
    assert_eq!(
        image::open(&path).unwrap().into_rgba8().get_pixel(0, 0).0,
        [12, 34, 56, 255]
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o400
    );
    let second = cache.publish(&[12, 34, 56, 255], 1, 1, &control()).unwrap();
    assert_ne!(png, second);
    let directory = path.parent().unwrap();
    for index in 0..126 {
        let file = directory.join(format!("{index:024}.png"));
        std::fs::write(&file, b"recent private image").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)).unwrap();
    }
    assert!(cache.publish(&[1, 2, 3, 255], 1, 1, &control()).is_err());
    assert!(path.exists());
    let old = directory.join(format!("{:024}.png", 0));
    File::open(&old)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(25 * 60 * 60))
        .unwrap();
    assert!(cache.publish(&[1, 2, 3, 255], 1, 1, &control()).is_ok());
    assert!(!old.exists());
    assert!(path.exists());
}

#[test]
fn cache_refuses_symlink_redirection_and_unowned_looking_entries() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("cache");
    std::fs::create_dir_all(home.join("canopa/portal")).unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    symlink(&outside, home.join("canopa/portal/screenshots")).unwrap();
    let cache = Cache::new(home.clone());
    assert!(cache.publish(&[1, 2, 3, 255], 1, 1, &control()).is_err());
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    std::fs::remove_file(home.join("canopa/portal/screenshots")).unwrap();
    let uri = cache.publish(&[1, 2, 3, 255], 1, 1, &control()).unwrap();
    let path = url::Url::parse(&uri).unwrap().to_file_path().unwrap();
    std::fs::write(
        path.parent().unwrap().join("éééééééééééé.png"),
        b"unrelated",
    )
    .unwrap();
    assert!(cache.publish(&[1, 2, 3, 255], 1, 1, &control()).is_err());
    assert!(path.exists());
}

#[tokio::test]
async fn sealed_source_survives_exec_without_relaxing_parent_descriptor_flags() {
    use std::os::fd::AsFd;
    let original = rustix::fs::memfd_create(
        c"screenshot-test",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )
    .unwrap();
    let mut file = File::from(rustix::io::fcntl_dupfd_cloexec(&original, 3).unwrap());
    file.write_all(&[10, 20, 30, 255, 40, 50, 60, 255]).unwrap();
    let source = Source {
        descriptor: file.as_raw_fd(),
        bytes: 8,
        frames: vec![frame("source", 0, 2, 2, 1, 0)],
    };
    assert!(source.open().is_err());
    rustix::fs::fcntl_add_seals(
        &file,
        rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::SEAL,
    )
    .unwrap();
    assert!(source.open().is_ok());
    let process = crate::process::DialogProcess {
        executable: "/usr/bin/python3".into(),
        arguments: vec![
            "-c".into(),
            r#"import json,os,sys
d=json.load(sys.stdin);f=d['descriptor']
try:
 os.pwrite(f,b'changed',0); protected=False
except OSError:
 protected=True
print(json.dumps({'protected':protected,'bytes':list(os.pread(f,d['bytes'],0))}))
"#
            .into(),
        ],
    };
    let (_sender, cancelled) = watch::channel(false);
    let result: serde_json::Value = process
        .exchange_inheriting(
            "--screenshot-dialog",
            source,
            cancelled,
            None,
            &[file.as_fd()],
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result["protected"], true);
    assert_eq!(
        result["bytes"],
        serde_json::json!([10, 20, 30, 255, 40, 50, 60, 255])
    );
    assert!(
        rustix::io::fcntl_getfd(&file)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
}

#[test]
fn single_output_keeps_every_physical_pixel_despite_logical_rounding() {
    let mut file = tempfile::tempfile().unwrap();
    let mut metadata = frame("display", 0, 1280, 853, 800, 0);
    metadata.logical.height = 533;
    let source = Source {
        descriptor: file.as_raw_fd(),
        bytes: 1280 * 800 * 4,
        frames: vec![metadata],
    };
    let pixels: Vec<u8> = (0..1280 * 800)
        .flat_map(|v| [(v % 255) as u8, 17, 200, 255])
        .collect();
    file.write_all(&pixels).unwrap();
    let canvas = Canvas::new(&source.frames).unwrap();
    assert_eq!((canvas.width, canvas.height), (1280, 800));
    assert_eq!(
        source
            .compose(&file, &canvas, canvas.full(), &control())
            .unwrap(),
        pixels
    );
}
#[test]
fn mixed_scale_outputs_keep_negative_origins_gaps_and_source_pixel_identity() {
    let source = Source {
        descriptor: 9,
        bytes: 40,
        frames: vec![frame("left", -3, 2, 2, 1, 0), frame("right", 0, 4, 2, 2, 8)],
    };
    source.validate().unwrap();
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&[10, 20, 30, 255, 40, 50, 60, 255]).unwrap();
    file.write_all(&[100, 110, 120, 255].repeat(8)).unwrap();
    let canvas = Canvas::new(&source.frames).unwrap();
    assert_eq!((canvas.width, canvas.height), (10, 4));
    assert_eq!(canvas.pixel_at(&source.frames, 1.5, 0.5), Some((0, 0, 0)));
    assert_eq!(canvas.pixel_at(&source.frames, 4.5, 0.5), None);
    assert_eq!(canvas.pixel_at(&source.frames, 7.5, 0.5), Some((1, 1, 0)));
    let pixels = source
        .compose(&file, &canvas, canvas.full(), &control())
        .unwrap();
    assert_eq!(&pixels[4 * 4..4 * 5], &[0; 4]);
    assert_eq!(source.pixel(&file, 1, 3, 1).unwrap(), [100, 110, 120, 255]);
    assert!(
        source
            .compose(
                &file,
                &canvas,
                Rect {
                    x: 4,
                    y: 0,
                    width: 2,
                    height: 1
                },
                &control()
            )
            .is_err()
    );
}
#[test]
fn malformed_source_offsets_and_selection_overflows_fail_before_allocation() {
    let mut source = Source {
        descriptor: 0,
        bytes: 8,
        frames: vec![frame("one", 0, 2, 2, 1, 0)],
    };
    source.validate().unwrap();
    assert!(source.open().is_err());
    source.frames[0].offset = 1;
    assert!(source.validate().is_err());
    let canvas = Canvas::new(&[frame("one", 0, 2, 2, 1, 0)]).unwrap();
    assert!(
        Rect {
            x: u32::MAX,
            y: 0,
            width: 1,
            height: 1
        }
        .validate(&canvas)
        .is_err()
    );
    assert!(
        Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 1
        }
        .validate(&canvas)
        .is_err()
    );
}
