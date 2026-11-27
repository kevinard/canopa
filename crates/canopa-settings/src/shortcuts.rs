//! Keyboard shortcuts for the Canopa session.
//!
//! Niri lets a later `binds` block override an inherited key but cannot remove
//! one. Settings therefore stores only the user's changes, as a delta in
//! `shortcuts.kdl`, and generates `niri-shortcuts.kdl`: the shipped defaults
//! with that delta applied. While the delta has changes, `niri.kdl` includes
//! the generated file in place of the shipped defaults. Session start
//! regenerates it, so new and changed shipped bindings still arrive, and a user
//! binding wins any conflict over its key. Niri validates every candidate,
//! including user includes, before Settings publishes it.
use crate::{
    niri,
    session::Paths,
    storage::{Snapshot, quote},
};
use anyhow::{Context, Result, bail, ensure};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
};

/// The user's changes to the shipped bindings.
pub const DELTA: &str = "shortcuts.kdl";
/// Shipped defaults with the user's changes applied. Settings replaces it.
pub const GENERATED: &str = "niri-shortcuts.kdl";
#[cfg(test)]
const DEFAULTS: &str = include_str!("../../../packaging/niri/defaults.kdl");
/// The shipped defaults that the former first-save materialization copied into
/// `niri.kdl` (887ec8a, the only release before the delta format). Migration
/// compares the copied block with this, not with the current defaults.
const LEGACY_DEFAULTS: &str = include_str!("shortcuts/legacy-defaults.kdl");
const DEFAULTS_HEADER: &str = "// Canopa compositor defaults.";
/// Shipped actions that a later release replaced: (old action, new action).
/// Changes that still name an old action follow it to its replacement (see
/// [`retire`]). The device keys moved from `wpctl` to Canopa Shell commands
/// (Shell ADR 0025).
const RETIRED_ACTIONS: &[(&str, &str)] = &[
    (
        r#"spawn "wpctl" "set-volume" "@DEFAULT_AUDIO_SINK@" "5%+""#,
        r#"spawn "canopa-shell" "volume" "up""#,
    ),
    (
        r#"spawn "wpctl" "set-volume" "@DEFAULT_AUDIO_SINK@" "5%-""#,
        r#"spawn "canopa-shell" "volume" "down""#,
    ),
    (
        r#"spawn "wpctl" "set-mute" "@DEFAULT_AUDIO_SINK@" "toggle""#,
        r#"spawn "canopa-shell" "volume" "mute""#,
    ),
    (
        r#"spawn "wpctl" "set-mute" "@DEFAULT_AUDIO_SOURCE@" "toggle""#,
        r#"spawn "canopa-shell" "microphone" "mute""#,
    ),
];
const MAX_INCLUDES: usize = 32;
fn legacy_read_only() -> String {
    fl!("settings-error-shortcuts-custom-blocks")
}

#[derive(Clone, Debug)]
pub struct Shortcut {
    pub label: String,
    pub key: String,
    pub saved_key: String,
    pub default_key: String,
    /// The shipped binding this row edits.
    shipped: KdlNode,
    /// The delta binding that currently carries this row's action, if any.
    claim: Option<usize>,
}

/// What a shipped shortcut is for, to show related shortcuts together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Session,
    Windows,
    Workspaces,
    Screenshots,
    Keys,
    Other,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Session,
        Category::Windows,
        Category::Workspaces,
        Category::Screenshots,
        Category::Keys,
        Category::Other,
    ];
}

impl Shortcut {
    /// Grouped by the shipped binding: its key for screenshots and hardware
    /// keys, otherwise its action.
    pub fn category(&self) -> Category {
        let key = self.shipped.name().value();
        let Ok(action) = action(&self.shipped) else {
            return Category::Other;
        };
        let action = action.name().value();
        if key.starts_with("XF86") {
            Category::Keys
        } else if key.ends_with("Print") || action.starts_with("screenshot") {
            Category::Screenshots
        } else if action.contains("workspace") {
            Category::Workspaces
        } else if action.starts_with("spawn") || action == "quit" || action.contains("power") {
            Category::Session
        } else if action.contains("column")
            || action.contains("window")
            || action.contains("overview")
            || action.contains("fullscreen")
            || action.contains("floating")
            || action.contains("tabbed")
        {
            Category::Windows
        } else {
            Category::Other
        }
    }
}

/// Added or changed bindings, and shipped actions that were moved or turned off.
#[derive(Clone, Debug, Default)]
struct Delta {
    defaults: Option<PathBuf>,
    binds: Vec<KdlNode>,
    removed: Vec<KdlNode>,
}

