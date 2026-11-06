use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

fn challenge() -> Challenge {
    Challenge {
        version: 1,
        interactive: true,
        id: 1,
        endpoint: Endpoint {
            server: "print.example".into(),
            port: 631,
            transport: Transport::Tls,
        },
        kind: Kind::Server {
            limits: crate::print::authentication::ServerLimits::Legacy,
            realm: "Office".into(),
            resource: "/printers/Office".into(),
        },
        attempt: 1,
        suggested_user: "initial-user".into(),
    }
}
fn reply(id: u32, values: &[&[u8]]) -> Secret {
    let mut bytes = vec![1];
    bytes.extend(id.to_be_bytes());
    bytes.push(1);
    bytes.push(values.len() as u8);
    for v in values {
        bytes.extend((v.len() as u16).to_be_bytes());
        bytes.extend(*v);
    }
    let secret = Secret::copy(&bytes);
    // Test helper also clears the temporary framing allocation.
    unsafe {
        libc::explicit_bzero(bytes.as_mut_ptr().cast(), bytes.len());
    }
    secret
}
fn read_challenge(stream: &mut UnixStream) -> Challenge {
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let mut data = vec![0; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}
fn decline(stream: &mut UnixStream, id: u32) {
    let mut bytes = vec![1];
    bytes.extend(id.to_be_bytes());
    bytes.push(0);
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(&bytes).unwrap();
}
#[test]
fn requirements_preserve_server_order_and_reject_ambiguous_or_unknown_fields() {
    assert_eq!(
        Requirement::parse(Some("password,domain,username")).unwrap(),
        Requirement::Fields {
            fields: vec![Field::Password, Field::Domain, Field::Username]
        }
    );
    assert_eq!(Requirement::parse(None).unwrap(), Requirement::None);
    assert_eq!(
        Requirement::parse(Some("negotiate")).unwrap(),
        Requirement::Negotiate
    );
    for v in [
        "",
        "none,password",
        "username,username",
        "username,token",
        " password",
        "domain,username,password,extra",
    ] {
        assert!(Requirement::parse(Some(v)).is_err(), "{v}");
    }
}
#[test]
fn binary_credentials_preserve_punctuation_unicode_and_do_not_debug_as_plaintext() {
    let request = challenge();
    let password = "p,a\\\"'ßword".as_bytes();
    let frame = reply(1, &[b"DOMAIN\\person", password]);
    let values = Credentials::decode(&request, frame.bytes())
        .unwrap()
        .unwrap();
    assert_eq!(values.values[1].bytes(), password);
    assert!(!format!("{values:?}").contains("word"));
    assert_eq!(format!("{:?}", values.values[0]), "<redacted>");
    let value = Secret::copy(password).c_string().unwrap();
    assert_eq!(&value.bytes()[..password.len()], password);
    assert_eq!(value.bytes().last(), Some(&0));
}
#[test]
fn malformed_wrong_scope_and_trailing_secret_frames_cannot_be_used() {
    let request = challenge();
    let mut wrong = reply(2, &[b"person", b"private-value"]).bytes().to_vec();
    assert!(Credentials::decode(&request, &wrong).is_err());
    wrong = reply(1, &[b"person", b"private-value"]).bytes().to_vec();
    wrong.push(0);
    assert!(Credentials::decode(&request, &wrong).is_err());
    for values in [
        vec![&b"colon:user"[..], &b"value"[..]],
        vec![&b""[..], &b"value"[..]],
        vec![&b"person"[..], &b"bad\0value"[..]],
        vec![&b"person"[..], &b"\xff"[..]],
    ] {
        assert!(Credentials::decode(&request, reply(1, &values).bytes()).is_err());
    }
    let mut invalid = request;
    invalid.endpoint.server = "server\nlookalike".into();
    assert!(invalid.validate().is_err());
}
#[test]
fn overlong_server_credentials_are_rejected_without_silent_cups_truncation() {
    let request = challenge();
    let valid = vec![b'x'; 187];
    let invalid = vec![b'x'; 188];
    assert!(Credentials::decode(&request, reply(1, &[b"u", &valid]).bytes()).is_ok());
    assert!(Credentials::decode(&request, reply(1, &[b"u", &invalid]).bytes()).is_err());
    let mut request = request;
    request.kind = Kind::Queue {
        destination: Destination {
            queue: "Office".into(),
            instance: Some("Saved".into()),
        },
        fields: vec![Field::Password],
    };
    let password = vec![b'x'; 1023];
    assert!(Credentials::decode(&request, reply(1, &[&password]).bytes()).is_ok());
}
#[test]
fn declining_once_prevents_reprompting_for_a_second_cups_request() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    let (cancel, writer) = rustix::pipe::pipe().unwrap();
    let scope = Scope::enter(socket, File::from(cancel)).unwrap();
    let sender = std::thread::spawn(move || {
        let request = read_challenge(&mut peer);
        decline(&mut peer, request.id);
        peer.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let mut byte = [0];
        assert!(peer.read(&mut byte).is_err());
    });
    let channel = current().unwrap();
    assert!(channel.request(challenge()).unwrap().is_none());
    assert!(channel.declined());
    assert!(channel.request(challenge()).unwrap().is_none());
    sender.join().unwrap();
    drop(scope);
    drop(writer);
    assert!(current().is_none());
}

