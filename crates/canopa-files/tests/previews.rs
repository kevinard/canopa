use canopa_files::{
    Cancellation,
    preview::{PreviewContent, PreviewRequest},
};
use std::fs;

#[test]
fn image_and_text_previews_read_real_files_and_bound_large_text() {
    let root = tempfile::tempdir().unwrap();
    let image_path = root.path().join("coast.png");
    image::RgbaImage::from_pixel(12, 8, image::Rgba([24, 80, 64, 255]))
        .save(&image_path)
        .unwrap();
    let preview = PreviewRequest::new(&image_path)
        .prepare(&Cancellation::default())
        .unwrap();
    let PreviewContent::Image(bitmap) = preview.content else {
        panic!("expected image")
    };
    assert_eq!((bitmap.width, bitmap.height), (12, 8));
    assert_eq!(&bitmap.rgba[..4], &[24, 80, 64, 255]);

    let text_path = root.path().join("notes.rs");
    fs::write(&text_path, "fn main() {}\n".repeat(200_000)).unwrap();
    let preview = PreviewRequest::new(&text_path)
        .prepare(&Cancellation::default())
        .unwrap();
    let PreviewContent::Text {
        text,
        truncated,
        language,
        highlights,
    } = preview.content
    else {
        panic!("expected text")
    };
    assert!(text.starts_with("fn main() {}\n"));
    assert!(truncated);
    assert!(text.len() <= 1024 * 1024);
    assert_eq!(language.as_deref(), Some("rs"));
    assert!(
        highlights
            .iter()
            .any(|span| span.start == 0 && span.end == 2)
    );

    let prefix = "α = 1;\n// café\n";
    fs::write(&text_path, format!("{prefix}fn main() {{}}\n")).unwrap();
    let PreviewContent::Text { highlights, .. } = PreviewRequest::new(&text_path)
        .prepare(&Cancellation::default())
        .unwrap()
        .content
    else {
        panic!("expected source preview")
    };
    assert!(
        highlights
            .iter()
            .any(|span| span.start == prefix.chars().count() as u32
                && span.end == prefix.chars().count() as u32 + 2)
    );
}

#[test]
fn previews_cancel_and_reject_special_files_without_waiting_for_a_writer() {
    let root = tempfile::tempdir().unwrap();
    let fifo = root.path().join("pipe");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert_eq!(
        PreviewRequest::new(&fifo)
            .prepare(&Cancellation::default())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        PreviewRequest::new(root.path())
            .prepare(&cancellation)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
}
#[test]
fn a_stored_filename_hint_classifies_media_without_changing_the_read_path() {
    use canopa_files::preview::{PreviewContent, PreviewRequest};
    let root = tempfile::tempdir().unwrap();
    let stored = root.path().join("opaque-storage-id");
    std::fs::write(&stored, b"RIFF\0\0\0\0WAVE\0\0\0\0").unwrap();
    let mut request = PreviewRequest::new(&stored);
    request.name_hint = Some("recording.wav".into());
    let preview = request
        .prepare(&canopa_files::Cancellation::default())
        .unwrap();
    assert_eq!(preview.path, stored);
    assert!(matches!(preview.content, PreviewContent::Audio(_)));
}

#[test]
fn prepared_media_keeps_the_opened_file_when_its_name_is_replaced() {
    use std::io::{Read, Seek, SeekFrom};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("recording.wav");
    fs::write(&path, b"RIFF\0\0\0\0WAVE original recording").unwrap();
    let preview = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap();
    let PreviewContent::Audio(source) = preview.content else {
        panic!("audio")
    };
    fs::rename(&path, root.path().join("moved-original.wav")).unwrap();
    fs::write(&path, b"RIFF\0\0\0\0WAVE unrelated replacement").unwrap();
    let mut reader = source.reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"RIFF\0\0\0\0WAVE original recording");
    reader.seek(SeekFrom::Start(12)).unwrap();
    bytes.clear();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b" original recording");
}

#[test]
fn media_readers_seek_independently_and_ignore_a_fifo_replacement() {
    use std::io::{Read, Seek, SeekFrom};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("clip.webm");
    let original = b"\0 original media bytes";
    fs::write(&path, original).unwrap();
    let prepared = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap();
    let PreviewContent::Video(source) = prepared.content else {
        panic!("video")
    };
    fs::rename(&path, root.path().join("original")).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &path,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert_eq!(
        PreviewRequest::new(&path)
            .prepare(&Cancellation::default())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut first = source.reader();
    let mut second = source.reader();
    drop(source);
    let (sender, result) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut prefix = [0; 3];
        first.read_exact(&mut prefix).unwrap();
        assert_eq!(&prefix, b"\0 o");
        let mut bytes = Vec::new();
        second.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, original);
        assert_eq!(first.stream_position().unwrap(), 3);
        first.seek(SeekFrom::End(-5)).unwrap();
        bytes.clear();
        first.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"bytes");
        first.seek(SeekFrom::Start(u64::MAX)).unwrap();
        assert_eq!(first.read(&mut prefix).unwrap(), 0);
        assert_eq!(
            first.seek(SeekFrom::Current(1)).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        first.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            first.seek(SeekFrom::Current(-1)).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        sender.send(()).unwrap();
    });
    result
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("a prepared media reader must never wait on the replacement FIFO");
}