impl Delta {
    fn parse(text: &str) -> Result<Self> {
        let mut delta = Self::default();
        if text.trim().is_empty() {
            return Ok(delta);
        }
        let document = KdlDocument::parse_v1(text).context("shortcuts.kdl is not valid KDL")?;
        let mut seen = HashSet::new();
        for node in document.nodes() {
            let name = node.name().value();
            ensure!(
                seen.insert(name.to_owned()),
                "shortcuts.kdl has more than one {name} section"
            );
            match name {
                "defaults" => {
                    let path = node
                        .get(0)
                        .and_then(|value| value.as_string())
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute())
                        .context("shortcuts.kdl needs an absolute defaults path")?;
                    delta.defaults = Some(path);
                }
                "binds" => delta.binds = bindings(node)?.to_vec(),
                "remove" => {
                    ensure!(
                        node.entries().is_empty(),
                        "Unsupported remove section in shortcuts.kdl"
                    );
                    delta.removed = node
                        .children()
                        .map(|children| children.nodes().to_vec())
                        .unwrap_or_default();
                }
                _ => bail!("shortcuts.kdl has an unsupported {name} section"),
            }
        }
        let mut keys = HashSet::new();
        for bind in &delta.binds {
            action(bind)?;
            ensure!(
                keys.insert(key_id(bind)),
                "{} is assigned more than once in shortcuts.kdl",
                bind.name().value()
            );
        }
        Ok(delta)
    }

    fn is_empty(&self) -> bool {
        self.binds.is_empty() && self.removed.is_empty()
    }

    fn text(&self) -> Result<String> {
        let mut text = String::from(
            "// Your changes to Canopa's shipped key bindings. Canopa Settings edits this file.\n\
             // Shipped bindings that are not listed here follow package updates.\n",
        );
        if let Some(path) = &self.defaults {
            text.push_str(&format!(
                "defaults {}\n",
                quote(path.to_str().context("defaults path must be UTF-8")?)
            ));
        }
        text.push_str(
            "// Added and changed bindings. Each replaces a shipped binding with the same key.\n",
        );
        text.push_str(&section("binds", &self.binds));
        text.push_str("// Shipped actions that you moved or turned off.\n");
        text.push_str(&section("remove", &self.removed));
        Ok(text)
    }
}

/// A block with one node per line, keeping each node's own source text.
fn section(name: &str, nodes: &[KdlNode]) -> String {
    let mut text = format!("{name} {{\n");
    for node in nodes {
        let mut node = node.clone();
        if let Some(format) = node.format_mut() {
            format.leading.clear();
            format.before_terminator.clear();
            format.terminator.clear();
            format.trailing.clear();
        }
        text.push_str("    ");
        text.push_str(node.to_string().trim());
        text.push('\n');
    }
    text.push_str("}\n");
    text
}

/// The packaged defaults file and its text.
#[derive(Clone, Debug)]
struct Shipped {
    path: PathBuf,
    text: String,
}

impl Shipped {
    fn read(path: &Path) -> Result<Self> {
        let bytes = canopa_file::read_canonical_bounded(path, crate::storage::MAX_BYTES)
            .with_context(|| format!("Could not read Canopa defaults {}", path.display()))?;
        Ok(Self {
            path: path.to_owned(),
            text: String::from_utf8(bytes)?,
        })
    }

    fn binds(&self) -> Result<Vec<KdlNode>> {
        let document = KdlDocument::parse_v1(&self.text)?;
        let binds = bindings(document.get("binds").context("Missing Canopa bindings")?)?;
        binds.iter().try_for_each(|node| action(node).map(drop))?;
        Ok(binds.to_vec())
    }

    /// The shipped defaults with `delta` applied, for `niri-shortcuts.kdl`.
    fn generate(&self, delta: &Delta) -> Result<String> {
        let document = KdlDocument::parse_v1(&self.text)?;
        ensure!(
            document
                .nodes()
                .iter()
                .all(|node| node.name().value() != "include"),
            "{}",
            fl!("settings-error-shortcuts-defaults-include")
        );
        let binds = effective(&self.binds()?, delta)?;
        let span = document
            .nodes()
            .iter()
            .find(|node| node.name().value() == "binds")
            .context("Missing Canopa bindings")?
            .span();
        let original = self
            .text
            .get(span.offset()..span.offset() + span.len())
            .unwrap_or_default()
            .trim_end();
        ensure!(
            original.starts_with("binds") && original.ends_with('}'),
            "Unsupported Canopa bindings block"
        );
        // Keep the rest of the shipped file byte for byte.
        let mut text = self.text.clone();
        text.replace_range(
            span.offset()..span.offset() + original.len(),
            section("binds", &binds).trim_end(),
        );
        Ok(format!(
            "// Generated by Canopa Settings from {} and {DELTA}.\n\
             // Change shortcuts in Settings or {DELTA}; Canopa replaces this file.\n{text}",
            self.path.display()
        ))
    }
}

/// Shipped bindings minus moved or turned-off actions and keys that the user
/// assigned, followed by the user's bindings.
fn effective(shipped: &[KdlNode], delta: &Delta) -> Result<Vec<KdlNode>> {
    let removed = delta.removed.iter().map(plain).collect::<HashSet<_>>();
    let keys = delta.binds.iter().map(key_id).collect::<HashSet<_>>();
    let mut result = Vec::with_capacity(shipped.len() + delta.binds.len());
    for node in shipped {
        if !removed.contains(&action_id(node)?) && !keys.contains(&key_id(node)) {
            result.push(node.clone());
        }
    }
    result.extend(delta.binds.iter().cloned());
    Ok(result)
}

