//! Native AT-SPI presentation. Hidden responses never enter a published tree.
use crate::{secret::Secret, view::SessionView};
use accesskit::{
    Action, ActionData, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Live,
    Node, NodeId, Role, TextPosition, TextSelection, TreeId, TreeInfo, TreeUpdate,
};
use std::sync::{Arc, Mutex};
use unicode_segmentation::UnicodeSegmentation;
const ROOT: NodeId = NodeId(1);
const INPUT: NodeId = NodeId(2);
const TEXT: NodeId = NodeId(5);

pub enum Operation {
    Control(usize),
    Submit,
    Cancel,
    Replace(Secret),
    Select {
        anchor: usize,
        focus: usize,
        echo: bool,
    },
}
pub struct Request {
    pub revision: u64,
    pub operation: Operation,
}
#[derive(Default, PartialEq, Eq)]
struct Meaning {
    epoch: (u64, u64),
    context: Vec<(String, String)>,
    prompt: Option<(u64, String, bool)>,
    hidden: bool,
    busy: bool,
    controls: Vec<(usize, String, bool, bool)>,
    layout: Option<String>,
}
struct Published {
    tree: TreeUpdate,
    revision: u64,
    meaning: Meaning,
}
struct Handler {
    published: Arc<Mutex<Published>>,
    send: Arc<dyn Fn(Request) + Send + Sync>,
}
impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.published.lock().unwrap().tree.clone())
    }
}
impl ActionHandler for Handler {
    fn do_action(&mut self, mut request: ActionRequest) {
        // Even rejected/unrecognized incoming values are erased on every return path.
        let value = if matches!(&request.data, Some(ActionData::Value(_))) {
            let Some(ActionData::Value(value)) = request.data.take() else {
                unreachable!()
            };
            Some(zeroize::Zeroizing::new(value))
        } else {
            None
        };
        if request.target_tree != TreeId::ROOT {
            return;
        }
        let state = self.published.lock().unwrap();
        let input = state
            .meaning
            .prompt
            .as_ref()
            .filter(|_| !state.meaning.hidden);
        let operation = match request.action {
            Action::SetValue if request.target_node == INPUT && input.is_some() => {
                let Some(value) = value else {
                    return;
                };
                let mut secret = Secret::with_capacity(8193);
                if value.len() < 8193 {
                    secret.push(&value);
                }
                let valid = value.len() < 8193 && !value.chars().any(char::is_control);
                if !valid {
                    return;
                }
                Operation::Replace(secret)
            }
            Action::SetTextSelection if request.target_node == INPUT && input.is_some() => {
                let Some(ActionData::SetTextSelection(s)) = request.data else {
                    return;
                };
                if s.anchor.node != TEXT || s.focus.node != TEXT {
                    return;
                }
                Operation::Select {
                    anchor: s.anchor.character_index,
                    focus: s.focus.character_index,
                    echo: input.unwrap().2,
                }
            }
            Action::Click => match request.target_node.0 {
                6 => Operation::Submit,
                7 => Operation::Cancel,
                n if n >= 10 => {
                    let Some((id, _, _, enabled)) = state.meaning.controls.get((n - 10) as usize)
                    else {
                        return;
                    };
                    if !enabled {
                        return;
                    }
                    Operation::Control(*id)
                }
                _ => return,
            },
            _ => return,
        };
        let revision = state.revision;
        drop(state);
        (self.send)(Request {
            revision,
            operation,
        });
    }
}
struct Deactivate;
impl DeactivationHandler for Deactivate {
    fn deactivate_accessibility(&mut self) {}
}
pub struct Accessibility {
    adapter: accesskit_unix::Adapter,
    published: Arc<Mutex<Published>>,
}
impl Accessibility {
    pub fn new(title: &str, send: impl Fn(Request) + Send + Sync + 'static) -> Self {
        let published = Arc::new(Mutex::new(Published {
            tree: tree(title, Vec::new(), Vec::new(), ROOT),
            revision: 0,
            meaning: Meaning::default(),
        }));
        let send: Arc<dyn Fn(Request) + Send + Sync> = Arc::new(send);
        let handler = || Handler {
            published: published.clone(),
            send: send.clone(),
        };
        Self {
            adapter: accesskit_unix::Adapter::new(handler(), handler(), Deactivate),
            published,
        }
    }
    pub fn accepts(&self, request: &Request) -> bool {
        self.published.lock().unwrap().revision == request.revision
    }
    pub fn update(
        &mut self,
        title: &str,
        view: &SessionView,
        settings: &crate::settings::Settings,
        focused: bool,
    ) {
        let visible = |name: &str| {
            settings.widgets.iter().any(|n| n == name)
                // One accessibility tree represents every lock surface. Do not
                // read out a widget the user chose to hide on any monitor.
                && !settings.hidden_widgets.values().flatten().any(|n| n == name)
                && settings
                    .layout
                    .regions
                    .iter()
                    .any(|r| r.widgets.iter().any(|n| n == name))
        };
        let mut meaning = Meaning {
            epoch: view.interaction_epoch,
            context: view
                .custom
                .iter()
                .filter(|(name, _)| visible(name))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            prompt: view.prompt.as_ref().map(|p| (p.id, p.text.clone(), p.echo)),
            hidden: view.hide_input,
            busy: view.busy,
            controls: view
                .controls
                .iter()
                .map(|c| (c.id, c.label.clone(), c.selected, c.enabled))
                .collect(),
            layout: None,
        };
        if view.can_reveal() {
            meaning.controls.push((
                crate::input::REVEAL_CONTROL,
                if view.revealed() {
                    fl!("session-hide-response")
                } else {
                    fl!("session-show-response")
                },
                false,
                true,
            ));
        }
        if view.can_select_layout {
            meaning.controls.push((
                crate::input::LAYOUT_CONTROL,
                fl!("session-keyboard-layout", layout = view.layout.as_str()),
                false,
                true,
            ));
            meaning.layout = Some(view.layout.clone());
        }
        let mut nodes = Vec::new();
        let mut children = Vec::new();
        let input = meaning.prompt.as_ref().filter(|_| !meaning.hidden);
        if let Some((_, prompt, echo)) = input {
            let mut node = Node::new(if *echo {
                Role::TextInput
            } else {
                Role::PasswordInput
            });
            node.set_label(prompt.clone());
            node.set_description(if view.caps {
                fl!(
                    "session-input-description-caps",
                    layout = view.layout.as_str()
                )
            } else {
                fl!("session-input-description", layout = view.layout.as_str())
            });
            node.add_action(Action::SetValue);
            node.add_action(Action::SetTextSelection);
            node.set_children([TEXT]);
            let value = if *echo {
                view.secret.text().to_owned()
            } else {
                "•".repeat(view.secret.grapheme_count())
            };
            let mut text = Node::new(Role::TextRun);
            text.set_value(value.clone());
            // Password runs expose grapheme-count bullets only. Visible input uses
            // scalar boundaries; the controller still validates grapheme selection.
            text.set_character_lengths(
                value
                    .chars()
                    .map(|c| c.len_utf8() as u8)
                    .collect::<Vec<_>>(),
            );
            let index = |byte| {
                if *echo {
                    view.secret.text()[..byte].chars().count()
                } else {
                    view.secret.text()[..byte].graphemes(true).count()
                }
            };
            node.set_text_selection(TextSelection {
                anchor: TextPosition {
                    node: TEXT,
                    character_index: index(view.secret.anchor()),
                },
                focus: TextPosition {
                    node: TEXT,
                    character_index: index(view.secret.cursor()),
                },
            });
            children.push(INPUT);
            nodes.push((INPUT, node));
            nodes.push((TEXT, text));
        }
        for (index, (_, value)) in view
            .custom
            .iter()
            .filter(|(name, _)| visible(name))
            .enumerate()
        {
            let id = NodeId(1000 + index as u64);
            let mut node = Node::new(Role::Label);
            node.set_value(value.clone());
            children.push(id);
            nodes.push((id, node));
        }
        let mut status = Node::new(Role::Status);
        status.set_label(if view.failed {
            fl!("session-status-error", message = view.message.as_str())
        } else {
            view.message.clone()
        });
        status.set_live(Live::Polite);
        status.set_live_atomic();
        children.push(NodeId(4));
        nodes.push((NodeId(4), status));
        let mut user = Node::new(Role::Label);
        user.set_value(view.username.clone());
        if visible("username") {
            children.push(NodeId(3));
            nodes.push((NodeId(3), user));
        }
        let (submit, cancel) = view
            .action_labels
            .clone()
            .unwrap_or_else(|| (fl!("session-action-submit"), fl!("session-action-cancel")));
        for (id, label) in [(6, submit), (7, cancel)] {
            let mut node = Node::new(Role::Button);
            node.set_label(label);
            node.add_action(Action::Click);
            children.push(NodeId(id));
            nodes.push((NodeId(id), node));
        }
        for (i, (control, label, selected, enabled)) in meaning.controls.iter().enumerate() {
            let id = NodeId(10 + i as u64);
            let mut node = Node::new(Role::Button);
            node.set_label(label.clone());
            if *selected {
                node.set_selected(true);
            }
            if let Some(panel) = &view.authentication_panel {
                match (
                    panel.checked_control == Some(*control),
                    panel.choice_details.get(control),
                ) {
                    (true, Some(detail)) => node.set_description(fl!(
                        "session-current-selection-detail",
                        detail = detail.as_str()
                    )),
                    (true, None) => node.set_description(fl!("session-current-selection")),
                    (false, Some(detail)) => node.set_description(detail.clone()),
                    (false, None) => {}
                }
            }
            if *enabled {
                node.add_action(Action::Click);
            } else {
                node.set_disabled();
            }
            children.push(id);
            nodes.push((id, node));
        }
        let update = tree(
            title,
            nodes,
            children,
            if input.is_some() && focused {
                INPUT
            } else {
                ROOT
            },
        );
        let mut state = self.published.lock().unwrap();
        if state.meaning != meaning {
            state.revision = state
                .revision
                .checked_add(1)
                .expect("accessibility revision exhausted");
        }
        state.meaning = meaning;
        state.tree = update.clone();
        drop(state);
        self.adapter.update_window_focus_state(focused);
        self.adapter.update_if_active(|| update);
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
    root.set_children(children);
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus,
    }
}
/// Character indices received from AT-SPI are validated against the live input.
pub fn select(secret: &mut Secret, anchor: usize, focus: usize, echo: bool) {
    let byte = |n| {
        if echo {
            secret
                .text()
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(secret.text().len()))
                .nth(n)
        } else {
            secret
                .text()
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(secret.text().len()))
                .nth(n)
        }
    };
    if let (Some(anchor), Some(focus)) = (byte(anchor), byte(focus)) {
        secret.set_selection(anchor, focus);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_controls_are_published_without_an_action_and_reject_direct_clicks() {
        let settings = crate::settings::Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let mut view = SessionView::preview(&settings, "Alice".into()).unwrap();
        let mut control = crate::view::Control::new(42, "Apply");
        control.enabled = false;
        view.controls.push(control);
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut a = Accessibility::new("Test", |_| {});
        a.update("Test", &view, &settings, true);
        let mut handler = Handler {
            published: a.published.clone(),
            send: Arc::new(move |request| {
                sender.send(request).unwrap();
            }),
        };
        let click = || ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(10),
            data: None,
        };
        handler.do_action(click());
        assert!(receiver.try_recv().is_err());
        {
            let state = a.published.lock().unwrap();
            let node = &state
                .tree
                .nodes
                .iter()
                .find(|(id, _)| *id == NodeId(10))
                .unwrap()
                .1;
            assert!(node.is_disabled());
            assert!(!node.supports_action(Action::Click));
        }
        view.controls[0].enabled = true;
        a.update("Test", &view, &settings, true);
        handler.do_action(click());
        assert!(matches!(
            receiver.try_recv().unwrap().operation,
            Operation::Control(42)
        ));
    }
    #[test]
    fn protected_tree_contains_no_response_and_old_actions_expire() {
        let settings = crate::settings::Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let mut view = SessionView::preview(&settings, "Alice".into()).unwrap();
        view.secret.push("unique-private-response");
        let mut a = Accessibility::new("Test", |_| {});
        a.update("Test", &view, &settings, true);
        let state = a.published.lock().unwrap();
        let dump = format!("{:?}", state.tree);
        assert!(!dump.contains("unique-private-response"));
        assert!(dump.contains("PasswordInput"));
        let revision = state.revision;
        drop(state);
        view.prompt.as_mut().unwrap().id += 1;
        a.update("Test", &view, &settings, true);
        assert!(!a.accepts(&Request {
            revision,
            operation: Operation::Submit
        }));
    }
}
