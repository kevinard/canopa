//! File URI drag/drop. Destination workers own every filesystem mutation.
use super::{clipboard, jobs, tab::Tab};
use crate::{
    Cancellation,
    browse::Entry,
    transfers::{DropPreference, TransferKind, TransferRequest},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    time::Duration,
};

const ACTIONS: gdk::DragAction = gdk::DragAction::COPY.union(gdk::DragAction::MOVE);
const URI_LIST: &str = "text/uri-list";

pub(super) fn cancel(app: &super::Desktop) -> bool {
    let source = app
        .active_drag
        .borrow_mut()
        .take()
        .and_then(|source| source.upgrade());
    if let Some(source) = source {
        if let Some(drag) = source.drag() {
            let mut cancelled = app.cancelled_drags.borrow_mut();
            cancelled.retain(|drag| drag.upgrade().is_some());
            cancelled.push(drag.downgrade());
        }
        source.drag_cancel();
        true
    } else {
        false
    }
}

pub(super) fn row(tab: &Weak<Tab>, row: &gtk::Box, item: &gtk::ListItem, current: bool) {
    let pressed = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    if current {
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = tab.clone();
        let snapshot = pressed.clone();
        click.connect_pressed(move |_, _, _, _| {
            if let Some(tab) = weak.upgrade() {
                *snapshot.borrow_mut() = tab.selection_paths();
            }
        });
        row.add_controller(click);
    }
    let source = gtk::DragSource::new();
    source.set_actions(ACTIONS);
    let weak = tab.clone();
    let weak_item = item.downgrade();
    source.connect_prepare(move |source, _, _| {
        let (tab, item) = (weak.upgrade()?, weak_item.upgrade()?);
        if let Some(app) = tab.window().and_then(|window| window.app.upgrade()) {
            app.drag_modifiers.set(source.current_event_state());
        }
        let object = item.item()?.downcast::<glib::BoxedAnyObject>().ok()?;
        let path = object.borrow::<Entry>().path.clone();
        let snapshot = pressed.borrow();
        let selected = tab.selection_paths();
        let paths = if current && snapshot.contains(&path) {
            snapshot.clone()
        } else if current && selected.contains(&path) {
            selected
        } else {
            vec![path]
        };
        if current {
            tab.select_for_drag(paths.clone());
        }
        let files: Vec<_> = paths.iter().map(gio::File::for_path).collect();
        let uris = files
            .iter()
            .map(|file| file.uri().to_string())
            .collect::<Vec<_>>()
            .join("\r\n")
            + "\r\n";
        Some(gdk::ContentProvider::new_union(&[
            gdk::ContentProvider::for_value(&gdk::FileList::from_array(&files).to_value()),
            gdk::ContentProvider::for_bytes(URI_LIST, &glib::Bytes::from_owned(uris.into_bytes())),
        ]))
    });
    let hold = Rc::new(RefCell::new(None::<gio::ApplicationHoldGuard>));
    let weak = tab.clone();
    let guard = hold.clone();
    source.connect_drag_begin(move |source, _| {
        if let Some(app) = weak
            .upgrade()
            .and_then(|tab| tab.window())
            .and_then(|window| window.app.upgrade())
        {
            *guard.borrow_mut() = Some(app.application.hold());
            *app.active_drag.borrow_mut() = Some(source.downgrade());
        }
        let icon = gio::ThemedIcon::new("text-x-generic-symbolic");
        if let Some(theme) = source
            .widget()
            .map(|widget| gtk::IconTheme::for_display(&widget.display()))
        {
            let paintable = theme.lookup_by_gicon(
                &icon,
                32,
                1,
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            );
            source.set_icon(Some(&paintable), 0, 0);
        }
    });
    let weak = tab.clone();
    source.connect_drag_end(move |source, _, _delete_data| {
        // File URIs are references. The receiving file manager performs its
        // transaction; never delete a source again on a generic drag-end signal.
        hold.borrow_mut().take();
        if let Some(app) = weak
            .upgrade()
            .and_then(|tab| tab.window())
            .and_then(|window| window.app.upgrade())
        {
            let matches = app
                .active_drag
                .borrow()
                .as_ref()
                .and_then(|source| source.upgrade())
                .as_ref()
                == Some(source);
            if matches {
                app.active_drag.borrow_mut().take();
            }
        }
    });
    row.add_controller(source);
    let weak_item = item.downgrade();
    if let Some(tab) = tab.upgrade() {
        destination(&tab, row.upcast_ref(), move || {
            let object = weak_item
                .upgrade()?
                .item()?
                .downcast::<glib::BoxedAnyObject>()
                .ok()?;
            let entry = object.borrow::<Entry>();
            entry.is_directory().then(|| entry.path.clone())
        });
    }
}

struct Hover {
    drop: gdk::Drop,
    destination: PathBuf,
    request: RefCell<Option<TransferRequest>>,
    error: RefCell<Option<String>>,
    cancellation: Cancellation,
    modifiers: Cell<gdk::ModifierType>,
    offered: Cell<gdk::DragAction>,
}
struct Target {
    tab: Weak<Tab>,
    widget: glib::WeakRef<gtk::Widget>,
    path: Box<dyn Fn() -> Option<PathBuf>>,
    hover: RefCell<Option<Rc<Hover>>>,
}