/// Separate the user's changes in a materialized binding block from the
/// shipped `base` it was copied from. The result must reproduce `user` exactly.
fn migrate(base: &[KdlNode], user: &[KdlNode]) -> Result<Delta> {
    let mut keys = HashSet::new();
    for node in user {
        action(node)?;
        ensure!(
            keys.insert(key_id(node)),
            "{} is assigned more than once in niri.kdl",
            node.name().value()
        );
    }
    let base_set = base.iter().map(binding_id).collect::<HashSet<_>>();
    let user_set = user.iter().map(binding_id).collect::<HashSet<_>>();
    let mut removed = Vec::new();
    let mut removed_ids = HashSet::new();
    for node in base {
        if !user_set.contains(&binding_id(node)) && removed_ids.insert(action_id(node)?) {
            removed.push(action(node)?.clone());
        }
    }
    let mut binds = Vec::new();
    for node in user {
        if !base_set.contains(&binding_id(node)) || removed_ids.contains(&action_id(node)?) {
            binds.push(node.clone());
        }
    }
    let delta = Delta {
        defaults: None,
        binds,
        removed,
    };
    let result = effective(base, &delta)?;
    ensure!(
        result.len() == user.len()
            && result.iter().map(binding_id).collect::<HashSet<_>>() == user_set,
        "{}",
        fl!("settings-error-shortcuts-not-separable")
    );
    Ok(delta)
}

/// Carry changes that name a retired shipped action over to its replacement,
/// once the shipped defaults have that replacement and no longer the old
/// action. A moved or turned-off binding stays moved or off, and a carried
/// binding that now equals a shipped one is inherited again. Bindings whose
/// action the user changed are theirs and are kept as written.
fn retire(delta: &mut Delta, shipped: &[KdlNode]) -> Result<()> {
    let actions = shipped
        .iter()
        .map(action_id)
        .collect::<Result<HashSet<_>>>()?;
    let mut carried = Vec::new();
    for (old, new) in RETIRED_ACTIONS {
        let parse = |text: &str| -> Result<KdlNode> {
            let document = KdlDocument::parse_v1(&format!("{text}\n"))?;
            document
                .nodes()
                .first()
                .cloned()
                .context("Invalid retired action")
        };
        let (old, new) = (plain(&parse(old)?), parse(new)?);
        if !actions.contains(&plain(&new)) || actions.contains(&old) {
            continue;
        }
        for node in &mut delta.removed {
            if plain(node) == old {
                *node = new.clone();
            }
        }
        for (index, bind) in delta.binds.iter_mut().enumerate() {
            if action_id(bind)? == old {
                let children = bind
                    .children_mut()
                    .as_mut()
                    .context("Missing binding action")?;
                children.nodes_mut()[0] = new.clone();
                carried.push(index);
            }
        }
    }
    let mut seen = HashSet::new();
    delta.removed.retain(|node| seen.insert(plain(node)));
    let removed = delta.removed.iter().map(plain).collect::<HashSet<_>>();
    let inherited = shipped.iter().map(binding_id).collect::<HashSet<_>>();
    let mut index = 0;
    delta.binds.retain(|bind| {
        let keep = !carried.contains(&index)
            || !inherited.contains(&binding_id(bind))
            || action_id(bind).is_ok_and(|action| removed.contains(&action));
        index += 1;
        keep
    });
    Ok(())
}

#[derive(Clone, Debug)]
struct Include {
    path: PathBuf,
    text: Option<String>,
}

impl Include {
    fn load(parent: &Path, node: &KdlNode) -> Result<Self> {
        let path = include_path(parent, node)?;
        let optional = node.get("optional").and_then(|e| e.as_bool()) == Some(true);
        Ok(Self {
            text: Self::read(&path, optional)?,
            path,
        })
    }

    fn read(path: &Path, optional: bool) -> Result<Option<String>> {
        if optional
            && matches!(std::fs::metadata(path), Err(err) if err.kind() == std::io::ErrorKind::NotFound)
        {
            return Ok(None);
        }
        let bytes = canopa_file::read_canonical_bounded(path, crate::storage::MAX_BYTES)
            .with_context(|| format!("Could not read Niri include {}", path.display()))?;
        Ok(Some(String::from_utf8(bytes)?))
    }

    fn shipped(&self) -> bool {
        self.path == Path::new(crate::session::SYSTEM_DEFAULTS)
            || self
                .text
                .as_deref()
                .is_some_and(|text| text.starts_with(DEFAULTS_HEADER))
    }

    fn unchanged(&self) -> Result<()> {
        ensure!(
            Self::read(&self.path, self.text.is_none())? == self.text,
            "{}",
            fl!("settings-error-shortcuts-includes-changed")
        );
        Ok(())
    }
}

fn include_path(parent: &Path, node: &KdlNode) -> Result<PathBuf> {
    let value = node
        .get(0)
        .and_then(|e| e.as_string())
        .context("Invalid Niri include path")?;
    Ok(if let Some(relative) = value.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("Home directory is unavailable")?
            .join(relative)
    } else {
        parent.join(value)
    })
}

/// Point an include at another file, keeping its other properties and format.
fn retarget(node: &mut KdlNode, value: &str) -> Result<()> {
    let entry = node
        .entries_mut()
        .iter_mut()
        .find(|entry| entry.name().is_none())
        .context("include needs a filename")?;
    // Replacing only the value would retain its old source representation.
    *entry = string_entry(value)?;
    Ok(())
}

/// A string entry with a KDL v1 representation, so the document needs no
/// conversion that would rewrite the representation of the user's values.
fn string_entry(value: &str) -> Result<KdlEntry> {
    let document = KdlDocument::parse_v1(&format!("include {}\n", quote(value)))?;
    document.nodes()[0]
        .entries()
        .first()
        .cloned()
        .context("Invalid include path")
}

