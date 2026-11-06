//! Application selection and live choice updates. The frontend owns launching.
use crate::requests::{Completion, Options, Requests, Response, option};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::watch;
use zbus::{fdo, message::Header, names::OwnedUniqueName, zvariant::OwnedObjectPath};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppRequest {
    pub app_id: String,
    pub parent_window: String,
    pub choices: Vec<String>,
    pub last_choice: Option<String>,
    pub modal: bool,
    pub content_type: Option<String>,
    pub uri: Option<String>,
    pub filename: Option<String>,
    pub activation_token: Option<String>,
}

fn choices(values: Vec<String>) -> fdo::Result<Vec<String>> {
    if values.len() > 4096 {
        return Err(fdo::Error::LimitsExceeded(
            "Too many application choices".into(),
        ));
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for value in values {
        if value.is_empty() || value.contains('\0') {
            return Err(fdo::Error::InvalidArgs("Invalid application ID".into()));
        }
        if seen.insert(value.clone()) {
            result.push(value);
        }
    }
    Ok(result)
}

impl AppRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        applications: Vec<String>,
        mut options: Options,
    ) -> fdo::Result<Self> {
        Ok(Self {
            app_id,
            parent_window,
            choices: choices(applications)?,
            last_choice: option(&mut options, "last_choice")?,
            modal: option(&mut options, "modal")?.unwrap_or(true),
            content_type: option(&mut options, "content_type")?,
            uri: option(&mut options, "uri")?,
            filename: option(&mut options, "filename")?,
            activation_token: option(&mut options, "activation_token")?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSelection {
    pub choice: String,
    /// A fresh token from the chooser's user interaction, when supported.
    pub activation_token: Option<String>,
}

pub trait Presenter: Send + Sync {
    fn present(
        &self,
        request: AppRequest,
        updates: watch::Receiver<Vec<String>>,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<AppSelection>>>;
}

pub use crate::process::DialogProcess as ProcessPresenter;

impl Presenter for ProcessPresenter {
    fn present(
        &self,
        mut request: AppRequest,
        mut updates: watch::Receiver<Vec<String>>,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<AppSelection>>> {
        Box::pin(async move {
            use crate::process::MAX_MESSAGE_BYTES;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            if *cancelled.borrow() {
                return Ok(None);
            }
            request.choices = updates.borrow_and_update().clone();
            let mut initial = serde_json::to_vec(&request)?;
            initial.push(b'\n');
            anyhow::ensure!(
                initial.len() <= MAX_MESSAGE_BYTES,
                "App chooser request too large"
            );
            let mut child = self.spawn("--app-chooser", request.activation_token.as_deref())?;
            let mut input = child.stdin.take().expect("piped stdin");
            let output = child.stdout.take().expect("piped stdout");
            let exchange = async {
                input.write_all(&initial).await?;
                let read = async {
                    let mut bytes = Vec::new();
                    output
                        .take(MAX_MESSAGE_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                        .await?;
                    anyhow::ensure!(
                        bytes.len() <= MAX_MESSAGE_BYTES,
                        "App chooser response too large"
                    );
                    Ok::<_, anyhow::Error>(bytes)
                };
                let write_updates = async {
                    while updates.changed().await.is_ok() {
                        let current = updates.borrow_and_update().clone();
                        let mut frame = serde_json::to_vec(&current)?;
                        frame.push(b'\n');
                        anyhow::ensure!(
                            frame.len() <= MAX_MESSAGE_BYTES,
                            "App chooser update too large"
                        );
                        input.write_all(&frame).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                };
                tokio::pin!(read);
                tokio::select! {
                    biased;
                    result = &mut read => result,
                    _ = write_updates => read.await,
                }
            };
            let bytes = tokio::select! {
                biased;
                _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => {
                    child.kill().await?;
                    return Ok(None);
                }
                result = exchange => result?,
            };
            anyhow::ensure!(child.wait().await?.success(), "App chooser stopped");
            if *cancelled.borrow() {
                return Ok(None);
            }
            Ok(serde_json::from_slice(&bytes)?)
        })
    }
}

struct Active {
    owner: OwnedUniqueName,
    choices: watch::Sender<Vec<String>>,
}
type Registry = Arc<Mutex<HashMap<OwnedObjectPath, Active>>>;

struct Lease {
    handle: OwnedObjectPath,
    registry: Registry,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.registry
            .lock()
            .expect("chooser registry")
            .remove(&self.handle);
    }
}

pub struct AppChooser {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
    registry: Registry,
}

impl AppChooser {
    pub fn new(presenter: Arc<dyn Presenter>) -> Self {
        Self::with_requests(presenter, Requests::default())
    }
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self {
            presenter,
            requests,
            registry: Arc::default(),
        }
    }

    async fn choose(
        &self,
        connection: &zbus::Connection,
        header: Header<'_>,
        handle: OwnedObjectPath,
        request: AppRequest,
    ) -> fdo::Result<Response> {
        let incoming_token = request.activation_token.clone();
        let response = self
            .requests
            .run(
                connection,
                header,
                handle.clone(),
                |owner, cancelled| async move {
                    let (current, updates) = watch::channel(request.choices.clone());
                    self.registry.lock().expect("chooser registry").insert(
                        handle.clone(),
                        Active {
                            owner,
                            choices: current.clone(),
                        },
                    );
                    let _lease = Lease {
                        handle,
                        registry: self.registry.clone(),
                    };
                    let Some(selected) =
                        self.presenter.present(request, updates, cancelled).await?
                    else {
                        return Ok(None);
                    };
                    // Serialize acceptance with UpdateChoices under the registry lock.
                    let registry = self.registry.lock().expect("chooser registry");
                    let current = registry
                        .get(&_lease.handle)
                        .expect("active chooser")
                        .choices
                        .borrow();
                    anyhow::ensure!(
                        !selected.choice.is_empty() && !selected.choice.contains('\0'),
                        "Invalid application choice"
                    );
                    anyhow::ensure!(
                        current.is_empty() || current.contains(&selected.choice),
                        "Application choice was removed"
                    );
                    Ok::<_, anyhow::Error>(Some(selected))
                },
            )
            .await?;
        Ok(match response {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Err(_)) => (2, Options::new()),
            Completion::Finished(Ok(Some(selection))) => {
                let mut results = Options::from([(
                    "choice".into(),
                    zbus::zvariant::Value::from(selection.choice)
                        .try_into()
                        .expect("application ID"),
                )]);
                if let Some(token) = selection
                    .activation_token
                    .filter(|token| !token.is_empty() && Some(token) != incoming_token.as_ref())
                {
                    results.insert(
                        "activation_token".into(),
                        zbus::zvariant::Value::from(token)
                            .try_into()
                            .expect("activation token"),
                    );
                }
                (0, results)
            }
        })
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.AppChooser")]
#[allow(clippy::too_many_arguments)] // Published wire signature and injected context.
impl AppChooser {
    #[zbus(out_args("response", "results"))]
    async fn choose_application(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        choices: Vec<String>,
        options: Options,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.choose(
            connection,
            header,
            handle,
            AppRequest::parse(app_id, parent_window, choices, options)?,
        )
        .await
    }

    fn update_choices(
        &self,
        handle: OwnedObjectPath,
        applications: Vec<String>,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let applications = choices(applications)?;
        let registry = self.registry.lock().expect("chooser registry");
        let active = registry
            .get(&handle)
            .ok_or_else(|| fdo::Error::InvalidArgs("Unknown chooser request".into()))?;
        if header.sender().map(|s| s.as_str()) != Some(active.owner.as_str()) {
            return Err(fdo::Error::AccessDenied(
                "Request belongs to another caller".into(),
            ));
        }
        active.choices.send_replace(applications);
        Ok(())
    }
}
