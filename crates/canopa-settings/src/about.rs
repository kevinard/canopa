//! The About page's model: the computer's names from systemd-hostnamed (the
//! device name can be changed, through polkit), and what this computer is,
//! read from `/etc/os-release`, `/proc` and `/sys`.
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::{
    ServiceState,
    hostname::{self, Host},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// What this computer is. Every part may be unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub os_build: String,
    pub kernel: String,
    pub vendor: String,
    pub model: String,
    pub firmware: String,
    pub cpu: String,
    pub cores: usize,
    pub memory_bytes: u64,
    pub graphics: Vec<String>,
    /// Built-in disks: their name and size.
    pub disks: Vec<(String, u64)>,
    pub canopa_version: String,
}

/// At most this much of a system file is read.
const MAX_FILE: usize = 4 * 1024 * 1024;

fn read(path: &Path) -> Option<String> {
    canopa_file::read_bounded(path, MAX_FILE)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn line(path: &Path) -> String {
    read(path)
        .map(|text| {
            text.trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(128)
                .collect()
        })
        .unwrap_or_default()
}

/// `os-release` values, unquoted.
pub fn os_release(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            (key.trim().to_owned(), value.replace("\\\"", "\""))
        })
        .collect()
}

/// A PCI device's vendor and device names from `pci.ids`.
pub fn pci_name(ids: &str, vendor: u16, device: u16) -> Option<(String, Option<String>)> {
    let vendor_key = format!("{vendor:04x}  ");
    let device_key = format!("\t{device:04x}  ");
    let mut lines = ids.lines();
    let vendor_name = lines
        .by_ref()
        .find_map(|line| line.strip_prefix(&vendor_key))?;
    let device_name = lines
        .take_while(|line| line.starts_with('\t') || line.starts_with('#') || line.is_empty())
        .find_map(|line| line.strip_prefix(&device_key));
    Some((vendor_name.to_owned(), device_name.map(str::to_owned)))
}

/// Shorter vendor names, as `lspci` users know them.
fn short_vendor(name: &str) -> &str {
    if name.starts_with("Intel") {
        "Intel"
    } else if name.starts_with("Advanced Micro Devices") {
        "AMD"
    } else if name.starts_with("NVIDIA") {
        "NVIDIA"
    } else {
        name
    }
}