/// Where `niri.kdl` gets the Canopa bindings from.
#[derive(Clone, Debug)]
enum Source {
    /// An include of the packaged defaults; there are no changes.
    Shipped(usize),
    /// An include of the generated defaults with the user's changes.
    Generated(usize),
    /// A full copy of the legacy defaults, spliced into `niri.kdl` by the
    /// former first save. `kept` holds copied sections the user edited.
    Legacy {
        start: usize,
        end: usize,
        kept: Vec<KdlNode>,
    },
}

/// Files a save writes. `None` leaves a file as it is.
#[derive(Debug, Default)]
struct Plan {
    main: Option<String>,
    generated: Option<Option<String>>,
    delta: Option<String>,
    backup: bool,
}

#[derive(Clone, Debug)]
pub struct Editor {
    main: Snapshot,
    document: KdlDocument,
    source: Source,
    delta_file: Snapshot,
    delta: Delta,
    generated: Snapshot,
    shipped: Shipped,
    includes: Vec<Include>,
    external_keys: HashSet<String>,
    pub rows: Vec<Shortcut>,
}

impl Editor {
    pub fn load(paths: &Paths) -> Result<Self> {
        let main = Snapshot::load(paths.niri())?;
        ensure!(
            !main.text.is_empty(),
            "{}",
            fl!("settings-error-shortcuts-session")
        );
        Self::parse(paths, main, None)
    }

