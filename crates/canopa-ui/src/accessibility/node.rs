//! Declarative semantics a view attaches to the widget it describes.
use accesskit::{Live, Role};
use iced::widget::Id;
use std::sync::Arc;

/// A value an assistive technology asks a control to take.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
}

/// Turns a requested value into the message ordinary input would send.
/// It runs on the accessibility thread and must reject what the control refuses.
pub type SetValue<M> = Arc<dyn Fn(Value) -> Option<M> + Send + Sync>;

/// The messages behind a node's actions; the same ones mouse and keyboard send.
pub struct Actions<M> {
    pub click: Option<M>,
    pub increment: Option<M>,
    pub decrement: Option<M>,
    pub expand: Option<M>,
    pub collapse: Option<M>,
    pub set_value: Option<SetValue<M>>,
}

impl<M> Default for Actions<M> {
    fn default() -> Self {
        Self {
            click: None,
            increment: None,
            decrement: None,
            expand: None,
            collapse: None,
            set_value: None,
        }
    }
}

impl<M: Clone> Clone for Actions<M> {
    fn clone(&self) -> Self {
        Self {
            click: self.click.clone(),
            increment: self.increment.clone(),
            decrement: self.decrement.clone(),
            expand: self.expand.clone(),
            collapse: self.collapse.clone(),
            set_value: self.set_value.clone(),
        }
    }
}

/// A numeric range for sliders, spin buttons and progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
}

/// One accessible object. Text drawn inside a node names it when the role is a
/// control (a button's caption); inside a container it becomes a label child.
pub struct Node<M> {
    pub(crate) role: Role,
    pub(crate) key: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) placeholder: Option<String>,
    pub(crate) range: Option<Range>,
    pub(crate) toggled: Option<bool>,
    pub(crate) selected: Option<bool>,
    pub(crate) expanded: Option<bool>,
    pub(crate) disabled: bool,
    pub(crate) read_only: bool,
    pub(crate) required: bool,
    pub(crate) invalid: bool,
    pub(crate) busy: bool,
    pub(crate) modal: bool,
    pub(crate) multiselectable: bool,
    pub(crate) live: Option<Live>,
    pub(crate) level: Option<usize>,
    pub(crate) position: Option<(usize, usize)>,
    pub(crate) shortcut: Option<String>,
    pub(crate) labelled_by: Vec<String>,
    pub(crate) described_by: Vec<String>,
    pub(crate) controls: Vec<String>,
    pub(crate) active_descendant: Option<String>,
    pub(crate) focus: Option<Id>,
    pub(crate) hidden_text: bool,
    pub(crate) focused: bool,
    pub(crate) actions: Actions<M>,
}

impl<M: Clone> Clone for Node<M> {
    fn clone(&self) -> Self {
        Self {
            role: self.role,
            key: self.key.clone(),
            label: self.label.clone(),
            value: self.value.clone(),
            description: self.description.clone(),
            placeholder: self.placeholder.clone(),
            range: self.range,
            toggled: self.toggled,
            selected: self.selected,
            expanded: self.expanded,
            disabled: self.disabled,
            read_only: self.read_only,
            required: self.required,
            invalid: self.invalid,
            busy: self.busy,
            modal: self.modal,
            multiselectable: self.multiselectable,
            live: self.live,
            level: self.level,
            position: self.position,
            shortcut: self.shortcut.clone(),
            labelled_by: self.labelled_by.clone(),
            described_by: self.described_by.clone(),
            controls: self.controls.clone(),
            active_descendant: self.active_descendant.clone(),
            focus: self.focus.clone(),
            hidden_text: self.hidden_text,
            focused: self.focused,
            actions: self.actions.clone(),
        }
    }
}

impl<M> std::fmt::Debug for Node<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("role", &self.role)
            .field("key", &self.key)
            .field("label", &self.label)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

