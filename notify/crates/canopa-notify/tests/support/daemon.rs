use canopa_notify::{Daemon, DaemonOptions};
use std::ops::Deref;

pub struct HeadlessDaemon {
    _temp: tempfile::TempDir,
    daemon: Daemon,
}

impl Deref for HeadlessDaemon {
    type Target = Daemon;

    fn deref(&self) -> &Self::Target {
        &self.daemon
    }
}

pub async fn open_headless_daemon() -> HeadlessDaemon {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    HeadlessDaemon {
        _temp: temp,
        daemon,
    }
}
