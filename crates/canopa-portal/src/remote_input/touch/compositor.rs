use super::*;
use crate::remote_input::pointer::compositor::EiRegion;
use crate::screencast_capture::bindings;
use std::{collections::BTreeSet, os::fd::AsFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zbus::zvariant::{ObjectPath, OwnedFd};

type Descriptors = Vec<(u64, String, (i32, i32), f64)>;
struct Region {
    node: u32,
    stream: u64,
    mapping_id: String,
    size: (i32, i32),
    physical_scale: Option<f64>,
}

pub(in crate::remote_input) struct Touch {
    socket: tokio::net::UnixStream,
    regions: Vec<Region>,
    sequence: u32,
    held: BTreeSet<u32>,
    active: bool,
}
impl Touch {
    pub async fn open(connection: &Connection, input: &Input) -> anyhow::Result<Self> {
        let binding = input
            .capture
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Touch requires capture bindings"))?;
        let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(input.compositor.as_str())?
            .path(PATH)?
            .interface(SERVICE)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        tokio::time::timeout(CALL_TIME, async {
            anyhow::ensure!(
                proxy.get_property::<u32>("TouchVersion").await? == 1,
                "Unsupported touch lease version"
            );
            let shapes = binding
                .current_shapes(connection, input.compositor_pid)
                .await?;
            anyhow::ensure!(
                shapes.iter().all(|s| s.physical_scale.is_some()),
                "Missing touch region scale"
            );
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        let session = ObjectPath::try_from(binding.session.as_str())?;
        let paths = binding
            .regions
            .iter()
            .map(|r| ObjectPath::try_from(r.stream.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let (fd, descriptors): (OwnedFd, Descriptors) =
            tokio::time::timeout(CALL_TIME, proxy.call("OpenMappedTouch", &(session, paths)))
                .await??;
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
            "Touch FD belongs to another process"
        );
        rustix::io::fcntl_setfd(socket.as_fd(), rustix::io::FdFlags::CLOEXEC)?;
        socket.set_nonblocking(true)?;
        let mut touch = Self {
            socket: tokio::net::UnixStream::from_std(socket)?,
            regions: Vec::new(),
            sequence: 0,
            held: BTreeSet::new(),
            active: true,
        };
        let setup = (|| {
            anyhow::ensure!(
                descriptors.len() == binding.regions.len(),
                "Invalid touch regions"
            );
            for (region, (stream, mapping_id, size, scale)) in
                binding.regions.iter().zip(descriptors)
            {
                anyhow::ensure!(
                    Some(stream) == bindings::path_id(&region.stream, "Stream")
                        && mapping_id == region.mapping_id
                        && bindings::valid_size(size)
                        && bindings::valid_scale(scale),
                    "Touch region does not match capture"
                );
                touch.regions.push(Region {
                    node: region.node,
                    stream,
                    mapping_id,
                    size,
                    physical_scale: Some(scale),
                });
            }
            Ok::<_, anyhow::Error>(())
        })();
        if let Err(error) = setup {
            touch.close().await?;
            return Err(error);
        }
        Ok(touch)
    }
    pub fn ei_regions(&self) -> anyhow::Result<Vec<EiRegion>> {
        self.regions
            .iter()
            .map(|r| {
                Ok(EiRegion {
                    node: r.node,
                    mapping_id: r.mapping_id.clone(),
                    size: r.size,
                    scale: r
                        .physical_scale
                        .ok_or_else(|| anyhow::anyhow!("Touch region scale unavailable"))?
                        as f32,
                })
            })
            .collect()
    }
    pub async fn send(&mut self, command: Command) -> anyhow::Result<Option<Status>> {
        anyhow::ensure!(self.active, "Touch source ended");
        if !command.valid() {
            return Ok(Some(Status::Invalid));
        }
        let mut probe = [0];
        match rustix::io::read(&self.socket, &mut probe) {
            Ok(0) => {
                self.retired();
                return Ok(None);
            }
            Ok(_) => anyhow::bail!("Unexpected idle touch reply"),
            Err(e) if e == rustix::io::Errno::CONNRESET => {
                self.confirm_ended().await?;
                self.retired();
                return Ok(None);
            }
            Err(e) if e == rustix::io::Errno::AGAIN => {}
            Err(e) => return Err(e.into()),
        }
        let stream = match command {
            Command::Down {
                contact,
                node,
                x,
                y,
            }
            | Command::Motion {
                contact,
                node,
                x,
                y,
            } => {
                let down = matches!(command, Command::Down { .. });
                if self.held.contains(&contact) == down {
                    return Ok(Some(Status::Invalid));
                }
                if down && self.held.len() >= 32 {
                    return Ok(Some(Status::Limit));
                }
                let Some(region) = self.regions.iter().find(|r| r.node == node) else {
                    return Ok(Some(Status::Invalid));
                };
                if x >= f64::from(region.size.0) || y >= f64::from(region.size.1) {
                    return Ok(Some(Status::Invalid));
                }
                Some(region.stream)
            }
            Command::Up { contact } if !self.held.contains(&contact) => {
                return Ok(Some(Status::Invalid));
            }
            _ => None,
        };
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Touch sequence exhausted"))?;
        let acknowledged = tokio::time::timeout(Duration::from_secs(2), async {
            if let Err(error) = self.socket.write_all(&command.packet(next, stream)).await {
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) {
                    return Err(error.into());
                }
                let mut byte = [0];
                anyhow::ensure!(
                    self.read(&mut byte).await? == 0,
                    "Unconfirmed touch retirement"
                );
                return Ok(None);
            }
            let mut bytes = [0; 16];
            let mut filled = 0;
            while filled < bytes.len() {
                let n = self.read(&mut bytes[filled..]).await?;
                if n == 0 {
                    return Ok(None);
                }
                filled += n;
            }
            anyhow::ensure!(
                wire::status(bytes, next)? == Status::Accepted,
                "Compositor rejected touch command"
            );
            Ok::<_, anyhow::Error>(Some(()))
        })
        .await??;
        if acknowledged.is_none() {
            self.retired();
            return Ok(None);
        }
        self.sequence = next;
        match command {
            Command::Down { contact, .. } => {
                self.held.insert(contact);
            }
            Command::Up { contact } => {
                self.held.remove(&contact);
            }
            _ => {}
        }
        Ok(Some(Status::Accepted))
    }
    async fn confirm_ended(&mut self) -> anyhow::Result<()> {
        let mut byte = [0];
        anyhow::ensure!(
            tokio::time::timeout(Duration::from_secs(2), self.socket.read(&mut byte)).await?? == 0,
            "Unconfirmed touch retirement"
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
            "Unexpected idle touch reply"
        );
        self.retired();
        Ok(())
    }
    pub async fn renew(&mut self, connection: &Connection, input: &Input) -> anyhow::Result<bool> {
        anyhow::ensure!(!self.active, "Touch source still active");
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
                anyhow::ensure!(filled <= 1024, "Touch cleanup exceeded limit");
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

#[cfg(test)]
mod tests {
    use super::*;
    fn actor(socket: tokio::net::UnixStream) -> Touch {
        Touch {
            socket,
            regions: vec![Region {
                node: 9,
                stream: 17,
                mapping_id: "m".into(),
                size: (400, 300),
                physical_scale: Some(1.),
            }],
            sequence: 7,
            held: [7].into(),
            active: true,
        }
    }
    #[tokio::test]
    async fn invalid_transitions_and_limits_do_not_write_or_change_owned_contacts() {
        let (socket, peer) = tokio::net::UnixStream::pair().unwrap();
        let mut touch = actor(socket);
        for command in [
            Command::Down {
                contact: 7,
                node: 9,
                x: 1.,
                y: 1.,
            },
            Command::Up { contact: 8 },
            Command::Motion {
                contact: 8,
                node: 9,
                x: 1.,
                y: 1.,
            },
            Command::Down {
                contact: 8,
                node: 10,
                x: 1.,
                y: 1.,
            },
            Command::Motion {
                contact: 7,
                node: 9,
                x: 400.,
                y: 1.,
            },
        ] {
            assert_eq!(touch.send(command).await.unwrap(), Some(Status::Invalid));
            assert_eq!(touch.held, [7].into());
        }
        touch.held = (0..32).collect();
        assert_eq!(
            touch
                .send(Command::Down {
                    contact: 33,
                    node: 9,
                    x: 1.,
                    y: 1.
                })
                .await
                .unwrap(),
            Some(Status::Limit)
        );
        assert_eq!(
            peer.try_read(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(touch.sequence, 7);
    }
    #[tokio::test]
    async fn unread_command_reset_confirms_eof_and_discards_contacts_without_replay() {
        let (socket, peer) = tokio::net::UnixStream::pair().unwrap();
        let mut touch = actor(socket);
        let remote = tokio::spawn(async move {
            peer.readable().await.unwrap();
            peer.into_std()
                .unwrap()
                .shutdown(std::net::Shutdown::Both)
                .unwrap();
        });
        assert_eq!(
            touch
                .send(Command::Down {
                    contact: 8,
                    node: 9,
                    x: 1.,
                    y: 2.
                })
                .await
                .unwrap(),
            None
        );
        assert!(!touch.active);
        assert!(touch.held.is_empty());
        assert_eq!(touch.sequence, 7);
        remote.await.unwrap();
    }
}
