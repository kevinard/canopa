//! Bind the selection inventory to the same process as the pinned compositor.
use super::*;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

pub(crate) async fn request(
    socket: &std::path::Path,
    pid: u32,
    request: &str,
) -> anyhow::Result<Value> {
    tokio::time::timeout(Duration::from_secs(2), async {
        anyhow::ensure!(socket.is_absolute(), "Capture source socket unavailable");
        let mut stream = UnixStream::connect(socket).await?;
        anyhow::ensure!(
            stream
                .peer_cred()?
                .pid()
                .is_some_and(|peer| u32::try_from(peer).ok() == Some(pid)),
            "Capture catalog belongs to another compositor"
        );
        let mut request = serde_json::to_vec(request)?;
        request.push(b'\n');
        stream.write_all(&request).await?;
        let mut bytes = Vec::new();
        BufReader::new(stream.take(1024 * 1024 + 1))
            .read_until(b'\n', &mut bytes)
            .await?;
        anyhow::ensure!(
            bytes.len() <= 1024 * 1024 && bytes.last() == Some(&b'\n'),
            "Capture source catalog exceeds limit"
        );
        Ok::<_, anyhow::Error>(serde_json::from_slice(&bytes)?)
    })
    .await?
}
fn label(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .chars()
        .filter(|c| !c.is_control())
        .take(128)
        .collect()
}
pub(crate) async fn read(
    socket: &std::path::Path,
    pid: u32,
    types: u32,
) -> anyhow::Result<Vec<Item>> {
    if types == 0 {
        let reply = request(socket, pid, "Version").await?;
        anyhow::ensure!(
            reply
                .pointer("/Ok/Version")
                .and_then(Value::as_str)
                .is_some_and(|version| !version.is_empty()),
            "Desktop unavailable"
        );
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    if types & 1 != 0 {
        let reply = request(socket, pid, "Outputs").await?;
        let outputs = reply
            .pointer("/Ok/Outputs")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("Invalid capture outputs"))?;
        anyhow::ensure!(outputs.len() <= 16, "Too many capture outputs");
        for (index, (name, output)) in outputs
            .iter()
            .filter(|(_, output)| output.get("logical").is_some_and(Value::is_object))
            .enumerate()
        {
            anyhow::ensure!(
                !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
                "Invalid capture output name"
            );
            let model = label(output.get("model"), "");
            let make = label(output.get("make"), "");
            let title =
                if model.is_empty() || model.eq_ignore_ascii_case(name) || model == "Unknown" {
                    {
                        let number: usize = index + 1;
                        fl!("portal-display-numbered", number = number)
                    }
                } else if !make.is_empty() && make != "Unknown" && !model.starts_with(&make) {
                    format!("{make} {model}")
                } else {
                    model
                };
            let subtitle = match (
                output.pointer("/logical/width").and_then(Value::as_u64),
                output.pointer("/logical/height").and_then(Value::as_u64),
            ) {
                (Some(width), Some(height)) if name == "winit" => format!("{width} × {height}"),
                (Some(width), Some(height)) => format!("{name} · {width} × {height}"),
                _ => name.clone(),
            };
            items.push(Item {
                source: Source::Monitor(name.clone().into()),
                title,
                subtitle,
                app_id: None,
            });
        }
    }
    if types & 2 != 0 {
        let reply = request(socket, pid, "Windows").await?;
        let windows = reply
            .pointer("/Ok/Windows")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("Invalid capture windows"))?;
        anyhow::ensure!(windows.len() <= 512, "Too many capture windows");
        let mut ids = HashSet::new();
        for window in windows {
            let id = window
                .get("id")
                .and_then(Value::as_u64)
                .filter(|id| *id != 0 && ids.insert(*id))
                .ok_or_else(|| anyhow::anyhow!("Invalid capture window identity"))?;
            if window.get("app_id").and_then(Value::as_str) == Some("org.canopa.Portal") {
                continue;
            }
            let app_id = label(window.get("app_id"), "");
            let subtitle = if app_id.is_empty() {
                fl!("portal-screencast-application-window")
            } else {
                crate::dialog_ui::requester(&app_id)
            };
            items.push(Item {
                source: Source::Window(id),
                title: label(
                    window.get("title"),
                    &fl!("portal-screencast-untitled-window"),
                ),
                subtitle,
                app_id: (!app_id.is_empty()).then_some(app_id),
            });
        }
    }
    items.sort_by_key(|item| {
        (
            matches!(item.source, Source::Window(_)),
            item.title.to_lowercase(),
        )
    });
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn input_only_inventory_still_requires_the_live_pinned_desktop() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            read(&root.path().join("missing"), std::process::id(), 0)
                .await
                .is_err()
        );
        let socket = root.path().join("ipc.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut line = Vec::new();
            BufReader::new(&mut stream)
                .read_until(b'\n', &mut line)
                .await
                .unwrap();
            assert_eq!(line, b"\"Version\"\n");
            stream
                .write_all(b"{\"Ok\":{\"Version\":\"niri private fixture\"}}\n")
                .await
                .unwrap();
        });
        assert!(
            read(&socket, std::process::id(), 0)
                .await
                .unwrap()
                .is_empty()
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn source_inventory_requires_the_pinned_compositor_process_and_sanitizes_labels() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("ipc.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut line = Vec::new();
            BufReader::new(&mut stream)
                .read_until(b'\n', &mut line)
                .await
                .unwrap();
            assert_eq!(line, b"\"Windows\"\n");
            let mut reply=serde_json::to_vec(&serde_json::json!({"Ok":{"Windows":[{"id":1,"title":"Editor\ntext","app_id":"edit"},{"id":2,"title":"Chooser","app_id":"org.canopa.Portal"}]}})).unwrap();
            reply.push(b'\n');
            stream.write_all(&reply).await.unwrap();
        });
        let items = read(&socket, std::process::id(), 2).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Editortext");
        assert_eq!(items[0].source, Source::Window(1));
        server.await.unwrap();
        let listener = tokio::net::UnixListener::bind(root.path().join("other.sock")).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.is_empty());
        });
        assert!(
            read(&root.path().join("other.sock"), std::process::id() + 1, 2)
                .await
                .is_err()
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn oversized_or_duplicate_window_inventory_is_not_a_source_grant() {
        for reply in [
            b"{\"Ok\":{\"Windows\":[{\"id\":1},{\"id\":1}]}}\n".to_vec(),
            vec![b'x'; 1024 * 1024 + 2],
        ] {
            let root = tempfile::tempdir().unwrap();
            let socket = root.path().join("ipc.sock");
            let listener = tokio::net::UnixListener::bind(&socket).unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut line = Vec::new();
                BufReader::new(&mut stream)
                    .read_until(b'\n', &mut line)
                    .await
                    .unwrap();
                let _ = stream.write_all(&reply).await;
            });
            assert!(read(&socket, std::process::id(), 2).await.is_err());
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn inventory_carries_live_monitor_identity_from_the_pinned_bus_and_ipc_owner() {
        use crate::screencast_capture::tests::fake;
        use std::sync::atomic::Ordering;
        let fixture = fake::Fixture::new().await;
        fixture.state.source_version.store(1, Ordering::SeqCst);
        fixture.state.monitor_identity.store(77, Ordering::SeqCst);
        let owner = Owner::resolve(&fixture.backend).await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("ipc.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut line = Vec::new();
                BufReader::new(&mut stream)
                    .read_until(b'\n', &mut line)
                    .await
                    .unwrap();
                assert_eq!(line, b"\"Outputs\"\n");
                stream.write_all(b"{\"Ok\":{\"Outputs\":{\"DP-1\":{\"logical\":{},\"model\":\"Display\"}}}}\n").await.unwrap();
            }
        });
        let initial = inventory(
            &fixture.backend,
            Some(&owner),
            &socket,
            std::process::id(),
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            initial[0].source,
            Source::Monitor(crate::screencast_capture::Monitor::pinned("DP-1", 77).unwrap())
        );
        fixture.state.monitor_identity.store(78, Ordering::SeqCst);
        let current = inventory(
            &fixture.backend,
            Some(&owner),
            &socket,
            std::process::id(),
            1,
        )
        .await
        .unwrap();
        assert_eq!(initial[0].title, current[0].title);
        assert_ne!(initial[0].source, current[0].source);
        server.await.unwrap();
        let input = DialogInput {
            app_id: "fixture".into(),
            parent_window: String::new(),
            modal: false,
            multiple: false,
            items: initial,
            socket,
            compositor_pid: std::process::id(),
            compositor_owner: Some(":1.9999999".into()),
            types: 1,
            activation_token: None,
        };
        let error = super::super::refresh_from(&input, &fixture.backend)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("compositor changed"), "{error}");
    }
}
