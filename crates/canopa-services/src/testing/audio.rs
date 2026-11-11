//! A private audio server for the Pulse-protocol clients. Requires
//! `pipewire`, `pipewire-pulse`, `pactl` and `pw-metadata`.
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// A private PipeWire and pipewire-pulse pair without hardware or session
/// manager. Load synthetic devices with `pactl load-module module-null-sink`.
pub struct AudioServer {
    children: Vec<Child>,
    directory: tempfile::TempDir,
    /// The Pulse server address to pass to [`crate::audio::monitor`].
    pub server: String,
}

impl AudioServer {
    pub async fn new() -> Self {
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

    pub fn command(&self, program: &str) -> Command {
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

    /// The pipewire-pulse process, for example to stop it with `kill -STOP`.
    pub fn pulse_pid(&self) -> u32 {
        self.children.last().unwrap().id()
    }

    /// Kill pipewire-pulse, as a crashed audio endpoint would.
    pub fn stop_pulse(&mut self) {
        let pulse = self.children.last_mut().unwrap();
        pulse.kill().unwrap();
        pulse.wait().unwrap();
    }

    /// Start pipewire-pulse again after [`AudioServer::stop_pulse`].
    pub fn restart_pulse(&mut self) {
        let child = self
            .command("pipewire-pulse")
            .args(["-c", "pipewire-pulse.conf"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.children.push(child);
    }

    pub async fn pactl(&self, args: &[&str]) -> String {
        self.run("pactl", args).await
    }

    pub async fn set_default_sink(&self, name: &str) {
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

    pub async fn run(&self, program: &str, args: &[&str]) -> String {
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
