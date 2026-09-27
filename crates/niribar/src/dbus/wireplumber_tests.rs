//! Real Pulse protocol tests with private PipeWire daemons and synthetic sinks.
//! These configs load no hardware discovery or desktop session manager.
use super::*;
use std::process::{Child, Command, Stdio};

struct AudioServer {
    children: Vec<Child>,
    directory: tempfile::TempDir,
    server: String,
}

impl AudioServer {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut server = Self {
            server: format!("unix:{}/pulse/native", directory.path().display()),
            directory,
            children: Vec::new(),
        };
        std::fs::write(
            server.directory.path().join("client.conf"),
            r#"
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-metadata }
            ]
        "#,
        )
        .unwrap();
        let core_config = r#"
            context.properties = { core.daemon = true core.name = pipewire-0 }
            context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-access }
                { name = libpipewire-module-metadata }
                { name = libpipewire-module-spa-node-factory }
                { name = libpipewire-module-client-node }
                { name = libpipewire-module-adapter }
                { name = libpipewire-module-link-factory }
            ]
            context.objects = [ { factory = metadata args = { metadata.name = default } } ]
        "#;
        let pulse_config = r#"
            context.properties = { }
            context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-client-node }
                { name = libpipewire-module-adapter }
                { name = libpipewire-module-metadata }
                { name = libpipewire-module-protocol-pulse args = { server.address = [ "unix:native" ] } }
            ]
        "#;
        for (program, config) in [("pipewire", core_config), ("pipewire-pulse", pulse_config)] {
            let path = server.directory.path().join(format!("{program}.conf"));
            std::fs::write(&path, config).unwrap();
            let log = std::fs::File::create(server.directory.path().join(format!("{program}.log")))
                .unwrap();
            let child = server
                .command(program)
                .arg("-c")
                .arg(path.file_name().unwrap())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log)
                .spawn()
                .expect("install pipewire and pipewire-pulse for the audio transport tests");
            server.children.push(child);
            let socket = if program == "pipewire" {
                "pipewire-0"
            } else {
                "pulse/native"
            };
            let deadline = Instant::now() + Duration::from_secs(3);
            while !server.directory.path().join(socket).exists() {
                assert!(
                    Instant::now() < deadline,
                    "{program} failed: {}",
                    std::fs::read_to_string(server.directory.path().join(format!("{program}.log")))
                        .unwrap()
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        server.pactl(&["info"]).await;
        server
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .env("XDG_RUNTIME_DIR", self.directory.path())
            .env("XDG_CONFIG_HOME", self.directory.path())
            .env("PIPEWIRE_RUNTIME_DIR", self.directory.path())
            .env("PIPEWIRE_CONFIG_DIR", self.directory.path())
            .env("PIPEWIRE_CONFIG_PREFIX", "")
            .env("PIPEWIRE_REMOTE", "pipewire-0")
            .env("PULSE_RUNTIME_PATH", self.directory.path().join("pulse"))
            .env("PULSE_SERVER", &self.server)
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
        command
    }

    async fn pactl(&self, args: &[&str]) -> String {
        self.run("pactl", args).await
    }

    async fn set_default_sink(&self, name: &str) {
        // A real session manager copies the configured choice into active
        // default metadata. This hardware-free fixture supplies that policy.
        self.pactl(&["set-default-sink", name]).await;
        self.run(
            "pw-metadata",
            &[
                "-n",
                "default",
                "0",
                "default.audio.sink",
                &format!(r#"{{"name":"{name}"}}"#),
                "Spa:String:JSON",
            ],
        )
        .await;
        self.pactl(&["get-sink-volume", "@DEFAULT_SINK@"]).await;
    }

    async fn run(&self, program: &str, args: &[&str]) -> String {
        let mut command = tokio::process::Command::from(self.command(program));
        command.args(args).kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(3), command.output())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "private {program} {args:?} deadline; core: {}; pulse: {}",
                    std::fs::read_to_string(self.directory.path().join("pipewire.log")).unwrap(),
                    std::fs::read_to_string(self.directory.path().join("pipewire-pulse.log"))
                        .unwrap()
                )
            })
            .expect("install pipewire-bin and pulseaudio-utils for audio tests");
        assert!(
            output.status.success(),
            "{program} {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
}

impl Drop for AudioServer {
    fn drop(&mut self) {
        for child in self.children.iter_mut().rev() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn expect(
    rx: &mut mpsc::UnboundedReceiver<Result<Option<(f64, bool)>, ()>>,
    expected: Option<(f64, bool)>,
) {
    let mut last = None;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let reading = rx.recv().await.unwrap().unwrap();
            last = Some(reading);
            match (reading, expected) {
                (None, None) => break,
                (Some((actual, muted)), Some((volume, expected_muted)))
                    if (actual - volume).abs() < 0.1 && muted == expected_muted =>
                {
                    break;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("audio subscription expected {expected:?}, last reading {last:?}"));
}

#[tokio::test]
async fn pulse_protocol_controls_and_tracks_default_sink_removal_and_reappearance() {
    let mut server = AudioServer::new().await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let address = server.server.clone();
    let observer = std::thread::spawn(move || run_pa_thread_on(Some(&address), tx));
    expect(&mut rx, None).await;
    assert!(adjust_volume_on(Some(&server.server), 5, true).is_err());
    let first = server
        .pactl(&["load-module", "module-null-sink", "sink_name=first"])
        .await;
    server.set_default_sink("first").await;
    server.pactl(&["set-sink-volume", "first", "30%"]).await;
    expect(&mut rx, Some((30.0, false))).await;
    adjust_volume_on(Some(&server.server), 5, true).unwrap();
    expect(&mut rx, Some((35.0, false))).await;
    adjust_volume_on(Some(&server.server), 10, false).unwrap();
    expect(&mut rx, Some((25.0, false))).await;
    let second = server
        .pactl(&["load-module", "module-null-sink", "sink_name=second"])
        .await;
    server.pactl(&["set-sink-volume", "second", "70%"]).await;
    server.pactl(&["set-sink-mute", "second", "1"]).await;
    server.set_default_sink("second").await;
    expect(&mut rx, Some((70.0, true))).await;
    server.pactl(&["unload-module", &first]).await;
    server.pactl(&["unload-module", &second]).await;
    expect(&mut rx, None).await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=returned"])
        .await;
    server.set_default_sink("returned").await;
    server.pactl(&["set-sink-volume", "returned", "45%"]).await;
    expect(&mut rx, Some((45.0, false))).await;
    server.children.last_mut().unwrap().kill().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while rx.recv().await.unwrap().is_ok() {}
    })
    .await
    .unwrap();
    drop(rx);
    observer.join().unwrap();
}
