//! Archive creation and extraction through the public engine, in private
//! temporary folders only.
use canopa_files::{
    Cancellation,
    archives::{self, ArchiveRequest, Format, Limits, Outcome},
};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn run(request: &ArchiveRequest) -> std::io::Result<Outcome> {
    archives::run(
        request,
        &Cancellation::default(),
        Limits::default(),
        &mut |_| {},
    )
}

fn compress(sources: &[PathBuf], destination: &Path, name: &str, format: Format) -> PathBuf {
    let request = ArchiveRequest::compress(
        sources.to_vec(),
        destination.to_owned(),
        name.into(),
        format,
    );
    run(&request).unwrap().output
}

fn extract(archive: &Path, destination: &Path) -> std::io::Result<Outcome> {
    run(&ArchiveRequest::extract(
        archive.to_owned(),
        destination.to_owned(),
    ))
}

/// A folder with nested content, a link inside it, an executable and a set-id
/// file.
fn sample(root: &Path) -> PathBuf {
    let project = root.join("project");
    fs::create_dir_all(project.join("src/deep")).unwrap();
    fs::write(project.join("README"), "read me\n").unwrap();
    fs::write(project.join("src/deep/data.bin"), vec![7u8; 300_000]).unwrap();
    fs::write(project.join("run.sh"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(project.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(project.join("setid"), "x").unwrap();
    fs::set_permissions(project.join("setid"), fs::Permissions::from_mode(0o4755)).unwrap();
    symlink("deep/data.bin", project.join("src/link")).unwrap();
    project
}

fn assert_sample(extracted: &Path) {
    assert_eq!(
        names(extracted),
        ["README", "run.sh", "setid", "src"],
        "{}",
        extracted.display()
    );
    assert_eq!(
        fs::read(extracted.join("src/deep/data.bin")).unwrap(),
        vec![7u8; 300_000]
    );
    assert_eq!(
        fs::read_link(extracted.join("src/link")).unwrap(),
        Path::new("deep/data.bin")
    );
    let mode = |name: &str| {
        fs::symlink_metadata(extracted.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    };
    assert_eq!(mode("run.sh"), 0o755);
    // Set-id bits are never restored.
    assert_eq!(mode("setid"), 0o755);
}

#[test]
fn every_writable_format_round_trips() {
    for format in Format::WRITABLE {
        let root = tempfile::tempdir().unwrap();
        let project = sample(root.path());
        let archive = compress(
            std::slice::from_ref(&project),
            root.path(),
            "project",
            format,
        );
        assert_eq!(
            archive.file_name().unwrap().to_string_lossy(),
            format!("project{}", format.suffix())
        );
        assert_eq!(Format::detect(archive.file_name().unwrap()), Some(format));
        let out = root.path().join("out");
        fs::create_dir(&out).unwrap();
        let outcome = extract(&archive, &out).unwrap();
        // One top folder is extracted as that folder.
        assert_eq!(outcome.output, out.join("project"), "{format:?}");
        assert_eq!(outcome.skipped_unsafe, 0);
        assert_sample(&outcome.output);
        assert_eq!(names(&out), ["project"]);
        // Nothing hidden is left beside the results.
        assert_eq!(names(root.path()), {
            let mut expected = vec![
                "out".to_owned(),
                "project".to_owned(),
                format!("project{}", format.suffix()),
            ];
            expected.sort();
            expected
        });
    }
}

#[test]
fn results_take_the_next_free_name() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("notes.txt");
    fs::write(&file, "notes").unwrap();
    let first = compress(
        std::slice::from_ref(&file),
        root.path(),
        "notes",
        Format::Zip,
    );
    let second = compress(
        std::slice::from_ref(&file),
        root.path(),
        "notes",
        Format::Zip,
    );
    assert_eq!(first, root.path().join("notes.zip"));
    assert_eq!(second, root.path().join("notes (2).zip"));
    // A single file is extracted as itself, numbered before its extension.
    let extracted = extract(&first, root.path()).unwrap();
    assert_eq!(extracted.output, root.path().join("notes (2).txt"));
    assert_eq!(fs::read_to_string(&extracted.output).unwrap(), "notes");
}

#[test]
fn several_top_items_extract_into_a_folder_named_after_the_archive() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a.txt");
    let b = root.path().join("b");
    fs::write(&a, "a").unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(b.join("c"), "c").unwrap();
    let archive = compress(&[a, b], root.path(), "Archive", Format::TarGz);
    fs::create_dir(root.path().join("Archive")).unwrap();
    let outcome = extract(&archive, root.path()).unwrap();
    assert_eq!(outcome.output, root.path().join("Archive (2)"));
    assert_eq!(names(&outcome.output), ["a.txt", "b"]);
    assert_eq!(outcome.entries, 3);
    assert_eq!(
        fs::symlink_metadata(&outcome.output)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

fn zip_with(path: &Path, build: impl FnOnce(&mut zip::ZipWriter<fs::File>)) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    build(&mut writer);
    writer.finish().unwrap();
}

#[test]
fn entries_leading_outside_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let archive = root.path().join("bad.zip");
    zip_with(&archive, |writer| {
        let options = zip::write::SimpleFileOptions::default();
        for name in [
            "../evil.txt",
            "/absolute.txt",
            "ok/../../evil2.txt",
            "ok/file.txt",
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"payload").unwrap();
        }
        writer
            .add_symlink("ok/out", "../../outside", options)
            .unwrap();
        writer.add_symlink("ok/absolute", "/etc", options).unwrap();
        writer.add_symlink("ok/in", "file.txt", options).unwrap();
        writer.add_symlink("top", "ok/file.txt", options).unwrap();
    });
    let outcome = extract(&archive, &destination).unwrap();
    assert_eq!(outcome.skipped_unsafe, 5);
    assert_eq!(outcome.output, destination.join("bad"));
    assert_eq!(names(&destination), ["bad"]);
    assert_eq!(names(&outcome.output), ["ok", "top"]);
    assert_eq!(names(&outcome.output.join("ok")), ["file.txt", "in"]);
    assert_eq!(
        fs::read_to_string(outcome.output.join("top")).unwrap(),
        "payload"
    );
    assert!(!root.path().join("evil.txt").exists());
    assert!(!root.path().join("evil2.txt").exists());
    assert_eq!(names(root.path()), ["bad.zip", "destination"]);
}

/// A tar member written with raw header bytes, since builders refuse `..`.
fn raw_tar_member(name: &[u8], kind: tar::EntryType, link: &[u8], data: &[u8]) -> Vec<u8> {
    let mut header = tar::Header::new_old();
    header.as_old_mut().name[..name.len()].copy_from_slice(name);
    header.as_old_mut().linkname[..link.len()].copy_from_slice(link);
    header.set_entry_type(kind);
    header.set_mode(0o644);
    header.set_size(data.len() as u64);
    header.set_cksum();
    let mut member = header.as_bytes().to_vec();
    member.extend_from_slice(data);
    member.resize(member.len().div_ceil(512) * 512, 0);
    member
}

#[test]
fn tar_traversal_and_hard_links_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(root.path().join("secret"), "secret").unwrap();
    let mut bytes = Vec::new();
    bytes.extend(raw_tar_member(
        b"../escape",
        tar::EntryType::Regular,
        b"",
        b"x",
    ));
    bytes.extend(raw_tar_member(
        b"kept",
        tar::EntryType::Regular,
        b"",
        b"kept",
    ));
    bytes.extend(raw_tar_member(b"hard", tar::EntryType::Link, b"kept", b""));
    bytes.extend(raw_tar_member(
        b"stolen",
        tar::EntryType::Link,
        b"../secret",
        b"",
    ));
    bytes.extend(raw_tar_member(b"fifo", tar::EntryType::Fifo, b"", b""));
    bytes.extend([0; 1024]);
    let archive = root.path().join("bad.tar");
    fs::write(&archive, bytes).unwrap();
    let outcome = extract(&archive, &destination).unwrap();
    assert_eq!(outcome.skipped_unsafe, 2);
    assert_eq!(outcome.skipped_special, 1);
    assert_eq!(names(&outcome.output), ["hard", "kept"]);
    assert_eq!(
        fs::read_to_string(outcome.output.join("hard")).unwrap(),
        "kept"
    );
    assert!(!root.path().join("escape").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("secret")).unwrap(),
        "secret"
    );
}

