use super::*;
use crate::print::test_bus::{Bus, handle};

#[tokio::test]
async fn direct_callers_cannot_borrow_the_identity_encoded_in_a_forged_handle() {
    let bus = Bus::new().await;
    let _frontend = bus.frontend().await;
    let caller = bus.connect().await;
    let other = bus.connect().await;
    let owner = bus.owner(&caller, &other, "").await;
    let other_owner = bus.owner(&other, &caller, "").await;
    assert_eq!(owner.client(), caller.unique_name().unwrap().as_str());
    assert!(!owner.same_identity(&other_owner));
    assert!(owner.confirm().await);
}

#[tokio::test]
async fn authenticated_frontend_keeps_same_app_id_native_clients_distinct() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let first = bus.connect().await;
    let second = bus.connect().await;
    let first_owner = bus.owner(&frontend, &first, "").await;
    let second_owner = bus.owner(&frontend, &second, "").await;
    assert_eq!(first_owner.client(), first.unique_name().unwrap().as_str());
    assert_eq!(
        second_owner.client(),
        second.unique_name().unwrap().as_str()
    );
    assert!(!first_owner.same_identity(&second_owner));
    let repeated = bus.owner(&frontend, &first, "").await;
    assert!(first_owner.same_identity(&repeated));
    let other_app_id = bus.owner(&frontend, &first, "example.Other").await;
    assert!(!first_owner.same_identity(&other_app_id));
}

#[tokio::test]
async fn frontend_paths_accept_canonical_collision_suffix_and_reject_extra_components() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let collision = handle(&client, "prepare/collision");
    let owner = Owner::authenticate(
        &bus.backend,
        frontend.unique_name().unwrap().to_owned(),
        &collision,
        "",
    )
    .await
    .unwrap();
    assert_eq!(owner.client(), client.unique_name().unwrap().as_str());
    for handle in [
        handle(&client, "prepare/collision/extra"),
        "/org/freedesktop/portal/desktop/request/invalid/prepare"
            .try_into()
            .unwrap(),
        "/org/freedesktop/portal/desktop/request/1_999999999/prepare"
            .try_into()
            .unwrap(),
    ] {
        assert!(
            Owner::authenticate(
                &bus.backend,
                frontend.unique_name().unwrap().to_owned(),
                &handle,
                ""
            )
            .await
            .is_err()
        );
    }
}

#[tokio::test]
async fn original_client_loss_revokes_a_proof_while_the_frontend_remains_alive() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let owner = bus.owner(&frontend, &client, "").await;
    let mut cancelled = owner.cancelled();
    client.close().await.unwrap();
    tokio::time::timeout(BUS_TIMEOUT, cancelled.wait_for(|lost| *lost))
        .await
        .unwrap()
        .unwrap();
    assert!(!owner.confirm().await);
}

#[tokio::test]
async fn frontend_name_reacquisition_does_not_revive_old_proofs() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let owner = bus.owner(&frontend, &client, "").await;
    frontend.release_name(FRONTEND).await.unwrap();
    frontend.request_name(FRONTEND).await.unwrap();
    // Deliberately no sleep: confirmation must process queued name loss even
    // though the same unique connection currently owns the frontend again.
    assert!(!owner.confirm().await);
    let replacement = bus.owner(&frontend, &client, "").await;
    assert!(replacement.confirm().await);
    assert!(!owner.confirm().await);
}

#[tokio::test]
async fn backend_bus_loss_revokes_authority() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "example.App").await;
    bus.backend.clone().close().await.unwrap();
    assert!(!owner.confirm().await);
}

#[tokio::test]
async fn reused_unique_names_on_a_different_bus_do_not_match_an_old_identity() {
    let first = Bus::new().await;
    let second = Bus::new().await;
    let first_peer = first.connect().await;
    let second_peer = second.connect().await;
    assert_eq!(first_peer.unique_name(), second_peer.unique_name());
    let first_owner = first.owner(&first_peer, &first_peer, "").await;
    let second_owner = second.owner(&second_peer, &second_peer, "").await;
    assert!(!first_owner.same_identity(&second_owner));
    assert!(!first_owner.same_actor(&second_owner));
}
