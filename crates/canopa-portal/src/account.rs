//! Explicit, request-scoped sharing of editable account information.
use crate::requests::{Completion, Options, Requests, Response, option};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{os::unix::ffi::OsStrExt, sync::Arc};
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

pub const FIELD_BYTES: usize = 4096;
const URI_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountRequest {
    pub app_id: String,
    pub parent_window: String,
    pub reason: String,
}

impl AccountRequest {
    pub fn parse(app_id: String, parent_window: String, mut options: Options) -> fdo::Result<Self> {
        if options.len() > 64 || app_id.len() > FIELD_BYTES || parent_window.len() > FIELD_BYTES {
            return Err(fdo::Error::InvalidArgs("Account request too large".into()));
        }
        let reason: String = option(&mut options, "reason")?.unwrap_or_default();
        if reason.chars().take(257).count() > 256 {
            return Err(fdo::Error::InvalidArgs("Account reason too long".into()));
        }
        Ok(Self {
            app_id,
            parent_window,
            reason,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserInformation {
    pub id: String,
    pub name: String,
    /// An accepted local avatar URI, or no shared avatar.
    pub image: Option<String>,
}

impl UserInformation {
    fn response(self) -> Option<Response> {
        if [&self.id, &self.name]
            .into_iter()
            .any(|value| value.len() > FIELD_BYTES || value.contains('\0'))
        {
            return None;
        }
        if let Some(image) = &self.image {
            if image.len() > URI_BYTES || image.is_empty() {
                return None;
            }
            let uri = url::Url::parse(image).ok()?;
            let path = uri.to_file_path().ok()?;
            if uri.scheme() != "file"
                || uri.host_str().is_some_and(|host| host != "localhost")
                || uri.query().is_some()
                || uri.fragment().is_some()
                || !path.is_absolute()
                || path.as_os_str().as_bytes().contains(&0)
            {
                return None;
            }
        }
        // Keep the complete standard result shape, including an empty string
        // when the user chooses not to share an avatar.
        let values = [
            ("id", self.id),
            ("name", self.name),
            ("image", self.image.unwrap_or_default()),
        ];
        let results = values
            .into_iter()
            .map(|(key, value)| {
                (
                    key.into(),
                    OwnedValue::try_from(Value::from(value)).expect("owned string"),
                )
            })
            .collect();
        Some((0, results))
    }
}

pub trait Presenter: Send + Sync + 'static {
    fn present(
        &self,
        request: AccountRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserInformation>>>;
}

pub use crate::process::DialogProcess as ProcessPresenter;
impl Presenter for ProcessPresenter {
    fn present(
        &self,
        request: AccountRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserInformation>>> {
        Box::pin(self.exchange("--account-dialog", request, cancelled))
    }
}

pub struct Account {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}

impl Account {
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

#[zbus::interface(name = "org.freedesktop.impl.portal.Account")]
impl Account {
    #[zbus(out_args("response", "results"))]
    async fn get_user_information(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        window: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let request = AccountRequest::parse(app_id, window, options)?;
        let completion = self
            .requests
            .run(connection, header, handle, |_, cancelled| {
                self.presenter.present(request, cancelled)
            })
            .await?;
        Ok(match completion {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Ok(Some(information))) => information
                .response()
                .unwrap_or_else(|| (2, Options::new())),
            Completion::Finished(Err(_)) => (2, Options::new()),
        })
    }
}
