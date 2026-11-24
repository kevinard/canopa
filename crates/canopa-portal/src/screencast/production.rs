use super::*;
use crate::{
    process::DialogProcess,
    screencast_capture::{Owner, Recorder, Recording},
    screencast_status::{Indicator, Input as StatusInput},
};
use std::path::PathBuf;

pub(crate) mod catalog;
pub(crate) mod serial;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub source: Source,
    pub title: String,
    pub subtitle: String,
    #[serde(default)]
    pub app_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogInput {
    pub app_id: String,
    pub parent_window: String,
    pub modal: bool,
    pub multiple: bool,
    pub items: Vec<Item>,
    pub socket: PathBuf,
    pub compositor_pid: u32,
    #[serde(default)]
    pub compositor_owner: Option<String>,
    pub types: u32,
    pub activation_token: Option<String>,
}
pub(crate) async fn refresh(input: &DialogInput) -> anyhow::Result<Vec<Item>> {
    if input.types == 0 {
        return catalog::read(&input.socket, input.compositor_pid, input.types).await;
    }
    let connection = tokio::time::timeout(Duration::from_secs(5), Connection::session()).await??;
    refresh_from(input, &connection).await
}
async fn refresh_from(input: &DialogInput, connection: &Connection) -> anyhow::Result<Vec<Item>> {
    let owner = Owner::resolve(connection).await?;
    anyhow::ensure!(
        input.compositor_owner.as_deref() == Some(owner.unique_name()),
        "Capture compositor changed during review"
    );
    inventory(
        connection,
        Some(&owner),
        &input.socket,
        input.compositor_pid,
        input.types,
    )
    .await
}
pub(crate) async fn inventory(
    connection: &Connection,
    owner: Option<&Owner>,
    socket: &std::path::Path,
    pid: u32,
    types: u32,
) -> anyhow::Result<Vec<Item>> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut items = catalog::read(socket, pid, types).await?;
        if types != 0 {
            let owner = owner.ok_or_else(|| anyhow::anyhow!("Capture owner unavailable"))?;
            anyhow::ensure!(
                owner.process_id(connection).await? == pid,
                "Capture source owner changed"
            );
            let names = items
                .iter()
                .filter_map(|item| match &item.source {
                    Source::Monitor(monitor) => Some(monitor.name().to_owned()),
                    Source::Window(_) => None,
                })
                .collect();
            let mut monitors = owner.monitors(connection, names).await?.into_iter();
            for item in &mut items {
                if matches!(item.source, Source::Monitor(_)) {
                    item.source =
                        Source::Monitor(monitors.next().expect("one identity per monitor"));
                }
            }
        }
        Ok(items)
    })
    .await?
}
pub struct Production {
    process: Arc<DialogProcess>,
    recorder: Recorder,
    serials: serial::Serials,
    socket: Option<PathBuf>,
}
impl Production {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self::with_socket(process, std::env::var_os("NIRI_SOCKET").map(PathBuf::from))
    }
    pub(crate) fn with_socket(process: Arc<DialogProcess>, socket: Option<PathBuf>) -> Self {
        Self {
            recorder: Recorder::new(process.clone()),
            process,
            serials: serial::Serials::default(),
            socket,
        }
    }
    pub(crate) fn socket(&self) -> anyhow::Result<&PathBuf> {
        self.socket
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Capture source catalog unavailable"))
    }
    /// Call after fresh consent and revalidation against this owner's inventory.
    /// The returned producer owns no consent dialog or ongoing control surface.
    pub(crate) async fn start_approved(
        &self,
        connection: &Connection,
        owner: &Owner,
        caller: OwnedUniqueName,
        selected: Vec<Source>,
        cursor: Cursor,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Box<dyn Capture>> {
        let recording = self
            .recorder
            .start(
                connection,
                owner,
                caller,
                selected,
                cursor,
                cancelled.clone(),
            )
            .await?;
        let nodes = recording
            .streams()
            .iter()
            .map(|stream| stream.node)
            .collect();
        let identities = self.serials.resolve(nodes, cancelled.clone()).await?;
        anyhow::ensure!(
            !*recording.closed().borrow() && !*cancelled.borrow(),
            "Capture ended before ready"
        );
        let streams = recording
            .streams()
            .iter()
            .map(|stream| Stream {
                node: stream.node,
                serial: identities[&stream.node],
                source: stream.source.clone(),
                geometry: stream.geometry,
                mapping: stream.mapping.as_ref().map(|mapping| super::StreamMapping {
                    id: mapping.mapping_id.clone(),
                    size: mapping.size,
                }),
            })
            .collect();
        Ok(Box::new(Recorded { recording, streams }))
    }
}
struct Recorded {
    recording: Recording,
    streams: Vec<Stream>,
}
impl Capture for Recorded {
    fn input_bindings(&self) -> Option<&crate::screencast_capture::Bindings> {
        self.recording.bindings()
    }
    fn streams(&self) -> &[Stream] {
        &self.streams
    }
    fn closed(&self) -> watch::Receiver<bool> {
        self.recording.closed()
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move { self.recording.stop().await })
    }
}
struct Active {
    capture: Box<dyn Capture>,
    indicator: Indicator,
}
impl Capture for Active {
    fn input_bindings(&self) -> Option<&crate::screencast_capture::Bindings> {
        self.capture.input_bindings()
    }
    fn streams(&self) -> &[Stream] {
        self.capture.streams()
    }
    fn closed(&self) -> watch::Receiver<bool> {
        let source = self.capture.closed();
        if *source.borrow() {
            source
        } else {
            self.indicator.closed()
        }
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>> {
        Box::pin(async move {
            let Self { capture, indicator } = *self;
            let (capture, controls) = tokio::join!(capture.stop(), indicator.stop());
            capture?;
            controls
        })
    }
}
impl Presenter for Production {
    fn present<'a>(
        &'a self,
        connection: Connection,
        request: StartRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<Box<dyn Capture>>>> {
        Box::pin(async move {
            let owner = Owner::resolve(&connection).await?;
            let pid = owner.process_id(&connection).await?;
            let socket = self.socket()?;
            let items = inventory(
                &connection,
                Some(&owner),
                socket,
                pid,
                request.options.types,
            )
            .await?;
            anyhow::ensure!(!items.is_empty(), "No capture sources available");
            let input = DialogInput {
                app_id: request.app_id.clone(),
                parent_window: request.parent_window,
                modal: request.modal,
                multiple: request.options.multiple,
                items,
                socket: socket.clone(),
                compositor_pid: pid,
                compositor_owner: Some(owner.unique_name().to_owned()),
                types: request.options.types,
                activation_token: request.activation_token.clone(),
            };
            let selected: Option<Vec<Source>> = self
                .process
                .exchange_activated(
                    "--screencast-dialog",
                    input,
                    cancelled.clone(),
                    request.activation_token.as_deref(),
                )
                .await?;
            let Some(selected) = selected else {
                return Ok(None);
            };
            let unique: HashSet<_> = selected.iter().collect();
            anyhow::ensure!(
                !selected.is_empty()
                    && selected.len() <= 16
                    && unique.len() == selected.len()
                    && (request.options.multiple || selected.len() == 1),
                "Invalid capture selection"
            );
            let current = inventory(
                &connection,
                Some(&owner),
                socket,
                pid,
                request.options.types,
            )
            .await?;
            anyhow::ensure!(
                selected
                    .iter()
                    .all(|s| current.iter().any(|i| &i.source == s)),
                "Selected capture source disappeared"
            );
            if *cancelled.borrow() {
                return Ok(None);
            }
            let labels = selected
                .iter()
                .map(|source| {
                    let item = current
                        .iter()
                        .find(|item| &item.source == source)
                        .expect("validated source");
                    let kind = match source {
                        Source::Monitor(_) => "Display",
                        Source::Window(_) => "Window",
                    };
                    if item.title.trim().is_empty() {
                        kind.to_owned()
                    } else {
                        format!("{kind} · {}", item.title)
                    }
                })
                .collect();
            let controls = StatusInput::new(&request.app_id, labels)?;
            let capture = self
                .start_approved(
                    &connection,
                    &owner,
                    request.caller,
                    selected,
                    request.options.cursor,
                    cancelled.clone(),
                )
                .await?;
            let indicator = match Indicator::start(
                &self.process,
                controls,
                capture.closed(),
                cancelled.clone(),
            )
            .await
            {
                Ok(indicator) => indicator,
                Err(error) => {
                    if capture.stop().await.is_err() {
                        eprintln!(
                            "level=warn component=screencast_portal event=start_cleanup_unconfirmed"
                        );
                    }
                    return Err(error);
                }
            };
            anyhow::ensure!(
                !*indicator.closed().borrow()
                    && !*capture.closed().borrow()
                    && !*cancelled.borrow(),
                "Capture ended before controls ready"
            );
            Ok(Some(
                Box::new(Active { capture, indicator }) as Box<dyn Capture>
            ))
        })
    }
}
