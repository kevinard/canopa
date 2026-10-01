//! Explicit adapter messages allow shell-owned protocol extensions alongside
//! the upstream layer-shell window operations.
use super::Message;
use iced::{Task, window::Id};
use iced_layershell::actions::{LayerShellCustomAction as Action, LayerShellCustomActionWithId};

impl TryInto<LayerShellCustomActionWithId> for Message {
    type Error = Self;

    fn try_into(self) -> Result<LayerShellCustomActionWithId, Self> {
        let (id, action) = match self {
            Self::LayerShell(action) => return Ok(action),
            Self::NewLayerShell { settings, id } => (None, Action::NewLayerShell { settings, id }),
            Self::SetInputRegion { id, callback } => (Some(id), Action::SetInputRegion(callback)),
            Self::SizeChange { id, size } => (Some(id), Action::SizeChange(size)),
            Self::KeyboardInteractivityChange {
                id,
                keyboard_interactivity,
            } => (
                Some(id),
                Action::KeyboardInteractivityChange(keyboard_interactivity),
            ),
            Self::RemoveWindow(id) => (Some(id), Action::RemoveWindow),
            message => return Err(message),
        };
        Ok(LayerShellCustomActionWithId(id, action))
    }
}

impl Message {
    pub(super) fn layershell_open(
        settings: iced_layershell::reexport::NewLayerShellSettings,
    ) -> (Id, Task<Self>) {
        let id = Id::unique();
        (id, Task::done(Self::NewLayerShell { settings, id }))
    }

    pub(super) fn popup_open(
        settings: iced_layershell::actions::IcedNewPopupSettings,
    ) -> (Id, Task<Self>) {
        let id = Id::unique();
        (
            id,
            Task::done(Self::LayerShell(LayerShellCustomActionWithId(
                None,
                Action::NewPopUp { settings, id },
            ))),
        )
    }
}
