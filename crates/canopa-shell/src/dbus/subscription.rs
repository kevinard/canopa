//! Iced messages from the shared recovering observers in `canopa-services`.
use canopa_services::ServiceState;
use iced::futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;

pub(super) use canopa_services::bus::while_service_owned;

/// Map [`canopa_services::bus::recovering`] states to messages: each reading
/// through `on_ok`, each outage once through `on_err`.
pub(super) fn build<T, M, Spawn, OnOk, OnErr>(
    spawn: Spawn,
    on_ok: OnOk,
    on_err: OnErr,
) -> Pin<Box<dyn Stream<Item = M> + Send + 'static>>
where
    T: Send + 'static,
    M: Send + 'static,
    Spawn: Fn(mpsc::UnboundedSender<Result<T, ()>>) + Send + 'static,
    OnOk: Fn(T) -> M + Send + 'static,
    OnErr: Fn() -> M + Send + 'static,
{
    Box::pin(
        canopa_services::bus::recovering(spawn).map(move |state| match state {
            ServiceState::Available(value) => on_ok(value),
            ServiceState::Unavailable => on_err(),
        }),
    )
}
