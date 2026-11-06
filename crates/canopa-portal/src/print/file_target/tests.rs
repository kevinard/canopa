use super::*;
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink, net::UnixListener},
};

fn handoff(review: &Reviewed) -> io::Result<Pinned> {
    let (send, receive) = UnixStream::pair()?;
    let selection = serde_json::from_slice(&serde_json::to_vec(review.selection())?)?;
    review.send(&send)?;
    drop(send);
    Pinned::receive(&receive, selection)
}

#[test]
fn file_pins_survive_the_producer_and_preparation_never_creates_or_changes_output() {
    let folder = tempfile::tempdir().unwrap();
    for existing in [false, true] {
        let path = folder
            .path()
            .join(if existing { "existing.pdf" } else { "new.pdf" });
        if existing {
            fs::write(&path, b"keep the original").unwrap();
        }
        let review = Reviewed::open(&path).unwrap();
        assert_eq!(review.selection().replaces(), existing);
        let (send, receive) = UnixStream::pair().unwrap();
        let evidence = review.selection().clone();
        review.send(&send).unwrap();
        drop(review);
        drop(send);
        let pin = Pinned::receive(&receive, evidence).unwrap();
        pin.verify().unwrap();
        assert_eq!(pin.selection().path(), path.as_os_str().as_bytes());
        if existing {
            assert_eq!(fs::read(&path).unwrap(), b"keep the original");
        } else {
            assert!(!path.exists());
        }
    }
}

#[test]
fn a_new_occupant_does_not_turn_create_consent_into_replacement_consent() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("file.pdf");
    let review = Reviewed::open(&path).unwrap();
    fs::write(&path, b"newer work").unwrap();
    assert!(handoff(&review).is_err());
    assert_eq!(fs::read(path).unwrap(), b"newer work");
}

#[test]
fn changed_existing_bytes_or_another_inode_require_fresh_review() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("file.pdf");
    fs::write(&path, b"original").unwrap();
    let review = Reviewed::open(&path).unwrap();
    fs::write(&path, b"changed original bytes").unwrap();
    assert!(handoff(&review).is_err());
    let review = Reviewed::open(&path).unwrap();
    fs::rename(&path, folder.path().join("retained.pdf")).unwrap();
    fs::write(&path, b"unreviewed replacement").unwrap();
    assert!(handoff(&review).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"unreviewed replacement");
    assert_eq!(
        fs::read(folder.path().join("retained.pdf")).unwrap(),
        b"changed original bytes"
    );
}

#[test]
fn a_retained_pin_cannot_follow_a_replaced_parent_or_a_later_leaf() {
    let folder = tempfile::tempdir().unwrap();
    let parent = folder.path().join("chosen");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("file.pdf");
    fs::write(&path, b"original").unwrap();
    let review = Reviewed::open(&path).unwrap();
    let pin = handoff(&review).unwrap();
    fs::rename(&parent, folder.path().join("moved")).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::write(&path, b"different directory").unwrap();
    assert!(pin.verify().is_err());
    assert!(handoff(&review).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"different directory");
    assert_eq!(
        fs::read(folder.path().join("moved/file.pdf")).unwrap(),
        b"original"
    );
}

#[test]
fn symlinks_and_special_entries_cannot_become_file_destinations() {
    let folder = tempfile::tempdir().unwrap();
    let original = folder.path().join("original");
    fs::write(&original, b"untouched").unwrap();
    let link = folder.path().join("link");
    symlink(&original, &link).unwrap();
    assert!(Reviewed::open(&link).is_err());
    assert!(Reviewed::open(folder.path()).is_err());
    let socket = folder.path().join("socket");
    let _listener = UnixListener::bind(&socket).unwrap();
    assert!(Reviewed::open(&socket).is_err());
    assert!(Reviewed::open(Path::new("relative.pdf")).is_err());
    assert_eq!(fs::read(&original).unwrap(), b"untouched");
}

#[test]
fn exact_unix_name_bytes_survive_json_and_descriptor_handoff() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder
        .path()
        .join(OsString::from_vec(b"non-UTF8-\xff.pdf".to_vec()));
    let review = Reviewed::open(&path).unwrap();
    let pin = handoff(&review).unwrap();
    assert_eq!(pin.selection().path(), path.as_os_str().as_bytes());
    assert!(!path.exists());
}

#[test]
fn missing_swapped_or_unexpected_descriptors_cannot_authorize_a_selection() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("file.pdf");
    fs::write(&path, b"original").unwrap();
    let review = Reviewed::open(&path).unwrap();
    let (a, b) = UnixStream::pair().unwrap();
    drop(a);
    assert!(Pinned::receive(&b, review.selection().clone()).is_err());
    let (a, b) = UnixStream::pair().unwrap();
    send(
        &a,
        &[
            review.0.entry.as_ref().unwrap().as_fd(),
            review.0.parent.as_fd(),
        ],
    )
    .unwrap();
    assert!(Pinned::receive(&b, review.selection().clone()).is_err());
    let (a, b) = UnixStream::pair().unwrap();
    review.send(&a).unwrap();
    drop(a);
    assert!(ensure_empty(&b).is_err());
    review.verify().unwrap();
    assert_eq!(fs::read(path).unwrap(), b"original");
}

#[test]
fn a_second_descriptor_message_or_trailing_bytes_cannot_follow_an_approval() {
    use std::io::Write;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("new.pdf");
    let review = Reviewed::open(&path).unwrap();
    for extra_descriptors in [false, true] {
        let (mut sender, receiver) = UnixStream::pair().unwrap();
        review.send(&sender).unwrap();
        if extra_descriptors {
            review.send(&sender).unwrap();
        } else {
            sender.write_all(b"extra data").unwrap();
        }
        drop(sender);
        assert!(Pinned::receive(&receiver, review.selection().clone()).is_err());
        assert!(!path.exists());
    }
    review.verify().unwrap();
}
