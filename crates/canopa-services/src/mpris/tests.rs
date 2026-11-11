use super::*;
use crate::testing::{
    TestBus,
    mpris::{export, player},
};

#[test]
fn text_is_bounded_and_handles_missing_metadata() {
    assert_eq!(bounded("a\nb\tc"), "abc");
    assert_eq!(bounded(&"é".repeat(1000)).chars().count(), 512);
    let mut p = player("Radio", "Playing");
    p.title.clear();
    p.artist.clear();
    assert_eq!(p.label(), "Radio");
}

#[tokio::test]
async fn players_are_read_and_controls_revalidate_owner_track_and_capability() {
    let bus = TestBus::new();
    let client = bus.connect().await;
    assert!(players(&client).await.unwrap().is_empty());
    let (_a, a) = export(&bus, player("a", "Playing")).await;
    let (_b, _) = export(&bus, player("b", "Paused")).await;
    let found = players(&client).await.unwrap();
    assert_eq!(
        found.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    let current = &found[0];
    assert_ne!(current.id.owner, ":1.a", "the real owner is read back");
    control(&client, &current.id, &current.track, Operation::Pause)
        .await
        .unwrap();
    assert_eq!(*a.1.lock().unwrap(), ["Pause"]);
    control(&client, &current.id, &current.track, Operation::Stop)
        .await
        .unwrap();
    assert_eq!(*a.1.lock().unwrap(), ["Pause", "Stop"]);
    assert_eq!(
        control(&client, &current.id, &current.track, Operation::Previous).await,
        Err(MprisError::Unsupported)
    );
    assert_eq!(
        control(&client, &current.id, "/track/two", Operation::Play).await,
        Err(MprisError::Changed)
    );
    let mut replaced = current.id.clone();
    replaced.owner = found[1].id.owner.clone();
    assert_eq!(
        control(&client, &replaced, &current.track, Operation::Play).await,
        Err(MprisError::Changed)
    );
    assert_eq!(a.1.lock().unwrap().len(), 2);
}
