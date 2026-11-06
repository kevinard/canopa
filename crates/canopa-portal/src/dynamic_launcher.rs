//! Launcher consent only. The frontend owns tokens and desktop-entry publication.
use crate::requests::{Completion, Options, Requests, Response, option};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

pub const NAME_BYTES: usize = 1024;
pub const ICON_BYTES: usize = 1024 * 1024;
const FIELD_BYTES: usize = 4096;
const TARGET_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LauncherType {
    Application,
    Webapp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherDraft {
    pub app_id: String,
    pub parent_window: String,
    pub name: String,
    pub launcher_type: LauncherType,
    pub target: String,
    pub modal: bool,
    pub editable_name: bool,
}

pub fn valid_name(name: &str) -> bool {
    name.len() <= NAME_BYTES && !name.trim().is_empty() && !name.chars().any(char::is_control)
}

fn invalid() -> fdo::Error {
    fdo::Error::InvalidArgs("Invalid launcher request".into())
}

impl LauncherDraft {
    pub fn parse(
        app_id: String,
        parent_window: String,
        name: String,
        mut options: Options,
    ) -> fdo::Result<Self> {
        if options.len() > 64 {
            return Err(invalid());
        }
        let launcher_type = match option::<u32>(&mut options, "launcher_type")?.unwrap_or(1) {
            1 => LauncherType::Application,
            2 => LauncherType::Webapp,
            _ => return Err(invalid()),
        };
        // Icon editing is optional. Preserve the approved original in its original
        // format, including when a caller asks for an editable icon.
        let _ = option::<bool>(&mut options, "editable_icon")?;
        let draft = Self {
            app_id,
            parent_window,
            name,
            launcher_type,
            target: option(&mut options, "target")?.unwrap_or_default(),
            modal: option(&mut options, "modal")?.unwrap_or(true),
            editable_name: option(&mut options, "editable_name")?.unwrap_or(true),
        };
        draft.validate()?;
        Ok(draft)
    }

    pub(crate) fn validate(&self) -> fdo::Result<()> {
        if self.app_id.len() > FIELD_BYTES
            || self.parent_window.len() > FIELD_BYTES
            || self.app_id.chars().any(char::is_control)
            || self.parent_window.chars().any(char::is_control)
            || self.name.len() > NAME_BYTES
            || self.name.chars().any(char::is_control)
            || (!self.editable_name && !valid_name(&self.name))
            || self.target.len() > TARGET_BYTES
            || self.target.chars().any(char::is_control)
            || (self.launcher_type == LauncherType::Webapp && self.target.is_empty())
            || (!self.target.is_empty() && url::Url::parse(&self.target).is_err())
        {
            return Err(invalid());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct LauncherIcon(Arc<Vec<u8>>);

impl LauncherIcon {
    /// Check the complete GBytesIcon shape before consuming its byte array.
    pub fn parse(value: OwnedValue) -> fdo::Result<Self> {
        if value.value_signature().to_string() != "(sv)" {
            return Err(invalid());
        }
        let (kind, payload) = <(String, OwnedValue)>::try_from(value).map_err(|_| invalid())?;
        let Value::Value(payload) = Value::from(payload) else {
            return Err(invalid());
        };
        if kind != "bytes" {
            return Err(invalid());
        }
        let Value::Array(array) = &*payload else {
            return Err(invalid());
        };
        if array.signature().to_string() != "ay" || array.is_empty() || array.len() > ICON_BYTES {
            return Err(invalid());
        }
        let bytes = Vec::<u8>::try_from(*payload).map_err(|_| invalid())?;
        Ok(Self(Arc::new(bytes)))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    fn response(&self) -> OwnedValue {
        let serialized = Value::from(("bytes", Value::from(self.bytes().to_vec())));
        // The vardict's implicit variant contains a value of type v, whose
        // contents are the serialized (sv). The frontend filters that exact type.
        Value::Value(Box::new(serialized))
            .try_into_owned()
            .expect("owned launcher icon")
    }
}

#[derive(Clone, Debug)]
pub struct LauncherRequest {
    pub draft: LauncherDraft,
    pub icon: LauncherIcon,
}

impl LauncherRequest {
    fn response(&self, name: String) -> Response {
        if !valid_name(&name) || (!self.draft.editable_name && name != self.draft.name) {
            return (2, Options::new());
        }
        (
            0,
            Options::from([
                (
                    "name".into(),
                    Value::from(name).try_into_owned().expect("owned name"),
                ),
                ("icon".into(), self.icon.response()),
            ]),
        )
    }
}

pub trait Presenter: Send + Sync + 'static {
    fn present(
        &self,
        request: LauncherRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>>;
}

pub use crate::process::DialogProcess as ProcessPresenter;
impl Presenter for ProcessPresenter {
    fn present(
        &self,
        request: LauncherRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        Box::pin(async move {
            #[cfg(feature = "desktop")]
            {
                let preview =
                    crate::dynamic_launcher_icon::prepare(request.icon, &cancelled).await?;
                self.exchange(
                    "--dynamic-launcher-dialog",
                    crate::dynamic_launcher_icon::DialogInput {
                        draft: request.draft,
                        preview,
                    },
                    cancelled,
                )
                .await
            }
            #[cfg(not(feature = "desktop"))]
            {
                let _ = (request, cancelled);
                anyhow::bail!("Launcher dialogs require the desktop feature")
            }
        })
    }
}

pub struct DynamicLauncher {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}

impl DynamicLauncher {
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

#[zbus::interface(name = "org.freedesktop.impl.portal.DynamicLauncher")]
impl DynamicLauncher {
    #[zbus(property)]
    fn supported_launcher_types(&self) -> u32 {
        1 | 2
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(out_args("response", "results"))]
    #[allow(clippy::too_many_arguments)]
    async fn prepare_install(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        name: String,
        icon_v: OwnedValue,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let draft = LauncherDraft::parse(app_id, parent_window, name, options)?;
        let completion = self
            .requests
            .run(connection, header, handle, |_, cancelled| async move {
                // Admission and owner tracking precede byte extraction and decoding.
                let request = LauncherRequest {
                    draft,
                    icon: LauncherIcon::parse(icon_v)?,
                };
                Ok(
                    match self.presenter.present(request.clone(), cancelled).await {
                        Ok(Some(name)) => request.response(name),
                        Ok(None) => (1, Options::new()),
                        Err(_) => (2, Options::new()),
                    },
                )
            })
            .await?;
        match completion {
            Completion::Cancelled => Ok((1, Options::new())),
            Completion::Finished(response) => response,
        }
    }

    #[zbus(out_args("response"))]
    fn request_install_token(&self, app_id: String, options: Options) -> fdo::Result<u32> {
        if app_id.len() > FIELD_BYTES || options.len() > 64 {
            return Err(invalid());
        }
        // No Canopa software-manager trust policy exists yet. Every resolved
        // identity must use interactive approval; no backend token store exists.
        Ok(2)
    }
}

#[cfg(test)]
mod tests;
