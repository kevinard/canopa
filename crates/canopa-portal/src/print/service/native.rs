//! Opt-in native host in the existing test harness. The controller, Iced
//! presenter and worker are real; only registration/lifetime are fixture-owned.
use super::*;
use crate::{BUS_NAME, OBJECT_PATH, lockdown::Source, print::Production, process::DialogProcess};
use std::{fs, path::PathBuf, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a private bus/compositor, normal executable and native driver"]
async fn native_production_backend() {
    let root = PathBuf::from(std::env::var_os("CANOPA_PRINT_NATIVE_ROOT").unwrap());
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    assert!(address.starts_with(&format!("unix:path={}/", root.display())));
    let binary = std::env::var_os("CANOPA_PRINT_BINARY").unwrap();
    let connection = Connection::session().await.unwrap();
    let policy = Lockdown::open(Source::file(root.join("policy.sqlite3"), false).unwrap())
        .await
        .unwrap();
    let print = Print::with_requests(
        Arc::new(Production::new(
            Arc::new(DialogProcess {
                executable: binary.into(),
                arguments: vec!["--canopa-config".into(), root.join("config.kdl").into()],
            }),
            crate::print::file_target::RecoveryStore::new(&root.join("recovery")).unwrap(),
        )),
        policy.clone(),
        Requests::default(),
    );
    connection
        .object_server()
        .at(OBJECT_PATH, policy.clone())
        .await
        .unwrap();
    connection
        .object_server()
        .at(OBJECT_PATH, print.clone())
        .await
        .unwrap();
    connection.request_name(BUS_NAME).await.unwrap();
    let relay = tokio::spawn({
        let policy = policy.clone();
        let connection = connection.clone();
        async move { policy.relay(connection).await }
    });
    fs::write(root.join("backend-ready"), b"ready").unwrap();
    tokio::time::timeout(Duration::from_secs(180), async {
        while !root.join("backend-stop").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    print.shutdown().await;
    policy.shutdown().await.unwrap();
    relay.await.unwrap().unwrap();
    connection.release_name(BUS_NAME).await.unwrap();
    fs::write(root.join("backend-stopped"), b"stopped").unwrap();
}
