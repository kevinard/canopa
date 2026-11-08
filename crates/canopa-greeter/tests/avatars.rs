use canopa_config::Request;
use canopa_greeter::config::Config;
use image::{ImageFormat, Rgba, RgbaImage};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

fn image(path: &Path, width: u32, height: u32, color: [u8; 4]) {
    RgbaImage::from_pixel(width, height, Rgba(color))
        .save_with_format(path, ImageFormat::Png)
        .unwrap();
}
fn decode(paths: &[PathBuf]) -> Vec<u8> {
    let mut command = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
    command
        .arg("internal-avatar")
        .arg("--")
        .args(paths)
        .timeout(Duration::from_secs(3));
    command.assert().success().get_output().stdout.clone()
}

#[test]
fn decoder_handles_extensionless_sources_precedence_crop_alpha_and_bad_files() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let broken = root.path().join("broken");
    fs::write(&broken, b"invalid image").unwrap();
    let first = root.path().join("extensionless");
    let second = root.path().join("second.png");
    RgbaImage::from_fn(6, 2, |x, _| {
        if (2..4).contains(&x) {
            Rgba([255, 0, 0, 128])
        } else {
            Rgba([0, 255, 0, 255])
        }
    })
    .save_with_format(&first, ImageFormat::Png)
    .unwrap();
    image(&second, 16, 16, [0, 0, 255, 255]);
    let bytes = decode(&[missing, broken.clone(), first.clone()]);
    assert_eq!(bytes.len(), 128 * 128 * 4);
    assert!(bytes.chunks_exact(4).all(|p| p == [255, 0, 0, 128]));
    assert!(
        decode(&[first, second.clone()])
            .chunks_exact(4)
            .all(|p| p == [255, 0, 0, 128])
    );
    assert!(decode(std::slice::from_ref(&broken)).is_empty());
    let big = root.path().join("big");
    image(&big, 1, 1, [255, 0, 0, 255]);
    {
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(&big)
            .unwrap()
            .write_all(&vec![0; 2 * 1024 * 1024])
            .unwrap();
    }
    assert!(image::load_from_memory(&fs::read(&big).unwrap()).is_ok());
    let dimensions = root.path().join("dimensions");
    image(&dimensions, 2049, 1, [0, 0, 0, 255]);
    let fallback = decode(&[big, dimensions, second]);
    assert_eq!(fallback.len(), 128 * 128 * 4);
    assert!(fallback.chunks_exact(4).all(|p| p == [0, 0, 255, 255]));
    let fifo = root.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(decode(&[fifo, broken]).is_empty());
    let literal = root.path().join("$(touch must-not-run)");
    image(&literal, 2, 2, [0, 255, 0, 255]);
    assert!(!decode(&[literal]).is_empty());
    assert!(!root.path().join("must-not-run").exists());
}

#[test]
fn avatar_configuration_does_not_require_files_and_can_be_disabled() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("greeter.kdl");
    fs::write(&config, "schema-version 1\ngreeter { users \"alice\"; avatar-dir \"faces\"; user-avatar \"alice\" \"alice.png\"; }\n").unwrap();
    let (cfg, _) = Config::load(&Request::common(&config)).unwrap();
    assert!(cfg.avatars_enabled());
    assert_eq!(cfg.avatars.directory, root.path().join("faces"));
    assert_eq!(cfg.avatars.users["alice"], root.path().join("alice.png"));
    fs::write(&config, "schema-version 1\ngreeter { user-avatars #false; view { widgets \"avatar\"; avatar \"missing.png\"; } }\n").unwrap();
    let (cfg, _) = Config::load(&Request::common(&config)).unwrap();
    assert!(!cfg.avatars_enabled());
    assert!(cfg.view.avatar.is_none());
    for setting in [
        "user-avatars \"true\";",
        "avatar-dir \"\";",
        "user-avatar \"bad user\" \"ok.png\";",
        "user-avatar \"alice\";",
        "user-avatar \"alice\" \"a.png\"; user-avatar \"alice\" \"b.png\";",
    ] {
        fs::write(
            &config,
            format!("schema-version 1\ngreeter {{ {setting} }}\n"),
        )
        .unwrap();
        assert!(
            Config::load(&Request::common(&config)).is_err(),
            "{setting}"
        );
    }
}

#[test]
fn preview_renders_only_explicit_user_assets_without_discovery_or_commands() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("greeter.kdl");
    let source = root.path().join("alice.png");
    let output = root.path().join("preview.png");
    image(&source, 8, 8, [255, 0, 0, 255]);
    fs::write(
        &config,
        "schema-version 1\ngreeter { users \"alice\"; user-avatar \"alice\" \"alice.png\"; }\n",
    )
    .unwrap();
    let mut app = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
    app.arg("--canopa-config")
        .arg(config)
        .arg("preview")
        .arg(&output)
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/unavailable-fixture-bus",
        )
        .env_remove("GREETD_SOCK")
        .env_remove("WAYLAND_DISPLAY")
        .assert()
        .success();
    assert!(
        image::open(output)
            .unwrap()
            .to_rgb8()
            .pixels()
            .filter(|p| p.0 == [255, 0, 0])
            .count()
            > 2000
    );
}