#[test]
fn cache_only_misses_are_not_user_cancellation_and_nested_guards_restore_interaction() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let (cancel, writer) = rustix::pipe::pipe().unwrap();
    let _scope = Scope::enter(socket, File::from(cancel)).unwrap();
    let sender = std::thread::spawn(move || {
        for _ in 0..2 {
            let request = read_challenge(&mut peer);
            assert!(!request.interactive);
            decline(&mut peer, request.id);
        }
        let request = read_challenge(&mut peer);
        assert!(request.interactive);
        let bytes = reply(request.id, &[b"person", b"synthetic-private-value"]);
        peer.write_all(&(bytes.bytes().len() as u32).to_be_bytes())
            .unwrap();
        peer.write_all(bytes.bytes()).unwrap();
    });
    let channel = current().unwrap();
    let outer = CacheOnly::enter();
    let inner = CacheOnly::enter();
    assert!(channel.request(challenge()).unwrap().is_none());
    assert!(!channel.declined());
    drop(inner);
    assert!(channel.request(challenge()).unwrap().is_none());
    assert!(!channel.declined());
    drop(outer);
    let credentials = channel.request(challenge()).unwrap().unwrap();
    assert_eq!(credentials.values[1].bytes(), b"synthetic-private-value");
    assert!(!channel.declined());
    sender.join().unwrap();
    drop(writer);
}
#[test]
fn caller_loss_interrupts_a_pending_password_without_a_reply_or_network_retry() {
    let (socket, mut peer) = UnixStream::pair().unwrap();
    let (cancel, writer) = rustix::pipe::pipe().unwrap();
    let scope = Scope::enter(socket, File::from(cancel)).unwrap();
    let sender = std::thread::spawn(move || {
        read_challenge(&mut peer);
        drop(writer);
        let mut data = [0];
        peer.set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let _ = peer.read(&mut data);
    });
    let start = std::time::Instant::now();
    let channel = current().unwrap();
    assert!(channel.request(challenge()).is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(!channel.declined());
    assert!(channel.request(challenge()).unwrap().is_none());
    sender.join().unwrap();
    drop(scope);
}
#[test]
fn oversized_or_wrong_nonce_replies_permanently_close_the_authentication_exchange() {
    for wrong_nonce in [false, true] {
        let (socket, mut peer) = UnixStream::pair().unwrap();
        let (cancel, _writer) = rustix::pipe::pipe().unwrap();
        let scope = Scope::enter(socket, File::from(cancel)).unwrap();
        let sender = std::thread::spawn(move || {
            let request = read_challenge(&mut peer);
            if wrong_nonce {
                decline(&mut peer, request.id + 1);
            } else {
                peer.write_all(&((MAX_FRAME + 1) as u32).to_be_bytes())
                    .unwrap();
            }
        });
        let channel = current().unwrap();
        assert!(channel.request(challenge()).is_err());
        assert!(!channel.declined());
        assert!(channel.request(challenge()).unwrap().is_none());
        sender.join().unwrap();
        drop(scope);
    }
}

#[test]
fn legacy_usernames_are_checked_in_bytes_before_cups_can_truncate_them() {
    let request = challenge();
    for valid in ["a".repeat(64), "é".repeat(32)] {
        assert!(Credentials::decode(&request, reply(1, &[valid.as_bytes(), b"p"]).bytes()).is_ok());
    }
    for invalid in [
        "a".repeat(65),
        format!("{}a", "é".repeat(32)),
        "é".repeat(33),
    ] {
        assert!(
            Credentials::decode(&request, reply(1, &[invalid.as_bytes(), b"p"]).bytes()).is_err()
        );
    }
}

#[test]
fn verified_capacity_preserves_maximum_ascii_and_utf8_values_and_refuses_overflow() {
    let mut request = challenge();
    let Kind::Server { limits, .. } = &mut request.kind else {
        unreachable!()
    };
    *limits = ServerLimits::CanopaV1;
    for user in ["u".repeat(127), format!("{}u", "é".repeat(63))] {
        for password in ["p".repeat(1023), format!("{}p", "é".repeat(511))] {
            let credentials = Credentials::decode(
                &request,
                reply(1, &[user.as_bytes(), password.as_bytes()]).bytes(),
            )
            .unwrap()
            .unwrap();
            let encoded = Credentials::encode(&request, Some(&credentials)).unwrap();
            let restored = Credentials::decode(&request, encoded.bytes())
                .unwrap()
                .unwrap();
            assert_eq!(restored.values[0].bytes(), user.as_bytes());
            assert_eq!(restored.values[1].bytes(), password.as_bytes());
        }
    }
    assert!(Credentials::decode(&request, reply(1, &[&[b'u'; 128], b"p"]).bytes()).is_err());
    assert!(Credentials::decode(&request, reply(1, &[b"u", &[b'p'; 1024]]).bytes()).is_err());
    let mut legacy = request.clone();
    let Kind::Server { limits, .. } = &mut legacy.kind else {
        unreachable!()
    };
    *limits = ServerLimits::Legacy;
    assert!(
        Credentials::decode(&legacy, reply(1, &[&[b'u'; 127], &[b'p'; 1023]]).bytes()).is_err()
    );
}

#[test]
fn older_private_challenges_default_to_legacy_and_unknown_capacity_is_rejected() {
    let value = serde_json::to_value(challenge()).unwrap();
    assert!(value["kind"].get("limits").is_none());
    let restored: Challenge = serde_json::from_value(value.clone()).unwrap();
    assert!(matches!(
        restored.kind,
        Kind::Server {
            limits: ServerLimits::Legacy,
            ..
        }
    ));
    let mut invalid = value;
    invalid["kind"]["limits"] = serde_json::json!("unbounded");
    assert!(serde_json::from_value::<Challenge>(invalid).is_err());
}