pub(super) fn destination(
    tab: &Rc<Tab>,
    widget: &gtk::Widget,
    path: impl Fn() -> Option<PathBuf> + 'static,
) {
    let state = Rc::new(Target {
        tab: Rc::downgrade(tab),
        widget: widget.downgrade(),
        path: Box::new(path),
        hover: RefCell::new(None),
    });
    let target = gtk::DropTargetAsync::new(Some(gdk::ContentFormats::new(&[URI_LIST])), ACTIONS);
    let active = state.clone();
    target.connect_accept(move |_, drop| {
        (active.path)().is_some()
            && drop.formats().contain_mime_type(URI_LIST)
            && drop.actions().intersects(ACTIONS)
    });
    let active = state.clone();
    target.connect_drag_enter(move |controller, drop, _, _| {
        active.motion(drop, controller.current_event_state())
    });
    let active = state.clone();
    target.connect_drag_motion(move |controller, drop, _, _| {
        active.motion(drop, controller.current_event_state())
    });
    let active = state.clone();
    target.connect_drag_leave(move |_, _| active.clear());
    let active = state;
    target.connect_drop(move |controller, drop, _, _| {
        let reject = || {
            active.clear();
            drop.finish(gdk::DragAction::empty());
            true
        };
        let hover = active.hover.borrow().clone();
        let Some(hover) = hover.filter(|hover| {
            hover.drop == *drop && Some(hover.destination.clone()) == (active.path)()
        }) else {
            return reject();
        };
        let request = hover.request.borrow().clone();
        let (Some(mut request), Some(tab)) = (request, active.tab.upgrade()) else {
            return reject();
        };
        let action = choose(
            &request,
            current_modifiers(&active.tab, drop, controller.current_event_state()),
            offered_actions(drop),
        );
        if action.is_empty() {
            return reject();
        }
        request.kind = if action == gdk::DragAction::MOVE {
            TransferKind::Move
        } else {
            TransferKind::Copy
        };
        let Some(app) = tab.window().and_then(|window| window.app.upgrade()) else {
            return reject();
        };
        if let Some(drag) = drop.drag()
            && app
                .cancelled_drags
                .borrow()
                .iter()
                .filter_map(glib::WeakRef::upgrade)
                .any(|cancelled| cancelled == drag)
        {
            return reject();
        }
        let accepted = jobs::dropped(&app, Rc::downgrade(&tab), request, drop.clone(), action);
        if !accepted {
            return reject();
        }
        active.clear();
        true
    });
    widget.add_controller(target);
}