#[test]
fn nothing_is_written_through_an_archived_link() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let archive = root.path().join("swap.zip");
    // The link would make `dir/x` land beside the destination if it were
    // created before the file.
    zip_with(&archive, |writer| {
        let options = zip::write::SimpleFileOptions::default();
        writer.add_symlink("dir", "..", options).unwrap();
        writer.add_symlink("here", ".", options).unwrap();
        writer.start_file("here/dir/x", options).unwrap();
        writer.write_all(b"x").unwrap();
    });
    let error = extract(&archive, &destination).unwrap_err();
    assert!(error.to_string().contains("conflicting"), "{error}");
    assert!(names(&destination).is_empty());
    assert_eq!(names(root.path()), ["destination", "swap.zip"]);
}

#[test]
fn expansion_is_bounded() {
    let root = tempfile::tempdir().unwrap();
    let big = root.path().join("zeros");
    fs::write(&big, vec![0u8; 4 << 20]).unwrap();
    let archive = compress(&[big], root.path(), "bomb", Format::TarZst);
    assert!(fs::metadata(&archive).unwrap().len() < 64 << 10);
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let request = ArchiveRequest::extract(archive.clone(), destination.clone());
    let limits = Limits {
        max_bytes: Some(1 << 20),
        ..Limits::default()
    };
    let error = archives::run(&request, &Cancellation::default(), limits, &mut |_| {}).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::StorageFull);
    assert!(names(&destination).is_empty());

    let many: Vec<_> = (0..10)
        .map(|index| {
            let path = root.path().join(format!("item{index}"));
            fs::write(&path, "x").unwrap();
            path
        })
        .collect();
    let archive = compress(&many, root.path(), "many", Format::Zip);
    let request = ArchiveRequest::extract(archive, destination.clone());
    let limits = Limits {
        max_entries: 3,
        ..Limits::default()
    };
    let error = archives::run(&request, &Cancellation::default(), limits, &mut |_| {}).unwrap_err();
    assert!(error.to_string().contains("more than"), "{error}");
    assert!(names(&destination).is_empty());
}

