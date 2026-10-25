use canopa_files::{interface::PlaceKind, personal_folders::PersonalFolders};
use std::{
    ffi::OsString,
    fs,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::symlink,
    },
    path::PathBuf,
};

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary
        .path()
        .join(OsString::from_vec(b"home-\xff".to_vec()));
    let config = temporary.path().join("configuration space");
    fs::create_dir(&home).unwrap();
    fs::create_dir(&config).unwrap();
    (temporary, home, config)
}

fn assignment(name: &str, path: &[u8]) -> Vec<u8> {
    let mut value = format!("XDG_{name}_DIR=\"").into_bytes();
    for byte in path {
        if matches!(byte, b'\\' | b'"' | b'$' | b'`') {
            value.push(b'\\');
        }
        value.push(*byte);
    }
    value.extend_from_slice(b"\"\n");
    value
}

#[test]
fn actual_definitions_preserve_localized_escaped_raw_paths_and_explicit_home() {
    let (temporary, home, config) = fixture();
    let documents = temporary.path().join(OsString::from_vec(
        b"Mes \"documents\" \\ raw-\xfe $cost `notes`".to_vec(),
    ));
    let mut definitions = b"# Personal folders\nXDG_DESKTOP_DIR=\"$HOME//Bureau\"\n".to_vec();
    definitions.extend(assignment("DOCUMENTS", documents.as_os_str().as_bytes()));
    definitions.extend_from_slice(
        b"XDG_DOWNLOAD_DIR=\"$HOME/\"\n\tXDG_PICTURES_DIR = \"$HOME/Images d'ete\" # localized\n",
    );
    let source = temporary.path().join("linked definitions");
    fs::write(&source, definitions).unwrap();
    symlink(source, config.join("user-dirs.dirs")).unwrap();
    let folders = PersonalFolders::load(&home, &config).unwrap();
    let mut desktop = home.as_os_str().as_bytes().to_vec();
    desktop.extend_from_slice(b"//Bureau");
    assert_eq!(
        folders
            .path(PlaceKind::Desktop)
            .unwrap()
            .as_os_str()
            .as_bytes(),
        desktop
    );
    assert_eq!(
        folders.path(PlaceKind::Documents),
        Some(documents.as_path())
    );
    assert_eq!(folders.path(PlaceKind::Downloads), Some(home.as_path()));
    let mut exact_home = home.as_os_str().as_bytes().to_vec();
    exact_home.push(b'/');
    assert_eq!(
        folders
            .path(PlaceKind::Downloads)
            .unwrap()
            .as_os_str()
            .as_bytes(),
        exact_home,
        "the explicit $HOME/ separator is part of the decoded Unix path",
    );
    assert_eq!(
        folders.path(PlaceKind::Pictures),
        Some(home.join("Images d'ete").as_path())
    );
    assert!(
        !documents.exists(),
        "resolution must not create or substitute a missing directory"
    );
}

#[test]
fn assignments_are_data_and_each_loaded_snapshot_keeps_its_last_valid_values() {
    let (temporary, home, config) = fixture();
    let marker = temporary.path().join("must not execute");
    let mut definitions = format!(
        "XDG_DESKTOP_DIR=\"$(touch {})\"\nXDG_PICTURES_DIR=\"${{HOME}}/Images\"\n",
        marker.display(),
    )
    .into_bytes();
    definitions.extend_from_slice(b"XDG_DOCUMENTS_DIR=\"$HOME/First\"\nXDG_DOCUMENTS_DIR=\"unterminated\nXDG_DOCUMENTS_DIR=\"/ignored\"; command\nXDG_DOWNLOAD_DIR=\"$HOME/A\"\nXDG_DOWNLOAD_DIR=\"$HOME/B\"\nXDG_UNKNOWN_DIR=\"/unknown\"\n");
    fs::write(config.join("user-dirs.dirs"), definitions).unwrap();
    let first = PersonalFolders::load(&home, &config).unwrap();
    assert_eq!(
        first.path(PlaceKind::Desktop),
        Some(home.join("Desktop").as_path())
    );
    assert_eq!(
        first.path(PlaceKind::Pictures),
        Some(home.join("Pictures").as_path())
    );
    assert_eq!(
        first.path(PlaceKind::Documents),
        Some(home.join("First").as_path())
    );
    assert_eq!(
        first.path(PlaceKind::Downloads),
        Some(home.join("B").as_path())
    );
    let literal = temporary.path().join("$(touch literal) $OTHER");
    let mut replacement = assignment("PICTURES", literal.as_os_str().as_bytes());
    replacement.extend_from_slice(b"XDG_DOCUMENTS_DIR=\"$HOME/Last\"\nXDG_DOWNLOAD_DIR=\"$HOME\"\nXDG_DESKTOP_DIR=\"\\$HOME/Bureau\"\n");
    fs::write(config.join("user-dirs.dirs"), replacement).unwrap();
    let second = PersonalFolders::load(&home, &config).unwrap();
    assert_eq!(
        first.path(PlaceKind::Documents),
        Some(home.join("First").as_path())
    );
    assert_eq!(
        second.path(PlaceKind::Documents),
        Some(home.join("Last").as_path())
    );
    assert_eq!(second.path(PlaceKind::Pictures), Some(literal.as_path()));
    assert_eq!(second.path(PlaceKind::Downloads), Some(home.as_path()));
    assert_eq!(
        second.path(PlaceKind::Desktop),
        Some(home.join("Desktop").as_path())
    );
    assert!(!marker.exists());
    assert!(!literal.exists());
}

#[test]
fn missing_nonregular_and_over_budget_sources_keep_conventional_folders() {
    let (_temporary, home, config) = fixture();
    let source = config.join("user-dirs.dirs");
    let check = || {
        let folders = PersonalFolders::load(&home, &config).unwrap();
        for (kind, name) in [
            (PlaceKind::Desktop, "Desktop"),
            (PlaceKind::Documents, "Documents"),
            (PlaceKind::Downloads, "Downloads"),
            (PlaceKind::Pictures, "Pictures"),
        ] {
            assert_eq!(folders.path(kind), Some(home.join(name).as_path()));
        }
    };
    check();
    fs::create_dir(&source).unwrap();
    check();
    fs::remove_dir(&source).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &source,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    check();
    fs::remove_file(&source).unwrap();
    let mut oversized = b"XDG_DESKTOP_DIR=\"$HOME/Should not partially apply\"\n".to_vec();
    oversized.resize(64 * 1024 + 1, b' ');
    fs::write(&source, oversized).unwrap();
    check();
    assert_eq!(
        PersonalFolders::load(&home, &PathBuf::from("relative"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        PersonalFolders::load(&PathBuf::from("relative"), &config)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
}
