use super::*;
use crate::preview::{PreviewContent, PreviewRequest};
use std::{fs, path::Path, time::Instant};

fn prepared(path: &Path, bytes: &[u8]) -> MediaSource {
    fs::write(path, bytes).unwrap();
    match PreviewRequest::new(path)
        .prepare(&Cancellation::default())
        .unwrap()
        .content
    {
        PreviewContent::Audio(source) | PreviewContent::Video(source) => source,
        other => panic!("expected a media source, got {other:?}"),
    }
}

fn wait(session: &Session, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let state = session.snapshot();
        if predicate(&state) {
            return state;
        }
        assert!(
            !matches!(state.phase, Phase::Failed(_)) && Instant::now() < until,
            "unexpected media state: phase={:?} position={:?} seeking={} notice={:?}",
            state.phase,
            state.position,
            state.seeking,
            state.notice
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn retired(playback: &Playback) {
    let until = Instant::now() + Duration::from_secs(8);
    while playback.holds_application() {
        assert!(Instant::now() < until, "decoder teardown did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn pixel(state: &Snapshot) -> [u8; 3] {
    let bitmap = &state.frame.as_ref().unwrap().bitmap;
    assert!(bitmap.width <= 1280 && bitmap.height <= 720);
    assert_eq!(
        bitmap.rgba.len(),
        bitmap.width as usize * bitmap.height as usize * 4
    );
    let start = ((bitmap.height / 2 * bitmap.width + bitmap.width / 2) * 4) as usize;
    bitmap.rgba[start..start + 3].try_into().unwrap()
}

#[test]
fn video_uses_the_opened_file_for_paused_frames_seeks_rewind_and_owner_drop() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("café.webm");
    let bytes = include_bytes!("../../tests/fixtures/preview-colors.webm");
    let source = prepared(&path, bytes);
    let retained = root.path().join("retained.webm");
    fs::rename(&path, &retained).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &path,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let playback = Playback::with_output(AudioOutput::Discard);
    let notices = playback.take_notices().unwrap();
    assert!(playback.take_notices().is_none());
    let session = playback.open(source.clone()).unwrap();
    assert!(playback.holds_application());
    assert_eq!(
        playback.shutdown().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    // Losing presentation observation does not stop a still-owned session.
    drop(notices);
    let first = wait(&session, |s| {
        s.phase == Phase::Paused && s.frame.is_some() && s.seekable
    });
    assert!(pixel(&first)[0] > 240 && pixel(&first)[1] < 15);
    assert_eq!(first.duration, Some(Duration::from_secs(6)));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(session.snapshot().phase, Phase::Paused);
    assert!(session.snapshot().position < Duration::from_millis(50));
    session
        .request(Command::Seek(Duration::from_secs(4)))
        .unwrap();
    let later = wait(&session, |s| {
        !s.seeking
            && s.frame
                .as_ref()
                .is_some_and(|f| f.timestamp.is_some_and(|t| t >= Duration::from_secs(4)))
    });
    assert!(pixel(&later)[0] < 15 && (110..140).contains(&pixel(&later)[1]));
    assert_eq!(later.phase, Phase::Paused);
    session.request(Command::Seek(Duration::ZERO)).unwrap();
    let rewind = wait(&session, |s| {
        !s.seeking
            && s.frame.as_ref().is_some_and(|f| {
                f.serial > later.frame.as_ref().unwrap().serial
                    && f.timestamp == Some(Duration::ZERO)
            })
    });
    assert!(pixel(&rewind)[0] > 240);
    session.request(Command::SeekBy(2)).unwrap();
    session.request(Command::SeekBy(2)).unwrap();
    let relative = wait(&session, |s| {
        !s.seeking
            && s.frame.as_ref().is_some_and(|f| {
                f.serial > rewind.frame.as_ref().unwrap().serial
                    && f.timestamp == Some(Duration::from_secs(4))
            })
    });
    assert!(pixel(&relative)[0] < 15 && (110..140).contains(&pixel(&relative)[1]));
    session.request(Command::SeekBy(-2)).unwrap();
    session.request(Command::SeekBy(-2)).unwrap();
    let composed = wait(&session, |s| {
        !s.seeking
            && s.frame.as_ref().is_some_and(|f| {
                f.serial > relative.frame.as_ref().unwrap().serial
                    && f.timestamp == Some(Duration::ZERO)
            })
    });
    assert!(pixel(&composed)[0] > 240);
    for position in [4, 0, 4] {
        session
            .request(Command::Seek(Duration::from_secs(position)))
            .unwrap();
    }
    let burst = wait(&session, |s| {
        !s.seeking
            && s.frame.as_ref().is_some_and(|f| {
                f.serial > composed.frame.as_ref().unwrap().serial
                    && f.timestamp.is_some_and(|t| t >= Duration::from_secs(4))
            })
    });
    assert!(pixel(&burst)[0] < 15 && (110..140).contains(&pixel(&burst)[1]));
    session.request(Command::Seek(Duration::ZERO)).unwrap();
    wait(&session, |s| {
        !s.seeking
            && s.frame.as_ref().is_some_and(|f| {
                f.serial > burst.frame.as_ref().unwrap().serial
                    && f.timestamp == Some(Duration::ZERO)
            })
    });
    session.request(Command::Playing(true)).unwrap();
    wait(&session, |s| {
        s.phase == Phase::Playing && s.position > Duration::from_millis(100)
    });
    session.request(Command::Playing(false)).unwrap();
    wait(&session, |s| s.phase == Phase::Paused);
    assert_eq!(fs::read(&retained).unwrap(), bytes);
    drop(session);
    retired(&playback);
    playback.shutdown().unwrap();
    assert!(matches!(
        playback.open(source),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe
    ));
}

fn wave() -> Vec<u8> {
    let samples = 12_000u32;
    let size = samples * 2;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(48_000u32.to_le_bytes());
    bytes.extend(96_000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for index in 0..samples {
        let sample =
            (6000.0 * (index as f64 * std::f64::consts::TAU * 440.0 / 48_000.0).sin()) as i16;
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

#[test]
fn audio_uses_the_id3_adjusted_input_and_orders_controls_completion_and_stop() {
    let root = tempfile::tempdir().unwrap();
    let mut tagged = b"ID3\x04\x00\x00\x00\x00\x00\x04tag!".to_vec();
    tagged.extend(wave());
    let source = prepared(&root.path().join("tagged.wav"), &tagged);
    let playback = Playback::with_output(AudioOutput::Discard);
    let session = playback.open(source.clone()).unwrap();
    wait(&session, |s| s.phase == Phase::Paused && s.seekable);
    assert!(session.snapshot().frame.is_none());
    assert!(session.request(Command::Volume(f64::NAN)).is_err());
    assert!(session.request(Command::Volume(1.1)).is_err());
    assert!(session.request(Command::VolumeBy(f64::NAN)).is_err());
    assert!(session.request(Command::VolumeBy(1.1)).is_err());
    session.request(Command::Volume(0.25)).unwrap();
    session.request(Command::Muted(true)).unwrap();
    session.request(Command::Playing(true)).unwrap();
    let ended = wait(&session, |s| s.phase == Phase::Ended);
    assert_eq!(ended.volume, 0.25);
    assert!(ended.muted);
    assert_eq!(ended.duration, Some(Duration::from_millis(250)));
    session.request(Command::TogglePlayback).unwrap();
    wait(&session, |s| s.phase == Phase::Playing);
    wait(&session, |s| {
        s.phase == Phase::Ended && s.revision > ended.revision
    });
    session.request(Command::Seek(Duration::ZERO)).unwrap();
    wait(&session, |s| {
        s.phase == Phase::Paused && !s.seeking && s.position < Duration::from_millis(20)
    });
    session.request(Command::TogglePlayback).unwrap();
    wait(&session, |s| s.phase == Phase::Playing);
    session.stop();
    retired(&playback);
    assert_eq!(session.snapshot().phase, Phase::Stopped);
    assert!(matches!(
        session.request(Command::Playing(true)),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe
    ));
    // Immediate owner loss must also retire an admitted, still-preparing worker.
    for _ in 0..4 {
        drop(playback.open(source.clone()).unwrap());
    }
    retired(&playback);
    playback.shutdown().unwrap();
}

#[test]
fn corrupt_input_reports_a_visible_failure_and_releases_its_activity() {
    let root = tempfile::tempdir().unwrap();
    let source = prepared(&root.path().join("broken.webm"), b"Invalid media input.\n");
    let playback = Playback::with_output(AudioOutput::Discard);
    let session = playback.open(source).unwrap();
    let failed = wait(&session, |s| matches!(s.phase, Phase::Failed(_)));
    assert!(
        matches!(failed.phase, Phase::Failed(ref message) if message.contains("usual application"))
    );
    retired(&playback);
    assert!(session.request(Command::Playing(true)).is_err());
    playback.shutdown().unwrap();
}

#[test]
fn closing_one_session_keeps_another_owned_session_playable() {
    let root = tempfile::tempdir().unwrap();
    let source = prepared(&root.path().join("audio.wav"), &wave());
    let playback = Playback::with_output(AudioOutput::Discard);
    let first = playback.open(source.clone()).unwrap();
    let second = playback.open(source).unwrap();
    wait(&first, |s| s.phase == Phase::Paused);
    wait(&second, |s| s.phase == Phase::Paused);
    first.stop();
    wait(&first, |s| s.phase == Phase::Stopped);
    assert!(playback.holds_application());
    assert_eq!(
        playback.shutdown().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    second.request(Command::Playing(true)).unwrap();
    wait(&second, |s| s.phase == Phase::Playing);
    wait(&second, |s| s.phase == Phase::Ended);
    drop(first);
    assert!(playback.holds_application());
    drop(second);
    retired(&playback);
    playback.shutdown().unwrap();
}

#[test]
fn a_fresh_headless_runtime_does_not_initialize_gstreamer() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "playback::tests::lazy_child", "--ignored"])
        .env("CANOPA_MEDIA_LAZY_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "invoked in a fresh process by the lazy-runtime parent"]
fn lazy_child() {
    if std::env::var_os("CANOPA_MEDIA_LAZY_CHILD").is_none() {
        return;
    }
    use std::sync::atomic::Ordering;
    assert!(!gstreamer::INITIALIZED.load(Ordering::SeqCst));
    let playback = Playback::default();
    assert!(!playback.holds_application());
    playback.shutdown().unwrap();
    assert!(!gstreamer::INITIALIZED.load(Ordering::SeqCst));
}