impl SystemInfo {
    /// Read everything under `root` (`/` outside tests). Runs on a worker.
    pub fn read(root: &Path) -> Self {
        let at = |path: &str| root.join(path.trim_start_matches('/'));
        let release = read(&at("/etc/os-release"))
            .or_else(|| read(&at("/usr/lib/os-release")))
            .map(|text| os_release(&text))
            .unwrap_or_default();
        let get = |key: &str| release.get(key).cloned().unwrap_or_default();
        let os_name = [get("NAME"), get("PRETTY_NAME")]
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_else(|| "Linux".into());
        let build = get("BUILD_ID");
        let os_version = [get("VERSION"), get("VERSION_ID")]
            .into_iter()
            .find(|version| !version.is_empty())
            .or_else(|| (build == "rolling").then(|| "Rolling release".to_owned()))
            .unwrap_or_default();
        let os_build = if build == "rolling" {
            String::new()
        } else {
            build.chars().take(64).collect()
        };

        let cpuinfo = read(&at("/proc/cpuinfo")).unwrap_or_default();
        let cpu = cpuinfo
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                matches!(key.trim(), "model name" | "Model" | "cpu model")
                    .then(|| value.split_whitespace().collect::<Vec<_>>().join(" "))
            })
            .unwrap_or_default();
        let cores = cpuinfo
            .lines()
            .filter(|line| {
                line.split(':')
                    .next()
                    .is_some_and(|key| key.trim() == "processor")
            })
            .count();
        let memory_bytes = read(&at("/proc/meminfo"))
            .unwrap_or_default()
            .lines()
            .find_map(|line| {
                let kilobytes = line.strip_prefix("MemTotal:")?.trim().strip_suffix("kB")?;
                kilobytes.trim().parse::<u64>().ok()
            })
            .map_or(0, |kilobytes| kilobytes * 1024);

        let pci_ids = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"]
            .into_iter()
            .find_map(|path| read(&at(path)))
            .unwrap_or_default();
        let mut graphics = Vec::new();
        if let Ok(cards) = std::fs::read_dir(at("/sys/class/drm")) {
            let mut cards: Vec<_> = cards
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| {
                    name.strip_prefix("card")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
                .collect();
            cards.sort();
            for card in cards {
                let device = at("/sys/class/drm").join(&card).join("device");
                let hex = |name: &str| {
                    u16::from_str_radix(line(&device.join(name)).trim_start_matches("0x"), 16).ok()
                };
                let driver = std::fs::read_link(device.join("driver"))
                    .ok()
                    .and_then(|link| link.file_name().map(|n| n.to_string_lossy().into_owned()));
                let name = match (hex("vendor"), hex("device")) {
                    (Some(vendor), Some(id)) => match pci_name(&pci_ids, vendor, id) {
                        Some((vendor, Some(device))) => {
                            format!("{} {device}", short_vendor(&vendor))
                        }
                        Some((vendor, None)) => format!("{} graphics", short_vendor(&vendor)),
                        None => format!("Graphics {vendor:04x}:{id:04x}"),
                    },
                    _ => match &driver {
                        Some(driver) => format!("Graphics ({driver})"),
                        None => continue,
                    },
                };
                if !graphics.contains(&name) {
                    graphics.push(name);
                }
            }
        }

        let mut disks = Vec::new();
        if let Ok(blocks) = std::fs::read_dir(at("/sys/block")) {
            let mut names: Vec<_> = blocks
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| {
                    !["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd"]
                        .iter()
                        .any(|prefix| name.starts_with(prefix))
                })
                .collect();
            names.sort();
            for name in names {
                let block = at("/sys/block").join(&name);
                if line(&block.join("removable")) == "1" {
                    continue;
                }
                let sectors: u64 = line(&block.join("size")).parse().unwrap_or(0);
                if sectors > 0 {
                    let model = line(&block.join("device/model"));
                    disks.push((if model.is_empty() { name } else { model }, sectors * 512));
                }
            }
        }

        Self {
            os_name,
            os_version,
            os_build,
            kernel: match line(&at("/proc/sys/kernel/osrelease")) {
                release if release.is_empty() => String::new(),
                release => format!("Linux {release}"),
            },
            vendor: line(&at("/sys/class/dmi/id/sys_vendor")),
            model: {
                let name = line(&at("/sys/class/dmi/id/product_name"));
                let version = line(&at("/sys/class/dmi/id/product_version"));
                // Lenovo puts the marketing name in the version.
                if version.starts_with("ThinkPad") || version.starts_with("IdeaPad") {
                    version
                } else {
                    name
                }
            },
            firmware: line(&at("/sys/class/dmi/id/bios_version")),
            cpu,
            cores,
            memory_bytes,
            graphics,
            disks,
            canopa_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

/// A size in decimal units, as disks are sold: "512 GB", "1.0 TB".
pub fn decimal_size(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < units.len() {
        value /= 1000.0;
        unit += 1;
    }
    if value >= 100.0 || unit == 0 {
        format!("{value:.0} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// Memory in binary units, as it is installed: "16 GiB".
pub fn binary_size(bytes: u64) -> String {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gib >= 10.0 {
        format!("{gib:.0} GiB")
    } else {
        format!("{gib:.1} GiB")
    }
}

/// What the page shows. `host` is `None` while hostnamed is not available,
/// which only hides the names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AboutState {
    pub host: Option<Host>,
    pub system: Arc<SystemInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AboutAction {
    /// The device name; the host name is derived from it.
    Rename(String),
}

impl LiveAction for AboutAction {
    fn control(&self) -> String {
        "name".into()
    }
}

#[derive(Clone)]
pub struct AboutService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    info: Arc<tokio::sync::OnceCell<Arc<SystemInfo>>>,
    root: PathBuf,
}

impl Default for AboutService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            info: Arc::default(),
            root: "/".into(),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    ServiceError::Failed(match error {
        Error::Unavailable => "systemd-hostnamed is not available.".into(),
        Error::NotAuthorized => "The system did not allow this change.".into(),
        Error::TimedOut => "The system did not answer in time.".into(),
        Error::Changed => "The name changed. Try again.".into(),
        Error::Invalid(what) => format!("That is not a valid {what}."),
        Error::Refused(message) => message,
    })
}

impl AboutService {
    /// A service on an existing connection, reading files under `root` (tests).
    pub fn with(system: zbus::Connection, root: PathBuf) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            root,
            ..Default::default()
        }
    }

    async fn info(&self) -> Arc<SystemInfo> {
        let root = self.root.clone();
        self.info
            .get_or_init(|| async move {
                let info = tokio::task::spawn_blocking(move || SystemInfo::read(&root))
                    .await
                    .unwrap_or_default();
                Arc::new(info)
            })
            .await
            .clone()
    }

    async fn host(&self) -> Option<Host> {
        let system = self
            .system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .ok()?;
        hostname::read(system).await.ok()
    }

    async fn read(&self) -> AboutState {
        AboutState {
            host: self.host().await,
            system: self.info().await,
        }
    }
}

