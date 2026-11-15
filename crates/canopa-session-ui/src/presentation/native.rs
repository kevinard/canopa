//! Native handles retain the connection and SCTK role for Iced's entire lifetime.
use anyhow::Result;
use calloop::{
    LoopHandle, ping,
    timer::{TimeoutAction, Timer},
};
use iced_renderer::graphics::{Shell, shell::Notifier};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::{
    session_lock::{SessionLock, SessionLockSurface},
    shell::{WaylandSurface, xdg::window::Window},
};
use std::{
    any::Any,
    cell::Cell,
    ptr::NonNull,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::{wl_callback::WlCallback, wl_surface::WlSurface},
};

#[derive(Clone)]
pub struct Graphics {
    connection: Connection,
    ping: ping::Ping,
    frame: Arc<dyn Fn(&WlSurface) + Send + Sync>,
    deadline: Arc<Mutex<Option<Instant>>>,
    layout: Arc<AtomicU64>,
}
impl Graphics {
    /// Renderer notifications are coalesced into the existing native event loop.
    ///
    /// `repaint` must only mark the host's surfaces dirty. It runs for widget
    /// animations, timed redraws and failure retries, none of which change the
    /// presentation model, so it must not call `SessionView::changed()`:
    /// that would rebuild every output's scene on each wakeup.
    pub fn new<T>(
        connection: &Connection,
        qh: &QueueHandle<T>,
        handle: &LoopHandle<'static, T>,
        repaint: impl Fn(&mut T) + 'static,
    ) -> Result<Self>
    where
        T: Dispatch<WlCallback, WlSurface> + 'static,
    {
        let (ping, source) = ping::make_ping()?;
        let deadline = Arc::new(Mutex::new(None::<Instant>));
        let requested = deadline.clone();
        let scheduled = Rc::new(Cell::new(None));
        let timer_handle = handle.downgrade();
        let redraw = Rc::new(repaint);
        handle
            .insert_source(source, move |_, _, app| {
                let Some(timer_handle) = timer_handle.upgrade() else {
                    return;
                };
                if let Some(token) = scheduled.take() {
                    timer_handle.remove(token);
                }
                let Some(at) = *requested.lock().expect("graphics wakeup") else {
                    return;
                };
                if at <= Instant::now() {
                    *requested.lock().expect("graphics wakeup") = None;
                    redraw(app);
                } else {
                    let requested = requested.clone();
                    let scheduled_done = scheduled.clone();
                    let redraw = redraw.clone();
                    let token = timer_handle
                        .insert_source(Timer::from_deadline(at), move |_, _, app| {
                            scheduled_done.set(None);
                            *requested.lock().expect("graphics wakeup") = None;
                            redraw(app);
                            TimeoutAction::Drop
                        })
                        .expect("graphics retry timer");
                    scheduled.set(Some(token));
                }
            })
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let qh = qh.clone();
        Ok(Self {
            deadline,
            layout: Arc::new(AtomicU64::new(0)),
            frame: Arc::new(move |surface| {
                surface.frame(&qh, surface.clone());
            }),
            connection: connection.clone(),
            ping,
        })
    }
    pub(super) fn redraw_at(&self, at: Instant) {
        let mut deadline = self.deadline.lock().expect("graphics wakeup");
        if deadline.is_none_or(|old| at < old) {
            *deadline = Some(at);
            self.ping.ping();
        }
    }
    /// Bumped when the renderer asks for a relayout (for example after a font
    /// or image finishes loading); surfaces rebuild their widget tree once.
    pub(super) fn layout_generation(&self) -> u64 {
        self.layout.load(Ordering::Relaxed)
    }
    pub fn retry(&self) {
        self.redraw_at(Instant::now() + Duration::from_millis(250));
    }
    pub(super) fn shell(&self) -> Shell {
        Shell::new(self.clone())
    }
    pub(super) fn display(&self) -> NativeDisplay {
        NativeDisplay(self.connection.clone())
    }
    pub(super) fn request_frame(&self, window: &NativeWindow) {
        (self.frame)(&window.surface);
    }
    pub fn window(&self, window: &Window) -> NativeWindow {
        NativeWindow {
            connection: self.connection.clone(),
            surface: window.wl_surface().clone(),
            _role: Arc::new(window.clone()),
        }
    }
    pub fn lock_surface(&self, surface: &SessionLockSurface, lock: &SessionLock) -> NativeWindow {
        NativeWindow {
            connection: self.connection.clone(),
            surface: surface.wl_surface().clone(),
            _role: Arc::new((surface.clone(), lock.clone())),
        }
    }
}
impl Notifier for Graphics {
    fn request_redraw(&self) {
        self.redraw_at(Instant::now());
    }
    fn invalidate_layout(&self) {
        self.layout.fetch_add(1, Ordering::Relaxed);
        self.redraw_at(Instant::now());
    }
}

/// A compositor may retain its display indefinitely (softbuffer does). This
/// handle owns only the connection, never a particular native window or role.
#[derive(Clone)]
pub(super) struct NativeDisplay(Connection);
impl HasDisplayHandle for NativeDisplay {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        display_handle(&self.0)
    }
}
fn display_handle(connection: &Connection) -> Result<DisplayHandle<'_>, HandleError> {
    let pointer =
        NonNull::new(connection.backend().display_ptr().cast()).ok_or(HandleError::Unavailable)?;
    // SAFETY: the owned connection keeps wl_display alive for this borrow.
    Ok(unsafe {
        DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            pointer,
        )))
    })
}

/// Constructed only from owned SCTK roles, so a cloned proxy alone cannot outlive
/// the role and leave a renderer holding a destroyed wl_surface.
#[derive(Clone)]
pub struct NativeWindow {
    _role: Arc<dyn Any + Send + Sync>,
    surface: WlSurface,
    connection: Connection,
}
impl HasDisplayHandle for NativeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        display_handle(&self.connection)
    }
}
impl HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let pointer =
            NonNull::new(self.surface.id().as_ptr().cast()).ok_or(HandleError::Unavailable)?;
        // SAFETY: the retained SCTK role owns this wl_surface and destroys it only
        // after all clones (including the renderer's handle) have been dropped.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer)))
        })
    }
}
