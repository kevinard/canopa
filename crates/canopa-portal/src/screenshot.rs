//! Screenshot workflow; the frontend owns sandbox identity and permissions.
use crate::requests::{Completion, Options, Requests, option};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Screen,
    Area,
    Color,
    Window,
    ActiveWindow,
}

#[derive(Clone, Debug)]
pub struct ScreenshotRequest {
    pub app_id: String,
    pub parent_window: String,
    pub modal: bool,
    pub kind: Kind,
    pub interactive: bool,
    pub permission_checked: bool,
    pub allow_area: bool,
}
impl ScreenshotRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        mut options: Options,
        color: bool,
    ) -> fdo::Result<Self> {
        if app_id.len() > 4096
            || parent_window.len() > 4096
            || app_id
                .chars()
                .chain(parent_window.chars())
                .any(char::is_control)
        {
            return Err(fdo::Error::InvalidArgs("Invalid screenshot context".into()));
        }
        let modal = option(&mut options, "modal")?.unwrap_or(true);
        let (kind, interactive, permission_checked, allow_area) = if color {
            (Kind::Color, true, false, false)
        } else {
            let target: Option<u32> = option(&mut options, "target")?;
            let kind = match target {
                None | Some(1) => Kind::Screen,
                Some(4) => Kind::Area,
                Some(2) => Kind::Window,
                Some(8) => Kind::ActiveWindow,
                _ => {
                    return Err(fdo::Error::InvalidArgs(
                        "Unsupported screenshot target".into(),
                    ));
                }
            };
            let interactive = option(&mut options, "interactive")?.unwrap_or(false);
            (
                kind,
                interactive,
                option(&mut options, "permission_store_checked")?.unwrap_or(false),
                target.is_none() && interactive,
            )
        };
        Ok(Self {
            app_id,
            parent_window,
            modal,
            kind,
            interactive,
            permission_checked,
            allow_area,
        })
    }
    fn target(&self) -> u32 {
        match self.kind {
            Kind::Screen => 1,
            Kind::Window => 2,
            Kind::Area => 4,
            Kind::ActiveWindow => 8,
            Kind::Color => 0,
        }
    }
    pub fn needs_review(&self) -> bool {
        self.kind != Kind::Screen || self.interactive || !self.permission_checked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value(v: impl Into<Value<'static>>) -> OwnedValue {
        v.into().try_into_owned().unwrap()
    }
    #[test]
    fn window_targets_keep_their_identity_and_always_require_review() {
        for (target, kind) in [(2, Kind::Window), (8, Kind::ActiveWindow)] {
            let request = ScreenshotRequest::parse(
                "app".into(),
                "".into(),
                Options::from([
                    ("target".into(), value(target)),
                    ("permission_store_checked".into(), value(true)),
                ]),
                false,
            )
            .unwrap();
            assert_eq!(request.kind, kind);
            assert!(request.needs_review());
            assert!(!request.allow_area);
            assert_eq!(request.target(), target);
        }
    }
    #[test]
    fn target_and_permission_hints_keep_noninteractive_capture_and_user_selection_distinct() {
        let parse = |options| {
            ScreenshotRequest::parse("app".into(), "wayland:parent".into(), options, false).unwrap()
        };
        assert!(parse(Options::new()).needs_review());
        assert!(
            !parse(Options::from([(
                "permission_store_checked".into(),
                value(true)
            )]))
            .needs_review()
        );
        let interactive = parse(Options::from([
            ("interactive".into(), value(true)),
            ("permission_store_checked".into(), value(true)),
        ]));
        assert!(interactive.needs_review() && interactive.allow_area);
        let screen = parse(Options::from([
            ("interactive".into(), value(true)),
            ("target".into(), value(1_u32)),
        ]));
        assert!(!screen.allow_area);
        let area = parse(Options::from([
            ("target".into(), value(4_u32)),
            ("permission_store_checked".into(), value(true)),
        ]));
        assert!(area.needs_review() && area.kind == Kind::Area);
        for target in [0, 3, 7, 15, u32::MAX] {
            assert!(
                ScreenshotRequest::parse(
                    "app".into(),
                    "".into(),
                    Options::from([("target".into(), value(target))]),
                    false
                )
                .is_err()
            );
        }
        for key in ["interactive", "modal", "permission_store_checked"] {
            assert!(
                ScreenshotRequest::parse(
                    "app".into(),
                    "".into(),
                    Options::from([(key.into(), value("true"))]),
                    false
                )
                .is_err()
            );
        }
    }
}

