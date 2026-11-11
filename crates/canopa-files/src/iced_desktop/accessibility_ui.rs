//! AT-SPI semantics for every Files window through the shared canopa-ui bridge.
//! One bridge serves the process; each Browser, utility window and transient
//! dialog publishes its own root, keyed by its Iced window id. While no
//! assistive technology listens nothing is collected.
use super::*;
use crate::browse::{Entry, EntryKind};
use crate::preferences::{Detail, ViewSettings};
use canopa_ui::accessibility::{self as a11y, Bridge, Event};
pub(super) use canopa_ui::accessibility::{Node, Role, semantic};
use std::collections::BTreeSet;

impl a11y::Message for Message {
    fn accessibility(event: Event<Self>) -> Self {
        Self::Accessibility(event)
    }
}

/// The root key of one window's tree. Iced window ids are never reused.
pub(super) fn window_key(id: window::Id) -> String {
    format!("files-window-{id}")
}

/// A key component for a filesystem path: lossless for non-UTF-8 names and
/// independent of any translated label.
pub(super) fn path_key(path: &Path) -> String {
    path.as_os_str().as_bytes().escape_ascii().to_string()
}

/// The accessibility key of a focusable control: its widget id, which never
/// depends on a translated label.
pub(super) fn focus_key(id: &iced::widget::Id) -> String {
    format!("files-control-{id:?}")
}

pub(super) fn entry_key(path: &Path) -> String {
    format!("files-entry-{}", path_key(path))
}

pub(super) fn list_key(path: &Path) -> String {
    format!("files-list-{}", path_key(path))
}

/// The bridge and the windows it currently publishes. Created by the running
/// application only; unit-test desktops have none.
pub(super) struct Accessibility {
    pub(super) bridge: Bridge<Message>,
    windows: BTreeSet<window::Id>,
}

impl Accessibility {
    pub(super) fn new() -> Self {
        Self {
            bridge: Bridge::new("canopa-files"),
            windows: BTreeSet::new(),
        }
    }
}

impl Desktop {
    /// Every window Files shows; each one is its own AT-SPI window.
    pub(super) fn window_ids(&self) -> BTreeSet<window::Id> {
        let mut ids: BTreeSet<window::Id> = self.windows.keys().copied().collect();
        ids.extend(self.previews.keys().copied());
        ids.extend(self.properties.keys().copied());
        ids.extend(
            [
                self.devices.dialog.as_ref().map(|dialog| dialog.window),
                self.recovery_window.as_ref().map(|window| window.window),
                self.recovery_destination
                    .as_ref()
                    .map(|dialog| dialog.window),
                self.trash_window.as_ref().map(|window| window.window),
                self.trash_restore.as_ref().map(|dialog| dialog.window),
                self.delete_dialog.as_ref().map(|dialog| dialog.window),
                self.network.window.as_ref().map(|dialog| dialog.window),
                self.open_dialog.as_ref().map(|dialog| dialog.window),
                self.file_operations
                    .name
                    .as_ref()
                    .map(|dialog| dialog.window),
                self.file_operations
                    .question
                    .as_ref()
                    .map(|dialog| dialog.window),
                self.file_operations
                    .details
                    .as_ref()
                    .map(|dialog| dialog.window),
                self.preferences_editor.as_ref().map(|editor| editor.window),
                self.action_editor.as_ref().map(|editor| editor.window),
                self.operations_window,
            ]
            .into_iter()
            .flatten(),
        );
        ids
    }

    /// Route an accessibility event to the bridge.
    pub(super) fn accessibility_event(&mut self, event: Event<Message>) -> Task<Message> {
        match &mut self.accessibility {
            Some(state) => state.bridge.update(event),
            None => Task::none(),
        }
    }

    /// Keep the bridge's window focus in step with the compositor's.
    pub(super) fn accessibility_focus(&mut self, id: window::Id, focused: bool) {
        if let Some(state) = &mut self.accessibility {
            state.bridge.set_window_focused(&window_key(id), focused);
        }
    }

    /// After every update: publish opened windows, withdraw closed ones and
    /// collect again. Free while no assistive technology listens.
    pub(super) fn sync_accessibility(&mut self) -> Task<Message> {
        let Some(state) = &self.accessibility else {
            return Task::none();
        };
        let ids = self.window_ids();
        if ids != state.windows {
            let opened: Vec<_> = ids
                .difference(&state.windows)
                .map(|id| (*id, self.title(*id)))
                .collect();
            let state = self.accessibility.as_mut().unwrap();
            for closed in state.windows.difference(&ids) {
                state.bridge.close(&window_key(*closed));
            }
            for (id, title) in opened {
                let key = window_key(id);
                state.bridge.open(key.clone(), &title, Some(id));
                // A new window normally takes focus; compositor events correct it.
                state.bridge.set_window_focused(&key, true);
            }
            state.windows = ids;
        }
        self.accessibility.as_mut().unwrap().bridge.refresh()
    }
}

