use super::*;
use crate::screencast_capture::bindings;
use std::{collections::BTreeSet, os::fd::AsFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zbus::zvariant::{ObjectPath, OwnedFd};

type Descriptors = Vec<(u64, String, (i32, i32), Option<f64>)>;
type LegacyDescriptors = Vec<(u64, String, (i32, i32))>;
type ScaledDescriptors = Vec<(u64, String, (i32, i32), f64)>;
#[derive(Clone, Debug)]
struct Region {
    node: u32,
    mapping_id: String,
    stream: u64,
    size: (i32, i32),
    physical_scale: Option<f64>,
}
pub(in crate::remote_input) struct Pointer {
    socket: tokio::net::UnixStream,
    regions: Vec<Region>,
    sequence: u32,
    held: BTreeSet<u32>,
    active: bool,
}
impl Pointer {
    pub async fn open(connection: &Connection, input: &Input) -> anyhow::Result<Self> {
        let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(input.compositor.as_str())?
            .path(PATH)?
            .interface(SERVICE)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        let version = tokio::time::timeout(CALL_TIME, async {
            anyhow::ensure!(
                proxy.get_property::<u32>("PointerVersion").await? == 1,
                "Unsupported pointer lease version"
            );
            if let Some(binding) = &input.capture {
                let version = proxy.get_property::<u32>("MappedPointerVersion").await?;
                anyhow::ensure!(
                    matches!(version, 1 | 2),
                    "Capture-bound pointer unavailable"
                );
                let shapes = binding
                    .current_shapes(connection, input.compositor_pid)
                    .await?;
                anyhow::ensure!(
                    version == 1 || shapes.iter().all(|s| s.physical_scale.is_some()),
                    "Missing capture physical scale"
                );
                Ok::<_, anyhow::Error>(version)
            } else {
                Ok(0)
            }
        })
        .await??;
        let mapped = version != 0;
        let (fd, caps, descriptors): (OwnedFd, u32, Descriptors) = if let Some(binding) =
            &input.capture
        {
            let session = ObjectPath::try_from(binding.session.as_str())?;
            let paths = binding
                .regions
                .iter()
                .map(|r| ObjectPath::try_from(r.stream.as_str()))
                .collect::<Result<Vec<_>, _>>()?;
            if version == 2 {
                let (fd, caps, regions): (OwnedFd, u32, ScaledDescriptors) = tokio::time::timeout(
                    CALL_TIME,
                    proxy.call("OpenMappedPointer", &(session, paths)),
                )
                .await??;
                (
                    fd,
                    caps,
                    regions
                        .into_iter()
                        .map(|(stream, id, size, scale)| (stream, id, size, Some(scale)))
                        .collect(),
                )
            } else {
                let (fd, caps, regions): (OwnedFd, u32, LegacyDescriptors) = tokio::time::timeout(
                    CALL_TIME,
                    proxy.call("OpenMappedPointer", &(session, paths)),
                )
                .await??;
                (
                    fd,
                    caps,
                    regions
                        .into_iter()
                        .map(|(stream, id, size)| (stream, id, size, None))
                        .collect(),
                )
            }
        } else {
            let (fd, caps): (OwnedFd, u32) =
                tokio::time::timeout(CALL_TIME, proxy.call("OpenPointer", &())).await??;
            (fd, caps, Vec::new())
        };
        let socket = UnixStream::from(std::os::fd::OwnedFd::from(fd));
        anyhow::ensure!(
            rustix::net::sockopt::socket_domain(&socket) == Ok(rustix::net::AddressFamily::UNIX)
                && rustix::net::sockopt::socket_type(&socket)
                    == Ok(rustix::net::SocketType::STREAM)
                && u32::try_from(
                    rustix::net::sockopt::socket_peercred(&socket)?
                        .pid
                        .as_raw_nonzero()
                        .get()
                )
                .ok()
                    == Some(input.compositor_pid),
            "Pointer FD belongs to another process"
        );
        rustix::io::fcntl_setfd(socket.as_fd(), rustix::io::FdFlags::CLOEXEC)?;
        socket.set_nonblocking(true)?;
        let mut pointer = Self {
            socket: tokio::net::UnixStream::from_std(socket)?,
            regions: Vec::new(),
            sequence: 0,
            held: BTreeSet::new(),
            active: true,
        };
        let setup = async {
            anyhow::ensure!(
                caps == if mapped { 31 } else { 15 },
                "Unsupported pointer capabilities"
            );
            if let Some(binding) = &input.capture {
                anyhow::ensure!(
                    descriptors.len() == binding.regions.len(),
                    "Invalid pointer regions"
                );
                for (region, (stream, mapping_id, size, physical_scale)) in
                    binding.regions.iter().zip(descriptors)
                {
                    anyhow::ensure!(
                        Some(stream) == bindings::path_id(&region.stream, "Stream")
                            && mapping_id == region.mapping_id
                            && bindings::valid_size(size)
                            && physical_scale.is_none_or(bindings::valid_scale)
                            && (version != 2 || physical_scale.is_some()),
                        "Pointer region does not match capture"
                    );
                    // Use the descriptors atomically pinned by Open, including
                    // any size change since the preceding read-only validation.
                    pointer.regions.push(Region {
                        node: region.node,
                        mapping_id,
                        stream,
                        size,
                        physical_scale,
                    });
                }
            } else {
                anyhow::ensure!(descriptors.is_empty(), "Unexpected pointer regions");
            }
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = setup {
            pointer.close().await?;
            return Err(error);
        }
        Ok(pointer)
    }
    /// Only atomically pinned descriptors with a known scale can become EI
    /// regions. Legacy mapped leases remain usable by the private Notify path.
    pub fn ei_regions(&self) -> anyhow::Result<Vec<EiRegion>> {
        self.regions
            .iter()
            .map(|region| {
                let scale = region
                    .physical_scale
                    .ok_or_else(|| anyhow::anyhow!("EI region scale unavailable"))?;
                Ok(EiRegion {
                    node: region.node,
                    mapping_id: region.mapping_id.clone(),
                    size: region.size,
                    scale: scale as f32,
                })
            })
            .collect()
    }
    pub async fn send(&mut self, command: Command) -> anyhow::Result<Option<Status>> {
        anyhow::ensure!(self.active, "Pointer source ended");
        if !command.valid() {
            return Ok(Some(Status::Invalid));
        }
        let mut probe = [0];
        match rustix::io::read(&self.socket, &mut probe) {
            Ok(0) => {
                self.retired();
                return Ok(None);
            }
            Ok(_) => anyhow::bail!("Unexpected idle pointer reply"),
            Err(error) if error == rustix::io::Errno::CONNRESET => {
                self.confirm_ended().await?;
                self.retired();
                return Ok(None);
            }
            Err(error) if error == rustix::io::Errno::AGAIN => {}
            Err(error) => return Err(error.into()),
        }
        let mut absolute = None;
        match command {
            Command::Absolute { node, x, y } => {
                if self.regions.is_empty() {
                    return Ok(Some(Status::Unavailable));
                }
                let Some(region) = self.regions.iter().find(|r| r.node == node) else {
                    return Ok(Some(Status::Invalid));
                };
                if x >= f64::from(region.size.0) || y >= f64::from(region.size.1) {
                    return Ok(Some(Status::Invalid));
                }
                absolute = Some(region.stream);
            }
            Command::Button { code, pressed } => {
                if self.held.contains(&code) == pressed {
                    return Ok(Some(Status::Invalid));
                }
                if pressed && self.held.len() >= 64 {
                    return Ok(Some(Status::Limit));
                }
            }
            _ => {}
        }
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Pointer sequence exhausted"))?;
        let acknowledged = tokio::time::timeout(Duration::from_secs(2), async {
            if let Err(error) = self.socket.write_all(&command.packet(next, absolute)).await {
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) {
                    return Err(error.into());
                }
                let mut byte = [0];
                anyhow::ensure!(
                    self.read(&mut byte).await? == 0,
                    "Unconfirmed pointer retirement"
                );
                return Ok(None);
            }
            let mut feedback = [0; 16];
            let mut filled = 0;
            while filled < feedback.len() {
                let n = self.read(&mut feedback[filled..]).await?;
                if n == 0 {
                    return Ok(None);
                }
                filled += n;
            }
            anyhow::ensure!(
                wire::status(feedback, next)? == Status::Accepted,
                "Compositor rejected pointer command"
            );
            Ok::<_, anyhow::Error>(Some(()))
        })
        .await??;
        if acknowledged.is_none() {
            self.retired();
            return Ok(None);
        }
        self.sequence = next;
        if let Command::Button { code, pressed } = command {
            if pressed {
                self.held.insert(code);
            } else {
                self.held.remove(&code);
            }
        }
        Ok(Some(Status::Accepted))
    }
    async fn confirm_ended(&mut self) -> anyhow::Result<()> {
        let mut byte = [0];
        anyhow::ensure!(
            tokio::time::timeout(Duration::from_secs(2), self.socket.read(&mut byte)).await?? == 0,
            "Unconfirmed pointer retirement"
        );
        Ok(())
    }
    async fn read(&mut self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        match self.socket.read(bytes).await {
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
                // A compositor shutdown with an unread command leaves a pending
                // reset. Only subsequent EOF confirms retirement, never reset alone.
                self.confirm_ended().await?;
                Ok(0)
            }
            result => Ok(result?),
        }
    }
    fn retired(&mut self) {
        self.active = false;
        self.held.clear();
    }
    pub async fn ended(&mut self) -> anyhow::Result<()> {
        let mut byte = [0];
        anyhow::ensure!(
            self.read(&mut byte).await? == 0,
            "Unexpected idle pointer reply"
        );
        self.retired();
        Ok(())
    }
    pub async fn renew(&mut self, connection: &Connection, input: &Input) -> anyhow::Result<bool> {
        anyhow::ensure!(!self.active, "Pointer source still active");
        let Some(binding) = &input.capture else {
            return Ok(false);
        };
        let shapes = match binding
            .current_shapes(connection, input.compositor_pid)
            .await
        {
            Ok(shapes) => shapes,
            Err(_) => return Ok(false),
        };
        let scale_lost = self.regions.iter().any(|r| {
            r.physical_scale.is_some()
                && !shapes
                    .iter()
                    .any(|shape| shape.node == r.node && shape.physical_scale.is_some())
        });
        if scale_lost
            || shapes.iter().all(|shape| {
                self.regions.iter().any(|r| {
                    r.node == shape.node
                        && r.size == shape.size
                        && (r.physical_scale.is_none() || r.physical_scale == shape.physical_scale)
                })
            })
        {
            return Ok(false);
        }
        // EOF has already acknowledged old held-input release. Never reopen on
        // an unexplained EOF or a different producer/source/mapping identity.
        match Self::open(connection, input).await {
            Ok(next) => {
                *self = next;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
    pub async fn close(&mut self) -> anyhow::Result<()> {
        if !self.active {
            return Ok(());
        }
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            let _ = self.socket.shutdown().await;
            let mut bytes = [0; 1025];
            let mut filled = 0;
            loop {
                let n = self.read(&mut bytes[filled..]).await?;
                if n == 0 {
                    break;
                }
                filled += n;
                anyhow::ensure!(filled <= 1024, "Pointer cleanup exceeded limit");
            }
            Ok::<_, anyhow::Error>(())
        })
        .await?;
        if result.is_ok() {
            self.active = false;
            self.held.clear();
        }
        result
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::remote_input) struct EiRegion {
    pub node: u32,
    pub mapping_id: String,
    pub size: (i32, i32),
    pub scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retiring_with_an_unread_command_confirms_eof_after_reset_without_replaying() {
        let (socket, peer) = tokio::net::UnixStream::pair().unwrap();
        let mut pointer = Pointer {
            socket,
            regions: Vec::new(),
            sequence: 7,
            held: [273].into_iter().collect(),
            active: true,
        };
        let remote = tokio::spawn(async move {
            // Reproduce Niri invalidating a region before consuming its queued
            // command: shutdown then drop leaves unread bytes and a pending reset.
            peer.readable().await.unwrap();
            peer.into_std()
                .unwrap()
                .shutdown(std::net::Shutdown::Both)
                .unwrap();
        });
        assert_eq!(
            pointer
                .send(Command::Button {
                    code: 272,
                    pressed: true
                })
                .await
                .unwrap(),
            None
        );
        assert!(!pointer.active);
        assert!(pointer.held.is_empty());
        assert_eq!(pointer.sequence, 7);
        remote.await.unwrap();
    }

    #[tokio::test]
    async fn eof_before_or_during_acknowledgement_retires_without_committing_or_replaying() {
        for partial in [false, true] {
            let (socket, mut peer) = tokio::net::UnixStream::pair().unwrap();
            let mut pointer = Pointer {
                socket,
                regions: Vec::new(),
                sequence: 7,
                held: [273].into_iter().collect(),
                active: true,
            };
            let remote = tokio::spawn(async move {
                let mut bytes = [0; 32];
                peer.read_exact(&mut bytes).await.unwrap();
                assert_eq!(
                    Command::decode(&bytes, 8).unwrap(),
                    Command::Button {
                        code: 272,
                        pressed: true
                    }
                );
                if partial {
                    peer.write_all(&wire::feedback(8, Status::Accepted)[..8])
                        .await
                        .unwrap();
                }
            });
            assert_eq!(
                pointer
                    .send(Command::Button {
                        code: 272,
                        pressed: true
                    })
                    .await
                    .unwrap(),
                None
            );
            assert!(!pointer.active);
            assert!(pointer.held.is_empty());
            assert_eq!(pointer.sequence, 7);
            remote.await.unwrap();
        }
    }
}
