use super::canopa_semantics::{
    Body, Key as SemanticKey, Node as SemanticNode, body as selected_body,
};
// AT-SPI semantics for the displayed Canopa provider, with opaque host action IDs.
use super::canopa_accessibility_bounds::{Bounds, Measure};
use super::{App, Message, messages::ExternalPanelEvent};
use accesskit::{
    Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Live, Node,
    NodeId, Role, TreeId, TreeInfo, TreeUpdate,
};
use canopa_panel_protocol::{InteractionTarget, PanelAction};
use canopa_shell::external_panel::canopa::CanopaSnapshot;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

const ROOT: NodeId = NodeId(1);

struct Published {
    tree: TreeUpdate,
    actions: BTreeMap<NodeId, String>,
    provider: String,
    sliders: BTreeMap<NodeId, canopa_panel_protocol::PanelSlider>,
}

struct Handler {
    published: Arc<Mutex<Published>>,
    sender: tokio::sync::mpsc::Sender<Message>,
}

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.published.lock().unwrap().tree.clone())
    }
}
impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        if request.target_tree != TreeId::ROOT {
            return;
        }
        let published = self.published.lock().unwrap();
        let Some(id) = published.actions.get(&request.target_node).cloned() else {
            return;
        };
        if let Some(slider) = published.sliders.get(&request.target_node)
            && request.action != Action::Focus
        {
            let value = match (request.action, request.data) {
                (Action::SetValue, Some(accesskit::ActionData::NumericValue(value)))
                    if value.is_finite()
                        && value >= 0.0
                        && value <= f64::from(u16::MAX)
                        && value.fract() == 0.0 =>
                {
                    value as u16
                }
                (Action::SetValue, Some(accesskit::ActionData::Value(value))) => {
                    let Some(value) = slider.parse_value(Some(&value)) else {
                        return;
                    };
                    value
                }
                (Action::Increment, _) => slider.value.saturating_add(slider.step).min(slider.max),
                (Action::Decrement, _) => slider.value.saturating_sub(slider.step).max(slider.min),
                _ => return,
            };
            if slider.accepts(value)
                && let Some(element_id) =
                    canopa_shell::external_panel::canopa::slider_value_action(&id, value)
            {
                let _ = self.sender.try_send(Message::ExternalPanel(
                    ExternalPanelEvent::ButtonActivated {
                        provider_name: published.provider.clone(),
                        element_id,
                    },
                ));
            }
            return;
        }
        let message = match request.action {
            Action::Click => Message::ExternalPanel(ExternalPanelEvent::ButtonActivated {
                provider_name: published.provider.clone(),
                element_id: id,
            }),
            Action::Focus => Message::CanopaAccessibilityFocus {
                provider: published.provider.clone(),
                action_id: id,
            },
            _ => return,
        };
        let _ = self.sender.try_send(message);
    }
}
struct Deactivate;
impl DeactivationHandler for Deactivate {
    fn deactivate_accessibility(&mut self) {}
}

#[derive(Clone)]
struct Events {
    window: iced::window::Id,
    receiver: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Message>>>,
}
impl Hash for Events {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.window.hash(state);
    }
}

fn events(source: &Events) -> impl iced::futures::Stream<Item = Message> + use<> {
    iced::futures::stream::unfold(source.receiver.clone(), |receiver| async move {
        let message = receiver.lock().await.recv().await?;
        Some((message, receiver))
    })
}

pub(super) struct PanelAccessibility {
    window: iced::window::Id,
    adapter: accesskit_unix::Adapter,
    published: Arc<Mutex<Published>>,
    events: Events,
    ids: BTreeMap<SemanticKey, NodeId>,
    next_id: u64,
    last: Option<(u64, Option<String>)>,
    bounds: Bounds,
}