impl Target {
    fn clear(&self) {
        if let Some(hover) = self.hover.borrow_mut().take() {
            hover.cancellation.cancel();
        }
        if let Some(widget) = self.widget.upgrade() {
            widget.remove_css_class("drop-target");
        }
        if let (Some(tab), Some(widget)) = (self.tab.upgrade(), self.widget.upgrade()) {
            tab.drop_hint(&widget, None, gdk::DragAction::empty());
        }
    }
    fn motion(self: &Rc<Self>, drop: &gdk::Drop, modifiers: gdk::ModifierType) -> gdk::DragAction {
        let modifiers = current_modifiers(&self.tab, drop, modifiers);
        let Some(path) = (self.path)() else {
            return gdk::DragAction::empty();
        };
        let existing = self.hover.borrow().clone();
        if let Some(hover) =
            existing.filter(|hover| hover.drop == *drop && hover.destination == path)
        {
            hover.modifiers.set(modifiers);
            return self.feedback(&hover);
        }
        self.clear();
        let hover = Rc::new(Hover {
            drop: drop.clone(),
            destination: path,
            request: RefCell::new(None),
            error: RefCell::new(None),
            cancellation: Cancellation::default(),
            modifiers: Cell::new(modifiers),
            offered: Cell::new(offered_actions(drop)),
        });
        *self.hover.borrow_mut() = Some(hover.clone());
        let weak = Rc::downgrade(self);
        let watched = hover.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(target) = weak
                .upgrade()
                .filter(|_| !watched.cancellation.is_cancelled())
            else {
                return glib::ControlFlow::Break;
            };
            let modifiers = current_modifiers(&target.tab, &watched.drop, watched.modifiers.get());
            if watched.modifiers.replace(modifiers) != modifiers
                || watched.offered.replace(offered_actions(&watched.drop))
                    != offered_actions(&watched.drop)
            {
                let action = target.feedback(&watched);
                watched.drop.status(
                    if action.is_empty() {
                        gdk::DragAction::empty()
                    } else {
                        ACTIONS
                    },
                    action,
                );
            }
            glib::ControlFlow::Continue
        });
        if let (Some(tab), Some(widget)) = (self.tab.upgrade(), self.widget.upgrade()) {
            tab.drop_hint(
                &widget,
                Some("Checking drop destination…"),
                gdk::DragAction::empty(),
            );
        }
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = glib::future_with_timeout(Duration::from_secs(5), async {
                let (stream, _) = hover
                    .drop
                    .read_future(&[URI_LIST], glib::Priority::DEFAULT)
                    .await
                    .map_err(|error| error.to_string())?;
                let mut bytes = Vec::new();
                loop {
                    if hover.cancellation.is_cancelled() {
                        return Err(String::from("Drag left this destination"));
                    }
                    let chunk = stream
                        .read_bytes_future(64 * 1024, glib::Priority::DEFAULT)
                        .await
                        .map_err(|error| error.to_string())?;
                    if chunk.is_empty() {
                        break;
                    }
                    if bytes.len().saturating_add(chunk.len()) > 16 * 1024 * 1024 {
                        return Err(String::from("The dragged file list is too large"));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| String::from("Invalid dragged file list"))?;
                let sources = text
                    .lines()
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .map(clipboard::local_path)
                    .collect::<Result<Vec<_>, _>>()?;
                let destination = hover.destination.clone();
                let cancellation = hover.cancellation.clone();
                gio::spawn_blocking(move || {
                    TransferRequest::from_drop(
                        sources,
                        destination,
                        DropPreference::Automatic,
                        &cancellation,
                    )
                })
                .await
                .map_err(|_| String::from("Drop destination check failed"))?
                .map_err(|error| error.to_string())
            })
            .await;
            if let Some(target) = weak
                .upgrade()
                .filter(|_| !hover.cancellation.is_cancelled())
            {
                match result {
                    Ok(Ok(request)) => *hover.request.borrow_mut() = Some(request),
                    Ok(Err(error)) => *hover.error.borrow_mut() = Some(error),
                    Err(_) => {
                        *hover.error.borrow_mut() =
                            Some(String::from("The drag source did not respond"))
                    }
                }
                let action = target.feedback(&hover);
                hover.drop.status(
                    if action.is_empty() {
                        gdk::DragAction::empty()
                    } else {
                        ACTIONS
                    },
                    action,
                );
            }
        });
        gdk::DragAction::empty()
    }
    fn feedback(&self, hover: &Hover) -> gdk::DragAction {
        let mut action = hover
            .request
            .borrow()
            .as_ref()
            .map_or(gdk::DragAction::empty(), |request| {
                choose(request, hover.modifiers.get(), offered_actions(&hover.drop))
            });
        if let (Some(tab), Some(widget)) = (self.tab.upgrade(), self.widget.upgrade()) {
            if let Some(error) = hover.error.borrow().as_ref() {
                action =
                    tab.drop_hint(&widget, Some(&format!("Cannot drop here: {error}")), action);
            } else if let Some(request) = hover.request.borrow().as_ref() {
                let label = if action == gdk::DragAction::MOVE {
                    "Move"
                } else if action == gdk::DragAction::COPY {
                    "Copy"
                } else {
                    "Cannot drop"
                };
                action = tab.drop_hint(
                    &widget,
                    Some(&format!(
                        "{label} {} item{} to {} · Ctrl: Copy · Shift: Move",
                        request.sources.len(),
                        if request.sources.len() == 1 { "" } else { "s" },
                        hover
                            .destination
                            .file_name()
                            .unwrap_or(hover.destination.as_os_str())
                            .to_string_lossy()
                    )),
                    action,
                );
            }
        }
        if let Some(widget) = self.widget.upgrade() {
            if action.is_empty() {
                widget.remove_css_class("drop-target");
            } else {
                widget.add_css_class("drop-target");
            }
        }
        action
    }
}

fn current_modifiers(
    tab: &Weak<Tab>,
    drop: &gdk::Drop,
    fallback: gdk::ModifierType,
) -> gdk::ModifierType {
    if let Some(drag) = drop.drag()
        && let Some(app) = tab
            .upgrade()
            .and_then(|tab| tab.window())
            .and_then(|window| window.app.upgrade())
    {
        let active = app
            .active_drag
            .borrow()
            .as_ref()
            .and_then(glib::WeakRef::upgrade)
            .and_then(|source| source.drag());
        if active.as_ref() == Some(&drag) {
            return app.drag_modifiers.get();
        }
    }
    drop.device()
        .seat()
        .keyboard()
        .map_or(fallback, |keyboard| keyboard.modifier_state())
}

fn choose(
    request: &TransferRequest,
    modifiers: gdk::ModifierType,
    offered: gdk::DragAction,
) -> gdk::DragAction {
    let preferred = if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        gdk::DragAction::COPY
    } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        || request.kind == TransferKind::Move
    {
        gdk::DragAction::MOVE
    } else {
        gdk::DragAction::COPY
    };
    if offered.contains(preferred) {
        preferred
    } else if !modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK)
        && offered.contains(gdk::DragAction::COPY)
    {
        gdk::DragAction::COPY
    } else {
        gdk::DragAction::empty()
    }
}

fn offered_actions(drop: &gdk::Drop) -> gdk::DragAction {
    drop.drag()
        .map_or_else(|| drop.actions(), |drag| drag.actions())
}
