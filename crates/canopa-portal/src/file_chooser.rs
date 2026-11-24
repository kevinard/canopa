//! FileChooser wire contract and request lifetime, independent of the renderer.
pub mod history;
pub use crate::process::DialogProcess as ProcessPresenter;
use crate::requests::{Completion, Requests, option};
pub use crate::requests::{Options, Response};
use futures_util::future::BoxFuture;
use std::{
    collections::HashMap, ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf, sync::Arc,
};
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

pub use canopa_file::picker::{ChooserRequest, Filter, Mode, Selection, file_name, path_bytes};

pub(crate) fn parse_request(
    mode: Mode,
    app_id: String,
    parent_window: String,
    title: String,
    mut options: Options,
) -> fdo::Result<ChooserRequest> {
    let filters: Vec<Filter> = option(&mut options, "filters")?.unwrap_or_default();
    let mut current_filter: Option<Filter> = option(&mut options, "current_filter")?;
    for filter in filters.iter().chain(current_filter.iter()) {
        if filter.1.is_empty()
            || filter
                .1
                .iter()
                .any(|(kind, value)| *kind > 1 || value.is_empty())
        {
            return Err(fdo::Error::InvalidArgs("Invalid filter".into()));
        }
    }
    if !filters.is_empty() && !current_filter.as_ref().is_some_and(|f| filters.contains(f)) {
        current_filter = filters.first().cloned();
    }
    let choices = crate::choices::parse(&mut options)?;
    let current_folder: Option<Vec<u8>> = option(&mut options, "current_folder")?;
    let current_file: Option<Vec<u8>> = option(&mut options, "current_file")?;
    if current_folder
        .iter()
        .chain(current_file.iter())
        .any(|p| !path_bytes(p).is_some_and(|p| p.is_absolute()))
    {
        return Err(fdo::Error::InvalidArgs("Invalid suggested path".into()));
    }
    let files: Vec<Vec<u8>> = option(&mut options, "files")?.unwrap_or_default();
    if mode == Mode::SaveMany
        && (files.is_empty()
            || files
                .iter()
                .any(|p| !path_bytes(p).is_some_and(|p| file_name(&p))))
    {
        return Err(fdo::Error::InvalidArgs("Invalid save file names".into()));
    }
    let current_name: Option<String> = option(&mut options, "current_name")?;
    if current_name
        .as_ref()
        .is_some_and(|name| name.contains('\0') || !file_name(std::path::Path::new(name)))
    {
        return Err(fdo::Error::InvalidArgs("Invalid suggested name".into()));
    }
    Ok(ChooserRequest {
        mode,
        app_id,
        parent_window,
        title,
        accept_label: option(&mut options, "accept_label")?.unwrap_or_else(|| {
            if mode == Mode::Open {
                fl!("portal-chooser-accept-open")
            } else {
                fl!("portal-chooser-accept-save")
            }
        }),
        modal: option(&mut options, "modal")?.unwrap_or(true),
        multiple: mode == Mode::Open && option(&mut options, "multiple")?.unwrap_or(false),
        directory: mode == Mode::SaveMany
            || (mode == Mode::Open && option(&mut options, "directory")?.unwrap_or(false)),
        filters,
        current_filter,
        choices,
        current_folder,
        current_file,
        current_name,
        files,
    })
}

trait SelectionResponse {
    fn folder(&self, request: &ChooserRequest) -> Option<PathBuf>;
    fn response(self, request: &ChooserRequest) -> Option<Response>;
}