impl PanelAccessibility {
    fn new(window: iced::window::Id) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(64);
        let published = Arc::new(Mutex::new(Published {
            tree: tree("Notifications", Vec::new(), Vec::new(), ROOT),
            actions: BTreeMap::new(),
            provider: String::new(),
            sliders: BTreeMap::new(),
        }));
        let handler = || Handler {
            published: published.clone(),
            sender: sender.clone(),
        };
        let adapter = accesskit_unix::Adapter::new(handler(), handler(), Deactivate);
        Self {
            window,
            adapter,
            published,
            events: Events {
                window,
                receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            },
            ids: BTreeMap::new(),
            next_id: 2,
            last: None,
            bounds: Bounds::new(),
        }
    }

    pub(super) fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::Subscription::run_with(self.events.clone(), events),
            iced::event::listen_raw(|event, _, window| match event {
                iced::Event::Window(iced::window::Event::RedrawRequested(_)) => {
                    Some(Message::CanopaAccessibilityFrame(window))
                }
                _ => None,
            }),
        ])
    }

    fn update(&mut self, snapshot: &CanopaSnapshot, provider: &str, focused: bool) {
        self.adapter.update_window_focus_state(focused);
        let identity = (
            snapshot.generation(),
            snapshot.focused_action_id().map(str::to_owned),
        );
        if self.last.as_ref() == Some(&identity) {
            return;
        }
        let mut builder = Builder {
            snapshot,
            provider,
            previous: std::mem::take(&mut self.ids),
            ids: BTreeMap::new(),
            next_id: &mut self.next_id,
            nodes: Vec::new(),
            actions: BTreeMap::new(),
            sliders: BTreeMap::new(),
            focus: ROOT,
        };
        let mut children = Vec::new();
        if let Some(error) = &snapshot.content.error {
            children.push(builder.label(&SemanticNode::Error, error));
        }
        for action in &snapshot.content.actions {
            children.push(builder.action(
                InteractionTarget::Content,
                action,
                ActionSemantics::Button,
            ));
        }
        for section in &snapshot.content.sections {
            let mut group_children = Vec::new();
            for control in &section.controls {
                let mut options = Vec::new();
                for option in &control.options {
                    let action = PanelAction {
                        request_activation_token: false,
                        key: control.key.clone(),
                        label: option.label.clone(),
                        value: Some(option.value.clone()),
                    };
                    options.push(builder.action(
                        InteractionTarget::Section {
                            id: section.id.clone(),
                        },
                        &action,
                        ActionSemantics::Radio {
                            selected: option.value == control.value,
                        },
                    ));
                }
                group_children.push(builder.group(
                    &SemanticNode::Control(section.id.clone(), control.key.clone()),
                    &control.label,
                    Role::RadioGroup,
                    options,
                ));
            }
            for item in &section.items {
                let mut item_children = Vec::new();
                if let Some(source) = &item.attribution {
                    item_children.push(
                        builder.label(&SemanticNode::Attribution(item.id.clone()), &source.label),
                    );
                }
                match selected_body(item) {
                    Body::Rich(body) => {
                        let mut pending = String::new();
                        let mut run_start = 0;
                        for (index, segment) in body.segments.iter().enumerate() {
                            let key = SemanticNode::Body(item.id.clone(), Some(index));
                            match segment {
                                canopa_panel_protocol::RichBodySegment::Text { text, .. } => {
                                    if pending.is_empty() {
                                        run_start = index;
                                    }
                                    pending.push_str(text);
                                }
                                canopa_panel_protocol::RichBodySegment::Link {
                                    action,
                                    uri,
                                    ..
                                } => {
                                    if !pending.is_empty() {
                                        item_children.push(builder.label(
                                            &SemanticNode::Body(item.id.clone(), Some(run_start)),
                                            &pending,
                                        ));
                                        pending.clear();
                                    }
                                    item_children.push(builder.action(
                                        InteractionTarget::Item {
                                            id: item.id.clone(),
                                        },
                                        action,
                                        ActionSemantics::Link { uri },
                                    ));
                                }
                                canopa_panel_protocol::RichBodySegment::Image { image } => {
                                    if !pending.is_empty() {
                                        item_children.push(builder.label(
                                            &SemanticNode::Body(item.id.clone(), Some(run_start)),
                                            &pending,
                                        ));
                                        pending.clear();
                                    }
                                    item_children.push(builder.group(
                                        &key,
                                        image.alt.as_deref().unwrap_or("Image"),
                                        Role::Image,
                                        Vec::new(),
                                    ));
                                }
                            }
                        }
                        if !pending.is_empty() {
                            item_children.push(builder.label(
                                &SemanticNode::Body(item.id.clone(), Some(run_start)),
                                &pending,
                            ));
                        }
                    }
                    Body::Styled(body) => {
                        let text: String = body.runs.iter().map(|run| run.text.as_str()).collect();
                        item_children
                            .push(builder.label(&SemanticNode::Body(item.id.clone(), None), &text));
                    }
                    Body::Plain(body) => {
                        item_children
                            .push(builder.label(&SemanticNode::Body(item.id.clone(), None), body));
                    }
                    Body::None => {}
                }
                for (index, metadata) in item.metadata.iter().enumerate() {
                    item_children.push(builder.label(
                        &SemanticNode::Metadata(item.id.clone(), index),
                        &super::canopa_semantics::metadata_label(metadata),
                    ));
                }
                if let Some(slider) = &item.slider {
                    item_children.push(builder.slider(&item.id, slider));
                }
                for action in &item.actions {
                    item_children.push(builder.action(
                        InteractionTarget::Item {
                            id: item.id.clone(),
                        },
                        action,
                        ActionSemantics::Button,
                    ));
                }
                group_children.push(builder.group(
                    &SemanticNode::Item(item.id.clone()),
                    &item.title,
                    Role::Group,
                    item_children,
                ));
            }
            children.push(builder.group(
                &SemanticNode::Section(section.id.clone()),
                &section.title,
                Role::Group,
                group_children,
            ));
        }
        self.ids = builder.ids;
        self.ids.insert(
            super::canopa_panel::semantic_key(snapshot, provider, &SemanticNode::Root),
            ROOT,
        );
        self.bounds.retain(|key, _| self.ids.contains_key(key));
        let mut update = tree(
            &snapshot.content.title,
            builder.nodes,
            children,
            builder.focus,
        );
        apply_bounds(&mut update, &self.ids, &self.bounds);
        *self.published.lock().unwrap() = Published {
            tree: update.clone(),
            actions: builder.actions,
            sliders: builder.sliders,
            provider: provider.to_owned(),
        };
        self.adapter.update_if_active(|| update);
        self.last = Some(identity);
    }

    fn measure(&self) -> iced::Task<Message> {
        let window = self.window;
        let generation = self.last.as_ref().map(|last| last.0);
        iced::advanced::widget::operate(Measure::new(self.ids.keys().cloned(), self.bounds.clone()))
            .map(move |bounds| Message::CanopaAccessibilityBounds {
                window,
                generation,
                bounds,
            })
    }

    fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
        let update = {
            let mut published = self.published.lock().unwrap();
            apply_bounds(&mut published.tree, &self.ids, &self.bounds);
            published.tree.clone()
        };
        self.adapter.update_if_active(|| update);
    }
}