#[test]
fn cancellation_part_way_keeps_nothing() {
    let root = tempfile::tempdir().unwrap();
    let project = sample(root.path());
    let cancellation = Cancellation::default();
    let request = ArchiveRequest::compress(
        vec![project.clone()],
        root.path().to_owned(),
        OsString::from("project"),
        Format::TarXz,
    );
    let mut reports = 0;
    let error = archives::run(
        &request,
        &cancellation,
        Limits::default(),
        &mut |progress| {
            reports += 1;
            if progress.bytes > 0 {
                cancellation.cancel();
            }
        },
    )
    .unwrap_err();
    assert!(archives::cancelled(&error), "{error}");
    assert!(reports > 0);
    assert_eq!(names(root.path()), ["project"]);

    let archive = compress(&[project], root.path(), "project", Format::Zip);
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let cancellation = Cancellation::default();
    let request = ArchiveRequest::extract(archive, destination.clone());
    let error = archives::run(
        &request,
        &cancellation,
        Limits::default(),
        &mut |progress| {
            if progress.entries >= 3 {
                cancellation.cancel();
            }
        },
    )
    .unwrap_err();
    assert!(archives::cancelled(&error), "{error}");
    assert!(names(&destination).is_empty());
}

#[test]
fn an_interrupted_partial_result_is_discarded() {
    let root = tempfile::tempdir().unwrap();
    let request = ArchiveRequest::extract(root.path().join("a.zip"), root.path().to_owned());
    let partial = request.partial_path();
    fs::create_dir_all(partial.join("nested")).unwrap();
    fs::write(partial.join("nested/file"), "x").unwrap();
    fs::set_permissions(partial.join("nested"), fs::Permissions::from_mode(0o500)).unwrap();
    assert!(request.discard_partial().unwrap());
    assert!(!partial.exists());
    assert!(!request.discard_partial().unwrap());

    let request = ArchiveRequest::compress(
        vec![root.path().join("x")],
        root.path().to_owned(),
        "x".into(),
        Format::Zip,
    );
    // A partial name that is not this run's kind of result is left alone.
    fs::create_dir(request.partial_path()).unwrap();
    assert!(!request.discard_partial().unwrap());
    assert!(request.partial_path().is_dir());
}