impl<M> Node<M> {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            key: None,
            label: None,
            value: None,
            description: None,
            placeholder: None,
            range: None,
            toggled: None,
            selected: None,
            expanded: None,
            disabled: false,
            read_only: false,
            required: false,
            invalid: false,
            busy: false,
            modal: false,
            multiselectable: false,
            live: None,
            level: None,
            position: None,
            shortcut: None,
            labelled_by: Vec::new(),
            described_by: Vec::new(),
            controls: Vec::new(),
            active_descendant: None,
            focus: None,
            hidden_text: false,
            focused: false,
            actions: Actions::default(),
        }
    }

    /// The top-level node of one window; `key` selects it among windows.
    pub fn window(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new(Role::Window).key(key).label(title)
    }
    pub fn dialog(title: impl Into<String>) -> Self {
        Self::new(Role::Dialog).label(title).modal(true)
    }
    pub fn alert_dialog(title: impl Into<String>) -> Self {
        Self::new(Role::AlertDialog).label(title).modal(true)
    }
    pub fn group(label: impl Into<String>) -> Self {
        Self::new(Role::Group).label(label)
    }
    /// An unnamed container whose text becomes label children.
    pub fn container() -> Self {
        Self::new(Role::GenericContainer)
    }
    pub fn navigation(label: impl Into<String>) -> Self {
        Self::new(Role::Navigation).label(label)
    }
    pub fn toolbar(label: impl Into<String>) -> Self {
        Self::new(Role::Toolbar).label(label)
    }
    pub fn heading(text: impl Into<String>, level: usize) -> Self {
        Self::new(Role::Heading).label(text).level(level)
    }
    /// Static text. Prefer drawing ordinary text inside a container.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(Role::Label).value(text)
    }
    pub fn image(alternative: impl Into<String>) -> Self {
        Self::new(Role::Image).label(alternative)
    }
    pub fn button(label: impl Into<String>) -> Self {
        Self::new(Role::Button).label(label)
    }
    pub fn link(label: impl Into<String>) -> Self {
        Self::new(Role::Link).label(label)
    }
    pub fn check_box(label: impl Into<String>, checked: bool) -> Self {
        Self::new(Role::CheckBox).label(label).toggled(checked)
    }
    pub fn switch(label: impl Into<String>, on: bool) -> Self {
        Self::new(Role::Switch).label(label).toggled(on)
    }
    pub fn radio(label: impl Into<String>, selected: bool) -> Self {
        Self::new(Role::RadioButton).label(label).toggled(selected)
    }
    pub fn slider(label: impl Into<String>, range: Range) -> Self {
        Self::new(Role::Slider).label(label).range(range)
    }
    pub fn progress(label: impl Into<String>, fraction: Option<f64>) -> Self {
        let node = Self::new(Role::ProgressIndicator).label(label);
        match fraction {
            Some(fraction) => node.range(Range {
                value: (fraction.clamp(0.0, 1.0) * 100.0).round(),
                min: 0.0,
                max: 100.0,
                step: None,
            }),
            None => node.busy(true),
        }
    }
    pub fn text_input(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(Role::TextInput).label(label).value(value)
    }
    pub fn search_input(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(Role::SearchInput).label(label).value(value)
    }
    /// A pick list: the value is the current choice.
    pub fn combo_box(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(Role::ComboBox).label(label).value(value)
    }
    pub fn list(label: impl Into<String>) -> Self {
        Self::new(Role::ListBox).label(label)
    }
    pub fn option(label: impl Into<String>, selected: bool) -> Self {
        Self::new(Role::ListBoxOption)
            .label(label)
            .selected(selected)
    }
    pub fn tab_list(label: impl Into<String>) -> Self {
        Self::new(Role::TabList).label(label)
    }
    pub fn tab(label: impl Into<String>, selected: bool) -> Self {
        Self::new(Role::Tab).label(label).selected(selected)
    }
    pub fn menu(label: impl Into<String>) -> Self {
        Self::new(Role::Menu).label(label)
    }
    pub fn menu_item(label: impl Into<String>) -> Self {
        Self::new(Role::MenuItem).label(label)
    }
    pub fn tree(label: impl Into<String>) -> Self {
        Self::new(Role::Tree).label(label)
    }
    pub fn tree_item(label: impl Into<String>, selected: bool) -> Self {
        Self::new(Role::TreeItem).label(label).selected(selected)
    }
    pub fn grid(label: impl Into<String>) -> Self {
        Self::new(Role::Grid).label(label)
    }
    /// A polite live region: each change of its text is announced.
    pub fn status(text: impl Into<String>) -> Self {
        Self::new(Role::Status)
            .label(text)
            .live(Live::Polite)
            .hide_text()
    }
    /// An assertive live region for errors that need attention now.
    pub fn alert(text: impl Into<String>) -> Self {
        Self::new(Role::Alert)
            .label(text)
            .live(Live::Assertive)
            .hide_text()
    }

    /// Stable identity across updates; also names the widget for bounds.
    /// Keys must not depend on translated text.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = (!label.is_empty()).then_some(label);
        self
    }
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }
    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.is_empty()).then_some(description);
        self
    }
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        let placeholder = placeholder.into();
        self.placeholder = (!placeholder.is_empty()).then_some(placeholder);
        self
    }
    pub fn range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = Some(on);
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }
    pub fn multiselectable(mut self, multiselectable: bool) -> Self {
        self.multiselectable = multiselectable;
        self
    }
    pub fn live(mut self, live: Live) -> Self {
        self.live = Some(live);
        self
    }
    pub fn level(mut self, level: usize) -> Self {
        self.level = Some(level);
        self
    }
    /// One-based position among `size` siblings, for long or virtual lists.
    pub fn position(mut self, position: usize, size: usize) -> Self {
        self.position = Some((position, size));
        self
    }
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
    pub fn labelled_by(mut self, key: impl Into<String>) -> Self {
        self.labelled_by.push(key.into());
        self
    }
    pub fn described_by(mut self, key: impl Into<String>) -> Self {
        self.described_by.push(key.into());
        self
    }
    pub fn controls(mut self, key: impl Into<String>) -> Self {
        self.controls.push(key.into());
        self
    }
    /// The option a focused input or list currently points at.
    pub fn active_descendant(mut self, key: Option<String>) -> Self {
        self.active_descendant = key;
        self
    }
    /// The widget that keyboard focus moves to for this node. Without one, the
    /// first focusable widget inside it is used.
    pub fn focus_id(mut self, id: impl Into<Id>) -> Self {
        self.focus = Some(id.into());
        self
    }
    /// Keyboard focus held in application state rather than by an Iced
    /// focusable widget (a selected bar module, a highlighted menu row).
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
    /// Ignore text drawn inside; the label already says it.
    pub fn hide_text(mut self) -> Self {
        self.hidden_text = true;
        self
    }
    pub fn on_click(mut self, message: Option<M>) -> Self {
        self.actions.click = message;
        self
    }
    pub fn on_increment(mut self, message: Option<M>) -> Self {
        self.actions.increment = message;
        self
    }
    pub fn on_decrement(mut self, message: Option<M>) -> Self {
        self.actions.decrement = message;
        self
    }
    pub fn on_expand(mut self, message: Option<M>) -> Self {
        self.actions.expand = message;
        self
    }
    pub fn on_collapse(mut self, message: Option<M>) -> Self {
        self.actions.collapse = message;
        self
    }
    pub fn on_set_value(
        mut self,
        change: impl Fn(Value) -> Option<M> + Send + Sync + 'static,
    ) -> Self {
        self.actions.set_value = Some(Arc::new(change));
        self
    }
    /// Text entry: `SetValue` with text sends `change(text)`.
    pub fn on_text(self, change: impl Fn(String) -> M + Send + Sync + 'static) -> Self {
        self.on_set_value(move |value| match value {
            Value::Text(text) => Some(change(text)),
            Value::Number(_) => None,
        })
    }

    pub fn role(&self) -> Role {
        self.role
    }
    pub fn key_str(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Whether drawn text names this node rather than becoming its children.
    pub(crate) fn absorbs_text(&self) -> bool {
        use Role::*;
        matches!(
            self.role,
            Button
                | CheckBox
                | Switch
                | RadioButton
                | Link
                | ListBoxOption
                | ListItem
                | MenuItem
                | MenuItemCheckBox
                | MenuItemRadio
                | Tab
                | TreeItem
                | Row
                | Cell
                | GridCell
                | Heading
                | Label
                | Status
                | Alert
                | Image
                | ProgressIndicator
                | Slider
                | TextInput
                | SearchInput
                | PasswordInput
                | ComboBox
                | SpinButton
                | Tooltip
        )
    }
}
