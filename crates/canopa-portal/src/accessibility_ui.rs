//! AT-SPI semantics for the portal's Iced dialogs through the shared
//! canopa-ui bridge. Each helper process shows one dialog window and keeps one
//! bridge for it; while no assistive technology listens nothing is collected.
use canopa_ui::accessibility::Bridge;
pub(crate) use canopa_ui::accessibility::{Node, Value, semantic};
use iced::{Element, Task, window};
use std::{os::unix::ffi::OsStrExt, path::Path};

/// A key component for a filesystem path: lossless for non-UTF-8 names and
/// independent of any translated label.
pub(crate) fn path_key(path: &Path) -> String {
    path.as_os_str().as_bytes().escape_ascii().to_string()
}

/// The bridge of a helper's single dialog window, keyed by `key`.
pub(crate) struct Accessibility<M> {
    pub(crate) bridge: Bridge<M>,
    key: &'static str,
}

impl<M: canopa_ui::accessibility::Message> Accessibility<M> {
    pub(crate) fn new(name: &'static str, key: &'static str) -> Self {
        Self {
            bridge: Bridge::new(name),
            key,
        }
    }

    /// Publish the dialog once its window exists. A new dialog normally takes
    /// focus; compositor events correct it.
    pub(crate) fn opened(&mut self, title: &str, window: window::Id) {
        self.bridge.open(self.key, title, Some(window));
        self.bridge.set_window_focused(self.key, true);
    }

    /// Withdraw the dialog before the helper exits.
    pub(crate) fn close(&mut self) {
        self.bridge.close(self.key);
    }
}

/// Route an accessibility event to the bridge, if the dialog has one.
pub(crate) fn update<M: canopa_ui::accessibility::Message>(
    accessibility: &mut Option<Accessibility<M>>,
    event: canopa_ui::accessibility::Event<M>,
) -> Task<M> {
    accessibility
        .as_mut()
        .map_or_else(Task::none, |state| state.bridge.update(event))
}

/// Collect again after a non-accessibility update. Free while inactive.
pub(crate) fn refresh<M: canopa_ui::accessibility::Message>(
    accessibility: &mut Option<Accessibility<M>>,
    task: Task<M>,
) -> Task<M> {
    match accessibility {
        Some(state) => Task::batch([task, state.bridge.refresh()]),
        None => task,
    }
}

/// A push button with an explicit name; disabled without a message.
pub(crate) fn button<'a, M: Clone + Send + 'static>(
    key: impl Into<String>,
    label: impl Into<String>,
    message: Option<M>,
    content: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    semantic(
        Node::button(label)
            .key(key)
            .disabled(message.is_none())
            .on_click(message),
        content,
    )
}

/// A check box; clicking sends what toggling it with the pointer sends.
pub(crate) fn check_box<'a, M: Clone + Send + 'static>(
    key: impl Into<String>,
    label: impl Into<String>,
    checked: bool,
    message: Option<M>,
    content: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    semantic(
        Node::check_box(label, checked)
            .key(key)
            .disabled(message.is_none())
            .on_click(message),
        content,
    )
}

/// A pick list: its value is the current choice. Clicking and incrementing
/// choose the next option, decrementing the previous one, and an assistive
/// technology may set any listed option by its label.
pub(crate) fn combo_box<'a, M: Clone + Send + Sync + 'static>(
    key: impl Into<String>,
    label: impl Into<String>,
    options: Vec<(String, M)>,
    current: Option<usize>,
    content: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    let count = options.len();
    let next = current.map_or(0, |index| (index + 1) % count.max(1));
    let previous = current.map_or(count.saturating_sub(1), |index| {
        index.checked_sub(1).unwrap_or(count.saturating_sub(1))
    });
    let message = |index: usize| options.get(index).map(|(_, message)| message.clone());
    let (next, previous) = (message(next), message(previous));
    let value = current
        .and_then(|index| options.get(index))
        .map(|(label, _)| label.clone())
        .unwrap_or_default();
    let mut node = Node::combo_box(label, value)
        .key(key)
        .disabled(options.is_empty())
        .on_click(next.clone())
        .on_increment(next)
        .on_decrement(previous);
    if let Some(index) = current {
        node = node.description(fl!(
            "portal-combo-position",
            position = index.saturating_add(1),
            count = count
        ));
    }
    semantic(
        node.on_set_value(move |requested| match requested {
            Value::Text(text) => options
                .iter()
                .find(|(label, _)| *label == text)
                .map(|(_, message)| message.clone()),
            Value::Number(_) => None,
        }),
        content,
    )
}

/// A live region that draws nothing: polite, or assertive for errors. Its key
/// keeps it in the tree, so a change of `text` is announced as a change.
pub(crate) fn announcer<'a, M: Clone + Send + 'static>(
    key: impl Into<String>,
    text: impl Into<String>,
    assertive: bool,
) -> Element<'a, M> {
    let node = if assertive {
        Node::alert(text)
    } else {
        Node::status(text)
    };
    // Rows and columns drop a child with a fixed zero size; a shrinking space
    // lays out as 0×0 and stays.
    semantic(node.key(key), iced::widget::space())
}

/// A dialog window: the window root and the dialog inside it.
pub(crate) fn dialog_window<'a, M: Clone + Send + 'static>(
    window_key: &'static str,
    window_title: impl Into<String>,
    dialog: Node<M>,
    content: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    semantic(
        Node::window(window_key, window_title),
        semantic(dialog, content),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    #[test]
    fn path_keys_are_lossless_and_ascii() {
        let raw = Path::new("/tmp").join(OsString::from_vec(b"r\xffw".to_vec()));
        assert_eq!(path_key(&raw), "/tmp/r\\xffw");
        assert_ne!(path_key(Path::new("/a b")), path_key(Path::new("/a_b")));
    }
}
