//! Explicit native-fixture entry point. Ordinary checks do not connect to a
//! compositor; scripts/test-background-observer.py supplies the private session.
#[tokio::test]
#[ignore = "requires the disposable Niri/bubblewrap fixture"]
async fn native_observer_fixture() {
    use canopa_portal::background::Observer;
    use std::io::{BufRead, Write};
    use std::time::Duration;
    assert_eq!(
        std::env::var("CANOPA_BACKGROUND_NATIVE_FIXTURE").as_deref(),
        Ok("1")
    );
    let path = std::env::var_os("NIRI_SOCKET").expect("private Niri socket");
    let observer = Observer::spawn(Some(path.into())).unwrap();
    let state = observer.subscribe();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
        let _ = stop.send(());
    });
    let mut timer = tokio::time::interval(Duration::from_millis(50));
    let deadline = tokio::time::sleep(Duration::from_secs(90));
    tokio::pin!(deadline);
    let mut previous = String::new();
    loop {
        tokio::select! {
            _ = &mut stopped => break,
            _ = &mut deadline => panic!("native fixture did not stop its observer"),
            _ = timer.tick() => {
                let value = match state.borrow().apps() {
                    Ok(apps) => serde_json::json!({"apps": apps.iter().map(|(id, activity)| (id, *activity as u32)).collect::<std::collections::BTreeMap<_, _>>()}),
                    Err(reason) => serde_json::json!({"unavailable": format!("{reason:?}")}),
                }.to_string();
                if value != previous {
                    println!("OBSERVATION {value}");
                    std::io::stdout().flush().unwrap();
                    previous = value;
                }
            }
        }
    }
    drop(observer);
    assert_eq!(
        state.borrow().apps(),
        Err(canopa_portal::background::Unavailable::Shutdown)
    );
}
