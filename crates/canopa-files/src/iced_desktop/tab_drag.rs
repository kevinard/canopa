//! Tabs dragged between windows as native drags.
//!
//! A tab's drag carries only its identity, in a type no other application
//! reads: every Files window accepts it, anywhere, and a drop moves the tab
//! there (onto the bar, between the tabs it falls between; elsewhere in
//! another window, after them). A drop nothing accepted, outside every Files
//! window, gives the tab a window of its own. Escape reaches the source window
//! during the drag, since only the pointer is grabbed; it gives the drag up
//! without that.
//!
//! The picture that follows the pointer is the tab itself, cut from a capture
//! of its window taken when the drag starts.
use super::*;
use std::collections::BTreeSet;

/// The drag type of a tab: the dragged tab's ID, meaningful in this process.
pub(super) const MIME: &str = "application/x-canopa-files-tab";

/// A press on a tab that may become its drag.
pub(in super::super) struct TabGesture {
    pub window: window::Id,
    pub tab: u64,
    /// The tab's bounds, to cut its picture from the window's capture.
    pub bounds: Rectangle,
    /// Where the pointer pressed it, from its corner: the picture's hotspot.
    pub grab: iced::Vector,
    pub native: Option<native::Origin>,
    pub requested: bool,
    pub serial: u64,
    /// `Some(None)`: the capture failed, and the drag starts without a picture.
    pub picture: Option<Option<native::Icon>>,
}

pub(in super::super) struct TabSource {
    pub source: native::Source,
    pub window: window::Id,
    pub tab: u64,
    /// Escape gave it up: its cancellation opens no window.
    pub abandoned: bool,
    /// Offers this drag was accepted on, and those it was dropped on.
    pub accepted: BTreeSet<u64>,
    pub dropped: bool,
}

/// A tab dragged over a window: where the pointer is, which tab, and whether
/// it comes from that window.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct TabHover {
    pub point: Point,
    pub tab: u64,
    pub own: bool,
}

/// `screenshot` cut to a tab's logical `bounds`, as a drag picture whose
/// hotspot is `grab` from its corner.
pub(super) fn picture(
    screenshot: &window::Screenshot,
    bounds: Rectangle,
    grab: iced::Vector,
) -> Option<native::Icon> {
    let scale = screenshot.scale_factor.max(1.0);
    let region = Rectangle {
        x: (bounds.x * scale).round().max(0.0) as u32,
        y: (bounds.y * scale).round().max(0.0) as u32,
        width: (bounds.width * scale).round() as u32,
        height: (bounds.height * scale).round() as u32,
    };
    if region.width == 0 || region.height == 0 {
        return None;
    }
    let cut = screenshot.crop(region).ok()?;
    // Whole buffer pixels per logical pixel; a fractional scale shows the
    // picture slightly smaller, never blurred.
    let buffer_scale = scale.floor().max(1.0) as u32;
    let mut rgba = cut.rgba.to_vec();
    // Slightly see-through, like dragged files.
    for alpha in rgba.iter_mut().skip(3).step_by(4) {
        *alpha = ((*alpha as u16 * 225) / 255) as u8;
    }
    let logical = |pixels: u32| pixels as f32 / buffer_scale as f32;
    Some(native::Icon {
        width: cut.size.width,
        height: cut.size.height,
        scale: buffer_scale,
        rgba: rgba.into(),
        hotspot: (
            grab.x.clamp(0.0, logical(cut.size.width)).round() as i32,
            grab.y.clamp(0.0, logical(cut.size.height)).round() as i32,
        ),
    })
}

impl State {
    pub fn tab_hover(&self, id: window::Id) -> Option<TabHover> {
        self.tab_hovers.get(&id).copied()
    }
    /// The tab being dragged away from window `id`. Only once its drag has
    /// started: its picture is captured from the window before.
    pub fn lifted_tab(&self, id: window::Id) -> Option<u64> {
        self.tab_source
            .as_ref()
            .filter(|source| source.window == id && !source.source.snapshot().finished)
            .map(|source| source.tab)
    }
    /// Whether window `id` starts its tab drags natively.
    pub fn native(&self, id: window::Id) -> bool {
        self.hosts
            .get(&id)
            .is_some_and(|host| host.endpoint.is_some())
    }

