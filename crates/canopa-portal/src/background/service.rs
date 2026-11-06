//! The frontend owns permission storage, termination and modern autostart.
use super::{Observation, Unavailable, process};
use crate::{
    OBJECT_PATH,
    requests::{Completion, Options, Requests},
};
use futures_util::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::watch;
use zbus::{
    Connection, fdo, message::Header, object_server::SignalEmitter, zvariant::OwnedObjectPath,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackgroundRequest {
    pub app_id: String,
    pub name: String,
}
impl BackgroundRequest {
    pub fn parse(app_id: String, name: String) -> fdo::Result<Self> {
        if !process::app_id(&app_id) || name.len() > 4096 || name.contains('\0') {
            return Err(fdo::Error::InvalidArgs(
                "Invalid background application".into(),
            ));
        }
        Ok(Self {
            name: if name.is_empty() {
                app_id.clone()
            } else {
                name
            },
            app_id,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u32)]
pub enum Decision {
    Forbid = 0,
    Allow = 1,
    AllowOnce = 2,
}

pub trait Presenter: Send + Sync + 'static {
    fn present(
        &self,
        request: BackgroundRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Decision>>;
}

pub struct Background {
    presenter: Arc<dyn Presenter>,
    state: watch::Receiver<Observation>,
    requests: Requests,
}
impl Background {
    pub fn with_requests(
        presenter: Arc<dyn Presenter>,
        state: watch::Receiver<Observation>,
        requests: Requests,
    ) -> Self {
        Self {
            presenter,
            state,
            requests,
        }
    }

    pub fn changes(&self) -> watch::Receiver<Observation> {
        self.state.clone()
    }

    /// Called after registering the interface. Intermediate worker updates do
    /// not announce a successful new observation; stable health changes do.
    pub async fn relay_changes(
        mut state: watch::Receiver<Observation>,
        connection: Connection,
    ) -> zbus::Result<()> {
        let emitter = SignalEmitter::new(&connection, OBJECT_PATH)?;
        let mut previous = state.borrow_and_update().result.clone();
        while state.changed().await.is_ok() {
            let current = state.borrow_and_update().result.clone();
            if current == Err(Unavailable::Updating) || current == previous {
                continue;
            }
            previous = current;
            Self::running_applications_changed(&emitter).await?;
        }
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Background")]
impl Background {
    #[zbus(out_args("apps"))]
    fn get_app_state(&self) -> fdo::Result<Options> {
        let apps = self
            .state
            .borrow()
            .apps()
            .map_err(|reason| fdo::Error::Failed(reason.to_string()))?;
        Ok(apps
            .iter()
            .map(|(id, activity)| (id.clone(), (*activity as u32).into()))
            .collect())
    }

    #[zbus(out_args("response", "results"))]
    async fn notify_background(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        name: String,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        let request = BackgroundRequest::parse(app_id, name)?;
        let completion = self
            .requests
            .run_background(connection, header, handle, |_, cancelled| {
                self.presenter.present(request, cancelled)
            })
            .await?;
        let (response, decision) = match completion {
            Completion::Finished(Ok(decision)) => (0, decision),
            Completion::Cancelled | Completion::Finished(Err(_)) => (2, Decision::AllowOnce),
        };
        Ok((
            response,
            Options::from([("result".into(), (decision as u32).into())]),
        ))
    }

    /// Modern frontends own autostart files. Do not silently write a competing
    /// store for a deprecated method used only by older frontend versions.
    #[zbus(out_args("result"))]
    fn enable_autostart(
        &self,
        app_id: String,
        enable: bool,
        commandline: Vec<String>,
        flags: u32,
    ) -> fdo::Result<bool> {
        let _ = (app_id, enable, commandline, flags);
        Err(fdo::Error::NotSupported(
            "Autostart requires a frontend that manages it directly".into(),
        ))
    }

    #[zbus(signal)]
    async fn running_applications_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
