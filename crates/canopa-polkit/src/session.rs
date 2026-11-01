//! Resolve ownership from logind; a user manager's PID need not belong to a login.
use anyhow::{Context, Result, ensure};
use std::time::Duration;
use zbus::{Connection, Proxy, zvariant::OwnedObjectPath};

pub async fn discover(selected: Option<&str>) -> Result<String> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let bus = Connection::system().await?;
        let manager = Proxy::new(
            &bus,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await?;
        let explicit = selected.map(str::to_owned).or_else(|| {
            std::env::var("XDG_SESSION_ID")
                .ok()
                .filter(|s| !s.is_empty())
        });
        let path: OwnedObjectPath = if let Some(id) = explicit {
            manager.call("GetSession", &(id,)).await?
        } else if let Ok(path) = manager
            .call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
            .await
        {
            path
        } else {
            let list: Vec<(String, u32, String, String, OwnedObjectPath)> =
                manager.call("ListSessions", &()).await?;
            ensure!(list.len() <= 256, "session list exceeds budget");
            let mut found = Vec::new();
            for (_, uid, _, _, path) in list {
                if uid != unsafe { libc::getuid() } {
                    continue;
                }
                let proxy = Proxy::new(
                    &bus,
                    "org.freedesktop.login1",
                    path.as_str(),
                    "org.freedesktop.login1.Session",
                )
                .await?;
                let kind: String = proxy.get_property("Type").await?;
                let class: String = proxy.get_property("Class").await?;
                if kind == "wayland" && class == "user" {
                    found.push(path);
                }
            }
            ensure!(
                found.len() == 1,
                "set --session or XDG_SESSION_ID to select a unique graphical login"
            );
            found.remove(0)
        };
        let proxy = Proxy::new(
            &bus,
            "org.freedesktop.login1",
            path.as_str(),
            "org.freedesktop.login1.Session",
        )
        .await?;
        let (uid, _): (u32, OwnedObjectPath) = proxy.get_property("User").await?;
        ensure!(
            uid == unsafe { libc::getuid() },
            "session belongs to another user"
        );
        let kind: String = proxy.get_property("Type").await?;
        let class: String = proxy.get_property("Class").await?;
        ensure!(
            kind == "wayland" && class == "user",
            "session is not a graphical user login"
        );
        let id: String = proxy.get_property("Id").await?;
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control),
            "invalid session identifier"
        );
        Ok(id)
    })
    .await
    .context("logind session discovery timed out")?
}
