//! One Settings window per session, reachable on the session bus.
//!
//! The first `canopa-settings` that opens a window owns [`NAME`] and serves
//! [`INTERFACE`] at [`PATH`]. A later launch, or any D-Bus caller, asks it to
//! show a page with `OpenPage`, or just to come forward with `Activate`; the
//! later launch then exits. Without a session bus each launch opens its own
//! window, as before.
//!
//! ```text
//! org.canopa.Settings
//!   Activate()                          raise the window
//!   OpenPage(s page, a{sv} options)     open a page by id, then raise
//!     options: "display-profile" s      Displays: select a saved layout
//!   Pages                               as, read-only: visible page ids
//! ```
//!
//! An unknown page id, or an option of the wrong type, is
//! `org.freedesktop.DBus.Error.InvalidArgs`; unknown options are ignored so
//! newer callers keep working with an older window.
use crate::{Page, registry};
use anyhow::{Context, Result};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::{fdo::RequestNameFlags, zvariant::OwnedValue};

pub const NAME: &str = "org.canopa.Settings";
pub const PATH: &str = "/org/canopa/Settings";
pub const INTERFACE: &str = "org.canopa.Settings";
/// The longest display profile name a request may carry.
const PROFILE_LIMIT: usize = 256;
/// How long a launch waits for the bus or the running window.
const TIMEOUT: Duration = Duration::from_secs(10);

/// What a launch or a D-Bus call asks the window to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    /// `None` only raises the window.
    pub page: Option<Page>,
    /// A saved display layout to select on Displays.
    pub display_profile: Option<String>,
}

impl Request {
    /// The request for `OpenPage(page, options)`.
    pub fn open(page: &str, options: &HashMap<String, OwnedValue>) -> Result<Self, String> {
        let page = page.parse::<Page>()?;
        let display_profile = match options.get("display-profile") {
            None => None,
            Some(value) => {
                let name = String::try_from(value.try_clone().map_err(|e| e.to_string())?)
                    .map_err(|_| "display-profile must be a string".to_owned())?;
                if name.len() > PROFILE_LIMIT || name.chars().any(char::is_control) {
                    return Err("invalid display-profile name".into());
                }
                if page != Page::Displays {
                    return Err("display-profile opens the Displays page".into());
                }
                Some(name)
            }
        };
        Ok(Self {
            page: Some(page),
            display_profile,
        })
    }

    /// The `OpenPage` options for this request.
    fn options(&self) -> HashMap<&'static str, zbus::zvariant::Value<'_>> {
        self.display_profile
            .iter()
            .map(|name| ("display-profile", name.as_str().into()))
            .collect()
    }
}

struct Interface {
    requests: UnboundedSender<Request>,
}

impl Interface {
    fn send(&self, request: Request) -> zbus::fdo::Result<()> {
        self.requests
            .send(request)
            .map_err(|_| zbus::fdo::Error::Failed("the Settings window is closing".into()))
    }
}

#[zbus::interface(name = "org.canopa.Settings")]
impl Interface {
    /// Raise the window.
    fn activate(&self) -> zbus::fdo::Result<()> {
        self.send(Request::default())
    }

    /// Open a page by id, then raise the window.
    fn open_page(&self, page: &str, options: HashMap<String, OwnedValue>) -> zbus::fdo::Result<()> {
        let request = Request::open(page, &options).map_err(zbus::fdo::Error::InvalidArgs)?;
        self.send(request)
    }

    /// Visible page ids, in sidebar order.
    #[zbus(property)]
    fn pages(&self) -> Vec<String> {
        registry::ids().map(str::to_owned).collect()
    }
}

/// The running window's end: requests from later launches and D-Bus callers.
pub struct Instance {
    pub requests: UnboundedReceiver<Request>,
}

pub enum Claim {
    /// This process owns the name and should open the window.
    Primary(Instance),
    /// The running window accepted the request; this process can exit.
    Forwarded,
}

/// Own the session's Settings name, or hand `request` to the window that
/// does. The connection lives on its own thread for the rest of the process.
pub fn claim(request: &Request) -> Result<Claim> {
    claim_at(None, request)
}

/// [`claim`] on the bus at `address`, or the session bus.
pub fn claim_at(address: Option<&str>, request: &Request) -> Result<Claim> {
    let (reply, outcome) = std::sync::mpsc::sync_channel(1);
    let address = address.map(str::to_owned);
    let request = request.clone();
    std::thread::Builder::new()
        .name("settings-bus".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = reply.send(Err(error.into()));
                    return;
                }
            };
            runtime.block_on(async move {
                let (requests, receiver) = unbounded_channel();
                let connection =
                    match tokio::time::timeout(TIMEOUT, own(address.as_deref(), requests))
                        .await
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("the session bus did not answer")))
                    {
                        Ok(Some(connection)) => connection,
                        Ok(None) => {
                            let forwarded = tokio::time::timeout(
                                TIMEOUT,
                                forward(address.as_deref(), &request),
                            )
                            .await
                            .unwrap_or_else(|_| {
                                Err(anyhow::anyhow!("the Settings window did not answer"))
                            });
                            let _ = reply.send(forwarded.map(|()| Claim::Forwarded));
                            return;
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                            return;
                        }
                    };
                if reply
                    .send(Ok(Claim::Primary(Instance { requests: receiver })))
                    .is_ok()
                {
                    // Serve requests until the process exits.
                    std::future::pending::<()>().await;
                }
                drop(connection);
            });
        })?;
    outcome.recv().context("the session bus thread stopped")?
}

