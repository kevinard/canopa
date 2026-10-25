use super::*;
use crate::file_clipboard::{GNOME, KDE, URIS};
use std::{
    fs::File,
    io::{Read, Write},
    time::{Duration, Instant},
};

pub(super) const DEADLINE: Duration = Duration::from_secs(5);

pub(super) struct Payload {
    pub selection: Selection,
    gnome: Arc<[u8]>,
    uris: Arc<[u8]>,
    kde: Arc<[u8]>,
}
impl Payload {
    pub fn new(selection: Selection) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            gnome: selection.encode(GNOME)?.into(),
            uris: selection.encode(URIS)?.into(),
            kde: selection.encode(KDE)?.into(),
            selection,
        }))
    }
    pub fn bytes(&self, mime: &str) -> Option<Arc<[u8]>> {
        match mime {
            GNOME => Some(self.gnome.clone()),
            URIS => Some(self.uris.clone()),
            KDE => Some(self.kde.clone()),
            _ => None,
        }
    }
}

pub(super) struct Send {
    pub file: File,
    bytes: Arc<[u8]>,
    offset: usize,
    pub deadline: Instant,
}
impl Send {
    pub fn new(file: File, bytes: Arc<[u8]>) -> io::Result<Self> {
        rustix::fs::fcntl_setfl(
            &file,
            rustix::fs::fcntl_getfl(&file)? | rustix::fs::OFlags::NONBLOCK,
        )?;
        Ok(Self {
            file,
            bytes,
            offset: 0,
            deadline: Instant::now() + DEADLINE,
        })
    }
    pub fn pump(&mut self) -> bool {
        let end = self.offset.saturating_add(64 * 1024).min(self.bytes.len());
        match self.file.write(&self.bytes[self.offset..end]) {
            Ok(0) => true,
            Ok(count) => {
                self.offset += count;
                self.offset == self.bytes.len()
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                false
            }
            Err(_) => true,
        }
    }
}

pub(super) struct Part {
    pub file: Option<File>,
    bytes: Vec<u8>,
    limit: usize,
}
impl Part {
    pub fn new(limit: usize) -> io::Result<(Self, File)> {
        let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        rustix::fs::fcntl_setfl(
            &reader,
            rustix::fs::fcntl_getfl(&reader)? | rustix::fs::OFlags::NONBLOCK,
        )?;
        Ok((
            Self {
                file: Some(reader.into()),
                bytes: Vec::new(),
                limit,
            },
            writer.into(),
        ))
    }
    fn pump(&mut self) -> io::Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        let mut buffer = [0u8; 64 * 1024];
        match file.read(&mut buffer) {
            Ok(0) => self.file = None,
            Ok(count) if self.bytes.len().saturating_add(count) <= self.limit => {
                self.bytes.extend_from_slice(&buffer[..count])
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The clipboard file list is too large",
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

pub(super) struct Receive {
    pub revision: u64,
    pub owned: Option<Intent>,
    pub parts: Vec<Part>,
    pub decoded: Option<Selection>,
    pub mime: &'static str,
    pub reply: Reply<Received>,
    pub deadline: Instant,
}
impl Receive {
    pub fn pump(&mut self) -> io::Result<bool> {
        if self.decoded.is_some() {
            return Ok(false);
        }
        for part in &mut self.parts {
            part.pump()?;
        }
        if self.parts.iter().all(|part| part.file.is_none()) {
            self.decoded = Some(Selection::decode(
                self.mime,
                &self.parts[0].bytes,
                self.parts.get(1).map(|part| part.bytes.as_slice()),
            )?);
            return Ok(true);
        }
        Ok(false)
    }
}
