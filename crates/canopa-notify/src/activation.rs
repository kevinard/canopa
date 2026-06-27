use std::future::Future;
use std::pin::Pin;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_token_v1;
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1;

pub type ActivationTokenFuture = Pin<Box<dyn Future<Output = Option<String>> + Send + 'static>>;

pub trait ActivationTokenProvider: Send + Sync {
    fn available(&self) -> bool;
    fn activation_token(&self, app_identity: String) -> ActivationTokenFuture;
}

#[derive(Clone, Debug, Default)]
pub struct NoopActivationTokenProvider;

impl ActivationTokenProvider for NoopActivationTokenProvider {
    fn available(&self) -> bool {
        false
    }

    fn activation_token(&self, _app_identity: String) -> ActivationTokenFuture {
        Box::pin(async { None })
    }
}

#[derive(Clone, Debug)]
pub struct WaylandActivationTokenProvider {
    available: bool,
}

impl WaylandActivationTokenProvider {
    pub fn connect_to_env() -> Self {
        Self {
            available: wayland_activation_available(),
        }
    }
}

impl ActivationTokenProvider for WaylandActivationTokenProvider {
    fn available(&self) -> bool {
        self.available
    }

    fn activation_token(&self, app_identity: String) -> ActivationTokenFuture {
        if !self.available {
            return Box::pin(async { None });
        }

        Box::pin(async move {
            tokio::task::spawn_blocking(move || wayland_activation_token(app_identity))
                .await
                .ok()
                .flatten()
        })
    }
}

#[derive(Default)]
struct WaylandActivationState {
    token: Option<String>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandActivationState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_activation_v1::XdgActivationV1, ()> for WaylandActivationState {
    fn event(
        _state: &mut Self,
        _proxy: &xdg_activation_v1::XdgActivationV1,
        _event: xdg_activation_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for WaylandActivationState {
    fn event(
        state: &mut Self,
        _proxy: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            state.token = Some(token);
        }
    }
}

fn wayland_activation_available() -> bool {
    let Ok(connection) = Connection::connect_to_env() else {
        return false;
    };
    let Ok((globals, queue)) = registry_queue_init::<WaylandActivationState>(&connection) else {
        return false;
    };
    globals
        .bind::<xdg_activation_v1::XdgActivationV1, _, _>(&queue.handle(), 1..=1, ())
        .is_ok()
}

fn wayland_activation_token(app_identity: String) -> Option<String> {
    let connection = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<WaylandActivationState>(&connection).ok()?;
    let qh = queue.handle();
    let activation = globals
        .bind::<xdg_activation_v1::XdgActivationV1, _, _>(&qh, 1..=1, ())
        .ok()?;
    let token_request = activation.get_activation_token(&qh, ());
    token_request.set_app_id(app_identity);
    token_request.commit();
    connection.flush().ok()?;

    let mut state = WaylandActivationState::default();
    while state.token.is_none() {
        queue.blocking_dispatch(&mut state).ok()?;
    }
    token_request.destroy();
    state.token
}
