use super::*;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

#[test]
fn configured_identity_does_not_fall_back_to_unselected_account_data() {
    let fields = Fields {
        id: Some("configured".into()),
        name: Some(String::new()),
        image: None,
    };
    assert_eq!(
        identity(Some(&fields), || panic!(
            "complete service data needs no NSS query"
        )),
        ("configured".into(), String::new())
    );
    assert_eq!(
        identity(None, || ("fallback".into(), "Fallback name".into())),
        ("fallback".into(), "Fallback name".into())
    );
    let partial = Fields {
        id: Some("configured".into()),
        ..Default::default()
    };
    assert_eq!(
        identity(Some(&partial), || (
            "fallback".into(),
            "Fallback name".into()
        )),
        ("configured".into(), "Fallback name".into())
    );
}

struct User;
#[zbus::interface(name = "org.freedesktop.Accounts.User")]
impl User {
    #[zbus(property)]
    fn user_name(&self) -> &str {
        "service-user"
    }
    #[zbus(property)]
    fn real_name(&self) -> &str {
        "Service name"
    }
    #[zbus(property)]
    fn icon_file(&self) -> &str {
        "/tmp/service-avatar.png"
    }
}
struct Bus(Child);
impl Drop for Bus {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

#[tokio::test]
async fn account_service_reads_only_the_selected_uid_and_failed_lookup_can_fall_back() {
    let mut bus = Bus(Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap());
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let server = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .serve_at("/org/freedesktop/Accounts/User731", User)
        .unwrap()
        .name("org.freedesktop.Accounts")
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::connection::Builder::address(address.trim())
        .unwrap()
        .build()
        .await
        .unwrap();
    let fields = service_fields(&client, 731).await.unwrap();
    assert_eq!(fields.id.as_deref(), Some("service-user"));
    assert_eq!(fields.name.as_deref(), Some("Service name"));
    assert_eq!(fields.image, Some(PathBuf::from("/tmp/service-avatar.png")));
    assert!(service_fields(&client, 732).await.is_err());
    server.close().await.unwrap();
    assert!(service_fields(&client, 731).await.is_err());
}

#[test]
fn avatar_preparation_is_bounded_read_only_and_removes_original_metadata() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(std::ffi::OsString::from_vec(b"raw-\xff.png".to_vec()));
    let pixels = vec![17, 90, 210, 255, 40, 120, 20, 255];
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&pixels, 2, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    let marker = b"<svg>private location metadata</svg>";
    bytes.extend(marker);
    std::fs::write(&path, &bytes).unwrap();
    let avatar = prepare_avatar(&path, canopa_theme::Rgb(0, 0, 0)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(avatar.raster.pixels, pixels);
    assert!(!avatar.png.windows(marker.len()).any(|part| part == marker));
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        image::load_from_memory(&avatar.png)
            .unwrap()
            .into_rgba8()
            .into_raw(),
        pixels
    );
    let svg = root.path().join("avatar.svg");
    std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="red"/></svg>"#).unwrap();
    let avatar = prepare_avatar(&svg, canopa_theme::Rgb(0, 0, 0)).unwrap();
    assert_eq!((avatar.raster.width, avatar.raster.height), (256, 128));
    assert!(avatar.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let large = root.path().join("large.png");
    std::fs::File::create(&large)
        .unwrap()
        .set_len(SOURCE_BYTES as u64 + 1)
        .unwrap();
    assert!(prepare_avatar(&large, canopa_theme::Rgb(0, 0, 0)).is_err());
}
