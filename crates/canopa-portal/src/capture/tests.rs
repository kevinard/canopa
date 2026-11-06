use super::*;
use std::io::Write;
use tokio::{io::AsyncReadExt, net::UnixListener};
use wayland_client::{
    WEnum,
    protocol::{wl_output::Transform, wl_shm::Format},
};

#[test]
fn display_resolution_does_not_guess_or_reuse_an_inherited_connection() {
    let absolute = || Some(PathBuf::from("/private/wayland-1"));
    let relative = || Some(PathBuf::from("wayland-1"));
    let runtime = || Some(PathBuf::from("/private"));
    assert_eq!(display_socket(absolute(), None, false), absolute());
    assert_eq!(display_socket(relative(), runtime(), false), absolute());
    assert_eq!(display_socket(None, runtime(), false), None);
    assert_eq!(display_socket(relative(), None, false), None);
    assert_eq!(
        display_socket(relative(), Some("relative".into()), false),
        None
    );
    assert_eq!(display_socket(Some("".into()), runtime(), false), None);
    assert_eq!(display_socket(absolute(), runtime(), true), None);
}

#[test]
fn buffer_limits_reject_untrusted_sizes_formats_and_stride() {
    for (width, height, stride) in [
        (0, 1, 4),
        (1, 0, 4),
        (MAX_SIDE + 1, 1, (MAX_SIDE + 1) * 4),
        (2, 1, 7),
        (1, 1, 70_000),
        (MAX_SIDE, MAX_SIDE, MAX_SIDE * 4),
        (1, u32::MAX, 4),
    ] {
        assert!(
            wayland::Layout::checked(WEnum::Value(Format::Xrgb8888), width, height, stride)
                .is_err()
        );
    }
    assert!(wayland::Layout::checked(WEnum::Unknown(u32::MAX), 1, 1, 4).is_err());
    assert!(wayland::Layout::checked(WEnum::Value(Format::Rgb565), 1, 1, 4).is_err());
}

#[test]
fn native_endian_pixels_skip_padding_and_normalize_alpha_and_y_inversion() {
    let mut file = tempfile::tempfile().unwrap();
    for word in [
        0xff112233_u32,
        0xff445566,
        0xdeadbeef,
        0xff778899,
        0xffaabbcc,
        0xcafebabe,
    ] {
        file.write_all(&word.to_ne_bytes()).unwrap();
    }
    let layout = wayland::Layout::checked(WEnum::Value(Format::Xrgb8888), 2, 2, 12).unwrap();
    let stop = AtomicBool::new(false);
    let image = layout
        .pixels(
            &file,
            true,
            Transform::Normal,
            &stop,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    assert_eq!(image.get_pixel(0, 0).0, [0x77, 0x88, 0x99, 255]);
    assert_eq!(image.get_pixel(1, 1).0, [0x44, 0x55, 0x66, 255]);
    let mut alpha = tempfile::tempfile().unwrap();
    alpha.write_all(&0x80402010_u32.to_ne_bytes()).unwrap();
    let layout = wayland::Layout::checked(WEnum::Value(Format::Abgr8888), 1, 1, 4).unwrap();
    assert_eq!(
        layout
            .pixels(
                &alpha,
                false,
                Transform::Normal,
                &stop,
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap()
            .get_pixel(0, 0)
            .0,
        [32, 64, 128, 128]
    );
}

#[test]
fn all_output_transforms_preserve_the_pixel_grid_and_cancel_during_conversion() {
    let mut file = tempfile::tempfile().unwrap();
    for word in 1_u32..=6 {
        file.write_all(&word.to_ne_bytes()).unwrap();
    }
    let layout = wayland::Layout::checked(WEnum::Value(Format::Xrgb8888), 3, 2, 12).unwrap();
    let stop = AtomicBool::new(false);
    for (transform, size, expected) in [
        (Transform::Normal, (3, 2), vec![1, 2, 3, 4, 5, 6]),
        (Transform::_90, (2, 3), vec![4, 1, 5, 2, 6, 3]),
        (Transform::_180, (3, 2), vec![6, 5, 4, 3, 2, 1]),
        (Transform::_270, (2, 3), vec![3, 6, 2, 5, 1, 4]),
        (Transform::Flipped, (3, 2), vec![3, 2, 1, 6, 5, 4]),
        (Transform::Flipped90, (2, 3), vec![1, 4, 2, 5, 3, 6]),
        (Transform::Flipped180, (3, 2), vec![4, 5, 6, 1, 2, 3]),
        (Transform::Flipped270, (2, 3), vec![6, 3, 5, 2, 4, 1]),
    ] {
        let image = layout
            .pixels(
                &file,
                false,
                transform,
                &stop,
                Instant::now() + Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(image.dimensions(), size);
        assert_eq!(image.pixels().map(|p| p[2]).collect::<Vec<_>>(), expected);
    }
    stop.store(true, Ordering::Relaxed);
    assert!(
        layout
            .pixels(
                &file,
                false,
                Transform::Normal,
                &stop,
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
    );
}

async fn wait_slots(captures: &Captures) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while captures.slots.available_permits() != 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cancelled worker must retire");
}

#[tokio::test]
async fn stalled_wayland_handshake_is_cancellable_and_admission_survives_future_drop() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("display");
    let listener = UnixListener::bind(&path).unwrap();
    let captures = Captures::new(Some(path));
    let (sender, cancelled) = watch::channel(false);
    let first = tokio::spawn({
        let captures = captures.clone();
        let cancelled = cancelled.clone();
        async move { captures.capture(cancelled).await }
    });
    let second = tokio::spawn({
        let captures = captures.clone();
        let cancelled = cancelled.clone();
        async move { captures.capture(cancelled).await }
    });
    let mut streams = Vec::new();
    for _ in 0..2 {
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut request = [0; 8];
        tokio::time::timeout(Duration::from_secs(2), stream.read_exact(&mut request))
            .await
            .unwrap()
            .unwrap();
        streams.push(stream);
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while captures.slots.available_permits() > 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        captures.capture(cancelled).await.err().unwrap().to_string(),
        "Screenshot busy"
    );
    first.abort();
    sender.send(true).unwrap();
    assert!(second.await.unwrap().is_err());
    wait_slots(&captures).await;
    drop((listener, streams));
}

#[tokio::test]
async fn stalled_handshake_deadline_and_disconnected_compositor_release_admission() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("display");
    let listener = UnixListener::bind(&path).unwrap();
    let mut captures = Captures::new(Some(path));
    captures.deadline = Duration::from_millis(500);
    let (_sender, cancelled) = watch::channel(false);
    let task = tokio::spawn({
        let captures = captures.clone();
        let cancelled = cancelled.clone();
        async move { captures.capture(cancelled).await }
    });
    let (stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        task.await.unwrap().err().unwrap().to_string(),
        "Screenshot timed out"
    );
    wait_slots(&captures).await;
    drop(stream);
    let listener = UnixListener::bind(directory.path().join("closed")).unwrap();
    let captures = Captures::new(Some(directory.path().join("closed")));
    let close = tokio::spawn(async move {
        drop(
            tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap(),
        );
    });
    assert!(captures.capture(cancelled).await.is_err());
    close.await.unwrap();
    wait_slots(&captures).await;
}
