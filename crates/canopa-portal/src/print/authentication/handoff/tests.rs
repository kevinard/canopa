use super::*;
use crate::print::authentication::{Endpoint, Kind, Transport};

fn retained() -> Retained {
    Retained {
        prompts: 1,
        entries: vec![Entry {
            challenge: Challenge {
                version: 1,
                interactive: true,
                id: 3,
                endpoint: Endpoint {
                    server: "office.example".into(),
                    port: 631,
                    transport: Transport::Tls,
                },
                kind: Kind::Server {
                    limits: crate::print::authentication::ServerLimits::Legacy,
                    realm: "Office".into(),
                    resource: "/".into(),
                },
                attempt: 2,
                suggested_user: "initial".into(),
            },
            credentials: Credentials {
                values: vec![
                    Secret::copy(b"user"),
                    Secret::copy(b"synthetic,private\\password"),
                ],
            },
        }],
    }
}
#[test]
fn sealed_memory_handoff_preserves_exact_credentials_and_normalizes_operation_nonce() {
    let (send, receive) = UnixStream::pair().unwrap();
    retained().send(&send).unwrap();
    drop(send);
    let restored = Retained::receive(&receive).unwrap();
    assert_eq!(restored.prompts, 1);
    assert_eq!(restored.entries.len(), 1);
    assert_eq!(
        restored.entries[0].credentials.values[1].bytes(),
        b"synthetic,private\\password"
    );
    assert_eq!(restored.entries[0].challenge.id, 1);
    assert_eq!(restored.entries[0].challenge.attempt, 1);
    assert!(!format!("{restored:?}").contains("password"));
}
#[test]
fn empty_context_requires_a_real_handoff_and_accepts_no_trailing_descriptors() {
    let (send, receive) = UnixStream::pair().unwrap();
    Retained::default().send(&send).unwrap();
    drop(send);
    assert!(Retained::receive(&receive).unwrap().entries.is_empty());
    let (send, receive) = UnixStream::pair().unwrap();
    drop(send);
    assert!(Retained::receive(&receive).is_err());
    let (send, receive) = UnixStream::pair().unwrap();
    retained().send(&send).unwrap();
    retained().send(&send).unwrap();
    drop(send);
    assert!(Retained::receive(&receive).is_err());
}

#[test]
fn cache_only_metadata_cannot_be_transferred_as_a_new_approved_scope() {
    let (send, _) = UnixStream::pair().unwrap();
    let mut cache = retained();
    cache.entries[0].challenge.interactive = false;
    assert!(cache.send(&send).is_err());
}
#[test]
fn ordinary_files_unsealed_memory_and_malformed_cache_are_rejected() {
    let (send, receive) = UnixStream::pair().unwrap();
    let file = tempfile::tempfile().unwrap();
    file.write_all_at(b"CANOPAPRINTAUTH\x01\x00\x00", 0)
        .unwrap();
    file_target::send_many(&send, &[file.as_fd()], 1).unwrap();
    drop(send);
    assert!(Retained::receive(&receive).is_err());
    let (send, receive) = UnixStream::pair().unwrap();
    let file = File::from(
        rustix::fs::memfd_create(c"unsealed-test", rustix::fs::MemfdFlags::ALLOW_SEALING).unwrap(),
    );
    file.write_all_at(b"CANOPAPRINTAUTH\x01\x00\x00", 0)
        .unwrap();
    file_target::send_many(&send, &[file.as_fd()], 1).unwrap();
    drop(send);
    assert!(Retained::receive(&receive).is_err());
    for frame in [
        b"".as_slice(),
        b"CANOPAPRINTAUTH\x01\x00\x01",
        b"CANOPAPRINTAUTH\x01\x07\x00",
        b"CANOPAPRINTAUTH\x01\x00\x00trailing",
    ] {
        assert!(Retained::decode(frame).is_err());
    }
}

#[test]
fn prepared_handoff_retains_the_exact_verified_capacity_and_larger_values() {
    let mut cache = retained();
    let Kind::Server { limits, .. } = &mut cache.entries[0].challenge.kind else {
        unreachable!()
    };
    *limits = crate::print::authentication::ServerLimits::CanopaV1;
    cache.entries[0].credentials.values =
        vec![Secret::copy(&[b'u'; 127]), Secret::copy(&[b'p'; 1023])];
    let (send, receive) = UnixStream::pair().unwrap();
    cache.send(&send).unwrap();
    drop(send);
    let restored = Retained::receive(&receive).unwrap();
    assert_eq!(
        restored.entries[0].challenge.kind,
        cache.entries[0].challenge.kind
    );
    for (before, after) in cache.entries[0]
        .credentials
        .values
        .iter()
        .zip(&restored.entries[0].credentials.values)
    {
        assert_eq!(before.bytes(), after.bytes());
    }
}
