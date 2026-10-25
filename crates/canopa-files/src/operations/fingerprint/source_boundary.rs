use super::*;
use std::{cell::RefCell, os::unix::fs::symlink};

#[test]
fn opened_regular_reads_keep_the_actual_leaf_after_leaf_or_parent_replacement() {
    for replace_parent in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("item");
        fs::write(&path, b"Original opened bytes").unwrap();
        let source = source::Opened::open(&path).unwrap();
        let mut readable = source.regular().unwrap();
        if replace_parent {
            fs::rename(&parent, root.path().join("original-parent")).unwrap();
            fs::create_dir(&parent).unwrap();
        } else {
            fs::rename(&path, parent.join("original-item")).unwrap();
        }
        fs::write(&path, b"Replacement bytes").unwrap();
        let mut actual = Vec::new();
        readable.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, b"Original opened bytes");
        assert!(source.require_current().is_err());
        assert_eq!(fs::read(path).unwrap(), b"Replacement bytes");
    }
}

#[test]
fn opened_links_keep_raw_targets_without_following_a_replacement() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("link");
    let target = PathBuf::from(OsString::from_vec(b"missing-\xff".to_vec()));
    symlink(&target, &path).unwrap();
    let source = source::Opened::open(&path).unwrap();
    fs::rename(&path, root.path().join("opened-link")).unwrap();
    symlink("replacement", &path).unwrap();
    assert_eq!(source.link().unwrap(), target);
    assert!(source.regular().is_err());
    assert!(source.require_current().is_err());
}

#[test]
fn nonregular_sources_are_refused_before_a_readable_open() {
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("fifo");
    rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::RUSR).unwrap();
    for path in [&fifo, root.path()] {
        let error = source::Opened::open(path).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}

#[test]
fn replacement_at_the_final_copy_checkpoint_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("item");
    fs::write(&path, b"Original").unwrap();
    let registered = RefCell::new(None);
    let mut checkpoints = 0;
    let result = prepare_copy(
        &path,
        root.path(),
        &Cancellation::default(),
        |stage| {
            *registered.borrow_mut() = Some(stage.stage().clone());
            Ok(())
        },
        |_| {},
        || {
            checkpoints += 1;
            // The first two checks precede copying; two reads (data/EOF) and
            // the metadata checkpoint precede the final return checkpoint.
            if checkpoints == 6 {
                fs::rename(&path, root.path().join("original"))?;
                fs::write(&path, b"Replacement")?;
            }
            Ok(())
        },
    );
    assert_eq!(checkpoints, 6);
    assert!(result.is_err());
    assert_eq!(fs::read(path).unwrap(), b"Replacement");
    let stage = registered.into_inner().unwrap();
    assert_eq!(fs::read(stage.payload()).unwrap(), b"Original");
}

#[test]
fn a_link_replaced_during_stage_registration_is_never_copied() {
    for replace_parent in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("source-parent");
        fs::create_dir(&parent).unwrap();
        let source = parent.join("link");
        symlink("original-missing-target", &source).unwrap();
        let stage = RefCell::new(None);
        let result = prepare_copy(
            &source,
            root.path(),
            &Cancellation::default(),
            |record| {
                *stage.borrow_mut() = Some(record.stage().clone());
                if replace_parent {
                    fs::rename(&parent, root.path().join("parked-parent"))?;
                    fs::create_dir(&parent)?;
                } else {
                    fs::rename(&source, parent.join("parked-link"))?;
                }
                symlink("unselected-replacement-target", &source)
            },
            |_| {},
            || Ok(()),
        );
        assert!(result.is_err(), "changed source must be refused");
        let stage = stage.into_inner().unwrap();
        assert!(
            fs::symlink_metadata(stage.payload()).is_err(),
            "unselected link target entered the durable stage: {:?}",
            fs::read_link(stage.payload())
        );
        assert_eq!(
            fs::read_link(&source).unwrap(),
            Path::new("unselected-replacement-target")
        );
    }
}

#[test]
fn copying_refuses_replaced_payload_stage_or_destination_parent() {
    for replaced in ["payload", "stage", "parent"] {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("destination");
        fs::create_dir(&parent).unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"Original source bytes").unwrap();
        let registered = RefCell::new(None::<Stage>);
        let mut changed = false;
        let result = prepare_copy(
            &source,
            &parent,
            &Cancellation::default(),
            |stage| {
                *registered.borrow_mut() = Some(stage.stage().clone());
                Ok(())
            },
            |_| {
                if changed {
                    return;
                }
                changed = true;
                let stage = registered.borrow();
                let stage = stage.as_ref().unwrap();
                match replaced {
                    "payload" => {
                        fs::rename(stage.payload(), stage.directory.join("parked-payload")).unwrap()
                    }
                    "stage" => {
                        fs::rename(&stage.directory, root.path().join("parked-stage")).unwrap();
                        fs::create_dir(&stage.directory).unwrap();
                    }
                    _ => {
                        fs::rename(&parent, root.path().join("parked-parent")).unwrap();
                        fs::create_dir(&parent).unwrap();
                        fs::create_dir(&stage.directory).unwrap();
                    }
                }
                fs::write(stage.payload(), b"Foreign staged bytes").unwrap();
            },
            || Ok(()),
        );
        assert!(changed);
        assert!(
            result.is_err(),
            "{replaced}: adopted an unselected staged payload"
        );
        let stage = registered.into_inner().unwrap();
        assert_eq!(fs::read(stage.payload()).unwrap(), b"Foreign staged bytes");
        assert_eq!(fs::read(source).unwrap(), b"Original source bytes");
    }
}
