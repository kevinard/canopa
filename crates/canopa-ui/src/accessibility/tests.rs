use super::tree::{self, Ids};
use super::*;
use accesskit::{NodeId, Toggled};
use iced::widget::{button, column, scrollable, text, text_input};
use iced::{Element, Size, Theme};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Save,
    Name(String),
    Level(f64),
    Up,
    Down,
}

type View<'a> = Element<'a, Msg, Theme, ()>;

fn view<'a>(extra: bool, status: &'a str) -> View<'a> {
    let mut content = column![];
    if extra {
        content = content.push(semantic(
            Node::button("Inserted")
                .key("inserted")
                .on_click(Some(Msg::Save)),
            button(text("Inserted")).on_press(Msg::Save),
        ));
    }
    content = content
        .push(semantic(
            Node::heading("Profile", 2).key("heading"),
            text("Profile"),
        ))
        .push(text("Loose description"))
        .push(semantic(
            Node::button("Save changes")
                .key("save")
                .on_click(Some(Msg::Save)),
            button(text("Drawn caption")).on_press(Msg::Save),
        ))
        .push(semantic(
            Node::new(Role::Button)
                .key("absorbing")
                .on_click(Some(Msg::Save)),
            button(text("Absorbed caption")).on_press(Msg::Save),
        ))
        .push(semantic(
            Node::button("Unavailable")
                .key("off")
                .disabled(true)
                .on_click(Some(Msg::Save)),
            button(text("Unavailable")),
        ))
        .push(semantic(
            Node::text_input("Name", "Ann")
                .key("name")
                .labelled_by("heading")
                .controls("save")
                .on_text(Msg::Name),
            text_input("", "Ann").id("name-input").on_input(Msg::Name),
        ))
        .push(semantic(
            Node::slider(
                "Level",
                Range {
                    value: 40.0,
                    min: 0.0,
                    max: 100.0,
                    step: Some(5.0),
                },
            )
            .key("level")
            .on_increment(Some(Msg::Up))
            .on_decrement(Some(Msg::Down))
            .on_set_value(|value| match value {
                Value::Number(n) if (0.0..=100.0).contains(&n) => Some(Msg::Level(n)),
                _ => None,
            }),
            text("40"),
        ))
        .push(semantic(
            Node::check_box("Wrap", true).key("wrap"),
            text("Wrap"),
        ))
        .push(text("Speed"))
        .push(semantic(
            Node::new(Role::Slider).key("speed").range(Range {
                value: 1.0,
                min: 0.0,
                max: 2.0,
                step: None,
            }),
            text(""),
        ))
        .push(semantic(Node::status(status).key("status"), text("hidden")));
    semantic(Node::window("main", "Example window"), scrollable(content))
}

fn collect(view: View<'_>) -> Snapshot<Msg> {
    let renderer = ();
    let mut harness = Harness::new(view, &renderer, Size::new(400.0, 800.0));
    harness.collect()
}

fn node<'a>(built: &'a tree::Built<Msg>, ids: &Ids, key: &str) -> &'a accesskit::Node {
    let id = ids.get(key).unwrap_or_else(|| panic!("no node {key}"));
    &built
        .update
        .nodes
        .iter()
        .find(|(node, _)| *node == id)
        .unwrap()
        .1
}

#[test]
fn collected_view_becomes_a_named_tree_with_relations_and_states() {
    let snapshot = collect(view(false, "Ready"));
    assert_eq!(snapshot.roots().collect::<Vec<_>>(), ["main"]);
    let mut ids = Ids::default();
    let built = tree::build(&snapshot, "main", &mut ids, true, 1.0).unwrap();
    let root = node(&built, &ids, "main");
    assert_eq!(root.role(), Role::Window);
    assert_eq!(root.label(), Some("Example window"));
    assert_eq!(node(&built, &ids, "heading").level(), Some(2));
    // An explicit label wins over the drawn caption; without one the caption names it.
    assert_eq!(node(&built, &ids, "save").label(), Some("Save changes"));
    assert_eq!(
        node(&built, &ids, "absorbing").label(),
        Some("Absorbed caption")
    );
    // Loose text inside a container becomes a label child in reading order.
    let labels: Vec<_> = built
        .update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::Label)
        .filter_map(|(_, node)| node.value())
        .collect();
    assert!(labels.contains(&"Loose description"));
    assert!(!labels.contains(&"hidden"));
    let name = node(&built, &ids, "name");
    assert_eq!(name.value(), Some("Ann"));
    assert_eq!(name.labelled_by(), [ids.get("heading").unwrap()]);
    assert_eq!(name.controls(), [ids.get("save").unwrap()]);
    assert_eq!(name.children().len(), 1, "text run for AT-SPI Text");
    let level = node(&built, &ids, "level");
    assert_eq!(level.numeric_value(), Some(40.0));
    assert_eq!(level.numeric_value_step(), Some(5.0));
    assert_eq!(node(&built, &ids, "wrap").toggled(), Some(Toggled::True));
    // An unnamed slider takes the text drawn before it as name and label.
    let speed = node(&built, &ids, "speed");
    assert_eq!(speed.label(), Some("Speed"));
    assert_eq!(speed.labelled_by().len(), 1);
    let status = node(&built, &ids, "status");
    assert_eq!(status.live(), Some(Live::Polite));
    assert_eq!(status.label(), Some("Ready"));
    assert!(node(&built, &ids, "off").is_disabled());
    assert!(!built.actions.contains_key(&ids.get("off").unwrap()));
    assert!(node(&built, &ids, "save").supports_action(accesskit::Action::Click));
    assert!(level.supports_action(accesskit::Action::Increment));
}