    pub(super) fn tab_event(&mut self, desktop: &Desktop, event: Event) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        match event {
            Event::TabPress(window, tab, bounds, grab) => {
                self.tab_gesture = None;
                if self.tab_source.is_some() || !self.native(window) {
                    return tasks;
                }
                self.serial += 1;
                self.tab_gesture = Some(TabGesture {
                    window,
                    tab,
                    bounds,
                    grab,
                    native: None,
                    requested: false,
                    serial: self.serial,
                    picture: None,
                });
            }
            Event::TabStart(window, tab) => {
                if let Some(gesture) = self
                    .tab_gesture
                    .as_mut()
                    .filter(|gesture| gesture.window == window && gesture.tab == tab)
                {
                    gesture.requested = true;
                    let (serial, bounds, grab) = (gesture.serial, gesture.bounds, gesture.grab);
                    tasks.push(window::screenshot(window).map(move |screenshot| {
                        Message::Drag(Event::TabPicture(
                            serial,
                            Arc::new(picture(&screenshot, bounds, grab)),
                        ))
                    }));
                }
            }
            Event::TabPicture(serial, icon) => {
                if let Some(gesture) = self
                    .tab_gesture
                    .as_mut()
                    .filter(|gesture| gesture.serial == serial)
                {
                    gesture.picture = Some((*icon).clone());
                }
            }
            _ => {}
        }
        let _ = desktop;
        tasks
    }

    /// Start a requested tab drag once its press origin and picture are in;
    /// follow tab offers over every window, move the tab where one is
    /// dropped, and give it a window when the drag ends unaccepted.
    pub(super) fn poll_tabs(
        &mut self,
        desktop: &Desktop,
        offers: Vec<(window::Id, native::Offer)>,
    ) -> Vec<Task<Message>> {
        let mut tasks = Vec::new();
        if self.tab_gesture.as_ref().is_some_and(|gesture| {
            gesture.requested && gesture.native.is_some() && gesture.picture.is_some()
        }) {
            let gesture = self.tab_gesture.take().unwrap();
            let still_there = desktop
                .windows
                .get(&gesture.window)
                .is_some_and(|window| window.tabs.iter().any(|tab| tab.id == gesture.tab));
            let endpoint = self
                .hosts
                .get(&gesture.window)
                .and_then(|host| host.endpoint.as_ref())
                .filter(|_| still_there);
            // A refusal means the press was released first, and the strip
            // moved the tab itself.
            if let Some(endpoint) = endpoint
                && let Ok(source) = endpoint.start_with_icon(
                    gesture.native.unwrap(),
                    vec![native::Content {
                        mime: MIME.into(),
                        bytes: gesture.tab.to_string().into_bytes().into(),
                    }],
                    native::Actions {
                        copy: false,
                        move_files: true,
                    },
                    gesture.picture.unwrap(),
                )
            {
                self.tab_source = Some(TabSource {
                    source,
                    window: gesture.window,
                    tab: gesture.tab,
                    abandoned: false,
                    accepted: BTreeSet::new(),
                    dropped: false,
                });
            }
        }
        self.tab_hovers.clear();
        let local = self
            .tab_source
            .as_ref()
            .map(|source| source.source.snapshot().id);
        for (owner, offer) in offers {
            let snap = offer.snapshot();
            if snap.finished {
                continue;
            }
            let Some(source) = self
                .tab_source
                .as_mut()
                .filter(|_| snap.local_source.is_some() && snap.local_source == local)
            else {
                // Another process's tab means nothing here.
                if snap.dropped {
                    let _ = offer.finish(false);
                } else {
                    let _ = offer.accept(None, native::Actions::default(), None);
                }
                self.rejected.insert(snap.id, owner);
                continue;
            };
            if source.accepted.insert(snap.id) {
                let _ = offer.accept(
                    Some(MIME.into()),
                    native::Actions {
                        copy: false,
                        move_files: true,
                    },
                    Some(native::Action::Move),
                );
            }
            let point = Point::new(snap.position.0 as f32, snap.position.1 as f32);
            if snap.dropped {
                if !source.dropped {
                    source.dropped = true;
                    let landing = desktop.windows.get(&owner).and_then(|window| {
                        let geometry = window.tab_geometry.lock().ok()?.clone();
                        geometry.landing(
                            point,
                            (owner == source.window).then_some(source.tab),
                            window.tabs.len(),
                        )
                    });
                    if let Some(landing) = landing {
                        tasks.push(Task::done(Message::TransferTab {
                            from: source.window,
                            tab: source.tab,
                            to: owner,
                            index: landing.index,
                        }));
                    }
                }
                let _ = offer.finish(true);
            } else if !snap.left {
                self.tab_hovers.insert(
                    owner,
                    TabHover {
                        point,
                        tab: source.tab,
                        own: owner == source.window,
                    },
                );
            }
        }
        if let Some(source) = &self.tab_source {
            let snap = source.source.snapshot();
            if snap.finished {
                let source = self.tab_source.take().unwrap();
                if snap.cancelled && !snap.dropped && !source.dropped && !source.abandoned {
                    tasks.push(Task::done(Message::DetachTab(source.window, source.tab)));
                }
            }
        }
        tasks
    }

    /// Escape gives a tab drag up. Whether one was going on.
    pub(super) fn cancel_tab_drag(&mut self) -> bool {
        let gesture = self
            .tab_gesture
            .take()
            .is_some_and(|gesture| gesture.requested);
        let source = self.tab_source.as_mut().is_some_and(|source| {
            let live = !source.source.snapshot().finished;
            if live {
                source.abandoned = true;
                source.source.cancel();
            }
            live
        });
        gesture || source
    }
}