    fn parse(paths: &Paths, main: Snapshot, defaults: Option<&Path>) -> Result<Self> {
        let document = KdlDocument::parse_v1(&main.text)?;
        let parent = main
            .path
            .parent()
            .context("Niri configuration needs a directory")?
            .to_owned();
        let generated_path = paths.generated_shortcuts();
        let mut includes = Vec::new();
        let mut external_keys = HashSet::new();
        let mut include_budget = crate::storage::MAX_BYTES * 2;
        let mut source = None;
        let mut shipped_include = None;
        for (index, node) in document.nodes().iter().enumerate() {
            if node.name().value() != "include" {
                continue;
            }
            let found = if include_path(&parent, node)? == generated_path {
                Some(Source::Generated(index))
            } else {
                let include = Include::load(&parent, node)?;
                let found = if include.shipped() {
                    shipped_include = Some(include.path.clone());
                    Some(Source::Shipped(index))
                } else {
                    collect_included_keys(
                        &include,
                        &mut includes,
                        &mut external_keys,
                        &mut include_budget,
                    )?;
                    None
                };
                includes.push(include);
                found
            };
            if let Some(found) = found {
                ensure!(
                    source.is_none(),
                    "{}",
                    fl!("settings-error-shortcuts-ambiguous")
                );
                source = Some(found);
            }
            ensure!(
                includes.len() <= MAX_INCLUDES,
                "{}",
                fl!("settings-error-shortcuts-too-many-includes")
            );
        }
        let root_binds = document
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, node)| node.name().value() == "binds")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let delta_file = Snapshot::load(paths.shortcuts())?;
        let generated = Snapshot::load(&generated_path)?;
        let (source, mut delta) = match source {
            Some(source) => {
                // A hand-written block in niri.kdl overrides like an include.
                for index in root_binds {
                    external_keys.extend(
                        bindings(&document.nodes()[index])?
                            .iter()
                            .filter_map(|node| normalize(node.name().value())),
                    );
                }
                let delta = if matches!(source, Source::Generated(_)) {
                    Delta::parse(&delta_file.text)?
                } else {
                    Delta::default()
                };
                (source, delta)
            }
            None => {
                ensure!(
                    root_binds.len() == 1,
                    "{}",
                    fl!("settings-error-shortcuts-no-source")
                );
                legacy(&document, root_binds[0])?
            }
        };
        let shipped_path = match shipped_include
            .clone()
            .or_else(|| defaults.map(Path::to_owned))
            .or_else(|| delta.defaults.clone())
        {
            Some(path) => path,
            None => paths.shipped_defaults(None)?,
        };
        let shipped = Shipped::read(&std::path::absolute(shipped_path)?)?;
        if shipped_include.is_none() {
            includes.push(Include {
                path: shipped.path.clone(),
                text: Some(shipped.text.clone()),
            });
        }
        delta.defaults = Some(shipped.path.clone());
        let shipped_binds = shipped.binds()?;
        retire(&mut delta, &shipped_binds)?;
        let rows = rows(&shipped_binds, &delta)?;
        Ok(Self {
            main,
            document,
            source,
            delta_file,
            delta,
            generated,
            shipped,
            includes,
            external_keys,
            rows,
        })
    }

    /// The delta after applying the edited rows to the saved changes.
    fn changes(&self) -> Result<Delta> {
        let mut delta = self.delta.clone();
        let mut keys = HashSet::new();
        for row in &self.rows {
            let key = row.key.trim();
            if key.is_empty() {
                continue;
            }
            let normalized = normalize(key).with_context(|| {
                fl!(
                    "settings-error-shortcut-invalid",
                    action = row.label.to_string()
                )
            })?;
            ensure!(
                keys.insert(normalized),
                "{}",
                fl!("settings-error-shortcut-duplicate", key = key.to_string())
            );
        }
        let claimed = self
            .rows
            .iter()
            .filter_map(|row| row.claim)
            .collect::<HashSet<_>>();
        for (index, bind) in delta.binds.iter().enumerate() {
            ensure!(
                claimed.contains(&index) || !keys.contains(&key_id(bind)),
                "{} is already assigned in {DELTA}.",
                bind.name().value()
            );
        }
        let changed = self
            .rows
            .iter()
            .filter(|row| row.key.trim() != row.saved_key)
            .collect::<Vec<_>>();
        let released = changed
            .iter()
            .filter_map(|row| row.claim)
            .collect::<HashSet<_>>();
        let previous = delta.binds;
        delta.binds = previous
            .iter()
            .enumerate()
            .filter(|(index, _)| !released.contains(index))
            .map(|(_, node)| node.clone())
            .collect();
        for row in changed {
            let key = row.key.trim();
            let action = action_id(&row.shipped)?;
            if !key.is_empty() && normalize(key) != normalize(&row.saved_key) {
                ensure!(
                    !self.external_keys.contains(&key_id_of(key)),
                    "{}",
                    fl!("settings-error-shortcut-in-include", key = key.to_string())
                );
            }
            delta.removed.retain(|node| plain(node) != action);
            if normalize(key).is_some() && normalize(key) == normalize(&row.default_key) {
                continue;
            }
            delta
                .removed
                .push(crate::shortcuts::action(&row.shipped)?.clone());
            if !key.is_empty() {
                let mut node = row
                    .claim
                    .map(|index| previous[index].clone())
                    .unwrap_or_else(|| row.shipped.clone());
                node.set_name(key);
                delta.binds.push(node);
            }
        }
        Ok(delta)
    }

    fn plan(&self, delta: &Delta) -> Result<Plan> {
        let mut plan = Plan::default();
        let mut document = self.document.clone();
        let target = if delta.is_empty() {
            self.shipped
                .path
                .to_str()
                .context("defaults path must be UTF-8")?
                .to_owned()
        } else {
            GENERATED.to_owned()
        };
        match &self.source {
            Source::Shipped(index) => {
                if delta.is_empty() {
                    return Ok(plan);
                }
                retarget(&mut document.nodes_mut()[*index], &target)?;
            }
            Source::Generated(index) => {
                if delta.is_empty() {
                    retarget(&mut document.nodes_mut()[*index], &target)?;
                    plan.generated = Some(None);
                }
            }
            Source::Legacy { start, end, kept } => {
                let mut include = KdlDocument::parse_v1(&format!("include {}\n", quote(&target)))?
                    .nodes()[0]
                    .clone();
                if let (Some(origin), Some(format)) =
                    (document.nodes()[*start].format(), include.format_mut())
                {
                    // The copied defaults brought their header comment along.
                    format.leading = origin
                        .leading
                        .split_inclusive('\n')
                        .filter(|line| !line.trim_start().starts_with(DEFAULTS_HEADER))
                        .collect();
                }
                // Edited copies of other sections follow the include so they keep
                // overriding the shipped values, as they did before.
                let replacement = std::iter::once(include).chain(kept.iter().cloned());
                document.nodes_mut().splice(*start..=*end, replacement);
                plan.backup = true;
            }
        }
        let main = document.to_string();
        if main != self.main.text {
            plan.main = Some(main);
        }
        if !delta.is_empty() {
            let generated = self.shipped.generate(delta)?;
            if generated != self.generated.text || !self.generated.exists() {
                plan.generated = Some(Some(generated));
            }
        }
        if !delta.is_empty() || self.delta_file.exists() {
            let text = delta.text()?;
            if text != self.delta_file.text {
                plan.delta = Some(text);
            }
        }
        Ok(plan)
    }

    fn unchanged(&self) -> Result<()> {
        self.main.unchanged()?;
        self.delta_file.unchanged()?;
        self.generated.unchanged()?;
        for include in &self.includes {
            include.unchanged()?;
        }
        Ok(())
    }

    /// Publish `plan`. A live session validates and reloads the compositor;
    /// otherwise the candidate is only validated. Earlier writes roll back when
    /// a later one fails.
    fn apply(
        &self,
        paths: &Paths,
        plan: &Plan,
        live: bool,
        validate: &dyn Fn(&Path) -> Result<()>,
    ) -> Result<Option<PathBuf>> {
        self.unchanged()?;
        let main = plan.main.as_deref().unwrap_or(&self.main.text);
        let generated = match &plan.generated {
            Some(text) => text.as_deref(),
            None => self
                .generated
                .exists()
                .then_some(self.generated.text.as_str()),
        };
        validate_candidate(&paths.directory, main, generated, validate)?;
        let backup = if plan.backup {
            Some(backup(&self.main)?)
        } else {
            None
        };
        let written = match &plan.generated {
            Some(Some(text)) => Some(self.generated.save(text, |_| Ok(()))?),
            _ => None,
        };
        let restore_generated = || -> Result<()> {
            if let Some(saved) = &written {
                if self.generated.exists() {
                    saved.save(&self.generated.text, |_| Ok(()))?;
                } else {
                    std::fs::remove_file(&saved.path)?;
                }
            }
            Ok(())
        };
        let published = match &plan.main {
            Some(text) if live => niri::save_main(&self.main, text, || {
                for include in &self.includes {
                    include.unchanged()?;
                }
                Ok(())
            })
            .map(Some),
            Some(text) => self.main.save(text, validate).map(Some),
            None if live && written.is_some() => niri::Client::discover()
                .and_then(|client| client.reload())
                .map(|()| None),
            None => Ok(None),
        };
        let published = match published {
            Ok(value) => value,
            Err(error) => {
                restore_generated()?;
                if live && written.is_some() && plan.main.is_none() {
                    let _ = niri::Client::discover().and_then(|client| client.reload());
                }
                return Err(error);
            }
        };
        if let Some(text) = &plan.delta
            && let Err(error) = self.delta_file.save(text, |_| Ok(()))
        {
            if let Some(saved) = &published {
                if live {
                    niri::save_main(saved, &self.main.text, || Ok(()))?;
                } else {
                    saved.save(&self.main.text, |_| Ok(()))?;
                }
            }
            restore_generated()?;
            return Err(error);
        }
        if plan.generated == Some(None) && self.generated.exists() {
            match std::fs::remove_file(&self.generated.path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            }
        }
        Ok(backup)
    }

    pub fn save(&self, paths: &Paths) -> Result<Self> {
        ensure!(
            paths.active() && self.main.path == paths.niri(),
            "{}",
            fl!("settings-error-shortcuts-session")
        );
        let delta = self.changes()?;
        self.unchanged()?;
        if !self.dirty() && !self.migration_pending() {
            return Ok(self.clone());
        }
        let plan = self.plan(&delta)?;
        self.apply(paths, &plan, true, &niri::validate)?;
        Self::load(paths)
    }

    pub fn dirty(&self) -> bool {
        self.rows.iter().any(|row| row.key.trim() != row.saved_key)
    }

    /// Whether `niri.kdl` still holds a full copy of the legacy bindings.
    pub fn migration_pending(&self) -> bool {
        matches!(self.source, Source::Legacy { .. })
    }
}

