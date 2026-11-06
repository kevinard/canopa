//! GnuPG-owned quality, constraint and generation inquiries. No local password policy.
use canopa_session_ui::secret::Secret;
use std::{
    io::{self, BufRead, Read, Write},
    os::fd::AsRawFd,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};
pub enum Kind {
    Quality(Secret),
    Check(Secret),
    Generate,
}
pub struct Query {
    pub revision: u64,
    pub kind: Kind,
}
pub enum Answer {
    Quality(Option<i16>),
    Check(Result<Option<String>, ()>),
    Generated(Option<Secret>),
}
pub trait Agent: Send {
    fn inquire(&mut self, query: Query) -> (u64, Answer);
}
pub trait InquiryRead: BufRead + Send {
    fn deadline(&mut self, deadline: Option<Instant>);
}
/// Erased input buffer; the inquiry deadline covers all partial reads, not each read.
pub struct Input {
    file: std::fs::File,
    bytes: Zeroizing<[u8; 1000]>,
    start: usize,
    end: usize,
    deadline: Option<Instant>,
}
impl Input {
    pub fn new(file: std::fs::File) -> Self {
        Self {
            file,
            bytes: Zeroizing::new([0; 1000]),
            start: 0,
            end: 0,
            deadline: None,
        }
    }
}
impl BufRead for Input {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.start == self.end {
            self.bytes.zeroize();
            self.start = 0;
            self.end = 0;
            if let Some(deadline) = self.deadline {
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    let mut fd = libc::pollfd {
                        fd: self.file.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let result = unsafe {
                        libc::poll(
                            &mut fd,
                            1,
                            remaining.as_millis().clamp(1, i32::MAX as u128) as i32,
                        )
                    };
                    if result > 0 {
                        break;
                    }
                    if result == 0 {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            }
            self.end = self.file.read(&mut self.bytes[..])?;
        }
        Ok(&self.bytes[self.start..self.end])
    }
    fn consume(&mut self, size: usize) {
        let end = (self.start + size).min(self.end);
        self.bytes[self.start..end].zeroize();
        self.start = end;
    }
}
impl Read for Input {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let source = self.fill_buf()?;
        let n = source.len().min(bytes.len());
        bytes[..n].copy_from_slice(&source[..n]);
        self.consume(n);
        Ok(n)
    }
}
impl InquiryRead for Input {
    fn deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }
}
impl InquiryRead for &[u8] {
    fn deadline(&mut self, _: Option<Instant>) {}
}
impl<T: AsRef<[u8]> + Send> InquiryRead for io::Cursor<T> {
    fn deadline(&mut self, _: Option<Instant>) {}
}
pub struct Peer<'a, R, W> {
    pub reader: &'a mut R,
    pub writer: &'a mut W,
    pub broken: bool,
}
impl<R: InquiryRead, W: Write + Send> Peer<'_, R, W> {
    fn exchange(&mut self, kind: &Kind) -> io::Result<Option<Secret>> {
        let mut frame = Zeroizing::new([0; 1000]);
        let (prefix, value) = match kind {
            Kind::Quality(s) => (b"INQUIRE QUALITY ".as_slice(), Some(s)),
            Kind::Check(s) => (b"INQUIRE CHECKPIN ".as_slice(), Some(s)),
            Kind::Generate => (b"INQUIRE GENPIN".as_slice(), None),
        };
        frame[..prefix.len()].copy_from_slice(prefix);
        let mut end = prefix.len();
        if let Some(secret) = value {
            // This is the upstream pinentry inquiry's 300-byte limit. Required
            // checks refuse longer values rather than approving an unchecked tail.
            if matches!(kind, Kind::Check(_)) && secret.as_bytes().len() > 300 {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            for &b in secret.as_bytes().iter().take(300) {
                if b == b' ' {
                    frame[end] = b'+';
                    end += 1;
                } else if !(0x21..=0x7e).contains(&b) || matches!(b, b'%' | b'+') {
                    let hex = b"0123456789ABCDEF";
                    frame[end..end + 3].copy_from_slice(&[
                        b'%',
                        hex[(b >> 4) as usize],
                        hex[(b & 15) as usize],
                    ]);
                    end += 3;
                } else {
                    frame[end] = b;
                    end += 1;
                }
            }
        }
        frame[end] = b'\n';
        self.writer.write_all(&frame[..=end])?;
        self.writer.flush()?;
        let mut result = Zeroizing::new(Vec::<u8>::with_capacity(8192));
        let mut got_data = false;
        for _ in 0..32 {
            let mut line = Zeroizing::new(Vec::with_capacity(1001));
            let count = self
                .reader
                .by_ref()
                .take(1001)
                .read_until(b'\n', &mut line)?;
            if count == 0 || count > 1000 || line.last() != Some(&b'\n') {
                return Err(io::ErrorKind::InvalidData.into());
            }
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let command = line
                .split(|b| matches!(b, b' ' | b'\t'))
                .next()
                .unwrap_or_default();
            if command == b"END" {
                if !got_data {
                    return Ok(None);
                }
                let text = std::str::from_utf8(&result).map_err(|_| io::ErrorKind::InvalidData)?;
                let mut secret = Secret::with_capacity(8193);
                if matches!(kind, Kind::Check(_)) {
                    if text
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                    {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    let clean = Zeroizing::new(text.replace(['\n', '\r', '\t'], " "));
                    secret.push(&clean);
                } else {
                    if text.chars().any(char::is_control) {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    secret.push(text);
                }
                return Ok(Some(secret));
            }
            if command == b"CAN" || command == b"ERR" {
                return Err(io::ErrorKind::Other.into());
            }
            if line.is_empty() || line[0] == b'#' || line.starts_with(b"S ") {
                continue;
            }
            let Some(data) = line.strip_prefix(b"D ") else {
                return Err(io::ErrorKind::InvalidData.into());
            };
            let mut decoded = Zeroizing::new(Vec::with_capacity(1000));
            let mut i = 0;
            while i < data.len() {
                let b = if data[i] == b'%' {
                    if i + 2 >= data.len() {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    let hi = (data[i + 1] as char)
                        .to_digit(16)
                        .ok_or(io::ErrorKind::InvalidData)?;
                    let lo = (data[i + 2] as char)
                        .to_digit(16)
                        .ok_or(io::ErrorKind::InvalidData)?;
                    i += 2;
                    (hi * 16 + lo) as u8
                } else {
                    data[i]
                };
                decoded.push(b);
                i += 1;
            }
            if result.len() + decoded.len() > 8192 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            result.extend_from_slice(&decoded);
            got_data = true;
        }
        Err(io::ErrorKind::InvalidData.into())
    }
}
impl<R: InquiryRead, W: Write + Send> Agent for Peer<'_, R, W> {
    fn inquire(&mut self, query: Query) -> (u64, Answer) {
        self.reader
            .deadline(Some(Instant::now() + Duration::from_secs(2)));
        let result = if self.broken {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            self.exchange(&query.kind)
        };
        self.reader.deadline(None);
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e.kind(), io::ErrorKind::Other | io::ErrorKind::InvalidInput))
        {
            self.broken = true;
        }
        let answer = match query.kind {
            Kind::Quality(_) => Answer::Quality(
                result
                    .ok()
                    .flatten()
                    .and_then(|s| s.text().parse::<i16>().ok())
                    .map(|v| v.clamp(-100, 100)),
            ),
            Kind::Check(_) => Answer::Check(
                result
                    .map(|s| {
                        s.filter(|s| !s.is_empty())
                            .map(|s| s.text().chars().take(256).collect())
                    })
                    .map_err(|_| ()),
            ),
            Kind::Generate => Answer::Generated(
                result
                    .ok()
                    .flatten()
                    .filter(|s| !s.is_empty() && s.count() <= 2048),
            ),
        };
        (query.revision, answer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn secret(text: &str) -> Secret {
        let mut secret = Secret::with_capacity(8193);
        secret.push(text);
        secret
    }
    #[test]
    fn independent_inquiry_frames_escape_values_and_reject_errors_and_unchecked_tails() {
        let mut reader = io::Cursor::new(
            b"D -73\nEND\nD Too%20short\nEND\nERR 99 unsupported\nD %C3\nD %A9+word\nEND\n",
        );
        let mut writer = Vec::new();
        let mut peer = Peer {
            reader: &mut reader,
            writer: &mut writer,
            broken: false,
        };
        assert!(matches!(
            peer.inquire(Query {
                revision: 4,
                kind: Kind::Quality(secret("a+b %é"))
            }),
            (4, Answer::Quality(Some(-73)))
        ));
        assert!(
            matches!(peer.inquire(Query {revision:5,kind:Kind::Check(secret("weak"))}), (5,Answer::Check(Ok(Some(reason)))) if reason=="Too short")
        );
        assert!(matches!(
            peer.inquire(Query {
                revision: 6,
                kind: Kind::Generate
            }),
            (6, Answer::Generated(None))
        ));
        assert!(!peer.broken);
        assert!(
            matches!(peer.inquire(Query {revision:7,kind:Kind::Generate}), (7,Answer::Generated(Some(secret))) if secret.text()=="é+word")
        );
        assert!(matches!(
            peer.inquire(Query {
                revision: 8,
                kind: Kind::Check(secret(&"x".repeat(301)))
            }),
            (8, Answer::Check(Err(())))
        ));
        assert_eq!(writer,b"INQUIRE QUALITY a%2Bb+%25%C3%A9\nINQUIRE CHECKPIN weak\nINQUIRE GENPIN\nINQUIRE GENPIN\n");
    }
    #[test]
    fn partial_input_cannot_extend_an_absolute_inquiry_deadline() {
        use std::os::fd::OwnedFd;
        let (read, mut write) = std::os::unix::net::UnixStream::pair().unwrap();
        write.write_all(b"D partial").unwrap();
        let fd: OwnedFd = read.into();
        let mut input = Input::new(fd.into());
        input.deadline(Some(Instant::now() + Duration::from_millis(60)));
        let start = Instant::now();
        let mut output = Vec::new();
        assert_eq!(
            input.read_until(b'\n', &mut output).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        assert!(input.bytes.iter().all(|b| *b == 0));
    }
}
