//! systemd-localed: the system locale (the login screen's and new users'
//! language and formats) and the system keyboard layout.
//!
//! localed is started on demand and exits when idle; [`watch`] follows it
//! like timedated. Changes ask polkit, which may show a password prompt.
//! localed accepts only installed locales.
//!
//! ```no_run
//! use canopa_services::locale;
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! let current = locale::read(&system).await?;
//! println!("{:?} {}", current.get("LANG"), current.x11_layout);
//! locale::set_locale(&system, &[("LANG".into(), "fr_FR.UTF-8".into())]).await?;
//! # Ok(()) }
//! ```
use crate::{
    Error,
    bus::{StateStream, call_interactive},
};
use std::time::Duration;

pub const SERVICE: &str = "org.freedesktop.locale1";
pub const PATH: &str = "/org/freedesktop/locale1";
pub const INTERFACE: &str = "org.freedesktop.locale1";
const REREAD: Duration = Duration::from_secs(30);

/// The locale variables localed accepts.
pub const VARIABLES: [&str; 13] = [
    "LANG",
    "LANGUAGE",
    "LC_CTYPE",
    "LC_NUMERIC",
    "LC_TIME",
    "LC_COLLATE",
    "LC_MONETARY",
    "LC_MESSAGES",
    "LC_PAPER",
    "LC_NAME",
    "LC_ADDRESS",
    "LC_TELEPHONE",
    "LC_MEASUREMENT",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemLocale {
    /// `LANG`, `LC_TIME`… and their values, as localed lists them.
    pub assignments: Vec<(String, String)>,
    pub x11_layout: String,
    pub x11_model: String,
    pub x11_variant: String,
    pub x11_options: String,
    pub vconsole_keymap: String,
}

impl SystemLocale {
    pub fn get(&self, variable: &str) -> Option<&str> {
        self.assignments
            .iter()
            .find(|(name, _)| name == variable)
            .map(|(_, value)| value.as_str())
    }
}

/// A plausible locale name such as `fr_FR.UTF-8` or `C.UTF-8`.
pub fn valid_locale(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'@' | b'-'))
}

pub async fn read(connection: &zbus::Connection) -> Result<SystemLocale, Error> {
    let all = crate::bus::properties(connection, SERVICE, PATH, INTERFACE).await?;
    let text = |name: &str| {
        all.get(name)
            .and_then(|value| <&str>::try_from(value).ok())
            .map(|value| crate::bus::bounded_text(value, 256))
            .unwrap_or_default()
    };
    let assignments = all
        .get("Locale")
        .and_then(|value| <Vec<String>>::try_from(value.try_clone().ok()?).ok())
        .unwrap_or_default()
        .into_iter()
        .take(VARIABLES.len())
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            (VARIABLES.contains(&name) && (name == "LANGUAGE" || valid_locale(value)))
                .then(|| (name.to_owned(), value.to_owned()))
        })
        .collect();
    Ok(SystemLocale {
        assignments,
        x11_layout: text("X11Layout"),
        x11_model: text("X11Model"),
        x11_variant: text("X11Variant"),
        x11_options: text("X11Options"),
        vconsole_keymap: text("VConsoleKeymap"),
    })
}

pub fn watch(connection: zbus::Connection) -> StateStream<SystemLocale> {
    crate::bus::watch_on_demand(connection, SERVICE, PATH, REREAD, |connection| async move {
        read(&connection).await
    })
}

/// Replace the system locale with these assignments.
pub async fn set_locale(
    connection: &zbus::Connection,
    assignments: &[(String, String)],
) -> Result<(), Error> {
    let mut lines = Vec::new();
    for (name, value) in assignments {
        if !VARIABLES.contains(&name.as_str()) || !valid_locale(value) {
            return Err(Error::Invalid("locale"));
        }
        lines.push(format!("{name}={value}"));
    }
    let proxy = zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?;
    call_interactive(&proxy, "SetLocale", &(lines, true)).await
}

/// Set the system keyboard (login screen, and the console through
/// localed's conversion). Fields are XKB names, comma-separated for several.
pub async fn set_x11_keyboard(
    connection: &zbus::Connection,
    layout: &str,
    model: &str,
    variant: &str,
    options: &str,
) -> Result<(), Error> {
    let valid = |value: &str| {
        value.len() <= 256
            && value.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(b, b',' | b'_' | b'-' | b':' | b'(' | b')' | b'+')
            })
    };
    if layout.is_empty() || ![layout, model, variant, options].into_iter().all(valid) {
        return Err(Error::Invalid("keyboard layout"));
    }
    let proxy = zbus::Proxy::new_owned(connection.clone(), SERVICE, PATH, INTERFACE).await?;
    call_interactive(
        &proxy,
        "SetX11Keyboard",
        &(layout, model, variant, options, true, true),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ServiceState,
        testing::{TestBus, locale::FakeLocaled},
    };
    use futures_util::StreamExt;

    #[tokio::test]
    async fn reads_follows_and_changes_the_system_locale() {
        let bus = TestBus::new();
        let client = bus.connect().await;
        assert_eq!(read(&client).await, Err(Error::Unavailable));
        let fake = FakeLocaled::serve(&bus).await;
        let current = read(&client).await.unwrap();
        assert_eq!(current.get("LANG"), Some("en_US.UTF-8"));
        assert_eq!(current.x11_layout, "us");
        let mut states = watch(client.clone());
        let next = async |states: &mut StateStream<SystemLocale>| {
            tokio::time::timeout(Duration::from_secs(3), states.next())
                .await
                .unwrap()
                .unwrap()
        };
        assert!(matches!(
            next(&mut states).await,
            ServiceState::Available(_)
        ));
        set_locale(
            &client,
            &[
                ("LANG".into(), "fr_FR.UTF-8".into()),
                ("LC_TIME".into(), "en_GB.UTF-8".into()),
            ],
        )
        .await
        .unwrap();
        let ServiceState::Available(current) = next(&mut states).await else {
            panic!("a locale expected");
        };
        assert_eq!(current.get("LC_TIME"), Some("en_GB.UTF-8"));
        assert_eq!(
            set_locale(&client, &[("PATH".into(), "x".into())]).await,
            Err(Error::Invalid("locale"))
        );
        // localed refuses locales that are not installed.
        assert!(matches!(
            set_locale(&client, &[("LANG".into(), "xx_XX.UTF-8".into())]).await,
            Err(Error::Refused(_))
        ));
        set_x11_keyboard(&client, "fr,us", "", "", "grp:alt_shift_toggle")
            .await
            .unwrap();
        assert_eq!(fake.keyboard.lock().unwrap().0, "fr,us");
        assert!(fake.keyboard.lock().unwrap().1, "the console follows");
        assert_eq!(
            set_x11_keyboard(&client, "us; rm", "", "", "").await,
            Err(Error::Invalid("keyboard layout"))
        );
    }
}
