//! Native AT-SPI semantics and revision-owned actions for displayed banner cards.
use super::accessibility_bounds::{Bounds, Measure};
use super::{BannerInteraction, BannerTarget, BannerView};
use accesskit::{
    Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Live, Node,
    NodeId, Role, TreeId, TreeInfo, TreeUpdate,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, mpsc};

const ROOT: NodeId = NodeId(1);

struct Published {
    tree: TreeUpdate,
    actions: BTreeMap<NodeId, BannerInteraction>,
}

struct Handler {
    published: Arc<Mutex<Published>>,
    interactions: mpsc::Sender<BannerInteraction>,
    activation: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        if let Some(ready) = self.activation.lock().unwrap().take() {
            // Initial tree construction does not emit live-region announcements.
            // Register an empty window, then publish current cards as an update.
            let _ = ready.send(());
            Some(tree(Vec::new(), Vec::new()))
        } else {
            Some(self.published.lock().unwrap().tree.clone())
        }
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        if request.action != Action::Click || request.target_tree != TreeId::ROOT {
            return;
        }
        let action = self
            .published
            .lock()
            .unwrap()
            .actions
            .get(&request.target_node)
            .cloned();
        if let Some(action) = action {
            // Runtime validation still rejects a queued click after replacement/closure.
            let _ = self.interactions.send(action);
        }
    }
}

struct Deactivate;
impl DeactivationHandler for Deactivate {
    fn deactivate_accessibility(&mut self) {}
}

pub(super) struct BannerAccessibility {
    adapter: accesskit_unix::Adapter,
    published: Arc<Mutex<Published>>,
    ids: BTreeMap<(BannerTarget, usize), NodeId>,
    next_id: u64,
    bounds: Bounds,
    activation: Option<tokio::sync::oneshot::Receiver<()>>,
}

impl BannerAccessibility {
    pub(super) fn new(interactions: mpsc::Sender<BannerInteraction>) -> Self {
        let published = Arc::new(Mutex::new(Published {
            tree: tree(Vec::new(), Vec::new()),
            actions: BTreeMap::new(),
        }));
        let (ready, activation) = tokio::sync::oneshot::channel();
        let ready = Arc::new(Mutex::new(Some(ready)));
        let handler = || Handler {
            published: published.clone(),
            interactions: interactions.clone(),
            activation: ready.clone(),
        };
        let adapter = accesskit_unix::Adapter::new(handler(), handler(), Deactivate);
        Self {
            adapter,
            published,
            ids: BTreeMap::new(),
            next_id: 2,
            bounds: Bounds::new(),
            activation: Some(activation),
        }
    }

    pub(super) fn activation(&mut self) -> tokio::sync::oneshot::Receiver<()> {
        self.activation
            .take()
            .expect("activation receiver already taken")
    }

    pub(super) fn publish_after_activation(&mut self) {
        let published = self.published.clone();
        self.adapter
            .update_if_active(|| published.lock().unwrap().tree.clone());
    }

