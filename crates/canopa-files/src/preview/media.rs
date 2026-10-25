use std::{
    fs,
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::FileExt,
    sync::Arc,
};

/// A prepared media source supplies independent seekable readers to decoders.
#[derive(Clone, Debug)]
pub struct MediaSource {
    file: Arc<fs::File>,
    start: u64,
    length: u64,
}
impl MediaSource {
    pub(super) fn new(
        file: fs::File,
        length: u64,
        cancellation: &crate::Cancellation,
    ) -> io::Result<Self> {
        let start = leading_id3_end(&file, length, cancellation)?;
        Ok(Self {
            file: Arc::new(file),
            start,
            length: length - start,
        })
    }
    /// Readers retain the opened file, have independent positions and stop at
    /// its prepared length. In-place edits are not an immutable byte snapshot.
    /// Decoder input omits leading ID3 tags; the original file stays intact.
    pub fn reader(&self) -> impl Read + Seek + Send + 'static {
        Reader {
            source: self.clone(),
            position: 0,
        }
    }
}
struct Reader {
    source: MediaSource,
    position: u64,
}
impl Read for Reader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let available = self.source.length.saturating_sub(self.position);
        let amount = buffer
            .len()
            .min(usize::try_from(available).unwrap_or(usize::MAX));
        if amount == 0 {
            return Ok(0);
        }
        let count = self
            .source
            .file
            .read_at(&mut buffer[..amount], self.source.start + self.position)?;
        self.position += count as u64;
        Ok(count)
    }
}

fn leading_id3_end(
    file: &fs::File,
    length: u64,
    cancellation: &crate::Cancellation,
) -> io::Result<u64> {
    // GTK/GStreamer can abort while demuxing ID3 before MP3 audio (ADR 0025).
    // Skip only the bounded tag envelope, without decoding or modifying it.
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Invalid media metadata");
    let mut start = 0;
    let mut tags = 0;
    loop {
        super::check_cancel(cancellation)?;
        let mut header = [0; 10];
        let amount = (length - start).min(header.len() as u64) as usize;
        file.read_exact_at(&mut header[..amount], start)?;
        if !header.starts_with(b"ID3") {
            return Ok(start);
        }
        if amount < header.len() || tags == 32 || header[4] == 0xff {
            return Err(invalid());
        }
        let allowed_flags = match header[3] {
            2 => 0xc0,
            3 => 0xe0,
            4 => 0xf0,
            _ => return Err(invalid()),
        };
        if header[5] & !allowed_flags != 0 || header[6..].iter().any(|byte| byte & 0x80 != 0) {
            return Err(invalid());
        }
        let size = header[6..]
            .iter()
            .fold(0u64, |size, byte| (size << 7) | u64::from(*byte));
        let footer = header[3] == 4 && header[5] & 0x10 != 0;
        let end = start
            .checked_add(10 + size + if footer { 10 } else { 0 })
            .filter(|end| *end <= length)
            .ok_or_else(invalid)?;
        if footer {
            let mut actual = [0; 10];
            file.read_exact_at(&mut actual, end - 10)?;
            if &actual[..3] != b"3DI" || actual[3..] != header[3..] {
                return Err(invalid());
            }
        }
        start = end;
        tags += 1;
    }
}
impl Seek for Reader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
            SeekFrom::End(delta) => i128::from(self.source.length) + i128::from(delta),
        };
        self.position = u64::try_from(position)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid media position"))?;
        Ok(self.position)
    }
}
