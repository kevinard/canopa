use super::authority::Owner;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};
use zbus::{Connection, zvariant::OwnedObjectPath};

pub struct Bus {
    _daemon: Daemon,
    address: String,
    pub backend: Connection,
}
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Bus {
    pub async fn new() -> Self {
        let mut daemon = Daemon(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut address = String::new();
        BufReader::new(daemon.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();
        let backend = connect(&address).await;
        Self {
            _daemon: daemon,
            address,
            backend,
        }
    }
    pub async fn connect(&self) -> Connection {
        connect(&self.address).await
    }
    pub async fn frontend(&self) -> Connection {
        let frontend = self.connect().await;
        frontend
            .request_name("org.freedesktop.portal.Desktop")
            .await
            .unwrap();
        frontend
    }
    pub async fn owner(&self, peer: &Connection, client: &Connection, app_id: &str) -> Owner {
        Owner::authenticate(
            &self.backend,
            peer.unique_name().unwrap().to_owned(),
            &handle(client, "prepare"),
            app_id,
        )
        .await
        .unwrap()
    }
}
async fn connect(address: &str) -> Connection {
    zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap()
}
pub fn handle(client: &Connection, token: &str) -> OwnedObjectPath {
    let name = client
        .unique_name()
        .unwrap()
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    format!("/org/freedesktop/portal/desktop/request/{name}/{token}")
        .try_into()
        .unwrap()
}
