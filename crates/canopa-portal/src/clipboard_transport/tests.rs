use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    time::Instant,
};

#[test]
fn replacement_workers_share_serials_without_reusing_exhausted_values() {
    let serials = Serials(Arc::new(AtomicU32::new(0)));
    assert_eq!(serials.next(), Some(1));
    assert_eq!(serials.clone().next(), Some(2));
    serials.0.store(u32::MAX - 1, Ordering::Release);
    assert_eq!(serials.clone().next(), Some(u32::MAX));
    assert_eq!(serials.next(), None);
    assert_eq!(serials.clone().next(), None);
}

#[test]
fn dropping_a_pump_revokes_duplicated_application_descriptors() {
    let (source, app_writer) = pipes::pipe().unwrap();
    let (app_reader, target) = pipes::pipe().unwrap();
    let pump = pipes::Pump::new(source, target).unwrap();
    let mut writer = File::from(app_writer).try_clone().unwrap();
    let mut reader = File::from(app_reader).try_clone().unwrap();
    drop(pump);
    assert_eq!(
        writer.write(b"after revocation").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn bounded_pump_preserves_binary_bytes_and_eof_under_backpressure() {
    let (source, app_writer) = pipes::pipe().unwrap();
    let (app_reader, target) = pipes::pipe().unwrap();
    let mut pump = pipes::Pump::new(source, target).unwrap();
    let expected: Vec<_> = (0..2 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect();
    let bytes = expected.clone();
    let producer = std::thread::spawn(move || File::from(app_writer).write_all(&bytes).unwrap());
    let mut reader = File::from(app_reader);
    rustix::fs::fcntl_setfl(&reader, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut actual = Vec::new();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let finished = pump.step().unwrap();
        let mut chunk = [0; 503];
        match reader.read(&mut chunk) {
            Ok(count) => actual.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
        if finished {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    drop(pump);
    reader.read_to_end(&mut actual).unwrap();
    producer.join().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn malformed_or_reserved_metadata_is_rejected_without_normalizing_valid_types() {
    for types in [
        vec!["".into()],
        vec!["text/plain\n".into()],
        vec![format!("{OWNER_PREFIX}forged")],
        vec!["x".repeat(MAX_MIME_BYTES + 1)],
        vec!["text/plain".into(); MAX_TYPES + 1],
        vec!["text/plain".into(), "text/plain".into()],
    ] {
        assert!(validate_types(&types).is_err());
    }
    assert!(validate_types(&[]).is_ok());
    assert!(
        validate_types(&[
            "text/plain;charset=utf-8".into(),
            "application/octet-stream".into()
        ])
        .is_ok()
    );
}

#[test]
fn regular_files_cannot_block_the_stream_worker() {
    let (reader, app_writer) = pipes::pipe().unwrap();
    let target = tempfile::tempfile().unwrap();
    assert!(pipes::Pump::new(reader, target.into()).is_err());
    assert_eq!(
        File::from(app_writer).write(b"retired").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}