impl Desktop {
    /// Operation outcomes are announced once, from the focused window (or the
    /// first Browser while Files has no focus), never from every window.
    fn announces_operations(&self, id: window::Id) -> bool {
        match self.focused_window {
            Some(focused) => focused == id,
            None => self.windows.keys().next() == Some(&id),
        }
    }

    /// A zero-width live region for the newest operation's state in `id`.
    pub(super) fn operations_announcer(&self, id: window::Id) -> Element<'_, Message> {
        let space = announcer_space();
        match self
            .file_operations
            .announcement()
            .filter(|_| self.announces_operations(id))
        {
            Some((text, true)) => semantic(Node::alert(text).key("files-operation-alert"), space),
            Some((text, false)) => {
                semantic(Node::status(text).key("files-operation-status"), space)
            }
            None => space.into(),
        }
    }
}

/// The invisible carrier of a live region. Rows and columns drop a child with
/// a fixed zero width or height entirely; a shrinking space lays out as 0×0.
pub(super) fn announcer_space() -> iced::widget::Space {
    iced::widget::space()
}

/// A push button named by the text drawn inside it.
pub(super) fn button<'a>(
    key: impl Into<String>,
    message: Option<Message>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    semantic(
        Node::new(Role::Button)
            .key(key)
            .disabled(message.is_none())
            .on_click(message),
        content,
    )
}

/// A push button whose drawn content is an icon or an abbreviation.
pub(super) fn named_button<'a>(
    key: impl Into<String>,
    label: impl Into<String>,
    message: Option<Message>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    semantic(
        Node::button(label)
            .key(key)
            .disabled(message.is_none())
            .on_click(message),
        content,
    )
}

/// A check box named by its drawn label; clicking sends `message`.
pub(super) fn check_box<'a>(
    key: impl Into<String>,
    checked: bool,
    message: Option<Message>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    semantic(
        Node::new(Role::CheckBox)
            .key(key)
            .toggled(checked)
            .disabled(message.is_none())
            .on_click(message),
        content,
    )
}

/// A dialog window's content, described by its message for screen readers
/// that read a dialog's description when it opens.
pub(super) fn dialog<'a>(
    key: impl Into<String>,
    title: impl Into<String>,
    description: impl Into<String>,
    alert: bool,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title = title.into();
    let node = if alert {
        Node::alert_dialog(title)
    } else {
        Node::dialog(title)
    };
    semantic(node.key(key).description(description), content)
}

/// What a row's type and state sound like: "PDF file, 1.2 KiB, hidden".
pub(super) fn describe(entry: &Entry, settings: &ViewSettings) -> String {
    let mut parts = vec![match entry.kind {
        EntryKind::Symlink if entry.is_directory() => "Link to folder".to_owned(),
        EntryKind::Symlink => "Link".to_owned(),
        EntryKind::Directory => "Folder".to_owned(),
        _ => entry.path.extension().map_or_else(
            || "File".into(),
            |extension| format!("{} file", extension.to_string_lossy().to_uppercase()),
        ),
    }];
    if !entry.is_directory() {
        parts.push(super::listing_ui::size(entry.size));
    }
    if settings.details.contains(&Detail::Modified)
        && let Some(modified) = super::listing_ui::modified(entry)
    {
        parts.push(format!("modified {modified}"));
    }
    if entry.is_hidden() {
        parts.push("hidden".into());
    }
    parts.join(", ")
}

/// One file or folder row in a list, column or icon view.
pub(super) struct Row<'a> {
    pub entry: &'a Entry,
    pub settings: &'a ViewSettings,
    pub selected: bool,
    /// The keyboard cursor is on this row and no text field holds focus.
    pub focused: bool,
    /// One-based position among all rows of the (virtualized) list.
    pub position: (usize, usize),
    pub select: Message,
}

pub(super) fn row(row: Row<'_>) -> Node<Message> {
    Node::option(row.entry.name().into_owned(), row.selected)
        .key(entry_key(&row.entry.path))
        .description(describe(row.entry, row.settings))
        .position(row.position.0, row.position.1)
        .focused(row.focused)
        .on_click(Some(row.select))
}
