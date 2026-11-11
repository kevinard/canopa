//! Turn one collected window into an AccessKit tree with stable node IDs.
use super::collect::{Child, Collected, Snapshot};
use super::node::{Actions, Node};
use accesskit::{
    Action, NodeId, Rect, Role, TextPosition, TextSelection, Toggled, TreeId, TreeInfo, TreeUpdate,
};
use iced::Rectangle;
use iced::advanced::widget::Id;
use std::collections::HashMap;

/// What an incoming request on one published node may do.
pub(crate) struct Bound<M> {
    pub(crate) actions: Actions<M>,
    pub(crate) focus: Option<Id>,
}

/// Stable IDs: a key keeps its node ID while it stays displayed.
#[derive(Default)]
pub(crate) struct Ids {
    map: HashMap<String, NodeId>,
    next: u64,
}

impl Ids {
    /// The ID a window's root keeps from opening until it closes.
    pub(crate) fn reserve(&mut self, key: &str) -> NodeId {
        if let Some(id) = self.map.get(key) {
            return *id;
        }
        let mut next = std::mem::take(&mut self.map);
        let id = self.take(key, &mut next);
        self.map = next;
        id
    }
    fn take(&mut self, key: &str, next: &mut HashMap<String, NodeId>) -> NodeId {
        let id = self.map.remove(key).unwrap_or_else(|| {
            self.next = self
                .next
                .checked_add(1)
                .expect("accessibility IDs exhausted");
            NodeId(self.next)
        });
        next.insert(key.to_owned(), id);
        id
    }
    #[cfg(test)]
    pub(crate) fn get(&self, key: &str) -> Option<NodeId> {
        self.map.get(key).copied()
    }
}

pub(crate) struct Built<M> {
    pub(crate) update: TreeUpdate,
    pub(crate) actions: HashMap<NodeId, Bound<M>>,
    /// The key of the node that has keyboard focus, if any.
    pub(crate) focus: Option<String>,
}

fn rect(bounds: Rectangle) -> Rect {
    Rect::new(
        f64::from(bounds.x),
        f64::from(bounds.y),
        f64::from(bounds.x + bounds.width),
        f64::from(bounds.y + bounds.height),
    )
}

struct Pending<'a, M> {
    key: String,
    source: &'a Collected<M>,
    children: Vec<String>,
    texts: Vec<(String, String, Rectangle)>,
    name: Option<String>,
    /// The label drawn just before an unnamed control, as in `row![text, slider]`.
    inferred: Option<String>,
}

/// Roles that need a name but do not draw one inside themselves.
fn named_by_preceding_text(role: Role) -> bool {
    matches!(
        role,
        Role::Slider
            | Role::ComboBox
            | Role::TextInput
            | Role::SearchInput
            | Role::PasswordInput
            | Role::SpinButton
            | Role::Switch
            | Role::ProgressIndicator
    )
}