#[test]
fn encrypted_and_damaged_archives_fail_clearly() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("secret.zip");
    zip_with(&archive, |writer| {
        writer
            .start_file("secret.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"secret").unwrap();
    });
    // Mark every entry as encrypted, in its local and central headers.
    let mut bytes = fs::read(&archive).unwrap();
    for (signature, offset) in [(b"PK\x03\x04", 6), (b"PK\x01\x02", 8)] {
        let start = bytes
            .windows(4)
            .position(|window| window == signature)
            .unwrap();
        bytes[start + offset] |= 1;
    }
    fs::write(&archive, bytes).unwrap();
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let error = extract(&archive, &destination).unwrap_err();
    assert!(error.to_string().contains("encrypted"), "{error}");
    assert!(names(&destination).is_empty());

    let damaged = root.path().join("damaged.tar.gz");
    fs::write(&damaged, b"\x1f\x8b not really gzip").unwrap();
    let error = extract(&damaged, &destination).unwrap_err();
    assert!(error.to_string().contains("damaged"), "{error}");
    assert!(names(&destination).is_empty());
}

#[test]
fn zip_refuses_names_it_cannot_store() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let odd = root.path().join(OsString::from_vec(b"caf\xe9".to_vec()));
    fs::write(&odd, "x").unwrap();
    let request = ArchiveRequest::compress(
        vec![odd.clone()],
        root.path().to_owned(),
        "odd".into(),
        Format::Zip,
    );
    let error = run(&request).unwrap_err();
    assert!(error.to_string().contains(".tar"), "{error}");
    // Tar keeps the original bytes.
    let archive = compress(
        std::slice::from_ref(&odd),
        root.path(),
        "odd",
        Format::TarGz,
    );
    let destination = root.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let outcome = extract(&archive, &destination).unwrap();
    assert_eq!(outcome.output.file_name(), odd.file_name());
}

mod jobs {
    use super::*;
    use canopa_files::operation_jobs::{JobSnapshot, OperationJobs, Phase, Request};
    use std::time::{Duration, Instant};

    fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(Instant::now() < deadline, "the worker did not settle");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The history folder is kept apart from the archived folders.
    fn start(state: &Path) -> io::Result<OperationJobs> {
        let queue =
            OperationJobs::start(state.join("history.sqlite3"), state.join("volumes.json"))?;
        let state = wait_for(|| {
            let state = queue.snapshot();
            (!state.initializing).then_some(state)
        });
        match state.error {
            Some(error) => Err(io::Error::other(error)),
            None => Ok(queue),
        }
    }

    fn submit(queue: &OperationJobs, request: Request) -> u64 {
        let mut accepted = queue.submit(request, None).unwrap();
        wait_for(|| accepted.try_recv().unwrap()).unwrap()
    }

    fn job(queue: &OperationJobs, id: u64) -> Option<JobSnapshot> {
        queue.snapshot().jobs.into_iter().find(|job| job.id == id)
    }

    fn settled(queue: &OperationJobs, id: u64) -> JobSnapshot {
        wait_for(|| {
            job(queue, id).filter(|job| {
                matches!(
                    job.phase,
                    Phase::Finished | Phase::Stopped | Phase::Failed | Phase::Interrupted
                )
            })
        })
    }

    use std::io;

