//! Polite screen-reader announcements outside any Shell surface, such as the
//! on-screen display's level or a move to another column. Each announcer is a
//! small AT-SPI application whose live status region AccessKit announces
//! whenever its text changes; nothing is sent while no assistive technology
//! listens.
#[cfg(not(test))]
use std::sync::{Arc, Mutex};

#[cfg(not(test))]
pub(super) struct Announcer {
    adapter: accesskit_unix::Adapter,
    tree: Arc<Mutex<accesskit::TreeUpdate>>,
    name: String,
}

#[cfg(not(test))]
mod announce {
    use accesskit::{ActionRequest, Live, Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};
    use std::sync::{Arc, Mutex};

    const ROOT: NodeId = NodeId(1);
    const STATUS: NodeId = NodeId(2);

    pub(super) fn tree(name: &str, text: &str) -> TreeUpdate {
        let mut root = Node::new(Role::Window);
        root.set_label(name);
        root.set_children(vec![STATUS]);
        let mut status = Node::new(Role::Status);
        status.set_label(text);
        status.set_live(Live::Polite);
        TreeUpdate {
            nodes: vec![(ROOT, root), (STATUS, status)],
            tree: Some(TreeInfo::new(ROOT)),
            tree_id: TreeId::ROOT,
            focus: ROOT,
        }
    }

    pub(super) struct Handler(pub(super) Arc<Mutex<TreeUpdate>>);
    impl accesskit::ActivationHandler for Handler {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            Some(self.0.lock().unwrap().clone())
        }
    }
    impl accesskit::ActionHandler for Handler {
        fn do_action(&mut self, _: ActionRequest) {}
    }
    pub(super) struct Deactivate;
    impl accesskit::DeactivationHandler for Deactivate {
        fn deactivate_accessibility(&mut self) {}
    }
}

#[cfg(not(test))]
impl Announcer {
    /// An announcer whose AT-SPI application is called `name`.
    pub(super) fn new(name: String) -> Self {
        let tree = Arc::new(Mutex::new(announce::tree(&name, "")));
        let adapter = accesskit_unix::Adapter::new(
            announce::Handler(tree.clone()),
            announce::Handler(tree.clone()),
            announce::Deactivate,
        );
        Self {
            adapter,
            tree,
            name,
        }
    }

    pub(super) fn announce(&mut self, text: &str) {
        let update = announce::tree(&self.name, text);
        *self.tree.lock().unwrap() = update.clone();
        self.adapter.update_if_active(|| update);
    }
}
