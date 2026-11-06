use super::*;
use std::{fs::File, io::Write, os::fd::AsFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) struct Keyboard {
    lease: super::super::Lease,
    keymap: File,
    update: [u8; 16],
    filled: usize,
    held: Vec<u32>,
    frame: Vec<u8>,
    pub state: Option<State>,
}
impl Keyboard {
    pub async fn open(proxy: &zbus::Proxy<'_>) -> anyhow::Result<Self> {
        let mut lease = super::super::open(proxy).await?;
        let result = (|| {
            use rustix::fs::{MemfdFlags, SealFlags};
            let mut keymap = File::from(rustix::fs::memfd_create(
                "canopa-eis-keymap",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )?);
            keymap.write_all(lease.1.as_bytes())?;
            keymap.write_all(&[0])?;
            rustix::fs::fcntl_add_seals(
                &keymap,
                SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
            )?;
            Ok::<_, anyhow::Error>(keymap)
        })();
        let keymap = match result {
            Ok(map) => map,
            Err(error) => {
                super::super::release(&mut lease.0).await?;
                return Err(error);
            }
        };
        Ok(Self {
            lease,
            keymap,
            update: [0; 16],
            filled: 0,
            held: Vec::new(),
            frame: Vec::new(),
            state: None,
        })
    }
    pub fn add(
        &mut self,
        seat: &reis::request::Seat,
        converter: &EisRequestConverter,
        budget: &mut Pending,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(self.state.is_none(), "EI keyboard already bound");
        let added = seat.add_device(
            Some("Canopa keyboard"),
            eis::device::DeviceType::Virtual,
            DeviceCapability::Keyboard.into(),
            |device| {
                let keyboard: eis::Keyboard = device.interface().expect("keyboard");
                keyboard.keymap(
                    eis::keyboard::KeymapType::Xkb,
                    (self.lease.1.len() + 1) as u32,
                    self.keymap.as_fd(),
                );
            },
        );
        budget.attach(&added)?;
        self.state = Some(State::new(added, converter));
        self.modifiers(converter);
        Ok(())
    }
    pub fn modifiers(&self, converter: &EisRequestConverter) {
        if let Some(state) = &self.state {
            let keyboard: eis::Keyboard = state.device.interface().expect("keyboard");
            let mods = self.lease.2;
            converter.handle().with_next_serial(|serial| {
                keyboard.modifiers(serial, mods[0], mods[2], mods[1], mods[3])
            });
        }
    }
    pub async fn read(&mut self) -> anyhow::Result<bool> {
        let n = self.lease.0.read(&mut self.update[self.filled..]).await?;
        if n == 0 {
            anyhow::ensure!(self.filled == 0, "Truncated input state");
            return Ok(false);
        }
        self.filled += n;
        if self.filled == self.update.len() {
            self.lease.2 = std::array::from_fn(|i| {
                u32::from_le_bytes(self.update[i * 4..i * 4 + 4].try_into().unwrap())
            });
            self.filled = 0;
        }
        Ok(true)
    }
    pub fn key(&mut self, key: u32, state: eis::keyboard::KeyState) -> anyhow::Result<()> {
        anyhow::ensure!((1..=0x2ff).contains(&key), "Unapproved EI key");
        let pressed = state == eis::keyboard::KeyState::Press;
        let existing = self.held.iter().position(|k| *k == key);
        match (pressed, existing) {
            (true, None) if self.held.len() < 64 => self.held.push(key),
            (false, Some(i)) => {
                self.held.remove(i);
            }
            _ => anyhow::bail!("Invalid EI key transition"),
        }
        anyhow::ensure!(self.frame.len() < 512 * 8, "EI frame exceeded limit");
        self.frame.extend_from_slice(&key.to_le_bytes());
        self.frame
            .extend_from_slice(&u32::from(pressed).to_le_bytes());
        Ok(())
    }
    pub async fn frame(&mut self) -> anyhow::Result<()> {
        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            self.lease.0.write_all(&self.frame),
        )
        .await??;
        self.frame.clear();
        Ok(())
    }
    pub async fn reset(
        &mut self,
        proxy: &zbus::Proxy<'_>,
        stop: &watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        self.close().await?;
        anyhow::ensure!(!*stop.borrow(), "Input cancelled");
        let mut next = super::super::open(proxy).await?;
        if next.1 != self.lease.1 || *stop.borrow() {
            super::super::release(&mut next.0).await?;
            anyhow::bail!("Input keymap changed or cancelled");
        }
        self.lease = next;
        self.filled = 0;
        self.held.clear();
        self.frame.clear();
        if let Some(state) = &mut self.state {
            state.active = false;
        }
        Ok(())
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        super::super::release(&mut self.lease.0).await
    }
}
