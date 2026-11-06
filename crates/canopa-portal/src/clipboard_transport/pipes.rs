use super::*;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use std::{fs::File, io::Read, time::Instant};

pub(super) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    Ok(rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?)
}
fn nonblocking(file: &File) -> io::Result<()> {
    let kind = rustix::fs::FileType::from_raw_mode(rustix::fs::fstat(file)?.st_mode);
    let unix_stream = kind.is_socket()
        && rustix::net::sockopt::socket_domain(file)? == rustix::net::AddressFamily::UNIX
        && rustix::net::sockopt::socket_type(file)? == rustix::net::SocketType::STREAM;
    if !(kind.is_fifo() || unix_stream) {
        return Err(io::Error::other(
            "Clipboard transfer requires a pipe or Unix stream",
        ));
    }
    Ok(fcntl_setfl(file, fcntl_getfl(file)? | OFlags::NONBLOCK)?)
}
/// At most 16 KiB per transfer, irrespective of the payload's total size.
pub(super) struct Pump {
    pub reader: File,
    pub writer: File,
    buffer: Box<[u8; 16 * 1024]>,
    start: usize,
    end: usize,
    eof: bool,
    idle: Instant,
    expires: Instant,
}
impl Pump {
    pub fn new(reader: OwnedFd, writer: OwnedFd) -> io::Result<Self> {
        let reader = File::from(reader);
        let writer = File::from(writer);
        nonblocking(&reader)?;
        nonblocking(&writer)?;
        Ok(Self {
            reader,
            writer,
            buffer: Box::new([0; 16 * 1024]),
            start: 0,
            end: 0,
            eof: false,
            idle: Instant::now() + IDLE,
            expires: Instant::now() + LIFETIME,
        })
    }
    pub fn wants_read(&self) -> bool {
        !self.eof && self.start == self.end
    }
    pub fn wants_write(&self) -> bool {
        self.start < self.end
    }
    pub fn deadline(&self) -> Instant {
        self.idle.min(self.expires)
    }
    /// One bounded read and write per turn, with EOF retained until admitted bytes drain.
    pub fn step(&mut self) -> io::Result<bool> {
        if self.wants_read() {
            self.start = 0;
            self.end = 0;
            match self.reader.read(&mut *self.buffer) {
                Ok(0) => self.eof = true,
                Ok(count) => {
                    self.end = count;
                    self.idle = Instant::now() + IDLE;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        if self.wants_write() {
            match self.writer.write(&self.buffer[self.start..self.end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => {
                    self.start += count;
                    self.idle = Instant::now() + IDLE;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(self.eof && self.start == self.end)
    }
}
