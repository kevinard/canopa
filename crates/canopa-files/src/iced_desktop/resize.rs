//! Dragged column widths: the dividers between Columns view columns and
//! between List view headings. A drag only repaints until it ends; the chosen
//! width is then saved to the configuration in the background, like a
//! Preferences change, so every window and the Preferences form follow it.
use super::*;
use crate::{interface::ListColumns, preferences::Detail};
use iced::advanced::mouse::{self as core_mouse};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, renderer,
    widget::{Tree, tree},
};
use iced::{Color, Event as WidgetEvent, Length, Rectangle, Renderer, Size, Theme, mouse};

/// Pointer distance on each side of a divider that still grabs it.
const SLOP: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// Every Columns view column shares one width.
    Columns,
    Name,
    Detail(Detail),
    /// The sidebar, from the divider at its end.
    Sidebar,
    /// The Properties side panel, from the divider at its start.
    Properties,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    /// A divider was grabbed; the column was this wide.
    Begin(window::Id, Target, f32),
    /// The pointer moved this far from where it grabbed the divider.
    Move(window::Id, f32),
    End(window::Id),
    /// Double-click: back to the width that follows the window.
    Reset(window::Id, Target),
    Saved(u64, Option<Arc<configuration_ui::Reply>>),
}

pub(super) struct Resizing {
    window: window::Id,
    target: Target,
    start: f32,
    width: f32,
    /// How far the column may grow: the room after the last List column when
    /// the drag began, so the divider follows the pointer up to the list's
    /// end. `None`: only the column's own limits apply.
    room: Option<f32>,
    /// Name's width while a detail is dragged: it keeps it, so only the
    /// dragged column changes and every divider after it moves with it.
    name: Option<f32>,
}

/// The widths to save once no other configuration change is in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Wanted {
    window: window::Id,
    column_width: u16,
    list: ListColumns,
    panes: crate::interface::PaneWidths,
    tree: bool,
    folders_first: bool,
}

#[derive(Default)]
pub(super) struct Layouts {
    resizing: Option<Resizing>,
    wanted: Option<Wanted>,
    saving: Option<u64>,
}

impl Layouts {
    /// The width being dragged for `target` in `window`, if any.
    pub fn dragged(&self, window: window::Id, target: Target) -> Option<f32> {
        let resizing = self
            .resizing
            .as_ref()
            .filter(|resizing| resizing.window == window)?;
        if resizing.target == target {
            Some(resizing.width)
        } else if target == Target::Name {
            resizing.name
        } else {
            None
        }
    }
}

fn clamp(target: Target, width: f32) -> f32 {
    let range = match target {
        Target::Columns => 160..=640,
        Target::Name => ListColumns::NAME,
        Target::Detail(_) => ListColumns::DETAIL,
        Target::Sidebar => crate::interface::PaneWidths::SIDEBAR,
        Target::Properties => crate::interface::PaneWidths::PROPERTIES,
    };
    width.clamp(f32::from(*range.start()), f32::from(*range.end()))
}

impl Desktop {
    /// Whether column dividers in `window` take the pointer: not while a menu
    /// or View options lies over the folder, which they would otherwise
    /// reach through.
    pub(super) fn dividers_take_pointer(&self, window: window::Id) -> bool {
        self.action_menu
            .as_ref()
            .is_none_or(|menu| menu.window() != window)
            && self
                .folder_options
                .as_ref()
                .is_none_or(|target| target.window != window)
    }

    /// The sidebar's width in `window`, following a drag.
    pub(super) fn sidebar_width(&self, window: window::Id) -> f32 {
        self.layouts
            .dragged(window, Target::Sidebar)
            .or_else(|| self.configuration.settings.panes.sidebar.map(f32::from))
            .unwrap_or(chrome::SIDEBAR_WIDTH)
    }

    /// The Properties side panel's width in `window`, following a drag.
    pub(super) fn properties_width(&self, window: window::Id) -> f32 {
        self.layouts
            .dragged(window, Target::Properties)
            .or_else(|| self.configuration.settings.panes.properties.map(f32::from))
            .unwrap_or(super::properties_ui::PANEL_WIDTH)
    }

    /// The Columns view column width for `window`, following a drag.
    pub(super) fn column_width(&self, window: window::Id, settled: u16) -> f32 {
        self.layouts
            .dragged(window, Target::Columns)
            .unwrap_or(f32::from(settled))
    }

