use super::*;
use std::{
    io::{Cursor, Read},
    path::Path,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn png(width: u32, height: u32) -> Vec<u8> {
    let pixels = image::RgbaImage::from_pixel(width, height, image::Rgba([170, 23, 42, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn uri(path: &Path) -> String {
    url::Url::from_file_path(path).unwrap().to_string()
}

async fn request_headers(stream: &mut tokio::net::TcpStream) -> String {
    let mut headers = Vec::new();
    loop {
        let mut bytes = [0; 1024];
        let count = stream.read(&mut bytes).await.unwrap();
        assert!(
            count > 0,
            "client stopped before completing request headers"
        );
        headers.extend_from_slice(&bytes[..count]);
        assert!(headers.len() <= 8192);
        if headers.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(headers).unwrap()
}

#[tokio::test]
async fn frozen_image_is_full_resolution_sealed_and_independent_of_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wallpaper.png");
    std::fs::write(&path, png(1024, 600)).unwrap();
    let images = Images::new();
    let (_sender, cancelled) = watch::channel(false);
    let mut frozen = images.prepare(&uri(&path), &cancelled).await.unwrap();
    assert_eq!((frozen.width, frozen.height), (1024, 600));
    std::fs::write(&path, b"changed after review preparation").unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut bytes = Vec::new();
    frozen.file.read_to_end(&mut bytes).unwrap();
    let actual = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(actual.dimensions(), (1024, 600));
    assert_eq!(actual.get_pixel(500, 300).0, [170, 23, 42, 255]);
    assert!(frozen.file.write_all(b"replacement").is_err());
    assert!(frozen.file.set_len(0).is_err());
    let (width, height, pixels) = frozen.preview.clone().pixels().unwrap();
    assert_eq!(width, SIDE);
    assert_eq!(
        height, 225,
        "the review must preserve the source aspect ratio"
    );
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    let input = DialogInput {
        app_id: "app".into(),
        parent_window: "".into(),
        destination: crate::wallpaper::Destination::Both,
        width: frozen.width,
        height: frozen.height,
        preview: frozen.preview.clone(),
    };
    assert!(serde_json::to_vec(&input).unwrap().len() < crate::process::MAX_MESSAGE_BYTES);
}

#[tokio::test]
async fn admission_covers_the_whole_review_and_rejects_bad_local_sources() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.png");
    std::fs::write(&path, png(12, 9)).unwrap();
    let images = Images::new();
    let (_sender, cancelled) = watch::channel(false);
    let first = images.prepare(&uri(&path), &cancelled).await.unwrap();
    let second = images.prepare(&uri(&path), &cancelled).await.unwrap();
    assert!(images.prepare(&uri(&path), &cancelled).await.is_err());
    drop(first);
    assert!(images.prepare(&uri(&path), &cancelled).await.is_ok());
    drop(second);
    let pipe = directory.path().join("pipe");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &pipe,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    for source in [
        uri(&pipe),
        uri(directory.path()),
        "data:image/png,ignored".into(),
        "ftp://localhost/unsupported".into(),
        "https://user:secret@example.invalid/image".into(),
        "file:///missing?query".into(),
        "file://other-host/unsupported".into(),
    ] {
        let error = images
            .prepare(&source, &cancelled)
            .await
            .err()
            .expect("invalid source");
        assert!(!error.to_string().contains("secret"));
    }
    std::fs::write(&path, b"not an image").unwrap();
    assert!(images.prepare(&uri(&path), &cancelled).await.is_err());
    std::fs::File::create(&path)
        .unwrap()
        .set_len(INPUT_BYTES as u64 + 1)
        .unwrap();
    assert!(images.prepare(&uri(&path), &cancelled).await.is_err());
    std::fs::write(&path, png(16_385, 1)).unwrap();
    assert!(images.prepare(&uri(&path), &cancelled).await.is_err());
    assert_eq!(images.slots.available_permits(), 2);
}

fn network_images() -> Images {
    Images {
        slots: Arc::new(Semaphore::new(2)),
        client: Some(client_builder().no_proxy().build().unwrap()),
    }
}

#[tokio::test]
async fn remote_streaming_redirects_and_advertised_size_are_bounded() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = requests.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let request = request_headers(&mut stream).await;
            let body = png(800, 480);
            let response = if request.starts_with("GET /loop ") {
                "HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n".to_owned()
            } else if request.starts_with("GET /redirect ") {
                "HTTP/1.1 302 Found\r\nLocation: /image\r\nContent-Length: 0\r\n\r\n".to_owned()
            } else if request.starts_with("GET /oversize ") {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    INPUT_BYTES + 1
                )
            } else if request.starts_with("GET /scheme ") {
                "HTTP/1.1 302 Found\r\nLocation: file:///etc/passwd\r\nContent-Length: 0\r\n\r\n"
                    .to_owned()
            } else if request.starts_with("GET /encoding ") {
                "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n".to_owned()
            } else {
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len())
            };
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        }
    });
    let (_sender, cancelled) = watch::channel(false);
    let images = network_images();
    let frozen = images
        .prepare(&format!("http://{address}/redirect"), &cancelled)
        .await
        .unwrap();
    assert_eq!((frozen.width, frozen.height), (800, 480));
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
    for route in ["oversize", "scheme", "encoding"] {
        assert!(
            images
                .prepare(&format!("http://{address}/{route}"), &cancelled)
                .await
                .is_err()
        );
    }
    let before = requests.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        images
            .prepare(&format!("http://{address}/loop"), &cancelled)
            .await
            .is_err()
    );
    assert_eq!(
        requests.load(std::sync::atomic::Ordering::SeqCst) - before,
        6
    );
    server.abort();
}

