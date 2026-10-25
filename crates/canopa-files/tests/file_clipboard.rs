use canopa_files::{
    file_clipboard::{GNOME, KDE, Selection, URIS},
    transfers::TransferKind,
};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

#[test]
fn native_formats_preserve_raw_filenames_and_the_cut_action() {
    let raw = PathBuf::from(OsString::from_vec(b"/tmp/report \xff#%\n.txt".to_vec()));
    let selection = Selection::new(vec![raw.clone()], TransferKind::Move).unwrap();
    assert_eq!(
        selection.encode(GNOME).unwrap(),
        b"cut\nfile:///tmp/report%20%FF%23%25%0A.txt\n"
    );
    assert_eq!(
        selection.encode(URIS).unwrap(),
        b"file:///tmp/report%20%FF%23%25%0A.txt\r\n"
    );
    assert_eq!(selection.encode(KDE).unwrap(), b"1");
    let decoded = Selection::decode(
        GNOME,
        b"cut\nfile://localhost/tmp/report%20%FF%23%25%0A.txt\n",
        None,
    )
    .unwrap();
    assert_eq!(decoded.paths(), &[raw]);
    assert_eq!(decoded.kind(), TransferKind::Move);
}

#[test]
fn file_offers_reject_nonlocal_invalid_and_ambiguous_locations() {
    for uri in [
        "sftp://host/tmp/item",
        "file://host/tmp/item",
        "file:relative",
        "file:///tmp/item?query",
        "file:///tmp/item#fragment",
        "file:///tmp/%ZZ",
        "file:///tmp/%00",
        "file:///tmp/link/../item",
        "file:///tmp/link/%2e%2e/item",
    ] {
        assert!(
            Selection::decode(URIS, uri.as_bytes(), None).is_err(),
            "accepted {uri}"
        );
    }
    assert!(Selection::new(vec![PathBuf::from("/tmp/link/../item")], TransferKind::Copy).is_err());
}

#[test]
fn uri_parsing_never_silently_changes_control_characters_or_parent_traversal() {
    for uri in [
        "file:///tmp/link/..\\item",
        "file:///tmp/name\twith-tab",
        "file:///tmp/trailing-space ",
    ] {
        assert!(
            Selection::decode(URIS, uri.as_bytes(), None).is_err(),
            "normalized {uri:?} into another filename"
        );
    }
    let selected = Selection::decode(URIS, b"file:///tmp/encoded%09tab%5Cslash%20", None).unwrap();
    assert_eq!(
        selected.paths(),
        &[PathBuf::from("/tmp/encoded\ttab\\slash ")]
    );
}