pub enum Outcome {
    Image(String),
    Color([f64; 3]),
}
pub trait Presenter: Send + Sync + 'static {
    fn available_targets<'a>(&'a self, _connection: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async { 1 | 4 })
    }
    fn present<'a>(
        &'a self,
        request: ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Outcome>>>;
}
#[derive(Clone)]
pub struct Screenshot {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}
impl Screenshot {
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self {
            presenter,
            requests,
        }
    }
    async fn perform(
        &self,
        handle: OwnedObjectPath,
        request: ScreenshotRequest,
        connection: &Connection,
        header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let kind = request.kind;
        let result = self
            .requests
            .run(connection, header, handle, |_, cancelled| async move {
                let target = request.target();
                if target != 0 && self.presenter.available_targets(connection).await & target == 0 {
                    return Err(
                        fdo::Error::InvalidArgs("Screenshot target unavailable".into()).into(),
                    );
                }
                self.presenter.present(request, cancelled).await
            })
            .await?;
        Ok(match result {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Ok(Some(Outcome::Image(uri))))
                if kind != Kind::Color
                    && url::Url::parse(&uri)
                        .is_ok_and(|url| url.scheme() == "file" && url.to_file_path().is_ok()) =>
            {
                (
                    0,
                    Options::from([(
                        "uri".into(),
                        Value::from(uri).try_into_owned().expect("URI value"),
                    )]),
                )
            }
            Completion::Finished(Ok(Some(Outcome::Color(rgb))))
                if kind == Kind::Color
                    && rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) =>
            {
                (
                    0,
                    Options::from([(
                        "color".into(),
                        OwnedValue::try_from(Value::from((rgb[0], rgb[1], rgb[2])))
                            .expect("RGB value"),
                    )]),
                )
            }
            Completion::Finished(Err(error)) if error.is::<fdo::Error>() => {
                return Err(error
                    .downcast::<fdo::Error>()
                    .expect("checked portal error"));
            }
            _ => (2, Options::new()),
        })
    }
}

pub struct Production {
    process: Arc<crate::process::DialogProcess>,
    #[cfg(feature = "desktop")]
    captures: crate::capture::Captures,
    #[cfg(feature = "desktop")]
    cache: crate::screenshot_image::Cache,
    #[cfg(feature = "desktop")]
    windows: crate::capture::window::Windows,
}
impl Production {
    pub fn new(process: Arc<crate::process::DialogProcess>) -> Self {
        #[cfg(feature = "desktop")]
        let captures = crate::capture::Captures::from_environment();
        Self {
            #[cfg(feature = "desktop")]
            windows: crate::capture::window::Windows::new(captures.clone(), process.clone()),
            process,
            #[cfg(feature = "desktop")]
            captures,
            #[cfg(feature = "desktop")]
            cache: crate::screenshot_image::Cache::discover(),
        }
    }
}
mod availability;
#[cfg(feature = "desktop")]
mod production;
pub use availability::AvailabilityWatch;

impl Presenter for Production {
    fn available_targets<'a>(&'a self, connection: &'a Connection) -> BoxFuture<'a, u32> {
        Box::pin(async move {
            #[cfg(feature = "desktop")]
            if self.windows.available(connection).await {
                return 15;
            }
            let _ = connection;
            1 | 4
        })
    }
    fn present<'a>(
        &'a self,
        request: ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Outcome>>> {
        Box::pin(async move {
            #[cfg(feature = "desktop")]
            {
                self.capture_request(request, cancelled).await
            }
            #[cfg(not(feature = "desktop"))]
            {
                let _ = (&self.process, request, cancelled);
                anyhow::bail!("Screenshot requires the desktop feature")
            }
        })
    }
}
#[zbus::interface(name = "org.freedesktop.impl.portal.Screenshot")]
impl Screenshot {
    #[allow(clippy::too_many_arguments)]
    #[zbus(out_args("response", "results"))]
    async fn screenshot(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.perform(
            handle,
            ScreenshotRequest::parse(app_id, parent_window, options, false)?,
            connection,
            header,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    #[zbus(name = "PickColor", out_args("response", "results"))]
    async fn pick_color(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.perform(
            handle,
            ScreenshotRequest::parse(app_id, parent_window, options, true)?,
            connection,
            header,
        )
        .await
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    async fn available_targets(&self, #[zbus(connection)] connection: &Connection) -> u32 {
        self.presenter.available_targets(connection).await
    }
}