    pub(super) fn resize_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Begin(window, target, start) => {
                // A List column grows only into the room after the last one.
                let room = matches!(target, Target::Name | Target::Detail(_))
                    .then(|| {
                        let tab = self.active(window)?;
                        let browser = tab.browser.as_ref()?;
                        super::listing_ui::list_room(
                            self,
                            window,
                            browser.view(),
                            tab.viewport_width,
                        )
                    })
                    .flatten();
                let name = matches!(target, Target::Detail(_))
                    .then(|| {
                        let tab = self.active(window)?;
                        let browser = tab.browser.as_ref()?;
                        Some(super::listing_ui::name_width(
                            self,
                            window,
                            browser.view(),
                            tab.viewport_width,
                        ))
                    })
                    .flatten();
                self.layouts.resizing = Some(Resizing {
                    window,
                    target,
                    start,
                    width: clamp(target, start),
                    room,
                    name,
                });
                Task::none()
            }
            Event::Move(window, delta) => {
                if let Some(resizing) = &mut self.layouts.resizing
                    && resizing.window == window
                {
                    let mut width = clamp(resizing.target, resizing.start + delta);
                    if let Some(room) = resizing.room {
                        width = width.min(resizing.start.max(resizing.start + room));
                    }
                    resizing.width = width.round();
                }
                Task::none()
            }
            Event::End(window) => {
                let Some(resizing) = self
                    .layouts
                    .resizing
                    .take_if(|resizing| resizing.window == window)
                else {
                    return Task::none();
                };
                if (resizing.width - resizing.start).abs() < 1.0 {
                    return Task::none();
                }
                self.choose_width(window, resizing.target, Some(resizing.width as u16))
            }
            Event::Reset(window, target) => {
                self.layouts.resizing = None;
                self.choose_width(window, target, None)
            }
            Event::Saved(request, result) => self.layout_saved(request, result),
        }
    }

    /// Apply a chosen width at once, then save it. `None` restores the default.
    fn choose_width(
        &mut self,
        window: window::Id,
        target: Target,
        width: Option<u16>,
    ) -> Task<Message> {
        let mut tasks = Vec::new();
        let settings = &mut self.configuration.settings;
        match target {
            Target::Columns => {
                let width =
                    width.unwrap_or(crate::preferences::ViewSettings::default().column_width);
                settings.view.column_width = width;
                let defaults = settings.view.clone();
                let _ = self.preferences.set_defaults(defaults);
                // A folder's own width would hide the one just chosen.
                if let Some(folder) = self
                    .active(window)
                    .and_then(|tab| tab.browser.as_ref())
                    .map(|browser| browser.path().to_owned())
                    && self.preferences.forget_column_width(&folder)
                {
                    tasks.push(self.save_folder_views(folder));
                }
                tasks.push(self.publish_folder_views(None));
            }
            Target::Name => settings.list_columns.name = width,
            Target::Detail(detail) => *settings.list_columns.detail_mut(detail) = width,
            Target::Sidebar => settings.panes.sidebar = width,
            Target::Properties => settings.panes.properties = width,
        }
        // A wider or narrower pane changes how many columns fit beside it.
        if matches!(target, Target::Sidebar | Target::Properties) {
            tasks.push(self.load_columns(window));
        }
        tasks.push(self.want_layout_saved(window));
        Task::batch(tasks)
    }

    /// Save the current widths and every-folder view choices, after the
    /// configuration change in flight, if any.
    fn want_layout_saved(&mut self, window: window::Id) -> Task<Message> {
        let settings = &self.configuration.settings;
        self.layouts.wanted = Some(Wanted {
            window,
            column_width: settings.view.column_width,
            list: settings.list_columns,
            panes: settings.panes,
            tree: settings.view.tree,
            folders_first: settings.view.folders_first,
        });
        self.sync_layout_save()
    }

    /// Change a view choice that every folder shares, such as List view's tree
    /// or folders sorting first: at once in every window, then saved.
    pub(super) fn change_shared_view(
        &mut self,
        window: window::Id,
        change: impl FnOnce(&mut crate::preferences::ViewSettings),
    ) -> Task<Message> {
        let view = &mut self.configuration.settings.view;
        let before = view.clone();
        change(view);
        if *view == before {
            return Task::none();
        }
        let defaults = view.clone();
        let _ = self.preferences.set_defaults(defaults);
        let publish = self.publish_folder_views(None);
        Task::batch([publish, self.want_layout_saved(window)])
    }

    /// Start saving chosen widths when no other configuration change runs.
    pub(super) fn sync_layout_save(&mut self) -> Task<Message> {
        if self.layouts.saving.is_some() || self.configuration_pending.is_some() {
            return Task::none();
        }
        let Some(worker) = &self.configuration_worker else {
            return Task::none();
        };
        let Some(wanted) = self.layouts.wanted.take() else {
            return Task::none();
        };
        let mut settings = self.configuration.settings.clone();
        settings.view.column_width = wanted.column_width;
        settings.view.tree = wanted.tree;
        settings.view.folders_first = wanted.folders_first;
        settings.list_columns = wanted.list;
        settings.panes = wanted.panes;
        let receiver = match worker.save_settings(self.configuration.clone(), settings) {
            Ok(receiver) => receiver,
            Err(error) => {
                self.configuration_error = Some(error.to_string());
                return Task::none();
            }
        };
        self.configuration_request += 1;
        let request = self.configuration_request;
        self.layouts.saving = Some(request);
        // Other configuration changes wait for this one, as they do for each other.
        self.configuration_pending = Some((
            request,
            wanted.window,
            super::action_editor::Operation::Save,
        ));
        Task::perform(
            async move { receiver.await.ok().map(Arc::new) },
            move |result| Message::Resize(Event::Saved(request, result)),
        )
    }

    fn layout_saved(
        &mut self,
        request: u64,
        result: Option<Arc<configuration_ui::Reply>>,
    ) -> Task<Message> {
        if self.layouts.saving != Some(request) {
            return Task::none();
        }
        self.layouts.saving = None;
        if self
            .configuration_pending
            .is_some_and(|(pending, ..)| pending == request)
        {
            self.configuration_pending = None;
        }
        let result = result
            .as_deref()
            .cloned()
            .unwrap_or_else(|| Err(fl!("files-preferences-worker-stopped")));
        let published = match result {
            Ok(published) => published,
            Err(error) => {
                self.configuration_error = Some(error);
                return Task::none();
            }
        };
        let current = published.current.clone();
        let install = self.install_configuration(published);
        // An open Preferences form keeps its own changes and adopts the widths.
        let apply = self.adopt_layout(&current);
        Task::batch([install, apply, self.sync_layout_save()])
    }
}

