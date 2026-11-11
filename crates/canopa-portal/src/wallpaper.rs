//! Wallpaper requests retain source/installation policy in the backend, not the UI.
use std::sync::Arc;

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use zbus::{Connection, fdo, message::Header, zvariant::OwnedObjectPath};

use crate::requests::{Completion, Options, Requests, option};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    Background,
    Lockscreen,
    Both,
}

impl Destination {
    #[cfg(feature = "desktop")]
    pub(crate) fn mask(self) -> u32 {
        match self {
            Self::Background => 1,
            Self::Lockscreen => 2,
            Self::Both => 3,
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::Background => fl!("portal-wallpaper-background"),
            Self::Lockscreen => fl!("portal-wallpaper-lock-screen"),
            Self::Both => fl!("portal-wallpaper-both"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WallpaperRequest {
    pub app_id: String,
    pub parent_window: String,
    pub uri: String,
    pub destination: Destination,
}

impl WallpaperRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        uri: String,
        mut options: Options,
    ) -> fdo::Result<Self> {
        let _: Option<bool> = option(&mut options, "show-preview")?;
        let destination = match option::<String>(&mut options, "set-on")?.as_deref() {
            None | Some("background") => Destination::Background,
            Some("lockscreen") => Destination::Lockscreen,
            Some("both") => Destination::Both,
            _ => {
                return Err(fdo::Error::InvalidArgs(
                    "Invalid wallpaper destination".into(),
                ));
            }
        };
        if uri.is_empty()
            || app_id.len() > 4096
            || parent_window.len() > 4096
            || uri.len() > 16 * 1024
            || app_id.chars().any(char::is_control)
            || parent_window.chars().any(char::is_control)
            || uri.chars().any(char::is_control)
        {
            return Err(fdo::Error::InvalidArgs("Invalid wallpaper request".into()));
        }
        Ok(Self {
            app_id,
            parent_window,
            uri,
            destination,
        })
    }
}

pub trait Presenter: Send + Sync + 'static {
    fn present<'a>(
        &'a self,
        request: WallpaperRequest,
        connection: &'a Connection,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<bool>>;
}

pub struct Wallpaper {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}
impl Wallpaper {
    pub fn new(presenter: Arc<dyn Presenter>) -> Self {
        Self::with_requests(presenter, Requests::default())
    }
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self {
            presenter,
            requests,
        }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Wallpaper")]
impl Wallpaper {
    #[zbus(name = "SetWallpaperURI", out_args("response"))]
    #[allow(clippy::too_many_arguments)]
    async fn set_wallpaper_uri(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        uri: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<u32> {
        let request = WallpaperRequest::parse(app_id, parent_window, uri, options)?;
        Ok(
            match self
                .requests
                .run(connection, header, handle, |_, cancelled| {
                    self.presenter.present(request, connection, cancelled)
                })
                .await?
            {
                Completion::Cancelled | Completion::Finished(Ok(false)) => 1,
                Completion::Finished(Ok(true)) => 0,
                Completion::Finished(Err(_)) => 2,
            },
        )
    }
}

pub struct Production {
    process: Arc<crate::process::DialogProcess>,
    #[cfg(feature = "desktop")]
    images: crate::wallpaper_image::Images,
}

impl Production {
    pub fn new(process: Arc<crate::process::DialogProcess>) -> Self {
        Self {
            process,
            #[cfg(feature = "desktop")]
            images: crate::wallpaper_image::Images::new(),
        }
    }
}

impl Presenter for Production {
    fn present<'a>(
        &'a self,
        request: WallpaperRequest,
        connection: &'a Connection,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<bool>> {
        Box::pin(async move {
            #[cfg(feature = "desktop")]
            {
                use std::time::Duration;
                let owner = tokio::time::timeout(Duration::from_secs(3), async {
                    let bus = zbus::fdo::DBusProxy::new(connection).await?;
                    let name = bus
                        .get_name_owner(
                            "org.canopa.Shell.Wallpaper"
                                .try_into()
                                .expect("static wallpaper bus name"),
                        )
                        .await?;
                    let proxy = zbus::Proxy::new_owned(
                        connection.clone(),
                        name,
                        "/org/canopa/Shell/Wallpaper",
                        "org.canopa.Shell.Wallpaper1",
                    )
                    .await?;
                    let destinations: u32 = proxy.get_property("Destinations").await?;
                    anyhow::ensure!(
                        destinations & request.destination.mask() == request.destination.mask(),
                        "Wallpaper destination unavailable"
                    );
                    Ok::<_, anyhow::Error>(proxy)
                })
                .await??;
                let frozen = self.images.prepare(&request.uri, &cancelled).await?;
                let input = crate::wallpaper_image::DialogInput {
                    app_id: request.app_id,
                    parent_window: request.parent_window,
                    destination: request.destination,
                    width: frozen.width,
                    height: frozen.height,
                    preview: frozen.preview.clone(),
                };
                if !self
                    .process
                    .exchange::<_, bool>("--wallpaper-dialog", input, cancelled.clone())
                    .await?
                    .unwrap_or(false)
                    || *cancelled.borrow()
                {
                    return Ok(false);
                }
                let descriptor = zbus::zvariant::Fd::from(frozen.file());
                let current = tokio::time::timeout(Duration::from_secs(3), async {
                    zbus::fdo::DBusProxy::new(connection)
                        .await?
                        .get_name_owner(
                            "org.canopa.Shell.Wallpaper"
                                .try_into()
                                .expect("static wallpaper bus name"),
                        )
                        .await
                })
                .await??;
                anyhow::ensure!(
                    current.as_str() == owner.destination().as_str(),
                    "Wallpaper owner changed"
                );
                // A failed/ambiguous handoff is never retried against a replacement owner.
                tokio::time::timeout(
                    Duration::from_secs(45),
                    owner.call::<_, _, ()>("Apply", &(descriptor, request.destination.mask())),
                )
                .await??;
                Ok(true)
            }
            #[cfg(not(feature = "desktop"))]
            {
                let _ = (&self.process, request, connection, cancelled);
                anyhow::bail!("Wallpaper review requires the desktop feature")
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn value(v: impl Into<Value<'static>>) -> zbus::zvariant::OwnedValue {
        v.into().try_into_owned().unwrap()
    }

    #[test]
    fn destination_policy_and_preview_types_are_explicit() {
        let request = |options| {
            WallpaperRequest::parse(
                "app".into(),
                "wayland:parent".into(),
                "https://example.invalid/picture".into(),
                options,
            )
        };
        assert_eq!(
            request(Options::new()).unwrap().destination,
            Destination::Background
        );
        for (wire, expected) in [
            ("background", Destination::Background),
            ("lockscreen", Destination::Lockscreen),
            ("both", Destination::Both),
        ] {
            let parsed = request(Options::from([
                ("set-on".into(), value(wire)),
                ("show-preview".into(), value(false)),
            ]))
            .unwrap();
            assert_eq!(parsed.destination, expected);
        }
        for options in [
            Options::from([("set-on".into(), value("unknown"))]),
            Options::from([("set-on".into(), value(1u32))]),
            Options::from([("show-preview".into(), value("false"))]),
        ] {
            assert!(request(options).is_err());
        }
        assert!(
            WallpaperRequest::parse(
                "app".into(),
                "".into(),
                "x".repeat(16 * 1024 + 1),
                Options::new()
            )
            .is_err()
        );
        assert!(
            WallpaperRequest::parse(
                "app".into(),
                "\0".into(),
                "file:///image".into(),
                Options::new()
            )
            .is_err()
        );
    }
}