#[tokio::test]
async fn cancellation_stops_a_stalled_download_and_releases_admission() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (received, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = request_headers(&mut stream).await;
        received.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let images = Arc::new(network_images());
    let (sender, cancelled) = watch::channel(false);
    let source = images.clone();
    let preparation = tokio::spawn(async move {
        source
            .prepare(&format!("http://{address}/stall"), &cancelled)
            .await
    });
    ready.await.unwrap();
    sender.send_replace(true);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), preparation)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(images.slots.available_permits(), 2);
    server.abort();
}

#[test]
fn preview_transport_cannot_exceed_the_helper_frame_or_supply_unchecked_pixels() {
    let preview = Preview {
        width: SIDE,
        height: SIDE,
        rgba: STANDARD.encode(vec![255; (SIDE * SIDE * 4) as usize]),
    };
    let input = DialogInput {
        app_id: "a".repeat(4096),
        parent_window: "b".repeat(4096),
        destination: crate::wallpaper::Destination::Both,
        width: 16_384,
        height: 4096,
        preview,
    };
    assert!(serde_json::to_vec(&input).unwrap().len() < crate::process::MAX_MESSAGE_BYTES);
    assert!(input.preview.clone().pixels().is_ok());
    assert!(
        Preview {
            width: SIDE + 1,
            ..input.preview.clone()
        }
        .pixels()
        .is_err()
    );
    assert!(
        Preview {
            width: 2,
            height: 2,
            rgba: "AAAA".into()
        }
        .pixels()
        .is_err()
    );
}

#[tokio::test]
async fn chunked_download_cannot_bypass_the_byte_limit() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let _ = request_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        let block = vec![b'x'; 64 * 1024];
        for _ in 0..=(INPUT_BYTES / block.len()) {
            if stream.write_all(b"10000\r\n").await.is_err()
                || stream.write_all(&block).await.is_err()
                || stream.write_all(b"\r\n").await.is_err()
            {
                return;
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n").await;
    });
    let (_sender, cancelled) = watch::channel(false);
    let error = network_images()
        .prepare(&format!("http://{address}/chunked"), &cancelled)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("download too large"), "{error}");
    server.abort();
}

#[tokio::test]
async fn resolver_admission_is_independent_of_cancelled_request_admission() {
    use reqwest::dns::Resolve;
    let resolver = BoundedDns {
        slots: Arc::new(Semaphore::new(2)),
    };
    let first = resolver.slots.clone().try_acquire_owned().unwrap();
    let second = resolver.slots.clone().try_acquire_owned().unwrap();
    assert!(
        resolver
            .resolve("localhost".parse().unwrap())
            .await
            .is_err()
    );
    drop(first);
    let addresses: Vec<_> = resolver
        .resolve("127.0.0.1".parse().unwrap())
        .await
        .unwrap()
        .collect();
    assert_eq!(
        addresses,
        ["127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap()]
    );
    drop(second);
    assert_eq!(resolver.slots.available_permits(), 2);
}

#[tokio::test]
async fn common_picture_formats_have_the_same_frozen_output_contract() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("picture-without-an-extension");
    let images = Images::new();
    let (_sender, cancelled) = watch::channel(false);
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::Gif,
        image::ImageFormat::WebP,
    ] {
        let pixels = image::RgbImage::from_pixel(64, 32, image::Rgb([170, 23, 42]));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut encoded, format)
            .unwrap();
        std::fs::write(&source, encoded.into_inner()).unwrap();
        let mut frozen = images.prepare(&uri(&source), &cancelled).await.unwrap();
        let mut bytes = Vec::new();
        frozen.file.read_to_end(&mut bytes).unwrap();
        assert_eq!(
            image::guess_format(&bytes).unwrap(),
            image::ImageFormat::Png
        );
        let image = image::load_from_memory(&bytes).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (64, 32));
        for (actual, wanted) in image
            .get_pixel(20, 10)
            .0
            .into_iter()
            .zip([170u8, 23, 42, 255])
        {
            assert!(
                actual.abs_diff(wanted) <= 4,
                "{format:?}: {actual} != {wanted}"
            );
        }
    }
}