/// Build the window whose root node has `root`; `None` if it is not displayed.
pub(crate) fn build<M: Clone>(
    snapshot: &Snapshot<M>,
    root: &str,
    ids: &mut Ids,
    window_focused: bool,
    scale: f64,
) -> Option<Built<M>> {
    let start = snapshot
        .nodes
        .iter()
        .position(|node| node.parent.is_none() && node.node.key.as_deref() == Some(root))?;
    // Resolve keys depth first; generated keys follow the parent's key.
    let mut pending: Vec<Pending<'_, M>> = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut unique = |key: String| {
        let count = used.entry(key.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            key
        } else {
            format!("{key}#{count}")
        }
    };
    let mut stack = vec![(start, unique(root.to_owned()))];
    while let Some((index, key)) = stack.pop() {
        let source = &snapshot.nodes[index];
        let absorbs = source.node.absorbs_text();
        let mut children = Vec::new();
        let mut texts = Vec::new();
        let mut absorbed = Vec::new();
        let mut child_nodes = Vec::new();
        for (position, child) in source.children.iter().enumerate() {
            match child {
                Child::Node(child) => {
                    let node = &snapshot.nodes[*child];
                    let child_key = unique(
                        node.node
                            .key
                            .clone()
                            .unwrap_or_else(|| format!("{key}/{:?}{position}", node.node.role)),
                    );
                    children.push(child_key.clone());
                    child_nodes.push((*child, child_key));
                }
                Child::Text(text, bounds) => {
                    if absorbs {
                        absorbed.push(text.trim().to_owned());
                    } else if !source.node.hidden_text {
                        let text_key = unique(format!("{key}/text{position}"));
                        children.push(text_key.clone());
                        texts.push((text_key, text.clone(), *bounds));
                    }
                }
            }
        }
        let name = source
            .node
            .label
            .clone()
            .or_else(|| (!absorbed.is_empty()).then(|| absorbed.join(" ")));
        pending.push(Pending {
            key,
            source,
            children,
            texts,
            name,
            inferred: None,
        });
        // Preserve reading order: push children in reverse.
        stack.extend(child_nodes.into_iter().rev());
    }
    // Name unnamed controls after the text drawn just before them.
    let index_of: HashMap<String, usize> = pending
        .iter()
        .enumerate()
        .map(|(index, item)| (item.key.clone(), index))
        .collect();
    let mut inferred = Vec::new();
    for item in &pending {
        let mut previous: Option<(&String, &String)> = None;
        for child in &item.children {
            if let Some((key, text, _)) = item.texts.iter().find(|(key, _, _)| key == child) {
                previous = Some((key, text));
            } else if let Some(&index) = index_of.get(child) {
                let control = &pending[index];
                if control.name.is_none()
                    && named_by_preceding_text(control.source.node.role)
                    && let Some((key, text)) = previous
                {
                    inferred.push((index, text.trim().to_owned(), key.clone()));
                }
                previous = None;
            }
        }
    }
    for (index, name, key) in inferred {
        pending[index].name = Some(name);
        pending[index].inferred = Some(key);
    }
    // Allocate IDs for every key, retiring those no longer shown.
    let mut next = HashMap::new();
    let mut id_of = HashMap::new();
    for item in &pending {
        id_of.insert(item.key.clone(), ids.take(&item.key, &mut next));
        for (text_key, _, _) in &item.texts {
            id_of.insert(text_key.clone(), ids.take(text_key, &mut next));
        }
    }
    for item in &pending {
        if matches!(item.source.node.role, Role::TextInput | Role::SearchInput) {
            let run = format!("{}/run", item.key);
            id_of.insert(run.clone(), ids.take(&run, &mut next));
        }
    }
    ids.map = next;
    let resolve = |keys: &[String]| -> Vec<NodeId> {
        keys.iter()
            .filter_map(|key| id_of.get(key).copied())
            .collect()
    };
    let mut nodes = Vec::new();
    let mut actions = HashMap::new();
    let mut focus = None;
    for (position, item) in pending.iter().enumerate() {
        let id = id_of[&item.key];
        let source: &Node<M> = &item.source.node;
        let mut node = accesskit::Node::new(source.role);
        let mut children = resolve(&item.children);
        if let Some(name) = &item.name {
            if source.role == Role::Label && source.value.is_none() {
                node.set_value(name.clone());
            } else {
                node.set_label(name.clone());
            }
        }
        if let Some(value) = &source.value {
            node.set_value(value.clone());
        }
        if let Some(description) = &source.description {
            node.set_description(description.clone());
        }
        if let Some(placeholder) = &source.placeholder {
            node.set_placeholder(placeholder.clone());
        }
        if let Some(range) = source.range {
            node.set_numeric_value(range.value);
            node.set_min_numeric_value(range.min);
            node.set_max_numeric_value(range.max);
            if let Some(step) = range.step {
                node.set_numeric_value_step(step);
            }
            if source.role == Role::Slider {
                node.set_orientation(accesskit::Orientation::Horizontal);
            }
        }
        if let Some(on) = source.toggled {
            node.set_toggled(if on { Toggled::True } else { Toggled::False });
        }
        if let Some(selected) = source.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = source.expanded {
            node.set_expanded(expanded);
        }
        if source.disabled {
            node.set_disabled();
        }
        if source.read_only {
            node.set_read_only();
        }
        if source.required {
            node.set_required();
        }
        if source.invalid {
            node.set_invalid(accesskit::Invalid::True);
        }
        if source.busy {
            node.set_busy();
        }
        if source.modal {
            node.set_modal();
        }
        if source.multiselectable {
            node.set_multiselectable();
        }
        match source.live {
            Some(live) => {
                node.set_live(live);
                node.set_live_atomic();
            }
            None => node.set_live(accesskit::Live::Off),
        }
        if let Some(level) = source.level {
            node.set_level(level);
        }
        if let Some((index, size)) = source.position {
            node.set_position_in_set(index);
            node.set_size_of_set(size);
        }
        if let Some(shortcut) = &source.shortcut {
            node.set_keyboard_shortcut(shortcut.clone());
        }
        let mut labelled_by = resolve(&source.labelled_by);
        labelled_by.extend(item.inferred.as_ref().and_then(|key| id_of.get(key)));
        if !labelled_by.is_empty() {
            node.set_labelled_by(labelled_by);
        }
        let described_by = resolve(&source.described_by);
        if !described_by.is_empty() {
            node.set_described_by(described_by);
        }
        let controls = resolve(&source.controls);
        if !controls.is_empty() {
            node.set_controls(controls);
        }
        if let Some(target) = source
            .active_descendant
            .as_ref()
            .and_then(|key| id_of.get(key))
        {
            node.set_active_descendant(*target);
        }
        let enabled = !source.disabled;
        let bound = &source.actions;
        if enabled {
            for (present, action) in [
                (bound.click.is_some(), Action::Click),
                (bound.increment.is_some(), Action::Increment),
                (bound.decrement.is_some(), Action::Decrement),
                (bound.expand.is_some(), Action::Expand),
                (bound.collapse.is_some(), Action::Collapse),
                (bound.set_value.is_some(), Action::SetValue),
                (item.source.focus.is_some(), Action::Focus),
            ] {
                if present {
                    node.add_action(action);
                }
            }
        }
        if matches!(source.role, Role::TextInput | Role::SearchInput) {
            // AT-SPI's Text interface reads a text run; the caret sits at the end.
            let run_id = id_of[&format!("{}/run", item.key)];
            let value = source.value.clone().unwrap_or_default();
            let mut run = accesskit::Node::new(Role::TextRun);
            run.set_character_lengths(
                value
                    .chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            run.set_value(value.clone());
            run.set_bounds(rect(item.source.bounds));
            let end = TextPosition {
                node: run_id,
                character_index: value.chars().count(),
            };
            node.set_text_selection(TextSelection {
                anchor: end,
                focus: end,
            });
            children.push(run_id);
            nodes.push((run_id, run));
        }
        node.set_children(children);
        node.set_bounds(rect(item.source.bounds));
        if position == 0 && scale != 1.0 {
            node.set_transform(accesskit::Affine::scale(scale));
        }
        for (text_key, text, bounds) in &item.texts {
            let mut label = accesskit::Node::new(Role::Label);
            label.set_value(text.trim().to_owned());
            label.set_bounds(rect(*bounds));
            label.set_live(accesskit::Live::Off);
            nodes.push((id_of[text_key], label));
        }
        if item.source.focused && window_focused {
            // The deepest focused node wins; depth-first order visits it last.
            focus = Some((id, item.key.clone()));
        }
        nodes.push((id, node));
        if enabled {
            actions.insert(
                id,
                Bound {
                    actions: bound.clone(),
                    focus: item.source.focus.clone(),
                },
            );
        }
    }
    let root_id = id_of[&pending[0].key];
    Some(Built {
        update: TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(root_id)),
            tree_id: TreeId::ROOT,
            focus: focus.as_ref().map_or(root_id, |(id, _)| *id),
        },
        actions,
        focus: focus.map(|(_, key)| key),
    })
}

/// A window with no content yet: what an assistive technology sees first.
pub(crate) fn placeholder(root: NodeId, title: &str) -> TreeUpdate {
    let mut node = accesskit::Node::new(Role::Window);
    node.set_label(title);
    node.set_live(accesskit::Live::Off);
    TreeUpdate {
        nodes: vec![(root, node)],
        tree: Some(TreeInfo::new(root)),
        tree_id: TreeId::ROOT,
        focus: root,
    }
}