impl SelectionResponse for Selection {
    fn folder(&self, request: &ChooserRequest) -> Option<PathBuf> {
        let paths: Vec<_> = self
            .paths
            .iter()
            .map(|bytes| PathBuf::from(OsString::from_vec(bytes.clone())))
            .collect();
        if request.mode == Mode::Open && request.directory && paths.len() == 1 {
            return paths.into_iter().next();
        }
        let parent = paths.first()?.parent()?.to_owned();
        paths
            .iter()
            .all(|p| p.parent() == Some(parent.as_path()))
            .then_some(parent)
    }
    fn response(self, request: &ChooserRequest) -> Option<Response> {
        let count = self.paths.len();
        if count == 0
            || (request.mode == Mode::Save && count != 1)
            || (request.mode == Mode::Open && !request.multiple && count != 1)
            || (request.mode == Mode::SaveMany && count != request.files.len())
        {
            return None;
        }
        if !crate::choices::valid(&request.choices, &self.choices) {
            return None;
        }
        if self.current_filter.as_ref().is_some_and(|f| {
            !request.filters.contains(f) && request.current_filter.as_ref() != Some(f)
        }) {
            return None;
        }
        let uris: Option<Vec<_>> = self
            .paths
            .into_iter()
            .map(|p| {
                if p.contains(&0) {
                    return None;
                }
                url::Url::from_file_path(PathBuf::from(OsString::from_vec(p)))
                    .ok()
                    .map(String::from)
            })
            .collect();
        let mut results = HashMap::from([(
            "uris".into(),
            OwnedValue::try_from(Value::from(uris?)).ok()?,
        )]);
        results.insert(
            "choices".into(),
            OwnedValue::try_from(Value::from(self.choices)).ok()?,
        );
        if let Some(filter) = self.current_filter {
            results.insert(
                "current_filter".into(),
                OwnedValue::try_from(Value::from(filter)).ok()?,
            );
        }
        if request.mode == Mode::Open {
            results.insert("writable".into(), OwnedValue::from(self.writable));
        }
        Some((0, results))
    }
}

/// Presentation completes only after user acceptance, dismissal or cancellation.
pub trait Presenter: Send + Sync {
    fn present(
        &self,
        request: ChooserRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Selection>>>;
}

pub struct FileChooser {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
    history: history::History,
}

impl FileChooser {
    pub fn new(presenter: Arc<dyn Presenter>) -> Self {
        Self::with_requests(presenter, Requests::default())
    }

    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self::with_history(presenter, requests, history::History::disabled())
    }

    pub fn with_history(
        presenter: Arc<dyn Presenter>,
        requests: Requests,
        history: history::History,
    ) -> Self {
        Self {
            presenter,
            requests,
            history,
        }
    }

    async fn choose(
        &self,
        connection: &Connection,
        header: Header<'_>,
        handle: OwnedObjectPath,
        request: ChooserRequest,
    ) -> fdo::Result<Response> {
        let outcome = self
            .requests
            .run(connection, header, handle, |_, cancelled| async {
                let mut presented = request.clone();
                if presented.current_folder.is_none()
                    && presented.current_file.is_none()
                    && let Some(folder) = self.history.load(presented.app_id.clone()).await
                {
                    let mut bytes = folder.into_os_string().into_vec();
                    bytes.push(0);
                    presented.current_folder = Some(bytes);
                }
                self.presenter.present(presented, cancelled).await
            })
            .await?;
        Ok(match outcome {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Ok(Some(selection))) => {
                let folder = selection.folder(&request);
                match selection.response(&request) {
                    Some(response) => {
                        if let Some(folder) = folder {
                            self.history.save(request.app_id.clone(), folder).await;
                        }
                        response
                    }
                    None => (2, Options::new()),
                }
            }
            Completion::Finished(Err(_)) => (2, Options::new()),
        })
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
#[allow(clippy::too_many_arguments)] // The published wire signature plus injected sender/connection.
impl FileChooser {
    #[zbus(out_args("response", "results"))]
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.choose(
            connection,
            header,
            handle,
            parse_request(Mode::Open, app_id, parent_window, title, options)?,
        )
        .await
    }
    #[zbus(out_args("response", "results"))]
    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.choose(
            connection,
            header,
            handle,
            parse_request(Mode::Save, app_id, parent_window, title, options)?,
        )
        .await
    }
    #[zbus(out_args("response", "results"))]
    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        self.choose(
            connection,
            header,
            handle,
            parse_request(Mode::SaveMany, app_id, parent_window, title, options)?,
        )
        .await
    }
}

/// The shipped Files helper lives beside the portal in both package layouts.
#[cfg(feature = "desktop")]
pub fn files_picker_executable() -> std::io::Result<PathBuf> {
    Ok(canopa_file::executable::installed()?.with_file_name("canopa-files"))
}

impl Presenter for ProcessPresenter {
    fn present(
        &self,
        request: ChooserRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Selection>>> {
        #[cfg(feature = "desktop")]
        {
            Box::pin(async move {
                ProcessPresenter {
                    executable: files_picker_executable()?,
                    arguments: self.arguments.clone(),
                }
                .exchange("--internal-picker", request, cancelled)
                .await
            })
        }
        #[cfg(not(feature = "desktop"))]
        {
            let _ = (request, cancelled);
            Box::pin(async { anyhow::bail!("FileChooser requires the desktop feature") })
        }
    }
}
