//! Drafts for the Shell and lockscreen namespaces. Their owners validate every save.
use anyhow::{Context, Result, ensure};
use canopa_config::{Component, Document, Format};
use canopa_session_ui::{
    layout::{Anchor, Layout, Region},
    settings::{PASSIVE_WIDGETS, Settings},
};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::collections::BTreeSet;

use crate::{
    preferences::{block, save_common},
    storage::Snapshot,
};

const SHELL_DEFAULTS: &str = include_str!("../../canopa-shell/config/default.kdl");
const SYNTHESIZED: &[&str] = &[
    "battery",
    "clock",
    "focused-window",
    "idle-inhibitor",
    "media",
    "minimap",
    "network",
    "notifications",
    "tray",
    "volume",
    "workspaces",
];
pub(crate) const ZONES: [&str; 3] = ["start", "center", "end"];
pub(crate) const EDGES: [&str; 4] = ["left", "right", "top", "bottom"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    All,
    Include,
    Exclude,
}

impl Target {
    pub(crate) const ALL: [Self; 3] = [Self::All, Self::Include, Self::Exclude];
    pub(crate) fn label(&self) -> String {
        match self {
            Self::All => fl!("settings-widgets-all-monitors"),
            Self::Include => fl!("settings-widgets-selected-monitors"),
            Self::Exclude => fl!("settings-widgets-except-monitors"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Bar {
    pub(crate) name: String,
    pub(crate) edge: String,
    pub(crate) zones: [Vec<String>; 3],
    pub(crate) target: Target,
    pub(crate) outputs: BTreeSet<String>,
    node: KdlNode,
    changed: bool,
}

impl Bar {
    fn load(node: KdlNode) -> Result<Self> {
        let children = node.children();
        let names = strings(children.and_then(|doc| doc.get("output")))?;
        let (target, outputs) = match names.first().map(String::as_str) {
            None | Some("all") => (Target::All, BTreeSet::new()),
            Some(first) if first.starts_with('!') => (
                Target::Exclude,
                names.into_iter().map(|name| name[1..].to_owned()).collect(),
            ),
            Some(_) => (Target::Include, names.into_iter().collect()),
        };
        let edge = children
            .and_then(|doc| doc.get("edge"))
            .and_then(|node| node.get(0))
            .and_then(kdl::KdlValue::as_string)
            .unwrap_or("left");
        ensure!(EDGES.contains(&edge), "unknown bar edge");
        let modules = children
            .and_then(|doc| doc.get("modules"))
            .and_then(KdlNode::children);
        let mut zones = [Vec::new(), Vec::new(), Vec::new()];
        for (index, zone) in ZONES.iter().enumerate() {
            zones[index] = strings(modules.and_then(|doc| doc.get(zone)))?;
        }
        Ok(Self {
            name: name(&node)?.into(),
            edge: edge.into(),
            target,
            outputs,
            zones,
            node,
            changed: false,
        })
    }

    pub(crate) fn place(&mut self, widget: &str, zone: Option<usize>) {
        self.place_before(widget, zone, None);
    }

    /// Move `widget` into `zone` just before `before`, or to the zone's end
    /// when `before` is not in it. `None` removes the widget from the bar.
    pub(crate) fn place_before(&mut self, widget: &str, zone: Option<usize>, before: Option<&str>) {
        for widgets in &mut self.zones {
            widgets.retain(|name| name != widget);
        }
        if let Some(index) = zone {
            insert_before(&mut self.zones[index], widget, before);
        }
        self.changed = true;
    }

    pub(crate) fn change(&mut self) {
        self.changed = true;
    }

    pub(crate) fn visible_on(&self, output: &str) -> bool {
        match self.target {
            Target::All => true,
            Target::Include => self.outputs.contains(output),
            Target::Exclude => !self.outputs.contains(output),
        }
    }

    fn configured(&self) -> Result<KdlNode> {
        let mut node = self.node.clone();
        if name(&node)? != self.name {
            node.insert(0, self.name.clone());
        }
        if node.children().is_none() {
            node.set_children(KdlDocument::new());
        }
        let doc = node.children_mut().as_mut().unwrap();
        if self.edge != "left" || doc.get("edge").is_some() {
            set_strings(doc, "edge", std::slice::from_ref(&self.edge))?;
        }
        let output = match self.target {
            Target::All => vec!["all".into()],
            Target::Include => self.outputs.iter().cloned().collect(),
            Target::Exclude => self.outputs.iter().map(|name| format!("!{name}")).collect(),
        };
        ensure!(
            !output.is_empty(),
            "{}",
            fl!("settings-error-bar-no-monitor", bar = self.name.as_str())
        );
        let existing = strings(doc.get("output"))?;
        if (self.target != Target::All || doc.get("output").is_some())
            && existing.iter().collect::<BTreeSet<_>>() != output.iter().collect::<BTreeSet<_>>()
        {
            set_strings(doc, "output", &output)?;
        }
        if doc.get("modules").is_some() || self.zones.iter().any(|widgets| !widgets.is_empty()) {
            let modules = block(doc, "modules")?;
            for (zone, widgets) in ZONES.iter().zip(&self.zones) {
                if !widgets.is_empty() || modules.get(zone).is_some() {
                    set_strings(modules, zone, widgets)?;
                }
            }
        }
        Ok(node)
    }
}

/// Insert `widget` before `before` in `widgets`, or at the end.
fn insert_before(widgets: &mut Vec<String>, widget: &str, before: Option<&str>) {
    let index = before
        .and_then(|before| widgets.iter().position(|name| name == before))
        .unwrap_or(widgets.len());
    widgets.insert(index, widget.into());
}

#[derive(Clone, Debug)]
pub(crate) struct Editor {
    pub(crate) source: Snapshot,
    pub(crate) bars: Vec<Bar>,
    pub(crate) modules: BTreeSet<String>,
    pub(crate) lock: Settings,
    lock_changed: bool,
    changed_visibility: BTreeSet<String>,
    shell_defaults: bool,
    virtual_main: bool,
    removed_bars: BTreeSet<String>,
}

impl Editor {
    pub(crate) fn load(source: Snapshot) -> Result<Self> {
        let shell = Document::parse(
            source_text(&source),
            Some(source.path.clone()),
            Format::CanopaV1,
            Component::Shell,
            SHELL_DEFAULTS,
        )?;
        let mut bars = shell
            .selected()
            .nodes()
            .iter()
            .filter(|node| node.name().value() == "bar")
            .cloned()
            .map(Bar::load)
            .collect::<Result<Vec<_>>>()?;
        let virtual_main = bars.is_empty();
        if virtual_main {
            bars.push(Bar::load("bar \"main\"\n".parse()?)?);
        }
        ensure!(
            bars.iter()
                .map(|bar| &bar.name)
                .collect::<BTreeSet<_>>()
                .len()
                == bars.len(),
            "duplicate bar names"
        );
        let mut modules = SYNTHESIZED
            .iter()
            .map(|name| name.to_string())
            .collect::<BTreeSet<_>>();
        if let Some(declarations) = shell.selected().get("modules").and_then(KdlNode::children) {
            for declaration in declarations.nodes() {
                modules.insert(name(declaration)?.to_owned());
            }
        }
        let lock = Document::parse(
            source_text(&source),
            Some(source.path.clone()),
            Format::CanopaV1,
            Component::Lockscreen,
            "",
        )?;
        let lock = Settings::parse(
            lock.selected(),
            lock.appearance(),
            Some(&source.path),
            Component::Lockscreen,
        )?;
        Ok(Self {
            source,
            bars,
            modules,
            lock,
            lock_changed: false,
            changed_visibility: BTreeSet::new(),
            shell_defaults: shell.info().defaults,
            virtual_main,
            removed_bars: BTreeSet::new(),
        })
    }

    pub(crate) fn dirty(&self) -> bool {
        self.lock_changed || !self.changed_visibility.is_empty() || self.shell_changed()
    }

    fn shell_changed(&self) -> bool {
        !self.removed_bars.is_empty() || self.bars.iter().any(|bar| bar.changed)
    }

    pub(crate) fn duplicate_bar(&mut self, index: usize, name: &str) -> Result<usize> {
        ensure!(self.bars.len() < 32, "{}", fl!("settings-error-bar-limit"));
        ensure!(
            !name.is_empty() && name.len() <= 80 && !name.chars().any(char::is_control),
            "{}",
            fl!("settings-error-bar-name")
        );
        ensure!(
            !self.bars.iter().any(|bar| bar.name == name),
            "{}",
            fl!("settings-error-bar-exists")
        );
        let mut bar = self.bars[index].clone();
        bar.name = name.into();
        bar.target = Target::Include;
        bar.outputs.clear();
        bar.changed = true;
        self.bars.push(bar);
        Ok(self.bars.len() - 1)
    }

    pub(crate) fn remove_bar(&mut self, index: usize) {
        if self.bars.len() > 1 {
            self.removed_bars.insert(self.bars.remove(index).name);
        }
    }

    pub(crate) fn lock_catalog(&self) -> Vec<String> {
        ["clock", "date"]
            .into_iter()
            .chain(PASSIVE_WIDGETS.iter().copied())
            .map(str::to_owned)
            .chain(self.lock.custom.keys().cloned())
            .collect()
    }

    pub(crate) fn lock_active(&self, name: &str) -> bool {
        match name {
            "authentication" => true,
            "clock" | "date" => self
                .lock
                .layout
                .regions
                .iter()
                .any(|region| region.widgets.iter().any(|value| value == name)),
            _ => self.lock.widgets.iter().any(|value| value == name),
        }
    }

    pub(crate) fn place_lock(&mut self, widget: &str, anchor: Option<Anchor>) {
        self.place_lock_before(widget, anchor, None);
    }

    /// Move `widget` into the region at `anchor` just before `before`, or to
    /// the region's end. `None` takes it off the lock screen.
    pub(crate) fn place_lock_before(
        &mut self,
        widget: &str,
        anchor: Option<Anchor>,
        before: Option<&str>,
    ) {
        if widget == "authentication" && anchor.is_none() {
            return;
        }
        for region in &mut self.lock.layout.regions {
            region.widgets.retain(|name| name != widget);
        }
        if let Some(anchor) = anchor {
            insert_before(&mut self.region(anchor).widgets, widget, before);
            if !matches!(widget, "clock" | "date" | "authentication")
                && !self.lock.widgets.iter().any(|value| value == widget)
            {
                self.lock.widgets.push(widget.into());
            }
        } else {
            self.lock.widgets.retain(|name| name != widget);
        }
        self.lock_changed = true;
    }

    pub(crate) fn region(&mut self, anchor: Anchor) -> &mut Region {
        if let Some(index) = self
            .lock
            .layout
            .regions
            .iter()
            .position(|region| region.anchor == anchor)
        {
            return &mut self.lock.layout.regions[index];
        }
        self.lock.layout.regions.push(Region {
            anchor,
            widgets: vec![],
            row: false,
            x: 0.,
            y: 0.,
        });
        self.lock.layout.regions.last_mut().unwrap()
    }

    pub(crate) fn change_lock(&mut self) {
        self.lock_changed = true;
    }

    pub(crate) fn hide_lock_widget(&mut self, output: &str, name: &str, hide: bool) {
        if output.is_empty() || name == "authentication" {
            return;
        }
        let hidden = self
            .lock
            .hidden_widgets
            .entry(output.to_owned())
            .or_default();
        hidden.retain(|value| value != name);
        if hide {
            hidden.push(name.to_owned());
        }
        self.changed_visibility.insert(output.to_owned());
    }

    pub(crate) fn save(&self) -> Result<Snapshot> {
        save_common(&self.source, &self.candidate()?)
    }

    /// Keep pending edits on top of `saved`, written by Personalization from
    /// `previous`. External edits, or a save that touched the widget layout,
    /// still require a reload.
    pub(crate) fn rebase(&mut self, previous: &Snapshot, saved: &Snapshot) -> Result<()> {
        ensure!(
            self.source.same(previous)
                && crate::preferences::common_sections(&previous.text)?.1
                    == crate::preferences::common_sections(&saved.text)?.1,
            "{}",
            fl!("settings-error-widgets-changed")
        );
        self.source = saved.clone();
        Ok(())
    }

    pub(crate) fn candidate(&self) -> Result<String> {
        let mut document: KdlDocument = source_text(&self.source).parse()?;
        if self.shell_changed() {
            let shell = block(&mut document, "shell")?;
            if self.shell_defaults {
                *shell = SHELL_DEFAULTS.parse()?;
            }
            shell.nodes_mut().retain(|node| {
                node.name().value() != "bar"
                    || name(node).is_ok_and(|name| !self.removed_bars.contains(name))
            });
            for bar in self
                .bars
                .iter()
                .filter(|bar| bar.changed || self.virtual_main)
            {
                let updated = bar.configured()?;
                if let Some(node) = named_node(shell, "bar", &bar.name) {
                    *node = updated;
                } else {
                    shell.nodes_mut().push(updated);
                }
            }
            if self.shell_defaults {
                document.get_mut("shell").unwrap().autoformat();
            }
        }
        if self.lock_changed || !self.changed_visibility.is_empty() {
            let new_lock = document.get("lockscreen").is_none();
            let lock = block(&mut document, "lockscreen")?;
            if self.lock_changed {
                self.lock
                    .layout
                    .validate(&self.lock.widgets, &self.lock.custom)?;
                set_strings(lock, "widgets", &self.lock.widgets)?;
                write_regions(block(lock, "layout")?, &self.lock.layout)?;
            }
            for output in &self.changed_visibility {
                write_visibility(lock, output, self.lock.hidden_on(output))?;
            }
            if new_lock {
                document.get_mut("lockscreen").unwrap().autoformat();
            }
        }
        Ok(document.to_string())
    }
}

fn write_visibility(doc: &mut KdlDocument, output: &str, hidden: &[String]) -> Result<()> {
    if named_node(doc, "output", output).is_none() {
        let mut node = KdlNode::new("output");
        node.push(output.to_owned());
        node.set_children(KdlDocument::new());
        doc.nodes_mut().push(node);
    }
    let output_doc = named_node(doc, "output", output)
        .unwrap()
        .children_mut()
        .as_mut()
        .context("output needs a block")?;
    set_strings(output_doc, "hide-widgets", hidden)
}

fn write_regions(doc: &mut KdlDocument, layout: &Layout) -> Result<()> {
    for (key, value) in [("margin", layout.margin), ("gap", layout.gap)] {
        if doc.get(key).is_none() {
            let mut node = KdlNode::new(key);
            node.push(value as i128);
            doc.nodes_mut().push(node);
        }
    }
    for region in &layout.regions {
        let key = region.anchor.key();
        set_strings(doc, key, &region.widgets)?;
        let node = doc.get_mut(key).unwrap();
        if region.row {
            if node.get("flow").and_then(kdl::KdlValue::as_string) != Some("row") {
                node.insert("flow", "row");
            }
        } else if node.get("flow").and_then(kdl::KdlValue::as_string) == Some("row") {
            node.insert("flow", "column");
        }
        for (key, value) in [("x", region.x), ("y", region.y)] {
            if value != 0. && node.get(key).is_none() {
                node.insert(key, value as i128);
            }
        }
    }
    Ok(())
}

fn source_text(source: &Snapshot) -> String {
    if source.text.is_empty() {
        "schema-version 1\n".into()
    } else {
        source.text.clone()
    }
}

fn name(node: &KdlNode) -> Result<&str> {
    node.get(0)
        .and_then(kdl::KdlValue::as_string)
        .context("expected a named declaration")
}

fn named_node<'a>(doc: &'a mut KdlDocument, kind: &str, id: &str) -> Option<&'a mut KdlNode> {
    doc.nodes_mut()
        .iter_mut()
        .find(|node| node.name().value() == kind && name(node).ok() == Some(id))
}

fn strings(node: Option<&KdlNode>) -> Result<Vec<String>> {
    node.into_iter()
        .flat_map(|node| node.entries())
        .filter(|entry| entry.name().is_none())
        .map(|entry| {
            entry
                .value()
                .as_string()
                .map(str::to_owned)
                .context("expected widget or monitor name")
        })
        .collect()
}

fn set_strings(doc: &mut KdlDocument, key: &str, values: &[String]) -> Result<()> {
    if let Some(node) = doc.get_mut(key) {
        if strings(Some(node))? != values {
            let (mut arguments, properties): (Vec<_>, Vec<_>) = std::mem::take(node.entries_mut())
                .into_iter()
                .partition(|entry| entry.name().is_none());
            for value in values {
                let existing = arguments
                    .iter()
                    .position(|entry| entry.value().as_string() == Some(value));
                node.push(
                    existing.map_or_else(|| string_entry(value), |index| arguments.remove(index)),
                );
            }
            node.entries_mut().extend(properties);
        }
    } else {
        let mut node = KdlNode::new(key);
        for value in values {
            node.push(string_entry(value));
        }
        doc.nodes_mut().push(node);
    }
    Ok(())
}

fn string_entry(value: &str) -> KdlEntry {
    let mut entry = KdlEntry::new(value);
    entry.ensure_v1();
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(text: &str) -> (tempfile::TempDir, Editor) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.kdl");
        std::fs::write(&path, text).unwrap();
        let editor = Editor::load(Snapshot::load(&path).unwrap()).unwrap();
        (directory, editor)
    }

    #[test]
    fn materializes_default_shell_and_preserves_other_bar_fields() {
        let (_directory, mut editor) = edit("schema-version 1\n// my settings\n");
        editor.bars[0].place("clock", Some(0));
        let duplicate = editor.duplicate_bar(0, "Dock").unwrap();
        assert!(editor.candidate().is_err());
        editor.bars[duplicate].outputs.insert("DP-1".into());
        let candidate = editor.candidate().unwrap();
        assert!(candidate.contains("// my settings"));
        let (_directory, reloaded) = edit(&candidate);
        assert_eq!(reloaded.bars.len(), 2);
        assert!(reloaded.bars[duplicate].visible_on("DP-1"));
        assert!(!reloaded.bars[duplicate].visible_on("eDP-1"));
        assert!(reloaded.bars[0].zones[0].contains(&"clock".into()));
        let shell = candidate.parse::<KdlDocument>().unwrap();
        let shell = shell.get("shell").unwrap().children().unwrap();
        assert!(shell.get("menus").is_some());
        assert!(shell.get("modules").is_some());
        assert!(
            shell
                .get("bar")
                .unwrap()
                .children()
                .unwrap()
                .get("panel")
                .is_some()
        );
    }

    #[test]
    fn lock_visibility_edit_preserves_default_layout_and_background() {
        let (_directory, mut editor) = edit(
            r#"
schema-version 1
lockscreen {
    image "landscape.png"
    auth-timeout 45
    output "DP-1" { fit "fit"; }
}
"#,
        );
        editor.hide_lock_widget("DP-1", "media", true);
        editor.hide_lock_widget("DP-1", "authentication", true);
        let candidate = editor.candidate().unwrap();
        assert!(!candidate.contains("layout"));
        assert!(!candidate.contains("shell"));
        let (_directory, mut reloaded) = edit(&candidate);
        assert_eq!(reloaded.lock.auth_timeout, 45);
        assert!(
            reloaded
                .lock
                .for_output("DP-1")
                .image
                .as_ref()
                .unwrap()
                .ends_with("landscape.png")
        );
        assert_eq!(reloaded.lock.hidden_on("DP-1"), ["media"]);
        assert!(reloaded.lock.hidden_on("eDP-1").is_empty());

        reloaded.place_lock("authentication", Some(Anchor::BottomRight));
        reloaded.place_lock("authentication", None);
        reloaded.place_lock("media", None);
        let (_directory, reloaded) = edit(&reloaded.candidate().unwrap());
        assert!(!reloaded.lock_active("media"));
        let auth = reloaded
            .lock
            .layout
            .regions
            .iter()
            .find(|region| region.widgets.contains(&"authentication".into()))
            .unwrap();
        assert_eq!(auth.anchor, Anchor::BottomRight);
    }

    #[test]
    fn copying_an_implicit_bar_preserves_the_original() {
        let (_directory, mut editor) = edit("schema-version 1\nshell {}\n");
        let duplicate = editor.duplicate_bar(0, "Second").unwrap();
        editor.bars[duplicate].outputs.insert("DP-2".into());
        let (_directory, reloaded) = edit(&editor.candidate().unwrap());
        assert_eq!(reloaded.bars[0].name, "main");
        assert_eq!(reloaded.bars[1].name, "Second");
    }
}