/// A divider that can be dragged sideways, `width` wide (0 or 1), that grabs
/// the pointer a few pixels to either side.
pub(super) struct Grip {
    pub window: window::Id,
    pub target: Target,
    pub width: f32,
    /// The column's width when the drag starts.
    pub current: f32,
    pub height: Length,
    /// The drawn line's height, centred; `None` fills the divider.
    pub line: Option<f32>,
    pub color: Color,
    /// How much of the pointer's travel the column takes: when several
    /// columns share one width, each takes its part so the divider follows
    /// the pointer.
    pub share: f32,
    /// Drawn only while hovered or dragged, for a divider with nothing after it.
    pub quiet: bool,
    /// False while a menu or popover lies over the window: the divider is
    /// drawn but does not take the pointer.
    pub enabled: bool,
}

impl<'a> From<Grip> for Element<'a, Message> {
    fn from(grip: Grip) -> Self {
        Element::new(grip)
    }
}

#[derive(Default)]
struct GripState {
    origin: Option<f32>,
    previous: Option<core_mouse::Click>,
    hovered: bool,
}

impl Grip {
    fn hit(bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: bounds.x - SLOP,
            width: bounds.width + 2.0 * SLOP,
            ..bounds
        }
    }
}

impl Widget<Message, Theme, Renderer> for Grip {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<GripState>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(GripState::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), self.height)
    }
    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fixed(self.width), self.height)
    }
    /// The divider takes the pointer from an overlay: overlays receive events
    /// before the content beneath, so the columns on either side never see a
    /// press within its reach, and hover nothing under it.
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<GripState>();
        if !self.enabled {
            state.hovered = false;
            state.origin = None;
            return None;
        }
        let hit = Self::hit(layout.bounds()).intersection(viewport)?;
        Some(iced::advanced::overlay::Element::new(Box::new(
            GripOverlay {
                state,
                window: self.window,
                target: self.target,
                current: self.current,
                share: self.share,
                hit: hit + translation,
            },
        )))
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<GripState>();
        let _ = cursor;
        let active = state.origin.is_some() || state.hovered;
        if self.quiet && !active {
            return;
        }
        let height = self.line.unwrap_or(bounds.height).min(bounds.height);
        let mut color = self.color;
        if active && self.line.is_some() {
            color.a = (color.a * 2.0).min(1.0);
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + bounds.width / 2.0 - 0.5,
                    y: bounds.y + (bounds.height - height) / 2.0,
                    width: 1.0,
                    height,
                },
                ..Default::default()
            },
            color,
        );
    }
}

