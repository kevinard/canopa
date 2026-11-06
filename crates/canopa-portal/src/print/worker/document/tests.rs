use super::*;
use std::{io::Write, os::fd::OwnedFd};

fn target() -> File {
    File::from(
        rustix::fs::memfd_create(
            c"print-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    )
}
fn cancellation() -> (Cancellation, OwnedFd) {
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    (Cancellation(File::from(read)), write)
}

#[test]
fn regular_files_start_at_the_received_offset_without_changing_shared_position_or_flags() {
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(b"skip%PDF-1.7\nowned document").unwrap();
    source.seek(SeekFrom::Start(4)).unwrap();
    let before_flags = rustix::fs::fcntl_getfl(&source).unwrap();
    let output = target();
    let (cancel, _writer) = cancellation();
    let document = snapshot(
        source.try_clone().unwrap(),
        output.try_clone().unwrap(),
        &cancel,
    )
    .unwrap();
    assert_eq!(source.stream_position().unwrap(), 4);
    assert_eq!(rustix::fs::fcntl_getfl(&source).unwrap(), before_flags);
    assert_eq!(document.format, Some(Format::Pdf));
    assert_eq!(document.bytes, 23);
    source.write_at(b"changed", 4).unwrap();
    let mut actual = Vec::new();
    (&output).read_to_end(&mut actual).unwrap();
    assert_eq!(actual, b"%PDF-1.7\nowned document");
    assert!(rustix::fs::ftruncate(&output, 0).is_err());
    assert!(output.write_at(b"changed", 0).is_err());
    assert_eq!(verify(&output).unwrap().bytes, document.bytes);
}

#[test]
fn pipes_are_consumed_to_eof_and_opaque_printer_formats_are_retained() {
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let mut write = File::from(write);
    write.write_all(b"plain text for a printer").unwrap();
    drop(write);
    let output = target();
    let (cancel, _writer) = cancellation();
    let document = snapshot(File::from(read), output.try_clone().unwrap(), &cancel).unwrap();
    assert_eq!(document.format, None);
    assert_eq!(document.bytes, 24);
    assert!(verify(&output).is_ok());
}

#[test]
fn nonblocking_pipes_wait_for_data_without_changing_their_status_flags() {
    let (read, write) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    let flags = rustix::fs::fcntl_getfl(&read).unwrap();
    let observer = rustix::io::dup(&read).unwrap();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut write = File::from(write);
        write.write_all(b"delayed pipe data").unwrap();
    });
    let (cancel, _writer) = cancellation();
    let result = snapshot(File::from(read), target(), &cancel).unwrap();
    writer.join().unwrap();
    assert_eq!(result.bytes, 17);
    assert_eq!(rustix::fs::fcntl_getfl(&observer).unwrap(), flags);
}

#[test]
fn empty_oversized_and_unsealed_documents_do_not_become_print_snapshots() {
    let (cancel, _writer) = cancellation();
    assert!(snapshot(tempfile::tempfile().unwrap(), target(), &cancel).is_err());
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(b"12345678901234567").unwrap();
    source.rewind().unwrap();
    assert!(copy(source, target(), &cancel, 16, false).is_err());
    let mut unsealed = target();
    unsealed.write_all(b"%PDF-1.7\n").unwrap();
    assert!(verify(&unsealed).is_err());
}

#[test]
fn cancellation_precedes_reading_and_never_seals_the_output() {
    let (read, _source_writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (cancel, writer) = cancellation();
    drop(writer);
    let output = target();
    assert!(snapshot(File::from(read), output.try_clone().unwrap(), &cancel).is_err());
    assert_eq!(output.metadata().unwrap().len(), 0);
    assert!(rustix::fs::fcntl_get_seals(&output).unwrap().is_empty());
}

#[test]
fn output_must_be_a_fresh_distinct_sealable_file() {
    let (cancel, _writer) = cancellation();
    let mut source = target();
    source.write_all(b"document").unwrap();
    source.rewind().unwrap();
    assert!(
        snapshot(
            source.try_clone().unwrap(),
            source.try_clone().unwrap(),
            &cancel
        )
        .is_err()
    );
    assert!(
        snapshot(
            source.try_clone().unwrap(),
            tempfile::tempfile().unwrap(),
            &cancel
        )
        .is_err()
    );
    let mut used = target();
    used.write_all(b"not empty").unwrap();
    assert!(snapshot(source, used, &cancel).is_err());
}

#[test]
fn format_hints_are_optional_and_do_not_reject_other_readable_print_contents() {
    assert_eq!(identify(b"%PDF-1.7\n"), Some(Format::Pdf));
    assert_eq!(identify(b"%!PS-Adobe-3.0\n"), Some(Format::Ps));
    assert_eq!(
        identify(b"<?xml version=\"1.0\"?><svg />"),
        Some(Format::Svg)
    );
    assert_eq!(identify(b"<!-- comment --><svg>"), Some(Format::Svg));
    assert_eq!(identify(b"<svgfoo>"), None);
    assert_eq!(identify(b"arbitrary readable print contents"), None);
}

#[test]
fn file_staging_copies_only_the_approved_sealed_format_and_preserves_shared_offsets() {
    let (cancel, _writer) = cancellation();
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(b"%PDF-1.7\nstaged document").unwrap();
    source.rewind().unwrap();
    let frozen = target();
    snapshot(source, frozen.try_clone().unwrap(), &cancel).unwrap();
    (&frozen).seek(SeekFrom::Start(7)).unwrap();
    let stage_file = tempfile::tempfile().unwrap();
    stage(&frozen, &stage_file, Format::Pdf, &cancel).unwrap();
    assert_eq!((&frozen).stream_position().unwrap(), 7);
    assert_eq!(stage_file.metadata().unwrap().len(), 24);
    let mut bytes = [0; 24];
    stage_file.read_exact_at(&mut bytes, 0).unwrap();
    assert_eq!(&bytes, b"%PDF-1.7\nstaged document");
    let wrong = tempfile::tempfile().unwrap();
    assert!(stage(&frozen, &wrong, Format::Ps, &cancel).is_err());
    assert_eq!(wrong.metadata().unwrap().len(), 0);
    assert!(stage(&frozen, &stage_file, Format::Pdf, &cancel).is_err());
    assert!(stage(&target(), &wrong, Format::Pdf, &cancel).is_err());
}
