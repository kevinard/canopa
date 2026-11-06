use super::*;
use crate::print::authentication::{Endpoint, Field, Kind, Transport};
use std::{io::Write, path::Path};
use tokio::task::JoinHandle;

async fn start(
    directory: &Path,
) -> (
    JoinHandle<anyhow::Result<Option<Credentials>>>,
    UnixStream,
    watch::Sender<bool>,
    Challenge,
) {
    let script = directory.join("helper.py");
    let socket = directory.join("handoff");
    std::fs::write(
        &script,
        r#"import array,json,socket,sys
request=json.load(sys.stdin)
channel=socket.socket(fileno=request['channel'])
control=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
control.connect(sys.argv[1])
control.sendmsg([b'\x01'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[channel.fileno()]))])
control.close();channel.close()
print('true',flush=True)
"#,
    )
    .unwrap();
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let dialog = Dialog {
        process: DialogProcess {
            executable: "/usr/bin/python3".into(),
            arguments: vec![script.into_os_string(), socket.into_os_string()],
        },
        context: Context {
            app_id: String::new(),
            parent_window: String::new(),
            title: "Private reply fixture".into(),
            modal: false,
        },
    };
    let challenge = Challenge {
        version: 1,
        interactive: true,
        id: 1,
        endpoint: Endpoint {
            server: "localhost".into(),
            port: 631,
            transport: Transport::Loopback,
        },
        kind: Kind::Queue {
            destination: crate::print::worker::Destination {
                queue: "Fixture".into(),
                instance: None,
            },
            fields: vec![Field::Password],
        },
        attempt: 1,
        suggested_user: String::new(),
    };
    let (cancel, cancelled) = watch::channel(false);
    let request = challenge.clone();
    let pending = tokio::spawn(async move { dialog.show(request, cancelled).await });
    let (control, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    control.readable().await.unwrap();
    let mut descriptors = crate::print::file_target::receive_many(&control.into_std().unwrap(), 1)
        .unwrap()
        .unwrap();
    assert_eq!(descriptors.len(), 1);
    let writer = UnixStream::from(descriptors.pop().unwrap());
    (pending, writer, cancel, challenge)
}

fn reply(challenge: &Challenge) -> Vec<u8> {
    let values = Credentials {
        values: vec![Secret::copy(b"synthetic-private-value")],
    };
    let encoded = Credentials::encode(challenge, Some(&values)).unwrap();
    let mut reply = (encoded.bytes().len() as u32).to_be_bytes().to_vec();
    reply.extend_from_slice(encoded.bytes());
    reply
}

#[tokio::test]
async fn withdrawal_after_helper_exit_cancels_partial_and_unclosed_private_replies() {
    for prefix in [2, 6, usize::MAX] {
        for close_caller in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let (pending, mut writer, cancel, challenge) = start(directory.path()).await;
            let reply = reply(&challenge);
            writer.write_all(&reply[..prefix.min(reply.len())]).unwrap();
            // Drain of the writer's queue proves that the real parent already
            // accepted the helper's approval/exit and started its private read.
            // Keep this duplicate writer open until after cancellation returns.
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let mut queued = 0;
                    // SAFETY: this valid socket and writable integer live for
                    // the syscall; TIOCOUTQ writes only the queued-byte count.
                    assert_eq!(
                        unsafe { libc::ioctl(writer.as_raw_fd(), libc::TIOCOUTQ, &mut queued) },
                        0
                    );
                    if queued == 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert!(!pending.is_finished());
            if close_caller {
                drop(cancel);
            } else {
                cancel.send_replace(true);
            }
            assert!(
                tokio::time::timeout(Duration::from_secs(2), pending)
                    .await
                    .expect("private reply ignored withdrawal")
                    .unwrap()
                    .unwrap()
                    .is_none()
            );
            drop(writer);
        }
    }
}

#[tokio::test]
async fn complete_private_reply_requires_eof_and_preserves_the_approved_value() {
    let directory = tempfile::tempdir().unwrap();
    let (pending, mut writer, _cancel, challenge) = start(directory.path()).await;
    writer.write_all(&reply(&challenge)).unwrap();
    drop(writer);
    let credentials = tokio::time::timeout(Duration::from_secs(3), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(credentials.values[0].bytes(), b"synthetic-private-value");
}
