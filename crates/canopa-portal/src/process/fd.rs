//! Checked single-FD handoff from an isolated input worker.
use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
};

pub(crate) fn receive(socket: &UnixStream, pid: u32) -> io::Result<OwnedFd> {
    use rustix::net::{self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags};
    let mut bytes = [0];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut control = RecvAncillaryBuffer::new(&mut space);
    let reply = net::recvmsg(
        socket,
        &mut [io::IoSliceMut::new(&mut bytes)],
        &mut control,
        RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut fds = Vec::new();
    let mut unknown = false;
    for message in control.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
            _ => unknown = true,
        }
    }
    if reply.bytes != 1
        || bytes != [1]
        || fds.len() != 1
        || unknown
        || reply
            .flags
            .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid input worker FD response",
        ));
    }
    let fd = fds.pop().unwrap();
    if net::sockopt::socket_type(&fd)? != net::SocketType::STREAM
        || net::sockopt::socket_domain(&fd)? != net::AddressFamily::UNIX
        || u32::try_from(
            net::sockopt::socket_peercred(&fd)?
                .pid
                .as_raw_nonzero()
                .get(),
        )
        .ok()
            != Some(pid)
        || !rustix::io::fcntl_getfd(&fd)?.contains(rustix::io::FdFlags::CLOEXEC)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Input FD belongs to another process",
        ));
    }
    Ok(fd)
}

pub(crate) fn send_ready(socket: &UnixStream, client: &UnixStream) -> anyhow::Result<()> {
    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
    use std::{io::IoSlice, mem::MaybeUninit};
    let descriptors = [client.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    anyhow::ensure!(
        ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)),
        "Input FD response overflow"
    );
    anyhow::ensure!(
        rustix::net::sendmsg(
            socket,
            &[IoSlice::new(&[1])],
            &mut ancillary,
            SendFlags::NOSIGNAL
        )? == 1,
        "Input FD response failed"
    );
    Ok(())
}
