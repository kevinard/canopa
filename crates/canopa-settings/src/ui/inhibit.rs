//! While a shortcut is recorded, ask the compositor to pass every key to
//! Settings, so combinations the desktop already uses reach the recorder
//! instead of acting. Compositors without the keyboard-shortcuts-inhibit
//! protocol keep their shortcuts; those can still be typed.
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::sync::Arc;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    backend::{Backend, ObjectId},
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat, wl_surface::WlSurface},
};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1 as Manager,
    zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1 as Inhibitor,
};

/// The compositor's shortcuts, held off until this is dropped.
pub(super) struct Inhibition {
    connection: Connection,
    inhibitor: Inhibitor,
    _queue: EventQueue<Events>,
}

impl Drop for Inhibition {
    fn drop(&mut self) {
        self.inhibitor.destroy();
        let _ = self.connection.flush();
    }
}

/// A held [`Inhibition`], as messages carry it.
#[derive(Clone)]
pub(super) struct Held {
    _inhibition: Arc<Inhibition>,
}

impl std::fmt::Debug for Held {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Held")
    }
}

/// Inhibit the compositor's shortcuts for `window`'s surface on the first
/// seat. `None` outside Wayland or when the compositor does not offer it.
pub(super) fn inhibit(window: &dyn iced::window::Window) -> Option<Held> {
    let RawDisplayHandle::Wayland(display) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    let RawWindowHandle::Wayland(surface) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: the display and surface pointers come from the live window,
    // which outlives this call; the new connection only borrows the display
    // and never closes it.
    let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
    let connection = Connection::from_backend(backend);
    let (globals, queue) = registry_queue_init::<Events>(&connection).ok()?;
    let handle: QueueHandle<Events> = queue.handle();
    let manager: Manager = globals.bind(&handle, 1..=1, ()).ok()?;
    let seat: WlSeat = globals.bind(&handle, 1..=1, ()).ok()?;
    // SAFETY: as above; the pointer is a live `wl_surface`.
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast()) }
        .ok()?;
    let surface = WlSurface::from_id(&connection, id).ok()?;
    let inhibitor = manager.inhibit_shortcuts(&surface, &seat, &handle, ());
    manager.destroy();
    connection.flush().ok()?;
    Some(Held {
        _inhibition: Arc::new(Inhibition {
            connection,
            inhibitor,
            _queue: queue,
        }),
    })
}

/// Events nobody needs: the inhibitor's active and inactive notices.
pub(super) struct Events;

impl Dispatch<WlRegistry, GlobalListContents> for Events {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Events {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Manager, ()> for Events {
    fn event(
        _: &mut Self,
        _: &Manager,
        _: <Manager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Inhibitor, ()> for Events {
    fn event(
        _: &mut Self,
        _: &Inhibitor,
        _: <Inhibitor as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