struct GripOverlay<'b> {
    state: &'b mut GripState,
    window: window::Id,
    target: Target,
    current: f32,
    share: f32,
    hit: Rectangle,
}

impl iced::advanced::Overlay<Message, Theme, Renderer> for GripOverlay<'_> {
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        layout::Node::new(self.hit.size()).move_to(self.hit.position())
    }
    fn draw(
        &self,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
    }
    fn update(
        &mut self,
        event: &WidgetEvent,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let state = &mut *self.state;
        let hovered = cursor.is_over(layout.bounds());
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.request_redraw();
        }
        match event {
            WidgetEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if shell.is_event_captured() {
                    return;
                }
                let Some(position) = cursor.position_over(layout.bounds()) else {
                    return;
                };
                let click = core_mouse::Click::new(position, mouse::Button::Left, state.previous);
                state.previous = Some(click);
                shell.capture_event();
                if click.kind() == core_mouse::click::Kind::Double {
                    state.origin = None;
                    shell.publish(Message::Resize(Event::Reset(self.window, self.target)));
                } else {
                    state.origin = Some(position.x);
                    shell.publish(Message::Resize(Event::Begin(
                        self.window,
                        self.target,
                        self.current,
                    )));
                }
            }
            WidgetEvent::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(origin) = state.origin {
                    shell.capture_event();
                    shell.publish(Message::Resize(Event::Move(
                        self.window,
                        (position.x - origin) * self.share,
                    )));
                    shell.request_redraw();
                }
            }
            WidgetEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | WidgetEvent::Mouse(mouse::Event::CursorLeft)
                if state.origin.take().is_some() =>
            {
                shell.capture_event();
                shell.publish(Message::Resize(Event::End(self.window)));
            }
            _ => {}
        }
    }
    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.state.origin.is_some() || cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_widths_follow_the_pointer_within_bounds_and_settle_on_release() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let size = Target::Detail(Detail::Size);
        let _ = desktop.resize_event(Event::Begin(window, size, 80.0));
        let _ = desktop.resize_event(Event::Move(window, 33.4));
        assert_eq!(desktop.layouts.dragged(window, size), Some(113.0));
        assert_eq!(desktop.layouts.dragged(window, Target::Name), None);
        // The configuration keeps its width until the drag ends.
        assert_eq!(desktop.configuration.settings.list_columns.size, None);
        let _ = desktop.resize_event(Event::Move(window, -500.0));
        assert_eq!(desktop.layouts.dragged(window, size), Some(48.0));
        let _ = desktop.resize_event(Event::Move(window, 20.0));
        let _ = desktop.resize_event(Event::End(window));
        assert_eq!(desktop.layouts.dragged(window, size), None);
        assert_eq!(desktop.configuration.settings.list_columns.size, Some(100));
        // Saved now, or as soon as the configuration worker is free.
        assert!(desktop.layouts.saving.is_some() || desktop.layouts.wanted.is_some());
        let _ = desktop.resize_event(Event::Reset(window, size));
        assert_eq!(desktop.configuration.settings.list_columns.size, None);

        // Columns share one default width; the folder's own width gives way.
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        let mut own = desktop.preferences.for_folder(root.path()).clone();
        own.column_width = 300;
        desktop.preferences.set_folder(root.path(), own).unwrap();
        let _ = desktop.resize_event(Event::Begin(window, Target::Columns, 300.0));
        let _ = desktop.resize_event(Event::Move(window, 2000.0));
        assert_eq!(desktop.column_width(window, 300), 640.0);
        let _ = desktop.resize_event(Event::Move(window, 20.0));
        let _ = desktop.resize_event(Event::End(window));
        assert_eq!(desktop.configuration.settings.view.column_width, 320);
        assert_eq!(desktop.preferences.defaults().column_width, 320);
        assert_eq!(
            desktop.preferences.for_folder(root.path()).column_width,
            320
        );
        let browser = desktop.active(window).unwrap().browser.as_ref().unwrap();
        assert_eq!(browser.view().column_width, 320);
    }

    #[test]
    fn a_list_divider_resizes_only_the_column_before_it_up_to_the_lists_end() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        desktop.active_mut(window).unwrap().viewport_width = 1200.0;
        let mut view = desktop.preferences.for_folder(root.path()).clone();
        view.details = vec![Detail::Size, Detail::Modified, Detail::Kind];
        desktop.preferences.set_defaults(view.clone()).unwrap();
        let _ = desktop.publish_folder_views(None);
        let width = |desktop: &Desktop, detail| {
            super::super::listing_ui::detail_width(desktop, window, detail)
        };
        let name = |desktop: &Desktop| {
            super::super::listing_ui::name_width(desktop, window, &view, 1200.0)
        };
        let room = |desktop: &Desktop| {
            super::super::listing_ui::list_room(desktop, window, &view, 1200.0).unwrap()
        };
        // By default Name takes a bounded share, leaving room after the details.
        let (start_name, size, modified) = (
            name(&desktop),
            width(&desktop, Detail::Size),
            width(&desktop, Detail::Modified),
        );
        let spare = room(&desktop);
        assert!(start_name <= 440.0 && spare > 0.0);
        // Size's divider changes Size alone; Name and Modified keep theirs.
        let _ = desktop.resize_event(Event::Begin(window, Target::Detail(Detail::Size), size));
        let _ = desktop.resize_event(Event::Move(window, 30.0));
        assert_eq!(width(&desktop, Detail::Size), size + 30.0);
        assert_eq!(width(&desktop, Detail::Modified), modified);
        assert_eq!(name(&desktop), start_name);
        // It stops where the list meets the window's edge.
        let _ = desktop.resize_event(Event::Move(window, 5000.0));
        assert_eq!(width(&desktop, Detail::Size), (size + spare).round());
        assert_eq!(name(&desktop), start_name);
        let _ = desktop.resize_event(Event::Move(window, 30.0));
        let _ = desktop.resize_event(Event::End(window));
        let columns = desktop.configuration.settings.list_columns;
        assert_eq!(columns.size, Some((size + 30.0) as u16));
        assert_eq!((columns.name, columns.modified), (None, None));
        // Name's divider resizes Name, within the room after the last column.
        let spare = room(&desktop);
        let start_name = name(&desktop);
        let _ = desktop.resize_event(Event::Begin(window, Target::Name, start_name));
        let _ = desktop.resize_event(Event::Move(window, -40.0));
        assert_eq!(name(&desktop), start_name - 40.0);
        assert_eq!(width(&desktop, Detail::Size), size + 30.0);
        let _ = desktop.resize_event(Event::Move(window, 5000.0));
        assert_eq!(name(&desktop), (start_name + spare).round());
        let _ = desktop.resize_event(Event::Move(window, -40.0));
        let _ = desktop.resize_event(Event::End(window));
        assert_eq!(
            desktop.configuration.settings.list_columns.name,
            Some((start_name - 40.0) as u16)
        );
    }

    #[test]
    fn a_chosen_name_wider_than_the_list_gives_way_and_leaves_no_room() {
        let root = tempfile::tempdir().unwrap();
        let (mut desktop, window) =
            super::super::tests::desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        super::super::tests::loaded(&mut desktop, window, tab, 1, root.path());
        desktop.active_mut(window).unwrap().viewport_width = 1200.0;
        let view = desktop.preferences.for_folder(root.path()).clone();
        desktop.configuration.settings.list_columns.name = Some(1200);
        let name = super::super::listing_ui::name_width(&desktop, window, &view, 1200.0);
        assert!(name < 1200.0);
        assert_eq!(
            super::super::listing_ui::list_room(&desktop, window, &view, 1200.0),
            Some(0.0)
        );
        // A detail cannot grow past the window's edge, but can narrow.
        let detail = *view.details.first().unwrap();
        let start = super::super::listing_ui::detail_width(&desktop, window, detail);
        let _ = desktop.resize_event(Event::Begin(window, Target::Detail(detail), start));
        let _ = desktop.resize_event(Event::Move(window, 25.0));
        assert_eq!(
            super::super::listing_ui::detail_width(&desktop, window, detail),
            start
        );
        let _ = desktop.resize_event(Event::Move(window, -25.0));
        assert_eq!(
            super::super::listing_ui::detail_width(&desktop, window, detail),
            start - 25.0
        );
        assert_eq!(
            super::super::listing_ui::name_width(&desktop, window, &view, 1200.0),
            name
        );
    }
}
