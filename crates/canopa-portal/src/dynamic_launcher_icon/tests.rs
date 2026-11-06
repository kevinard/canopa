use super::*;
use std::io::Cursor;

fn encoded(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
        width,
        height,
        image::Rgb([20, 80, 190]),
    ))
    .write_to(&mut bytes, format)
    .unwrap();
    bytes.into_inner()
}

#[test]
fn png_jpeg_and_svg_get_small_owned_previews_without_changing_originals() {
    for bytes in [
        encoded(256, 128, image::ImageFormat::Png),
        encoded(256, 128, image::ImageFormat::Jpeg),
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="128"><rect width="256" height="128" fill="#1450be"/></svg>"##.to_vec(),
    ] {
        let before = bytes.clone();
        let preview = decode(&bytes, &|| false).unwrap();
        assert!(preview.valid());
        assert_eq!((preview.width, preview.height), (128, 64));
        assert_eq!(bytes, before);
        assert_eq!(preview.pixels[3], 255);
        assert!(preview.pixels[2] > preview.pixels[0]);
    }
}

#[test]
fn malformed_excessive_and_unsupported_icons_fail_closed() {
    for bytes in [
        b"not an image".to_vec(),
        vec![0; ICON_BYTES + 1],
        encoded(513, 1, image::ImageFormat::Png),
        b"GIF89a\x01\0\x01\0".to_vec(),
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="4097" height="1"/>"#.to_vec(),
    ] {
        assert!(decode(&bytes, &|| false).is_none());
    }
    assert!(decode(&encoded(32, 32, image::ImageFormat::Png), &|| true).is_none());
}

#[test]
fn svg_external_resources_are_unavailable_and_preview_shape_is_checked() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("private.png");
    std::fs::write(&file, encoded(32, 32, image::ImageFormat::Png)).unwrap();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><image href="{}" width="32" height="32"/></svg>"#,
        file.display()
    );
    let preview = decode(svg.as_bytes(), &|| false).unwrap();
    assert!(preview.pixels.chunks_exact(4).all(|pixel| pixel[3] == 0));
    assert!(
        !Preview {
            width: 129,
            height: 1,
            pixels: vec![0; 129 * 4]
        }
        .valid()
    );
    assert!(
        !Preview {
            width: 1,
            height: 1,
            pixels: vec![0; 3]
        }
        .valid()
    );
}

#[tokio::test]
async fn already_cancelled_preparation_cannot_produce_a_dialog() {
    let bytes = encoded(32, 32, image::ImageFormat::Png);
    let icon = LauncherIcon::parse(
        zbus::zvariant::Value::from(("bytes", zbus::zvariant::Value::from(bytes)))
            .try_into_owned()
            .unwrap(),
    )
    .unwrap();
    let (_sender, receiver) = watch::channel(true);
    assert!(prepare(icon, &receiver).await.is_err());
}
