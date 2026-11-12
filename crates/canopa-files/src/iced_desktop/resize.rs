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
}

/// The widths to save once no other configuration change is in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Wanted {
    window: window::Id,
    column_width: u16,
    list: ListColumns,
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
        self.resizing
            .as_ref()
            .filter(|resizing| resizing.window == window && resizing.target == target)
            .map(|resizing| resizing.width)
    }
}

fn clamp(target: Target, width: f32) -> f32 {
    let range = match target {
        Target::Columns => 160..=640,
        Target::Name => ListColumns::NAME,
        Target::Detail(_) => ListColumns::DETAIL,
    };
    width.clamp(f32::from(*range.start()), f32::from(*range.end()))
}

impl Desktop {
    /// The Columns view column width for `window`, following a drag.
    pub(super) fn column_width(&self, window: window::Id, settled: u16) -> f32 {
        self.layouts
            .dragged(window, Target::Columns)
            .unwrap_or(f32::from(settled))
    }

    pub(super) fn resize_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Begin(window, target, start) => {
                self.layouts.resizing = Some(Resizing {
                    window,
                    target,
                    start,
                    width: clamp(target, start),
                });
                Task::none()
            }
            Event::Move(window, delta) => {
                if let Some(resizing) = &mut self.layouts.resizing
                    && resizing.window == window
                {
                    resizing.width = clamp(resizing.target, resizing.start + delta).round();
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
        }
        self.layouts.wanted = Some(Wanted {
            window,
            column_width: self.configuration.settings.view.column_width,
            list: self.configuration.settings.list_columns,
        });
        tasks.push(self.sync_layout_save());
        Task::batch(tasks)
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
        settings.list_columns = wanted.list;
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
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &WidgetEvent,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<GripState>();
        match event {
            WidgetEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if shell.is_event_captured() {
                    return;
                }
                let Some(position) = cursor.position_over(Self::hit(layout.bounds())) else {
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
                    shell.publish(Message::Resize(Event::Move(
                        self.window,
                        (position.x - origin) * self.share,
                    )));
                    shell.request_redraw();
                }
            }
            WidgetEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | WidgetEvent::Mouse(mouse::Event::CursorLeft) => {
                if state.origin.take().is_some() {
                    shell.publish(Message::Resize(Event::End(self.window)));
                }
            }
            _ => {}
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<GripState>();
        if state.origin.is_some() || cursor.is_over(Self::hit(layout.bounds())) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::None
        }
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
        let active = state.origin.is_some() || cursor.is_over(Self::hit(bounds));
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
}
