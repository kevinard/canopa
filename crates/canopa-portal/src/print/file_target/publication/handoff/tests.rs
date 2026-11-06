use super::*;
use crate::print::file_target::Reviewed;
use std::fs;

fn pin(path: &std::path::Path) -> Pinned {
    let review = Reviewed::open(path).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    review.send(&send).unwrap();
    send.shutdown(std::net::Shutdown::Write).unwrap();
    drop(send);
    Pinned::receive(&receive, review.selection().clone()).unwrap()
}

#[test]
fn staging_handoff_retains_live_authority_for_both_new_and_replaced_outputs() {
    for existing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("output.pdf");
        if existing {
            fs::write(&output, b"original").unwrap();
        }
        let pin = pin(&output);
        let preparation = Preparation::new(&pin).unwrap();
        let publication = Publication::prepare_with(pin, preparation.clone()).unwrap();
        let (send, receive) = UnixStream::pair().unwrap();
        let inherited_writer = send.try_clone().unwrap();
        publication.send(&send).unwrap();
        send.shutdown(std::net::Shutdown::Write).unwrap();
        drop(send);
        receive.set_nonblocking(true).unwrap();
        let received =
            Prepared::receive(&receive, publication.info().clone(), &preparation).unwrap();
        assert!(
            rustix::io::fcntl_getfd(&inherited_writer)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        drop(inherited_writer);
        assert!(received.files.iter().all(|file| path_only(file).unwrap()));
        let inherited = Publication::inherited(received.info, received.files).unwrap();
        assert!(inherited.settle(false).unwrap().recovery.is_none());
        assert_eq!(
            fs::read(&output).ok(),
            existing.then(|| b"original".to_vec())
        );
    }
}

#[test]
fn staging_handoff_refuses_extra_packets_and_an_unclosed_writer_without_waiting() {
    for extra in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let pin = pin(&root.path().join("output.pdf"));
        let preparation = Preparation::new(&pin).unwrap();
        let publication = Publication::prepare_with(pin, preparation.clone()).unwrap();
        let (send, receive) = UnixStream::pair().unwrap();
        receive.set_nonblocking(true).unwrap();
        publication.send(&send).unwrap();
        if extra {
            publication.send(&send).unwrap();
            send.shutdown(std::net::Shutdown::Write).unwrap();
        }
        let error = Prepared::receive(&receive, publication.info().clone(), &preparation)
            .err()
            .unwrap();
        assert_eq!(
            error.kind(),
            if extra {
                std::io::ErrorKind::InvalidData
            } else {
                std::io::ErrorKind::WouldBlock
            }
        );
        assert!(publication.settle(false).unwrap().recovery.is_none());
    }
}

#[test]
fn staging_handoff_rejects_another_allocation_and_never_reopens_missing_paths() {
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("output.pdf");
    let pin = pin(&output);
    let preparation = Preparation::new(&pin).unwrap();
    let other = Preparation::new(&pin).unwrap();
    let publication = Publication::prepare_with(pin, preparation.clone()).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    publication.send(&send).unwrap();
    send.shutdown(std::net::Shutdown::Write).unwrap();
    drop(send);
    receive.set_nonblocking(true).unwrap();
    assert_eq!(
        Prepared::receive(&receive, publication.info().clone(), &other)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    let moved = root.path().join("moved");
    fs::create_dir(&moved).unwrap();
    let original = preparation.retained(false).recovery.unwrap();
    // A hint remains usable without consulting the on-disk journal, even after
    // the directory has moved. It must not become permission to replay it.
    let directory = PathBuf::from(OsString::from_vec(original.clone()));
    fs::rename(&directory, moved.join("work")).unwrap();
    let uncertain = preparation.retained(false);
    assert!(!uncertain.published && uncertain.uncertain);
    assert_eq!(uncertain.recovery.as_deref(), Some(original.as_slice()));
    let accepted = preparation.retained(true);
    assert!(accepted.published && !accepted.uncertain);
    assert_eq!(accepted.recovery, uncertain.recovery);
}