    #[test]
    fn jobs_publish_and_report_their_result() {
        let state = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let project = sample(root.path());
        let queue = start(state.path()).unwrap();
        let id = submit(
            &queue,
            Request::Archive(ArchiveRequest::compress(
                vec![project.clone()],
                root.path().to_owned(),
                "project".into(),
                Format::TarGz,
            )),
        );
        let compressed = settled(&queue, id);
        assert_eq!(compressed.phase, Phase::Finished, "{:?}", compressed.error);
        assert_eq!(compressed.title, "Compress 1 item into “project.tar.gz”");
        let archive = root.path().join("project.tar.gz");
        assert_eq!(&*compressed.paths, std::slice::from_ref(&archive));
        assert!(compressed.detail.unwrap().contains("project.tar.gz"));
        // Archive jobs never enter Undo.
        assert!(queue.snapshot().undo.is_none());

        let id = submit(
            &queue,
            Request::Archive(ArchiveRequest::extract(
                archive.clone(),
                root.path().to_owned(),
            )),
        );
        let extracted = settled(&queue, id);
        assert_eq!(extracted.phase, Phase::Finished, "{:?}", extracted.error);
        let output = root.path().join("project (2)");
        assert_eq!(&*extracted.paths, std::slice::from_ref(&output));
        assert_sample(&output);

        let id = submit(
            &queue,
            Request::Archive(ArchiveRequest::extract(
                root.path().join("missing.zip"),
                root.path().to_owned(),
            )),
        );
        let failed = settled(&queue, id);
        assert_eq!(failed.phase, Phase::Failed);
        assert!(failed.error.is_some());
        assert_eq!(
            names(root.path()),
            ["project", "project (2)", "project.tar.gz"]
        );
    }

    /// Starts a slow compression and returns its job and partial result's name.
    fn slow_compression(queue: &OperationJobs, root: &Path) -> (u64, PathBuf) {
        let large = root.join("large");
        let mut bytes = vec![0u8; 48 << 20];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for chunk in bytes.chunks_mut(8) {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            chunk.copy_from_slice(&seed.to_ne_bytes()[..chunk.len()]);
        }
        fs::write(&large, bytes).unwrap();
        let id = submit(
            queue,
            Request::Archive(ArchiveRequest::compress(
                vec![large],
                root.to_owned(),
                "large".into(),
                Format::TarXz,
            )),
        );
        let partial = wait_for(|| {
            job(queue, id).filter(|job| job.progress.as_ref().is_some_and(|p| p.bytes > 0))?;
            fs::read_dir(root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".canopa-archive-")
                })
        });
        (id, partial)
    }

    #[test]
    fn a_cancelled_job_stops_and_keeps_nothing() {
        let state = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let queue = start(state.path()).unwrap();
        let (id, _) = slow_compression(&queue, root.path());
        queue.cancel(id).unwrap();
        let cancelled = settled(&queue, id);
        assert_eq!(cancelled.phase, Phase::Stopped, "{:?}", cancelled.error);
        assert!(cancelled.detail.unwrap().contains("Cancelled"));
        assert!(cancelled.paths.is_empty());
        assert_eq!(names(root.path()), ["large"]);
    }

    #[test]
    fn an_interrupted_job_is_cleaned_up_when_history_reopens() {
        let state = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let queue = start(state.path()).unwrap();
        let (id, partial) = slow_compression(&queue, root.path());
        // Stopping the worker leaves the request recorded as running, as a
        // crash would.
        drop(queue);
        wait_for(|| (!partial.exists()).then_some(()));
        // The previous process left its partial result behind.
        fs::write(&partial, "unfinished").unwrap();
        let queue = wait_for(|| {
            start(state.path()).ok().or_else(|| {
                std::thread::sleep(Duration::from_millis(50));
                None
            })
        });
        let interrupted = job(&queue, id).unwrap();
        assert_eq!(interrupted.phase, Phase::Interrupted);
        assert!(
            interrupted
                .error
                .unwrap()
                .contains("unfinished result was removed"),
            "the cleanup is reported"
        );
        assert!(!partial.exists());
        assert_eq!(names(root.path()), ["large"]);
        drop(queue);
        // The settled report survives a second start without another cleanup.
        let queue = wait_for(|| {
            start(state.path()).ok().or_else(|| {
                std::thread::sleep(Duration::from_millis(50));
                None
            })
        });
        assert_eq!(job(&queue, id).unwrap().phase, Phase::Interrupted);
    }
}
