use super::*;

#[test]
fn image_is_sealed_and_preserves_the_selected_bytes_after_path_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("client.so");
    std::fs::write(&path, b"\x7fELForiginal-image").unwrap();
    let image = Runtime::snapshot(&path).unwrap();
    std::fs::rename(&path, dir.path().join("old.so")).unwrap();
    std::fs::write(&path, b"\x7fELFreplacement").unwrap();
    let mut descriptors = vec![];
    let selection = image.inherit(&mut descriptors).unwrap();
    let Runtime::Image(received) = Runtime::receive(selection).unwrap() else {
        panic!("missing image")
    };
    let mut bytes = [0; 18];
    received.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"\x7fELForiginal-image");
    assert!(received.write_all_at(b"changed", 0).is_err());
    assert!(rustix::fs::ftruncate(&*received, 0).is_err());
    assert!(
        rustix::fs::fcntl_get_seals(&*received)
            .unwrap()
            .contains(SEALS)
    );
}

#[test]
fn image_refuses_symlinks_nonregular_invalid_oversized_and_unsealed_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("client.so");
    std::fs::write(&path, b"\x7fELF").unwrap();
    let link = dir.path().join("link.so");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(Runtime::snapshot(&link).is_err());
    assert!(Runtime::snapshot(dir.path()).is_err());
    assert!(Runtime::snapshot(Path::new("/dev/null")).is_err());
    let file = File::open(&path).unwrap();
    assert!(
        Runtime::receive(Selection::Image {
            descriptor: file.as_raw_fd()
        })
        .is_err()
    );
    assert!(Runtime::receive(Selection::Image { descriptor: 2 }).is_err());
    std::fs::write(&path, b"bad!").unwrap();
    assert!(Runtime::snapshot(&path).is_err());
    let file = OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(MAX_IMAGE_BYTES + 1).unwrap();
    assert!(Runtime::snapshot(&path).is_err());
}

#[test]
fn absent_or_unavailable_selection_never_adds_descriptors_or_claims_a_capability() {
    let mut descriptors = vec![];
    for runtime in [Runtime::System, Runtime::Unavailable] {
        let selection = runtime.inherit(&mut descriptors).unwrap();
        assert!(!Runtime::receive(selection).unwrap().has_image());
        assert!(descriptors.is_empty());
    }
    assert!(
        serde_json::from_str::<Selection>(r#"{"kind":"image","descriptor":3,"path":"/tmp/other"}"#)
            .is_err()
    );
}