impl LiveService for AboutService {
    type State = AboutState;
    type Action = AboutAction;

    fn name(&self) -> &str {
        "systemd-hostnamed"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<AboutState, ServiceError>> {
        let service = self.clone();
        async move { Ok(service.read().await) }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<AboutState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Ok(system) = service
                .system
                .get_or_try_init(canopa_services::bus::system)
                .await
                .cloned()
            else {
                return futures_util::stream::pending().boxed();
            };
            let info = service.info().await;
            hostname::watch(system)
                .map(move |host| {
                    Ok(AboutState {
                        host: match host {
                            ServiceState::Available(host) => Some(host),
                            ServiceState::Unavailable => None,
                        },
                        system: info.clone(),
                    })
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: AboutAction,
    ) -> BoxFuture<'static, Result<Option<AboutState>, ServiceError>> {
        let service = self.clone();
        async move {
            let AboutAction::Rename(pretty) = action;
            let pretty = pretty.trim().to_owned();
            if pretty.is_empty() {
                return Err(ServiceError::Failed(
                    "Enter a name for this computer.".into(),
                ));
            }
            let system = service
                .system
                .get_or_try_init(canopa_services::bus::system)
                .await
                .map_err(|_| ServiceError::Failed("The system bus is not reachable.".into()))?
                .clone();
            hostname::rename(&system, &pretty, &hostname::static_name(&pretty))
                .await
                .map_err(failure)?;
            Ok(Some(service.read().await))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, hostname::FakeHostnamed};

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        write(
            r,
            "etc/os-release",
            "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nBUILD_ID=rolling\nID=arch\n",
        );
        write(
            r,
            "proc/cpuinfo",
            "processor\t: 0\nmodel name\t: AMD Ryzen 7  7840U w/ Radeon 780M Graphics\n\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7  7840U w/ Radeon 780M Graphics\n",
        );
        write(
            r,
            "proc/meminfo",
            "MemTotal:       32000000 kB\nMemFree: 1 kB\n",
        );
        write(
            r,
            "usr/share/hwdata/pci.ids",
            "# ids\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t15bf  Phoenix1\n\t15c8  Phoenix2\n8086  Intel Corporation\n\t46a6  Alder Lake-P GT2 [Iris Xe Graphics]\n",
        );
        write(r, "sys/class/drm/card1/device/vendor", "0x1002\n");
        write(r, "sys/class/drm/card1/device/device", "0x15bf\n");
        write(r, "sys/class/drm/card1-eDP-1/status", "connected\n");
        write(r, "sys/block/nvme0n1/size", "1000215216\n");
        write(r, "sys/block/nvme0n1/removable", "0\n");
        write(r, "sys/block/nvme0n1/device/model", "Test NVMe 512GB\n");
        write(r, "sys/block/loop0/size", "100\n");
        write(r, "sys/block/sda/size", "100\n");
        write(r, "sys/block/sda/removable", "1\n");
        write(r, "sys/class/dmi/id/sys_vendor", "LENOVO\n");
        write(r, "sys/class/dmi/id/product_name", "21K5CTO1WW\n");
        write(
            r,
            "sys/class/dmi/id/product_version",
            "ThinkPad T14s Gen 4\n",
        );
        root
    }

    #[test]
    fn the_computer_is_described_from_proc_and_sys() {
        let root = fixture();
        let info = SystemInfo::read(root.path());
        assert_eq!(info.os_name, "Arch Linux");
        assert_eq!(info.os_version, "Rolling release");
        assert_eq!(info.cpu, "AMD Ryzen 7 7840U w/ Radeon 780M Graphics");
        assert_eq!(info.cores, 2);
        assert_eq!(binary_size(info.memory_bytes), "31 GiB");
        assert_eq!(info.graphics, ["AMD Phoenix1"]);
        assert_eq!(
            info.disks,
            [("Test NVMe 512GB".to_owned(), 1000215216 * 512)]
        );
        assert_eq!(decimal_size(info.disks[0].1), "512 GB");
        assert_eq!(info.model, "ThinkPad T14s Gen 4");
        assert!(!info.canopa_version.is_empty());
        // Nothing there is fine.
        let empty = tempfile::tempdir().unwrap();
        let info = SystemInfo::read(empty.path());
        assert_eq!(info.os_name, "Linux");
        assert!(info.graphics.is_empty() && info.disks.is_empty());
    }

    #[test]
    fn small_parsers() {
        let release = os_release("NAME='Fedora Linux'\nVERSION=\"41 (Workstation Edition)\"\n#x\n");
        assert_eq!(release["NAME"], "Fedora Linux");
        assert_eq!(release["VERSION"], "41 (Workstation Edition)");
        let ids = "8086  Intel Corporation\n\t46a6  Alder Lake\n10de  NVIDIA Corporation\n";
        assert_eq!(
            pci_name(ids, 0x8086, 0x46a6),
            Some(("Intel Corporation".into(), Some("Alder Lake".into())))
        );
        assert_eq!(
            pci_name(ids, 0x8086, 0x1234),
            Some(("Intel Corporation".into(), None))
        );
        assert_eq!(pci_name(ids, 0x1234, 0x1), None);
        assert_eq!(decimal_size(1_000_000_000_000), "1.0 TB");
        assert_eq!(decimal_size(256_060_514_304), "256 GB");
    }

    #[tokio::test]
    async fn the_page_shows_and_renames_the_computer() {
        let bus = TestBus::new();
        let root = fixture();
        let service = AboutService::with(bus.connect().await, root.path().to_owned());
        // Without hostnamed the system is still described.
        let state = service.snapshot().await.unwrap();
        assert!(state.host.is_none());
        assert_eq!(state.system.os_name, "Arch Linux");
        let fake = FakeHostnamed::serve(&bus).await;
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.host.unwrap().static_hostname, "canopa-test");
        let state = service
            .apply(AboutAction::Rename("  Kevin's Laptop ".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.host.unwrap().pretty_hostname, "Kevin's Laptop");
        assert_eq!(fake.state.lock().unwrap().1, "kevins-laptop");
        assert!(matches!(
            service.apply(AboutAction::Rename("  ".into())).await,
            Err(ServiceError::Failed(_))
        ));
        let mut updates = service.updates();
        assert!(updates.next().await.unwrap().unwrap().host.is_some());
    }
}