#[cfg(test)]
mod tests {
    use super::super::tab_strip::{Landing, TabGeometry};
    use iced::{Point, Rectangle, Size};

    fn geometry(rtl: bool) -> TabGeometry {
        TabGeometry::fixture(
            Rectangle::new(Point::new(0.0, 40.0), Size::new(500.0, 30.0)),
            (0..3)
                .map(|index| {
                    Rectangle::new(
                        Point::new(index as f32 * 104.0, 41.0),
                        Size::new(100.0, 28.0),
                    )
                })
                .collect(),
            vec![10, 11, 12],
            rtl,
        )
    }

    #[test]
    fn a_tab_dropped_on_its_own_bar_moves_between_the_tabs_it_falls_between() {
        let bar = geometry(false);
        let on_bar = |x| Point::new(x, 55.0);
        // Over its own place, or the gaps beside it: it stays.
        assert_eq!(bar.landing(on_bar(10.0), Some(10), 3), None);
        assert_eq!(bar.landing(on_bar(120.0), Some(10), 3), None);
        assert_eq!(
            bar.landing(on_bar(170.0), Some(10), 3),
            Some(Landing { gap: 2, index: 1 })
        );
        assert_eq!(
            bar.landing(on_bar(400.0), Some(10), 3),
            Some(Landing { gap: 3, index: 2 })
        );
        assert_eq!(
            bar.landing(on_bar(10.0), Some(12), 3),
            Some(Landing { gap: 0, index: 0 })
        );
        // Off its own bar, a tab stays.
        assert_eq!(bar.landing(Point::new(170.0, 300.0), Some(10), 3), None);
    }

    #[test]
    fn a_tab_from_another_window_goes_where_dropped_or_after_the_tabs() {
        let bar = geometry(false);
        assert_eq!(
            bar.landing(Point::new(120.0, 55.0), None, 3),
            Some(Landing { gap: 1, index: 1 })
        );
        assert_eq!(
            bar.landing(Point::new(120.0, 300.0), None, 3),
            Some(Landing { gap: 3, index: 3 })
        );
        // A window that has not drawn its bar yet takes it after its tabs.
        assert_eq!(
            TabGeometry::default().landing(Point::new(5.0, 5.0), None, 1),
            Some(Landing { gap: 1, index: 1 })
        );
        // Right to left, the gaps count from the other end.
        let rtl = geometry(true);
        assert_eq!(
            rtl.landing(Point::new(10.0, 55.0), None, 3),
            Some(Landing { gap: 0, index: 3 })
        );
        assert_eq!(rtl.landing(Point::new(400.0, 55.0), Some(10), 3), None);
        assert_eq!(
            rtl.landing(Point::new(10.0, 55.0), Some(10), 3),
            Some(Landing { gap: 0, index: 2 })
        );
    }
}