fn apply_bounds(tree: &mut TreeUpdate, ids: &BTreeMap<SemanticKey, NodeId>, bounds: &Bounds) {
    for (key, bounds) in bounds {
        let Some(id) = ids.get(key) else {
            continue;
        };
        if let Some((_, node)) = tree.nodes.iter_mut().find(|(node_id, _)| node_id == id) {
            node.set_bounds(accesskit::Rect::new(
                bounds.x as f64,
                bounds.y as f64,
                (bounds.x + bounds.width) as f64,
                (bounds.y + bounds.height) as f64,
            ));
            if key.clips_children() {
                node.set_clips_children();
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ActionSemantics<'a> {
    Button,
    Radio { selected: bool },
    Link { uri: &'a str },
}

struct Builder<'a> {
    snapshot: &'a CanopaSnapshot,
    provider: &'a str,
    previous: BTreeMap<SemanticKey, NodeId>,
    ids: BTreeMap<SemanticKey, NodeId>,
    next_id: &'a mut u64,
    nodes: Vec<(NodeId, Node)>,
    actions: BTreeMap<NodeId, String>,
    sliders: BTreeMap<NodeId, canopa_panel_protocol::PanelSlider>,
    focus: NodeId,
}
impl Builder<'_> {
    fn slider(&mut self, item: &str, slider: &canopa_panel_protocol::PanelSlider) -> NodeId {
        let action_id = self.snapshot.slider_id(item, slider);
        let mut node = Node::new(Role::Slider);
        node.set_label(slider.label.clone());
        node.set_value(format!("{}{}", slider.value, slider.unit));
        node.set_numeric_value(f64::from(slider.value));
        node.set_min_numeric_value(f64::from(slider.min));
        node.set_max_numeric_value(f64::from(slider.max));
        node.set_numeric_value_step(f64::from(slider.step));
        node.set_orientation(accesskit::Orientation::Horizontal);
        for action in [
            Action::Focus,
            Action::SetValue,
            Action::Increment,
            Action::Decrement,
        ] {
            node.add_action(action);
        }
        let id = self.node(&SemanticNode::Action(action_id.clone()), node);
        if self.snapshot.focused_action_id() == Some(&action_id) {
            self.focus = id;
        }
        self.actions.insert(id, action_id);
        self.sliders.insert(id, slider.clone());
        id
    }
    fn node(&mut self, key: &SemanticNode, node: Node) -> NodeId {
        let key = super::canopa_panel::semantic_key(self.snapshot, self.provider, key);
        let id = self.previous.remove(&key).unwrap_or_else(|| {
            let id = NodeId(*self.next_id);
            *self.next_id = self
                .next_id
                .checked_add(1)
                .expect("accessibility IDs exhausted");
            id
        });
        self.ids.insert(key, id);
        self.nodes.push((id, node));
        id
    }
    fn label(&mut self, key: &SemanticNode, text: &str) -> NodeId {
        let mut node = Node::new(Role::Label);
        node.set_value(text);
        node.set_live(Live::Off);
        self.node(key, node)
    }
    fn group(
        &mut self,
        key: &SemanticNode,
        name: &str,
        role: Role,
        children: Vec<NodeId>,
    ) -> NodeId {
        let mut node = Node::new(role);
        node.set_label(name);
        node.set_live(Live::Off);
        node.set_children(children);
        self.node(key, node)
    }
    fn action(
        &mut self,
        target: InteractionTarget,
        action: &PanelAction,
        semantics: ActionSemantics<'_>,
    ) -> NodeId {
        let action_id = self.snapshot.action_id(target, action);
        let mut node = Node::new(match semantics {
            ActionSemantics::Button => Role::Button,
            ActionSemantics::Radio { .. } => Role::RadioButton,
            ActionSemantics::Link { .. } => Role::Link,
        });
        node.set_label(action.label.clone());
        node.set_live(Live::Off);
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        if let ActionSemantics::Link { uri } = semantics {
            node.set_url(uri);
        }
        if let ActionSemantics::Radio { selected } = semantics {
            node.set_toggled(if selected {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        let id = self.node(&SemanticNode::Action(action_id.clone()), node);
        if self.snapshot.focused_action_id() == Some(&action_id) {
            self.focus = id;
        }
        self.actions.insert(id, action_id);
        id
    }
}

fn tree(
    title: &str,
    mut nodes: Vec<(NodeId, Node)>,
    children: Vec<NodeId>,
    focus: NodeId,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_live(Live::Off);
    root.set_children(children);
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus,
    }
}

impl Drop for PanelAccessibility {
    fn drop(&mut self) {
        let mut published = self.published.lock().unwrap();
        published.actions.clear();
        published.sliders.clear();
    }
}

impl App {
    pub(super) fn measure_canopa_accessibility(
        &self,
        window: iced::window::Id,
    ) -> iced::Task<Message> {
        self.canopa_accessibility
            .as_ref()
            .filter(|bridge| bridge.window == window)
            .map(PanelAccessibility::measure)
            .unwrap_or_else(iced::Task::none)
    }

    pub(super) fn update_canopa_accessibility_bounds(
        &mut self,
        window: iced::window::Id,
        generation: Option<u64>,
        bounds: Bounds,
    ) {
        if let Some(bridge) = &mut self.canopa_accessibility
            && bridge.window == window
            && bridge.last.as_ref().map(|last| last.0) == generation
        {
            bridge.set_bounds(bounds);
        }
    }

    pub(super) fn sync_canopa_accessibility(&mut self) {
        let Some(window) = self.foreground.active_panel_window() else {
            self.canopa_accessibility = None;
            return;
        };
        let Some(tree) = self.foreground.active_panel_tree() else {
            self.canopa_accessibility = None;
            return;
        };
        let crate::surface::panel::PanelContent::ExternalProvider(content) = &tree.content else {
            self.canopa_accessibility = None;
            return;
        };
        let Some(snapshot) = content.document.canopa() else {
            self.canopa_accessibility = None;
            return;
        };
        if self
            .canopa_accessibility
            .as_ref()
            .is_none_or(|bridge| bridge.window != window)
        {
            self.canopa_accessibility = Some(PanelAccessibility::new(window));
        }
        self.canopa_accessibility.as_mut().unwrap().update(
            snapshot,
            &content.provider_name,
            self.accessibility_focused_window == Some(window),
        );
    }
}

#[cfg(test)]
mod tests;