    pub(super) fn update(&mut self, banners: &[BannerView]) {
        let mut nodes = Vec::new();
        let mut groups = Vec::new();
        let mut actions = BTreeMap::new();
        let mut previous = std::mem::take(&mut self.ids);
        let mut id_for = |target, slot| {
            let key = (target, slot);
            let id = previous.remove(&key).unwrap_or_else(|| {
                let id = NodeId(self.next_id);
                self.next_id = self
                    .next_id
                    .checked_add(1)
                    .expect("accessibility IDs exhausted");
                id
            });
            self.ids.insert(key, id);
            id
        };
        for banner in banners {
            let target = banner.target();
            let group_id = id_for(target, 0);
            groups.push(group_id);
            let mut group = Node::new(Role::Alert);
            // One polite announcement per new revision, including image alternative text.
            group.set_label(format!(
                "{}: {}. {}",
                banner.app_name, banner.summary, banner.body
            ));
            group.set_live(Live::Polite);
            group.set_live_atomic();
            let mut children = Vec::new();
            let body_id = id_for(target, 1);
            let mut body = Node::new(Role::Label);
            body.set_value(banner.body.clone());
            body.set_live(Live::Off);
            children.push(body_id);
            nodes.push((body_id, body));
            let mut add_action = |slot, label: String, role, action| {
                let id = id_for(target, slot);
                let mut node = Node::new(role);
                node.set_label(label);
                node.set_live(Live::Off);
                node.add_action(Action::Click);
                if let BannerInteraction::OpenLink { segment_index, .. } = &action
                    && let Some(crate::NotificationBodySegment::Link { uri, .. }) =
                        banner.body_content.segments.get(*segment_index)
                {
                    node.set_url(uri.as_str());
                }
                children.push(id);
                nodes.push((id, node));
                actions.insert(id, action);
            };
            for segment in &banner.body_segments {
                if let super::BannerBodySegment::Link {
                    text,
                    segment_index,
                    ..
                } = segment
                {
                    add_action(
                        super::accessibility_bounds::link_slot(*segment_index),
                        text.clone(),
                        Role::Link,
                        BannerInteraction::OpenLink {
                            target,
                            segment_index: *segment_index,
                            activation_token: None,
                        },
                    );
                }
            }
            add_action(
                2,
                "Dismiss notification".into(),
                Role::Button,
                BannerInteraction::Dismiss(target),
            );
            if banner.default_action_key.is_some() {
                add_action(
                    3,
                    "Open notification".into(),
                    Role::Button,
                    BannerInteraction::Activate {
                        target,
                        activation_token: None,
                    },
                );
            }
            for (index, action) in banner.actions.iter().enumerate() {
                add_action(
                    index + 4,
                    action.label.clone(),
                    Role::Button,
                    BannerInteraction::InvokeAction {
                        target,
                        action_key: action.key.clone(),
                        activation_token: None,
                    },
                );
            }
            group.set_children(children);
            nodes.push((group_id, group));
        }
        let mut update = tree(nodes, groups);
        self.bounds.retain(|key, _| self.ids.contains_key(key));
        apply_bounds(&mut update, &self.ids, &self.bounds);
        *self.published.lock().unwrap() = Published {
            tree: update.clone(),
            actions,
        };
        // Do not hold our mutex while the adapter invokes callbacks on its own thread.
        self.adapter.update_if_active(|| update);
    }

    pub(super) fn measure(
        &self,
        window: iced::window::Id,
        generation: u64,
    ) -> iced::Task<super::BannerUiMessage> {
        iced::advanced::widget::operate(Measure::new(self.ids.keys().copied(), self.bounds.clone()))
            .map(move |bounds| super::BannerUiMessage::AccessibilityBounds {
                window,
                generation,
                bounds,
            })
    }

    pub(super) fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
        let update = {
            let mut published = self.published.lock().unwrap();
            apply_bounds(&mut published.tree, &self.ids, &self.bounds);
            published.tree.clone()
        };
        self.adapter.update_if_active(|| update);
    }
}

fn apply_bounds(
    tree: &mut TreeUpdate,
    ids: &BTreeMap<(BannerTarget, usize), NodeId>,
    bounds: &Bounds,
) {
    let mut root: Option<iced::Rectangle> = None;
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
            if key.1 <= 1 {
                node.set_clips_children();
            }
        }
        if key.1 == 0 {
            root = Some(root.map_or(*bounds, |root| root.union(bounds)));
        }
    }
    if let Some(bounds) = root
        && let Some((_, node)) = tree.nodes.iter_mut().find(|(id, _)| *id == ROOT)
    {
        node.set_bounds(accesskit::Rect::new(
            bounds.x as f64,
            bounds.y as f64,
            (bounds.x + bounds.width) as f64,
            (bounds.y + bounds.height) as f64,
        ));
    }
}

fn tree(mut nodes: Vec<(NodeId, Node)>, groups: Vec<NodeId>) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label("Canopa Notify notifications");
    root.set_children(groups);
    root.set_live(Live::Off);
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus: ROOT,
    }
}

impl Drop for BannerAccessibility {
    fn drop(&mut self) {
        self.published.lock().unwrap().actions.clear();
    }
}