/// Session start: move a legacy copy of the shipped bindings to the delta
/// format and regenerate the combined defaults from the current shipped file.
/// Returns a description of any change.
pub fn refresh(
    paths: &Paths,
    defaults: Option<&Path>,
    validate: impl Fn(&Path) -> Result<()>,
) -> Result<Option<String>> {
    let main = Snapshot::load(paths.niri())?;
    if main.text.is_empty() {
        return Ok(None);
    }
    let editor = Editor::parse(paths, main, defaults)?;
    if matches!(editor.source, Source::Shipped(_)) {
        return Ok(None);
    }
    let plan = editor.plan(&editor.delta)?;
    if plan.main.is_none() && plan.generated.is_none() && plan.delta.is_none() {
        return Ok(None);
    }
    let backup = editor.apply(paths, &plan, false, &validate)?;
    Ok(Some(match backup {
        Some(path) => format!(
            "moved your shortcut changes to {}; the previous configuration is saved as {}",
            paths.shortcuts().display(),
            path.display()
        ),
        None => format!(
            "updated {} from the shipped defaults",
            paths.generated_shortcuts().display()
        ),
    }))
}

/// Recognize the former first-save materialization: the legacy defaults'
/// sections around the only top-level binding block, in their original order.
fn legacy(document: &KdlDocument, binds: usize) -> Result<(Source, Delta)> {
    let base = KdlDocument::parse_v1(LEGACY_DEFAULTS)?;
    let offset = base
        .nodes()
        .iter()
        .position(|node| node.name().value() == "binds")
        .context("Missing legacy Canopa bindings")?;
    let (before, after) = (&base.nodes()[..offset], &base.nodes()[offset + 1..]);
    let nodes = document.nodes();
    ensure!(
        binds >= before.len() && binds + after.len() < nodes.len(),
        "{}",
        legacy_read_only()
    );
    let (start, end) = (binds - before.len(), binds + after.len());
    let mut kept = Vec::new();
    for (node, base) in nodes[start..binds]
        .iter()
        .zip(before)
        .chain(nodes[binds + 1..=end].iter().zip(after))
    {
        ensure!(
            node.name().value() == base.name().value(),
            "{}",
            legacy_read_only()
        );
        if plain(node) != plain(base) {
            kept.push(node.clone());
        }
    }
    let delta = migrate(bindings(&base.nodes()[offset])?, bindings(&nodes[binds])?)?;
    Ok((Source::Legacy { start, end, kept }, delta))
}

fn rows(shipped: &[KdlNode], delta: &Delta) -> Result<Vec<Shortcut>> {
    let removed = delta.removed.iter().map(plain).collect::<HashSet<_>>();
    let keys = delta.binds.iter().map(key_id).collect::<HashSet<_>>();
    let mut rows = Vec::with_capacity(shipped.len());
    for node in shipped {
        let action = action_id(node)?;
        let moved = removed.contains(&action);
        let claim = delta.binds.iter().position(|bind| {
            action_id(bind).is_ok_and(|id| id == action) && (moved || key_id(bind) == key_id(node))
        });
        let key = match claim {
            Some(index) => delta.binds[index].name().value().to_owned(),
            // Turned off, or its key now belongs to another binding.
            None if moved || keys.contains(&key_id(node)) => String::new(),
            None => node.name().value().to_owned(),
        };
        rows.push(Shortcut {
            label: label(node)?,
            saved_key: key.clone(),
            key,
            default_key: node.name().value().to_owned(),
            shipped: node.clone(),
            claim,
        });
    }
    Ok(rows)
}