#[test]
fn ids_stay_stable_and_focus_follows_keyboard_focus() {
    let renderer = ();
    let mut harness = Harness::new(view(false, "Ready"), &renderer, Size::new(400.0, 800.0));
    let mut ids = Ids::default();
    let first = tree::build(&harness.collect(), "main", &mut ids, true, 1.0).unwrap();
    let save = ids.get("save").unwrap();
    let root = ids.get("main").unwrap();
    assert_eq!(first.update.focus, root);
    harness.focus(iced::widget::Id::from("name-input"));
    let focused = tree::build(&harness.collect(), "main", &mut ids, true, 1.0).unwrap();
    assert_eq!(focused.update.focus, ids.get("name").unwrap());
    assert_eq!(focused.focus.as_deref(), Some("name"));
    // An unfocused window reports its root.
    let unfocused = tree::build(&harness.collect(), "main", &mut ids, false, 1.0).unwrap();
    assert_eq!(unfocused.update.focus, root);
    // Inserting a sibling keeps keyed nodes' IDs.
    harness.rebuild(view(true, "Saved"), Size::new(400.0, 800.0));
    let changed = tree::build(&harness.collect(), "main", &mut ids, true, 1.0).unwrap();
    assert_eq!(ids.get("save"), Some(save));
    assert_eq!(ids.get("main"), Some(root));
    assert!(ids.get("inserted").is_some());
    assert_eq!(node(&changed, &ids, "status").label(), Some("Saved"));
    harness.rebuild(view(false, "Saved"), Size::new(400.0, 800.0));
    tree::build(&harness.collect(), "main", &mut ids, true, 1.0).unwrap();
    assert_eq!(ids.get("inserted"), None, "retired keys free their node");
}

#[test]
fn actions_map_to_the_messages_input_sends() {
    let snapshot = collect(view(false, "Ready"));
    let mut ids = Ids::default();
    let built = tree::build(&snapshot, "main", &mut ids, true, 1.0).unwrap();
    let bound = |key: &str| &built.actions[&ids.get(key).unwrap()];
    assert_eq!(bound("save").actions.click, Some(Msg::Save));
    assert_eq!(bound("level").actions.increment, Some(Msg::Up));
    let set = bound("level").actions.set_value.clone().unwrap();
    assert_eq!(set(Value::Number(55.0)), Some(Msg::Level(55.0)));
    assert_eq!(set(Value::Number(500.0)), None, "refused like the control");
    let text = bound("name").actions.set_value.clone().unwrap();
    assert_eq!(text(Value::Text("Bo".into())), Some(Msg::Name("Bo".into())));
    // The first focusable widget inside a control is its focus target.
    assert_eq!(
        bound("name").focus,
        Some(iced::widget::Id::from("name-input"))
    );
}

#[test]
fn windows_are_selected_by_root_key_and_scale_applies_once() {
    let renderer = ();
    let both: View<'_> = column![
        semantic(Node::window("a", "First"), text("one")),
        semantic(Node::window("b", "Second"), text("two")),
    ]
    .into();
    let snapshot = Harness::new(both, &renderer, Size::new(100.0, 100.0)).collect();
    let mut ids = Ids::default();
    let built = tree::build(&snapshot, "b", &mut ids, true, 2.0).unwrap();
    let root = node(&built, &ids, "b");
    assert_eq!(root.label(), Some("Second"));
    assert!(root.transform().is_some());
    assert!(tree::build(&snapshot, "missing", &mut ids, true, 1.0).is_none());
    let reserved = ids.reserve("b");
    assert_eq!(Some(reserved), ids.get("b"));
    assert_ne!(reserved, NodeId(0));
}