#[test]
fn prepared_media_stops_at_its_original_length_when_the_file_grows() {
    use std::io::{Read, Seek, SeekFrom, Write};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("growing.wav");
    let original = b"RIFF\0\0\0\0WAVE original";
    fs::write(&path, original).unwrap();
    let prepared = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap();
    let PreviewContent::Audio(source) = prepared.content else {
        panic!("audio")
    };
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b" appended recording")
        .unwrap();
    let mut reader = source.reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, original);
    assert_eq!(
        reader.seek(SeekFrom::End(0)).unwrap(),
        original.len() as u64
    );
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn mp3_preview_reads_audio_after_leading_id3_metadata_without_changing_the_file() {
    use std::io::{Read, Seek, SeekFrom};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tagged.mp3");
    // ID3v2.4 with 130 metadata bytes and its optional ten-byte footer.
    let header = [b'I', b'D', b'3', 4, 0, 0x10, 0, 0, 1, 2];
    let mut original = header.to_vec();
    original.extend(b"TIT2\0\0\0\x78\0\0\x03");
    original.extend([b'T'; 119]);
    original.extend(b"3DI");
    original.extend(&header[3..]);
    let audio = b"\xff\xfb\x90\x64 audio frames";
    original.extend(audio);
    fs::write(&path, &original).unwrap();
    let prepared = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap();
    assert_eq!(prepared.size, original.len() as u64);
    let PreviewContent::Audio(source) = prepared.content else {
        panic!("audio")
    };
    fs::rename(&path, root.path().join("original.mp3")).unwrap();
    fs::write(&path, b"unrelated replacement").unwrap();
    let mut reader = source.reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, audio);
    assert_eq!(
        reader.seek(SeekFrom::End(-3)).unwrap(),
        audio.len() as u64 - 3
    );
    bytes.clear();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"mes");
    assert_eq!(
        fs::read(root.path().join("original.mp3")).unwrap(),
        original
    );
}

#[test]
fn leading_id3_is_handled_by_content_in_audio_and_video_previews() {
    use std::io::Read;
    let root = tempfile::tempdir().unwrap();
    for extension in ["aac", "mp4"] {
        let path = root.path().join(format!("recording.{extension}"));
        let mut bytes = b"ID3\x03\0\0\0\0\0\x10TIT2\0\0\0\x06\0\0\0Title".to_vec();
        let frames = b"\xff\xfb\x90\x64 audio frames";
        bytes.extend(frames);
        fs::write(&path, &bytes).unwrap();
        let prepared = PreviewRequest::new(&path)
            .prepare(&Cancellation::default())
            .unwrap();
        let source = match prepared.content {
            PreviewContent::Audio(source) | PreviewContent::Video(source) => source,
            _ => panic!("media"),
        };
        let mut decoded = Vec::new();
        source.reader().read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, frames, "{extension}");
    }
}

#[test]
fn invalid_id3_envelopes_are_rejected_before_decoder_input() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("invalid.mp3");
    let mut cases = vec![
        b"ID3".to_vec(),
        b"ID3\x04\0\0\0\0\0\x7f".to_vec(),
        b"ID3\x04\0\0\0\0\0\x80".to_vec(),
        b"ID3\x04\0\x01\0\0\0\0".to_vec(),
        b"ID3\x03\0\x10\0\0\0\0".to_vec(),
        b"ID3\x05\0\0\0\0\0\0".to_vec(),
        b"ID3\x04\xff\0\0\0\0\0".to_vec(),
        b"ID3\x04\0\x10\0\0\0\0broken footer".to_vec(),
    ];
    cases.push(b"ID3\x04\0\0\0\0\0\0".repeat(33));
    for bytes in cases {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            PreviewRequest::new(&path)
                .prepare(&Cancellation::default())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}

#[test]
fn id3_input_skips_sparse_metadata_and_multiple_tag_versions() {
    use std::io::{Read, Seek, SeekFrom, Write};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("album.mp3");
    let frames = b"\xff\xfb\x90\x64 audio frames";
    let mut file = fs::File::create(&path).unwrap();
    // A valid title frame followed by sparse padding in a 128 MiB ID3v2.3 tag.
    file.write_all(b"ID3\x03\0\0\x40\0\0\0TIT2\0\0\0\x06\0\0\0Title")
        .unwrap();
    file.seek(SeekFrom::Start(10 + 128 * 1024 * 1024)).unwrap();
    file.write_all(frames).unwrap();
    drop(file);
    let preview = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap();
    assert!(preview.size > 128 * 1024 * 1024);
    let PreviewContent::Audio(source) = preview.content else {
        panic!("audio")
    };
    let mut bytes = Vec::new();
    source.reader().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, frames);

    let mut tagged = b"ID3\x02\0\0\0\0\0\x0cTT2\0\0\x06\0Title".to_vec();
    tagged.extend(b"ID3\x03\0\0\0\0\0\x10TIT2\0\0\0\x06\0\0\0Title");
    tagged.extend(frames);
    fs::write(&path, tagged).unwrap();
    let PreviewContent::Audio(source) = PreviewRequest::new(&path)
        .prepare(&Cancellation::default())
        .unwrap()
        .content
    else {
        panic!("audio")
    };
    bytes.clear();
    source.reader().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, frames);
}
