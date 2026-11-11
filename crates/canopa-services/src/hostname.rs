//! systemd-hostnamed: the computer's names and what it knows about the
//! hardware and the operating system.
//!
//! hostnamed is started on demand and exits when idle; [`watch`] follows it
//! like timedated. Renaming asks polkit, which may show a password prompt.
//!
//! ```no_run
//! use canopa_services::hostname;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let host = hostname::read(&system).await?;
//! println!("{} ({} {})", host.pretty_or_static(), host.vendor, host.model);
//! let name = hostname::static_name("Kevin's Laptop");
//! hostname::rename(&system, "Kevin's Laptop", &name).await?;
//! # Ok(()) }
//! ```
use crate::{
    Error,
    bus::{StateStream, call_interactive},
};
use std::time::Duration;

pub const SERVICE: &str = "org.freedesktop.hostname1";
pub const PATH: &str = "/org/freedesktop/hostname1";
pub const INTERFACE: &str = "org.freedesktop.hostname1";
const REREAD: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Host {
    /// The name in use now.
    pub hostname: String,
    /// The configured name (`/etc/hostname`).
    pub static_hostname: String,
    /// A free-form name for people, such as "Kevin's Laptop".
    pub pretty_hostname: String,
    /// `laptop`, `desktop`, `server`, `tablet`, `vm`…
    pub chassis: String,
    pub vendor: String,
    pub model: String,
    pub firmware: String,
    pub kernel: String,
    pub operating_system: String,
}

impl Host {
    /// The pretty name, else the static name, else the name in use.
    pub fn pretty_or_static(&self) -> &str {
        [&self.pretty_hostname, &self.static_hostname, &self.hostname]
            .into_iter()
            .find(|name| !name.is_empty())
            .map_or("", String::as_str)
    }
}

/// Read the names and hardware, starting hostnamed if needed.
pub async fn read(connection: &zbus::Connection) -> Result<Host, Error> {
    let all = crate::bus::properties(connection, SERVICE, PATH, INTERFACE).await?;
    let text = |name: &str| {
        all.get(name)
            .and_then(|value| <&str>::try_from(value).ok())
            .map(|value| crate::bus::bounded_text(value, 128).replace('\n', " "))
            .unwrap_or_default()
    };
    let kernel = [text("KernelName"), text("KernelRelease")]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(Host {
        hostname: text("Hostname"),
        static_hostname: text("StaticHostname"),
        pretty_hostname: text("PrettyHostname"),
        chassis: text("Chassis"),
        vendor: text("HardwareVendor"),
        model: text("HardwareModel"),
        firmware: text("FirmwareVersion"),
        kernel,
        operating_system: text("OperatingSystemPrettyName"),
    })
}

/// The names on each change.
pub fn watch(connection: zbus::Connection) -> StateStream<Host> {
    crate::bus::watch_on_demand(connection, SERVICE, PATH, REREAD, |connection| async move {
        read(&connection).await
    })
}

/// A valid static hostname for a pretty name, as GNOME derives it:
/// lowercase letters, digits and single hyphens, at most 63 characters,
/// `localhost` when nothing is left.
pub fn static_name(pretty: &str) -> String {
    let mut name = String::new();
    for c in pretty.chars() {
        let c = match c {
            'a'..='z' | '0'..='9' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ if c.is_whitespace() || c == '-' || c == '_' || c == '.' => '-',
            _ => continue,
        };
        if c == '-' && (name.is_empty() || name.ends_with('-')) {
            continue;
        }
        name.push(c);
    }
    name.truncate(63);
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "localhost".into()
    } else {
        name.into()
    }
}

/// Whether `name` is a valid static hostname.
pub fn valid_static(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Set the pretty name and the static name.
pub async fn rename(connection: &zbus::Connection, pretty: &str, name: &str) -> Result<(), Error> {
    if !valid_static(name) {
        return Err(Error::Invalid("host name"));
    }
    if pretty.chars().count() > 64 || pretty.chars().any(char::is_control) {
        return Err(Error::Invalid("device name"));
    }
    let proxy = zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?;
    call_interactive::<_, ()>(&proxy, "SetPrettyHostname", &(pretty, true)).await?;
    call_interactive(&proxy, "SetStaticHostname", &(name, true)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ServiceState,
        testing::{TestBus, hostname::FakeHostnamed},
    };
    use futures_util::StreamExt;

    #[test]
    fn static_names_follow_hostname_rules() {
        assert_eq!(static_name("Kevin's Laptop"), "kevins-laptop");
        assert_eq!(static_name("  --Été 2026!  "), "t-2026");
        assert_eq!(static_name("???"), "localhost");
        assert_eq!(static_name(&"a".repeat(80)).len(), 63);
        assert!(valid_static("kevins-laptop"));
        assert!(!valid_static("-x") && !valid_static("A") && !valid_static(""));
    }

    #[tokio::test]
    async fn reads_follows_and_renames() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, Err(Error::Unavailable));
        let fake = FakeHostnamed::serve(&bus).await;
        let host = read(&client).await.unwrap();
        assert_eq!(host.pretty_or_static(), "canopa-test");
        assert_eq!(host.model, "Test Book 14");
        assert_eq!(host.kernel, "Linux 6.12.0");
        let mut states = watch(client.clone());
        let next = async |states: &mut StateStream<Host>| {
            tokio::time::timeout(Duration::from_secs(3), states.next())
                .await
                .unwrap()
                .unwrap()
        };
        assert!(matches!(
            next(&mut states).await,
            ServiceState::Available(_)
        ));
        rename(&client, "Kevin's Laptop", "kevins-laptop")
            .await
            .unwrap();
        let ServiceState::Available(host) = next(&mut states).await else {
            panic!("a host expected");
        };
        assert_eq!(host.pretty_or_static(), "Kevin's Laptop");
        assert_eq!(fake.state.lock().unwrap().1, "kevins-laptop");
        assert_eq!(
            rename(&client, "x", "Bad Name").await,
            Err(Error::Invalid("host name"))
        );
    }
}
