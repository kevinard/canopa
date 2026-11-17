use canopa_files::{Cancellation, browse::EntryKind, file_properties};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

#[test]
fn metadata_inspects_links_and_keeps_individual_failures_without_reading_contents() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("private.txt");
    fs::write(&file, b"Private contents are not part of Properties.\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
    let link = root.path().join("dangling-link");
    symlink("missing-target", &link).unwrap();
    let missing = root.path().join("missing");
    let paths = vec![file.clone(), link.clone(), missing.clone()];
    let items = file_properties::read(&paths, &Cancellation::default()).unwrap();
    assert_eq!(
        items.iter().map(|i| &i.path).collect::<Vec<_>>(),
        paths.iter().collect::<Vec<_>>()
    );
    let metadata = items[0].metadata.as_ref().unwrap();
    assert_eq!(metadata.kind, EntryKind::File);
    assert_eq!(metadata.permissions, 0o640);
    assert_eq!(metadata.bytes, fs::metadata(file).unwrap().len());
    let metadata = items[1].metadata.as_ref().unwrap();
    assert_eq!(metadata.kind, EntryKind::Symlink);
    assert_eq!(
        metadata.link_target.as_deref(),
        Some(std::path::Path::new("missing-target"))
    );
    assert!(items[2].metadata.is_err());
    assert!(!missing.exists());
}

#[test]
fn cancelled_and_oversized_inspections_do_not_start() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        file_properties::read(&["/no-such-properties-file".into()], &cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(
        file_properties::read(
            &vec!["/no-such-properties-file".into(); 65],
            &Cancellation::default()
        )
        .is_err()
    );
}

#[test]
fn access_changes_act_only_on_the_item_shown_and_never_follow_links() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("notes.txt");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let shown = std::fs::symlink_metadata(&file).unwrap();
    let identity = (shown.dev(), shown.ino());
    canopa_files::file_properties::set_access(&file, identity, 0o600).unwrap();
    let mode = |path: &std::path::Path| std::fs::metadata(path).unwrap().mode() & 0o7777;
    assert_eq!(mode(&file), 0o600);

    // A link is refused, and its target keeps its access.
    let link = root.path().join("link");
    symlink(&file, &link).unwrap();
    let link_shown = std::fs::symlink_metadata(&link).unwrap();
    assert!(
        canopa_files::file_properties::set_access(
            &link,
            (link_shown.dev(), link_shown.ino()),
            0o777
        )
        .is_err()
    );
    assert_eq!(mode(&file), 0o600);

    // An item replaced since it was shown is refused. The replacement exists
    // before the original goes, so it cannot reuse the original's inode.
    let replacement = root.path().join("replacement");
    std::fs::write(&replacement, b"y").unwrap();
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::rename(&replacement, &file).unwrap();
    assert!(canopa_files::file_properties::set_access(&file, identity, 0o600).is_err());
    assert_eq!(mode(&file), 0o644);
}
