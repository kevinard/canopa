//! A sealed anonymous memfd carries the request's small credential cache across
//! PreparePrint. Only our retained helper can send this FD; grants stay public.
use super::{
    Challenge, Credentials, MAX_FRAME, Retained, Secret,
    broker::{CACHE_LIMIT, Entry, PROMPT_LIMIT},
};
use crate::print::file_target;
use std::{
    fs::File,
    os::{
        fd::AsFd,
        unix::{
            fs::{FileExt, MetadataExt},
            net::UnixStream,
        },
    },
};

const MAGIC: &[u8] = b"CANOPAPRINTAUTH\x01";
const MAX_CACHE: usize = 96 * 1024;
impl Retained {
    pub fn send(&self, channel: &UnixStream) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.entries.len() <= CACHE_LIMIT && self.prompts <= PROMPT_LIMIT,
            "Printer credential cache exceeds limits"
        );
        let encoded = self
            .entries
            .iter()
            .map(|entry| {
                anyhow::ensure!(
                    entry.challenge.interactive,
                    "Unapproved printer credential scope"
                );
                let mut challenge = entry.challenge.clone();
                challenge.id = 1;
                challenge.attempt = 1;
                Ok((
                    serde_json::to_vec(&challenge)?,
                    Credentials::encode(&challenge, Some(&entry.credentials))?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let length = MAGIC.len()
            + 2
            + encoded
                .iter()
                .map(|(c, v)| 8 + c.len() + v.bytes().len())
                .sum::<usize>();
        anyhow::ensure!(
            length <= MAX_CACHE,
            "Printer credential cache exceeds limits"
        );
        let mut frame = Secret::buffer(length);
        let bytes = frame.bytes_mut();
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[MAGIC.len()] = self.prompts;
        bytes[MAGIC.len() + 1] = encoded.len() as u8;
        let mut offset = MAGIC.len() + 2;
        for (challenge, credentials) in &encoded {
            for value in [challenge.as_slice(), credentials.bytes()] {
                bytes[offset..offset + 4].copy_from_slice(&(value.len() as u32).to_be_bytes());
                offset += 4;
                bytes[offset..offset + value.len()].copy_from_slice(value);
                offset += value.len();
            }
        }
        let file = File::from(rustix::fs::memfd_create(
            c"canopa-print-credentials",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )?);
        file.write_all_at(frame.bytes(), 0)?;
        rustix::fs::fcntl_add_seals(&file, seals())?;
        file_target::send_many(channel, &[file.as_fd()], 1)?;
        Ok(())
    }
    pub fn receive(channel: &UnixStream) -> anyhow::Result<Self> {
        let mut fds = file_target::receive_many(channel, 1)?
            .ok_or_else(|| anyhow::anyhow!("Missing printer credential handoff"))?;
        anyhow::ensure!(fds.len() == 1, "Invalid printer credential handoff");
        let file = File::from(fds.pop().expect("one descriptor"));
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file()
                && metadata.nlink() == 0
                && (MAGIC.len() + 2..=MAX_CACHE).contains(&(metadata.len() as usize))
                && rustix::fs::fcntl_get_seals(&file)?.contains(seals()),
            "Invalid printer credential memory"
        );
        let mut frame = Secret::buffer(metadata.len() as usize);
        file.read_exact_at(frame.bytes_mut(), 0)?;
        let result = Self::decode(frame.bytes())?;
        file_target::ensure_empty(channel)?;
        Ok(result)
    }
    fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() <= MAX_CACHE && bytes.starts_with(MAGIC) && bytes.len() >= MAGIC.len() + 2,
            "Invalid printer credential cache"
        );
        let prompts = bytes[MAGIC.len()];
        let count = bytes[MAGIC.len() + 1] as usize;
        anyhow::ensure!(
            count <= CACHE_LIMIT && prompts <= PROMPT_LIMIT && count <= prompts as usize,
            "Invalid printer credential cache limits"
        );
        let mut entries: Vec<Entry> = Vec::new();
        let mut offset = MAGIC.len() + 2;
        for _ in 0..count {
            let challenge: Challenge = serde_json::from_slice(next(bytes, &mut offset)?)?;
            challenge.validate()?;
            anyhow::ensure!(
                challenge.id == 1
                    && challenge.attempt == 1
                    && challenge.interactive
                    && !entries
                        .iter()
                        .any(|entry| entry.challenge.endpoint == challenge.endpoint
                            && entry.challenge.kind == challenge.kind),
                "Invalid printer credential cache scope"
            );
            let credentials = Credentials::decode(&challenge, next(bytes, &mut offset)?)?
                .ok_or_else(|| anyhow::anyhow!("Empty printer credential entry"))?;
            entries.push(Entry {
                challenge,
                credentials,
            });
        }
        anyhow::ensure!(
            offset == bytes.len(),
            "Trailing printer credential cache data"
        );
        Ok(Self { entries, prompts })
    }
}
fn seals() -> rustix::fs::SealFlags {
    rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::SEAL
}
fn next<'a>(bytes: &'a [u8], offset: &mut usize) -> anyhow::Result<&'a [u8]> {
    let size = bytes
        .get(*offset..*offset + 4)
        .ok_or_else(|| anyhow::anyhow!("Incomplete printer credential cache"))?;
    let size = u32::from_be_bytes(size.try_into()?) as usize;
    *offset += 4;
    anyhow::ensure!(size <= MAX_FRAME, "Printer credential entry exceeds limits");
    let value = bytes
        .get(*offset..*offset + size)
        .ok_or_else(|| anyhow::anyhow!("Incomplete printer credential entry"))?;
    *offset += size;
    Ok(value)
}

#[cfg(test)]
mod tests;
