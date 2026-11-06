//! Finite EIS reads are required by the separately owned remote-input worker.
use reis::{PendingRequestResult, eis};
use std::{
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

fn read(context: &eis::Context) -> std::io::Result<usize> {
    context.read_chunk()
}
use std::io::Write;

#[test]
fn socket_processing_yields_after_one_small_chunk() {
    let (server, mut client) = UnixStream::pair().unwrap();
    client.write_all(&[0_u8; 8192]).unwrap();
    let context = eis::Context::new(server).unwrap();
    let count = read(&context).unwrap();
    assert!(
        count > 0 && count <= 2048,
        "one read consumed {count} bytes"
    );
}

#[test]
fn partial_message_is_preserved_across_chunks() {
    let (server, client) = UnixStream::pair().unwrap();
    let context = eis::Context::new(server).unwrap();
    let sender = reis::ei::Context::new(client).unwrap();
    let name = "n".repeat(8192);
    sender.handshake().name(&name);
    sender.flush().unwrap();
    let mut chunks = 0;
    loop {
        let count = read(&context).unwrap();
        assert!(count > 0 && count <= 2048);
        chunks += 1;
        if let Some(request) = context.pending_request() {
            match request {
                PendingRequestResult::Request(eis::Request::Handshake(
                    _,
                    eis::handshake::Request::Name { name: received },
                )) => assert_eq!(received, name),
                other => panic!("unexpected request: {other:?}"),
            }
            break;
        }
    }
    assert!(chunks > 4);
}

#[test]
fn unread_buffer_is_bounded_and_peer_closure_is_distinct_from_would_block() {
    let (server, mut client) = UnixStream::pair().unwrap();
    let context = eis::Context::new(server).unwrap();
    assert_eq!(read(&context).unwrap(), 0);
    client
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let writer = std::thread::spawn(move || client.write_all(&vec![0; 1024 * 1024 + 2048]));
    let end = Instant::now() + Duration::from_secs(10);
    let mut total = 0;
    loop {
        assert!(Instant::now() < end);
        match read(&context) {
            Ok(count) => {
                assert!(count <= 2048);
                total += count;
                assert!(total <= 1024 * 1024);
                std::thread::yield_now();
            }
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert_eq!(total, 1024 * 1024);
                break;
            }
        }
    }
    drop(context);
    let _ = writer.join().unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let context = eis::Context::new(server).unwrap();
    drop(client);
    assert_eq!(
        read(&context).unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn incoming_descriptors_are_rejected() {
    use rustix::{io::IoSlice, net};
    use std::{fs::File, mem::MaybeUninit, os::fd::AsFd};
    let (server, client) = UnixStream::pair().unwrap();
    let context = eis::Context::new(server).unwrap();
    let file = File::open("/dev/null").unwrap();
    let descriptors = [file.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(&descriptors)));
    assert_eq!(
        net::sendmsg(
            &client,
            &[IoSlice::new(&[0])],
            &mut ancillary,
            net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        1
    );
    assert_eq!(
        read(&context).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}
