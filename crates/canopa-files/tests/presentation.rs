use canopa_files::presentation::{FileManagerRequest as Request, Presentation, View};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

#[test]
fn reveal_groups_exact_raw_names_and_selects_folders_in_their_parent() {
    let uris = [
        "file:///tmp/a/raw-%FF%23.txt",
        "file://localhost/tmp/a/.hidden",
        "file:///tmp/b/folder",
        "file:///tmp/a/.hidden",
    ]
    .map(str::to_owned);
    let request = Presentation::from_file_manager(Request::Items, &uris, "caller_TIME1")
        .unwrap()
        .unwrap();
    assert_eq!(request.startup_id, "caller_TIME1");
    let View::Browsers(targets) = request.view else {
        panic!("reveal must browse")
    };
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].directory, PathBuf::from("/tmp/a"));
    assert_eq!(
        targets[0].selected,
        vec![
            PathBuf::from(OsString::from_vec(b"/tmp/a/raw-\xff#.txt".to_vec())),
            PathBuf::from("/tmp/a/.hidden")
        ]
    );
    assert_eq!(targets[1].directory, PathBuf::from("/tmp/b"));
    assert_eq!(targets[1].selected, vec![PathBuf::from("/tmp/b/folder")]);
}

#[test]
fn complete_validation_precedes_any_admission_and_empty_requests_are_quiet() {
    for uri in [
        "https://example.test/file",
        "file://other-host/tmp/a",
        "file:///tmp/%00",
        "file:///tmp/%GG",
        "file:///tmp/a?query",
        "file:///tmp/a#fragment",
        "file:///tmp/link/../other",
    ] {
        assert!(
            Presentation::from_file_manager(
                Request::Folders,
                &["file:///tmp/good".into(), uri.into()],
                ""
            )
            .is_err(),
            "{uri}"
        );
    }
    for kind in [Request::Folders, Request::Items, Request::Properties] {
        assert!(
            Presentation::from_file_manager(kind, &[], "")
                .unwrap()
                .is_none()
        );
        assert!(
            Presentation::from_file_manager(kind, &vec!["file:///tmp/a".into(); 65], "").is_err()
        );
        assert!(
            Presentation::from_file_manager(kind, &["file:///tmp/a".into()], &"x".repeat(4097))
                .is_err()
        );
    }
}

#[test]
fn folder_root_and_properties_have_explicit_distinct_intent() {
    let request = Presentation::from_file_manager(Request::Folders, &["file:///tmp/a".into()], "")
        .unwrap()
        .unwrap();
    let View::Browsers(targets) = request.view else {
        panic!()
    };
    assert_eq!(targets[0].directory, PathBuf::from("/tmp/a"));
    assert!(targets[0].selected.is_empty());
    let request = Presentation::from_file_manager(Request::Items, &["file:///".into()], "")
        .unwrap()
        .unwrap();
    let View::Browsers(targets) = request.view else {
        panic!()
    };
    assert_eq!(targets[0].directory, PathBuf::from("/"));
    let request =
        Presentation::from_file_manager(Request::Properties, &["file:///tmp/a".into()], "")
            .unwrap()
            .unwrap();
    assert_eq!(
        request.view,
        View::Properties(vec![PathBuf::from("/tmp/a")])
    );
}