fn backup(main: &Snapshot) -> Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt;
    for attempt in 0..100 {
        let mut name = main
            .path
            .file_name()
            .context("niri.kdl needs a file name")?
            .to_owned();
        name.push(".before-shortcut-changes");
        if attempt > 0 {
            name.push(format!(".{attempt}"));
        }
        let path = main.path.with_file_name(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(main.text.as_bytes())?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    bail!("Could not back up niri.kdl before moving your shortcut changes")
}

/// Validate a candidate `niri.kdl` whose generated include reads `generated`.
fn validate_candidate(
    directory: &Path,
    main: &str,
    generated: Option<&str>,
    validate: &dyn Fn(&Path) -> Result<()>,
) -> Result<()> {
    let mut document = KdlDocument::parse_v1(main)?;
    let candidate = generated
        .map(|text| -> Result<_> {
            let mut file = tempfile::Builder::new()
                .prefix(".canopa-settings-")
                .suffix(".kdl")
                .tempfile_in(directory)?;
            file.write_all(text.as_bytes())?;
            file.as_file().sync_all()?;
            Ok(file)
        })
        .transpose()?;
    for node in document.nodes_mut() {
        if node.name().value() == "include"
            && include_path(directory, node)? == directory.join(GENERATED)
        {
            let file = candidate
                .as_ref()
                .with_context(|| format!("{GENERATED} is missing"))?;
            retarget(node, file.path().to_str().context("path must be UTF-8")?)?;
        }
    }
    let mut wrapper = tempfile::Builder::new()
        .prefix(".canopa-settings-")
        .suffix(".kdl")
        .tempfile_in(directory)?;
    wrapper.write_all(document.to_string().as_bytes())?;
    wrapper.as_file().sync_all()?;
    validate(wrapper.path())
}

fn bindings(node: &KdlNode) -> Result<&[KdlNode]> {
    ensure!(
        node.entries().is_empty() && node.ty().is_none(),
        "Unsupported bindings block"
    );
    Ok(node.children().map(KdlDocument::nodes).unwrap_or_default())
}

fn action(node: &KdlNode) -> Result<&KdlNode> {
    let actions = node.children().map(KdlDocument::nodes).unwrap_or_default();
    ensure!(
        actions.len() == 1,
        "{} needs exactly one action",
        node.name().value()
    );
    Ok(&actions[0])
}

/// A node without formatting, for comparing parsed sources.
fn plain(node: &KdlNode) -> String {
    let mut node = node.clone();
    node.clear_format_recursive();
    node.to_string().trim().to_owned()
}

fn action_id(node: &KdlNode) -> Result<String> {
    Ok(plain(action(node)?))
}

/// A binding's key and everything else, ignoring formatting.
fn binding_id(node: &KdlNode) -> (String, String) {
    let mut rest = node.clone();
    rest.set_name("binding");
    (key_id(node), plain(&rest))
}

fn key_id(node: &KdlNode) -> String {
    key_id_of(node.name().value())
}

/// Niri compares parsed keys. Keep unparsed names, such as mouse buttons, apart.
fn key_id_of(key: &str) -> String {
    normalize(key).unwrap_or_else(|| format!("raw:{}", key.to_ascii_lowercase()))
}

fn label(node: &KdlNode) -> Result<String> {
    let action = action(node)?;
    let label = match node.name().value() {
        "Mod+Return" => fl!("settings-shortcut-open-terminal"),
        "Mod+Space" => fl!("settings-shortcut-open-applications"),
        "Mod+B" if plain(action).contains("--toggle-bar") => {
            fl!("settings-shortcut-toggle-bar")
        }
        "Mod+E" => fl!("settings-shortcut-open-files"),
        "Mod+Comma" => fl!("settings-shortcut-open-settings"),
        "Mod+L" => fl!("settings-shortcut-lock-screen"),
        "Mod+Q" => fl!("settings-shortcut-close-window"),
        "Mod+F" => fl!("settings-shortcut-maximize-column"),
        "Mod+Home" => fl!("settings-shortcut-focus-first-column"),
        "Mod+End" => fl!("settings-shortcut-focus-last-column"),
        "Mod+Shift+Home" => fl!("settings-shortcut-move-column-to-first"),
        "Mod+Shift+End" => fl!("settings-shortcut-move-column-to-last"),
        "Mod+BracketLeft" => fl!("settings-shortcut-stack-window-left"),
        "Mod+BracketRight" => fl!("settings-shortcut-stack-window-right"),
        "Mod+W" => fl!("settings-shortcut-toggle-tabbed-column"),
        "Mod+R" => fl!("settings-shortcut-next-column-width"),
        "Mod+Shift+R" => fl!("settings-shortcut-previous-column-width"),
        "Mod+Ctrl+F" => fl!("settings-shortcut-fill-available-width"),
        "Mod+C" => fl!("settings-shortcut-center-column"),
        "Mod+P" => fl!("settings-shortcut-pin-column-left"),
        "Mod+Shift+P" => fl!("settings-shortcut-pin-column-right"),
        "Mod+Ctrl+P" => fl!("settings-shortcut-fold-pinned-columns"),
        "Mod+Grave" => fl!("settings-shortcut-toggle-scratchpad"),
        "Mod+Shift+Grave" => fl!("settings-shortcut-toggle-window-in-scratchpad"),
        "Mod+Shift+B" => fl!("settings-shortcut-toggle-window-bar"),
        "Mod+Shift+F" => fl!("settings-shortcut-toggle-fullscreen"),
        "Mod+V" => fl!("settings-shortcut-toggle-floating-window"),
        "Mod+O" => fl!("settings-shortcut-toggle-overview"),
        "Mod+Shift+E" => fl!("settings-shortcut-log-out"),
        "Print" => fl!("settings-shortcut-select-screenshot-region"),
        "Ctrl+Print" => fl!("settings-shortcut-capture-screen"),
        "Alt+Print" => fl!("settings-shortcut-capture-window"),
        "XF86AudioRaiseVolume" => fl!("settings-shortcut-raise-volume"),
        "XF86AudioLowerVolume" => fl!("settings-shortcut-lower-volume"),
        "XF86AudioMute" => fl!("settings-shortcut-mute-speakers"),
        "XF86AudioMicMute" => fl!("settings-shortcut-mute-microphone"),
        "XF86MonBrightnessUp" => fl!("settings-shortcut-brightness-up"),
        "XF86MonBrightnessDown" => fl!("settings-shortcut-brightness-down"),
        "XF86KbdBrightnessUp" => fl!("settings-shortcut-keyboard-brightness-up"),
        "XF86KbdBrightnessDown" => fl!("settings-shortcut-keyboard-brightness-down"),
        "XF86KbdLightOnOff" => fl!("settings-shortcut-toggle-keyboard-backlight"),
        "XF86AudioPlay" => fl!("settings-shortcut-play-or-pause"),
        "XF86AudioPause" => fl!("settings-shortcut-pause-media"),
        "XF86AudioStop" => fl!("settings-shortcut-stop-media"),
        "XF86AudioNext" => fl!("settings-shortcut-next-track"),
        "XF86AudioPrev" => fl!("settings-shortcut-previous-track"),
        _ => {
            // Numbered workspace bindings would otherwise all read the same.
            let number = action
                .entries()
                .first()
                .filter(|entry| entry.name().is_none())
                .and_then(|entry| entry.value().as_integer())
                .and_then(|number| i64::try_from(number).ok());
            match (action.name().value(), number) {
                ("focus-workspace", Some(number)) => {
                    return Ok(fl!("settings-shortcut-focus-workspace", number = number));
                }
                ("move-column-to-workspace", Some(number)) => {
                    return Ok(fl!(
                        "settings-shortcut-move-column-to-workspace",
                        number = number
                    ));
                }
                _ => {}
            }
            let mut label = action.name().value().replace('-', " ");
            if let Some(initial) = label.get_mut(..1) {
                initial.make_ascii_uppercase();
            }
            // Other actions are named by Niri's own (English) action name.
            return Ok(label);
        }
    };
    Ok(label)
}

pub(crate) fn normalize(key: &str) -> Option<String> {
    if key.is_empty() || key.len() > 100 {
        return None;
    }
    let (trigger, modifiers) = {
        let mut parts = key.split('+').map(str::trim);
        (parts.next_back()?, parts)
    };
    // Niri's pointer triggers are not keysyms.
    const POINTER_TRIGGERS: [&str; 13] = [
        "wheelscrolldown",
        "wheelscrollup",
        "wheelscrollleft",
        "wheelscrollright",
        "touchpadscrolldown",
        "touchpadscrollup",
        "touchpadscrollleft",
        "touchpadscrollright",
        "mouseleft",
        "mouseright",
        "mousemiddle",
        "mouseback",
        "mouseforward",
    ];
    let pointer = trigger.to_ascii_lowercase();
    let trigger = if POINTER_TRIGGERS.contains(&pointer.as_str()) {
        format!("pointer:{pointer}")
    } else {
        let keysym =
            xkbcommon::xkb::keysym_from_name(trigger, xkbcommon::xkb::KEYSYM_CASE_INSENSITIVE);
        if keysym.raw() == 0 {
            return None;
        }
        keysym.raw().to_string()
    };
    let mut set = Vec::new();
    for modifier in modifiers {
        // The supported physical Canopa session uses Super as Niri's Mod key.
        let modifier = match modifier.to_ascii_lowercase().as_str() {
            "mod" | "super" | "win" => "super",
            "ctrl" | "control" => "ctrl",
            "alt" => "alt",
            "shift" => "shift",
            "mod5" | "iso_level3_shift" => "mod5",
            "mod3" | "iso_level5_shift" => "mod3",
            _ => return None,
        };
        if set.contains(&modifier) {
            return None;
        }
        set.push(modifier);
    }
    set.sort_unstable();
    Some(format!("{}/{}", set.join("+"), trigger))
}

fn collect_included_keys(
    include: &Include,
    collected: &mut Vec<Include>,
    keys: &mut HashSet<String>,
    budget: &mut usize,
) -> Result<()> {
    let Some(text) = &include.text else {
        return Ok(());
    };
    *budget = budget
        .checked_sub(text.len())
        .context("Niri includes exceed the editor's size limit")?;
    let document = KdlDocument::parse_v1(text)?;
    let parent = include
        .path
        .parent()
        .context("Niri include needs a parent directory")?;
    for node in document.nodes() {
        match node.name().value() {
            "binds" => {
                keys.extend(
                    bindings(node)?
                        .iter()
                        .filter_map(|node| normalize(node.name().value())),
                );
            }
            "include" => {
                ensure!(
                    collected.len() < MAX_INCLUDES,
                    "{}",
                    fl!("settings-error-shortcuts-recursive-includes")
                );
                let nested = Include::load(parent, node)?;
                collected.push(nested.clone());
                collect_included_keys(&nested, collected, keys, budget)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
