//! Grant/deny presentation. The XDG frontend owns permission and resource policy.
use crate::{
    choices::{self, Choice},
    requests::{Completion, Options, Requests, option},
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessRequest {
    pub app_id: String,
    pub parent_window: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub modal: bool,
    pub deny_label: String,
    pub grant_label: String,
    pub icon: Option<String>,
    pub choices: Vec<Choice>,
}

impl AccessRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        title: String,
        subtitle: String,
        body: String,
        mut options: Options,
    ) -> fdo::Result<Self> {
        Ok(Self {
            app_id,
            parent_window,
            title,
            subtitle,
            body,
            modal: option(&mut options, "modal")?.unwrap_or(true),
            deny_label: option(&mut options, "deny_label")?
                .filter(|s: &String| !s.is_empty())
                .unwrap_or_else(|| fl!("portal-access-deny")),
            grant_label: option(&mut options, "grant_label")?
                .filter(|s: &String| !s.is_empty())
                .unwrap_or_else(|| fl!("portal-access-allow")),
            icon: option(&mut options, "icon")?,
            choices: choices::parse(&mut options)?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grant {
    pub choices: Vec<(String, String)>,
}

pub trait Presenter: Send + Sync + 'static {
    fn present(
        &self,
        request: AccessRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>>;
}

pub use crate::process::DialogProcess as ProcessPresenter;
impl Presenter for ProcessPresenter {
    fn present(
        &self,
        request: AccessRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>> {
        Box::pin(self.exchange("--access-dialog", request, cancelled))
    }
}

pub struct Access {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}
impl Access {
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

#[zbus::interface(name = "org.freedesktop.impl.portal.Access")]
#[allow(clippy::too_many_arguments)] // Published wire arguments plus injected context.
impl Access {
    #[zbus(out_args("response", "results"))]
    async fn access_dialog(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        subtitle: String,
        body: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let request = AccessRequest::parse(app_id, parent_window, title, subtitle, body, options)?;
        let outcome = self
            .requests
            .run(connection, header, handle, |_, cancelled| {
                self.presenter.present(request.clone(), cancelled)
            })
            .await?;
        Ok(match outcome {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Ok(Some(grant)))
                if choices::valid(&request.choices, &grant.choices) =>
            {
                let value =
                    OwnedValue::try_from(Value::from(grant.choices)).expect("serialized choices");
                (0, Options::from([("choices".into(), value)]))
            }
            Completion::Finished(_) => (2, Options::new()),
        })
    }
}
