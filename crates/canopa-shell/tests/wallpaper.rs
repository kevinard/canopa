use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Output},
};

fn lock(root: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canopa-shell"))
        .arg("lock")
        .env("CANOPA_LANGUAGE", "en")
        .env("PATH", root)
        .env("HOME", root)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_RUNTIME_DIR", root)
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("NIRI_SOCKET")
        .output()
        .unwrap()
}

#[test]
fn lock_executes_once_without_a_desktop_and_pins_the_selected_image_descriptor() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let locker = root.join("swaylock");
    fs::write(&locker, "#!/usr/bin/python3\nimport hashlib,json,sys\nprint(json.dumps({'args':sys.argv[1:],'hash':hashlib.sha256(open(sys.argv[2],'rb').read()).hexdigest() if len(sys.argv)>1 else None}))\n").unwrap();
    fs::set_permissions(&locker, fs::Permissions::from_mode(0o700)).unwrap();
    let empty = lock(root);
    assert!(empty.status.success(), "{empty:?}");
    let value: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(value["args"], serde_json::json!([]));
    assert!(!root.join("state/canopa/shell/wallpaper").exists());

    let state = root.join("state/canopa/shell/wallpaper");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join(".lock"), []).unwrap();
    let image_path = root.join("image.png");
    image::RgbaImage::from_pixel(13, 7, image::Rgba([10, 20, 30, 255]))
        .save(&image_path)
        .unwrap();
    use sha2::{Digest, Sha256};
    let bytes = fs::read(&image_path).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    fs::write(state.join(format!("{hash}.png")), bytes).unwrap();
    fs::write(
        state.join("selection.json"),
        serde_json::to_vec(&serde_json::json!({
            "version":1,"background":null,"lockscreen":hash
        }))
        .unwrap(),
    )
    .unwrap();
    fs::remove_file(image_path).unwrap();
    let selected = lock(root);
    assert!(selected.status.success(), "{selected:?}");
    let value: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(value["args"][0], "--image");
    assert!(
        value["args"][1]
            .as_str()
            .unwrap()
            .starts_with("/proc/self/fd/")
    );
    assert_eq!(value["hash"], hash);

    fs::write(state.join("selection.json"), "malformed").unwrap();
    let fallback = lock(root);
    assert!(fallback.status.success(), "{fallback:?}");
    let value: serde_json::Value = serde_json::from_slice(&fallback.stdout).unwrap();
    assert_eq!(value["args"], serde_json::json!([]));
    assert!(String::from_utf8_lossy(&fallback.stderr).contains("locking without an image"));
    fs::remove_file(locker).unwrap();
    assert_eq!(lock(root).status.code(), Some(1));
}