async fn connect(address: Option<&str>) -> Result<zbus::connection::Builder<'static>> {
    Ok(match address {
        Some(address) => zbus::connection::Builder::address(address)?,
        None => zbus::connection::Builder::session()?,
    })
}

/// Serve the interface, then ask for the name. `None` when another process
/// owns it.
async fn own(
    address: Option<&str>,
    requests: UnboundedSender<Request>,
) -> Result<Option<zbus::Connection>> {
    let connection = connect(address)
        .await?
        .serve_at(PATH, Interface { requests })?
        .build()
        .await
        .context("could not connect to the session bus")?;
    match connection
        .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(_) => Ok(Some(connection)),
        Err(zbus::Error::NameTaken) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn forward(address: Option<&str>, request: &Request) -> Result<()> {
    let connection = connect(address).await?.build().await?;
    let proxy = zbus::Proxy::new(&connection, NAME, PATH, INTERFACE).await?;
    match request.page {
        None => proxy.call_method("Activate", &()).await,
        Some(page) => {
            proxy
                .call_method("OpenPage", &(page.id(), request.options()))
                .await
        }
    }
    .context("the running Settings window refused the request")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Child, Command, Stdio},
    };

    struct Bus(Child, String);

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// A private session bus with no activatable services.
    fn bus(dir: &std::path::Path) -> Bus {
        let config = dir.join("bus.conf");
        std::fs::write(&config, "<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>").unwrap();
        let mut child = Command::new("dbus-daemon")
            .args(["--nofork", "--print-address=1", "--config-file"])
            .arg(&config)
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is required for the single-instance tests");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Bus(child, address.trim().to_owned())
    }

    #[test]
    fn requests_validate_pages_and_options() {
        let none = HashMap::new();
        assert_eq!(
            Request::open("power", &none),
            Ok(Request {
                page: Some(Page::Power),
                display_profile: None
            })
        );
        assert!(
            Request::open("unknown", &none)
                .unwrap_err()
                .contains("displays")
        );
        let profile = |value: zbus::zvariant::Value<'_>| {
            HashMap::from([(
                "display-profile".to_owned(),
                OwnedValue::try_from(value).unwrap(),
            )])
        };
        assert_eq!(
            Request::open("displays", &profile("Desk".into()))
                .unwrap()
                .display_profile
                .as_deref(),
            Some("Desk")
        );
        assert!(Request::open("displays", &profile(7u32.into())).is_err());
        assert!(Request::open("power", &profile("Desk".into())).is_err());
        assert!(Request::open("displays", &profile("a\nb".into())).is_err());
        let ignored = HashMap::from([("future".to_owned(), OwnedValue::from(1u32))]);
        assert!(Request::open("power", &ignored).is_ok());
    }

    #[test]
    fn the_first_launch_owns_the_name_and_later_launches_forward_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let bus = bus(dir.path());
        let first = Request::default();
        let Claim::Primary(mut instance) = claim_at(Some(&bus.1), &first).unwrap() else {
            panic!("the first launch must own the name");
        };
        let second = Request {
            page: Some(Page::Displays),
            display_profile: Some("Desk".into()),
        };
        assert!(matches!(
            claim_at(Some(&bus.1), &second).unwrap(),
            Claim::Forwarded
        ));
        assert_eq!(instance.requests.blocking_recv(), Some(second));
        assert!(matches!(
            claim_at(Some(&bus.1), &Request::default()).unwrap(),
            Claim::Forwarded
        ));
        assert_eq!(instance.requests.blocking_recv(), Some(Request::default()));

        // Other callers use the interface directly.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let connection = connect(Some(&bus.1)).await.unwrap().build().await.unwrap();
            let proxy = zbus::Proxy::new(&connection, NAME, PATH, INTERFACE)
                .await
                .unwrap();
            let pages: Vec<String> = proxy.get_property("Pages").await.unwrap();
            assert_eq!(pages.first().map(String::as_str), Some("displays"));
            assert!(pages.iter().any(|page| page == "widgets"));
            assert!(!pages.iter().any(|page| page == "live-example"));
            let error = proxy
                .call_method("OpenPage", &("nowhere", HashMap::<&str, zbus::zvariant::Value<'_>>::new()))
                .await
                .unwrap_err();
            assert!(
                matches!(&error, zbus::Error::MethodError(name, _, _) if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs"),
                "{error}"
            );
            proxy
                .call_method("OpenPage", &("shortcuts", HashMap::<&str, zbus::zvariant::Value<'_>>::new()))
                .await
                .unwrap();
        });
        assert_eq!(
            instance.requests.blocking_recv(),
            Some(Request {
                page: Some(Page::Shortcuts),
                display_profile: None
            })
        );
    }

    #[test]
    fn launches_without_a_bus_report_it() {
        assert!(
            claim_at(
                Some("unix:path=/nonexistent/canopa-settings-bus"),
                &Request::default()
            )
            .is_err()
        );
    }
}
