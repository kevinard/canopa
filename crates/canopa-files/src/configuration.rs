//! Files owns its namespace; graphical edits preserve other owners and reject stale snapshots.
use crate::actions::CustomAction;
use crate::interface::InterfaceSettings;
use canopa_config::{Code, Component, Diagnostic, Document, Format, Input, Request, SourceInfo};
pub mod jobs;
mod reload;
pub use jobs::{ConfigurationJobs, Published};
use kdl::{KdlDocument, KdlNode};
pub use reload::{ConfigurationRuntime, ReloadEvent};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
};

#[derive(Clone)]
pub struct ConfigurationSnapshot {
    pub appearance: canopa_theme::Appearance,
    pub actions: Vec<CustomAction>,
    pub settings: InterfaceSettings,
    pub source: SourceInfo,
    text: Option<String>,
    edit_path: Option<PathBuf>,
    target: Option<PathBuf>,
    request: Request,
}

impl std::fmt::Debug for ConfigurationSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigurationSnapshot")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl ConfigurationSnapshot {
    pub fn load(request: &Request) -> Result<Self, Diagnostic> {
        if matches!(
            request,
            Request::File {
                format: Format::Legacy,
                ..
            }
        ) {
            return Err(Diagnostic::new(Code::Envelope, None));
        }
        let request = match request {
            Request::File { .. } => request.resolve(Component::Files)?,
            other => other.clone(),
        };
        let input = Input::load(&request, Component::Files, "")?;
        let document = input.select(Component::Files, "")?;
        let mut actions = parse_actions(document.selected())?;
        let settings = crate::interface_config::parse(document.selected())
            .and_then(|settings| {
                settings.validate(&actions)?;
                Ok(settings)
            })
            .map_err(|_| Diagnostic::new(Code::Schema, None))?;
        for action in &mut actions {
            if settings.toolbar.contains(&format!("action:{}", action.id)) {
                action.toolbar = true;
            }
        }
        let source = document.info().clone();
        let edit_path = match &request {
            Request::Defaults => None,
            _ => Some(match &source.path {
                Some(path) => path.clone(),
                None => canopa_config::config_home()?.join("canopa/config.kdl"),
            }),
        };
        let text = source.path.as_ref().map(|_| input.text().to_owned());
        let target = source
            .path
            .as_deref()
            .map(fs::canonicalize)
            .transpose()
            .map_err(|_| Diagnostic::new(Code::Read, None))?;
        Ok(Self {
            appearance: document.appearance(),
            actions,
            settings,
            source,
            text,
            edit_path,
            target,
            request,
        })
    }

    pub fn reload(&self) -> Result<Self, Diagnostic> {
        Self::load(&self.request)
    }
    pub fn same_source(&self, other: &Self) -> bool {
        self.source == other.source
            && self.target == other.target
            && self.edit_path == other.edit_path
            && self.text == other.text
    }
    pub fn same_files(&self, other: &Self) -> bool {
        self.actions == other.actions && self.settings == other.settings
    }

    pub fn is_editable(&self) -> bool {
        self.edit_path.is_some()
    }

    pub fn save_actions(&self, actions: Vec<CustomAction>) -> io::Result<Self> {
        // Removing or disabling an action also removes its explicit position.
        let mut settings = self.settings.clone();
        settings.toolbar.retain(|id| {
            id.strip_prefix("action:").is_none_or(|id| {
                actions
                    .iter()
                    .any(|action| action.id == id && action.toolbar)
            })
        });
        self.save(settings, actions)
    }

    pub fn save_settings(&self, settings: InterfaceSettings) -> io::Result<Self> {
        let mut actions = self.actions.clone();
        for action in &mut actions {
            let id = format!("action:{}", action.id);
            if settings.toolbar.contains(&id) {
                action.toolbar = true;
            } else if self.settings.toolbar.contains(&id) {
                action.toolbar = false;
            }
        }
        self.save(settings, actions)
    }

    fn save(&self, settings: InterfaceSettings, actions: Vec<CustomAction>) -> io::Result<Self> {
        validate_actions(&actions)?;
        settings.validate(&actions)?;
        let selected = self
            .edit_path
            .as_ref()
            .ok_or_else(|| io::Error::other("built-in defaults cannot be edited"))?;
        let stale = || {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "configuration changed; reload the editor before saving",
            )
        };
        let current_target = match fs::symlink_metadata(selected) {
            Ok(_) => Some(fs::canonicalize(selected)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if current_target != self.target {
            return Err(stale());
        }
        let path = current_target.as_ref().unwrap_or(selected);
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("configuration needs a parent folder"))?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
        use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
        let lock = open(
            parent.join(".canopa-files-edit.lock"),
            OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )?;
        flock(&lock, FlockOperation::LockExclusive)?;
        let actual = match fs::symlink_metadata(path) {
            Ok(_) => Some(canopa_config::read(path).map_err(io::Error::other)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if actual != self.text {
            return Err(stale());
        }
        let mut raw = canopa_config::parse_kdl(actual.as_deref().unwrap_or("schema-version 1\n"))
            .map_err(io::Error::other)?;
        let mut node = KdlNode::new("files");
        let mut children = KdlDocument::new();
        children
            .nodes_mut()
            .extend(crate::interface_config::nodes(&settings));
        for action in &actions {
            children.nodes_mut().push(action_node(action));
        }
        node.set_children(children);
        node.autoformat();
        if let Some(old) = raw
            .nodes_mut()
            .iter_mut()
            .find(|node| node.name().value() == "files")
        {
            *old = node;
        } else {
            raw.nodes_mut().push(node);
        }
        let text = raw.to_string();
        // Validate the complete result, including envelope and shared appearance.
        let document = Document::parse(
            text.clone(),
            Some(path.clone()),
            Format::CanopaV1,
            Component::Files,
            "",
        )
        .map_err(io::Error::other)?;
        parse_actions(document.selected()).map_err(io::Error::other)?;
        crate::interface_config::parse(document.selected())
            .and_then(|settings| settings.validate(&actions))?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".canopa-config-")
            .tempfile_in(parent)?;
        temporary.write_all(text.as_bytes())?;
        temporary.as_file().sync_all()?;
        // Non-cooperating text editors don't take our lock. Recheck before installing.
        let latest = if path.exists() {
            Some(canopa_config::read(path).map_err(io::Error::other)?)
        } else {
            None
        };
        if latest != actual || fs::canonicalize(selected).ok() != self.target {
            return Err(stale());
        }
        if actual.is_none() {
            temporary
                .persist_noclobber(path)
                .map_err(|error| error.error)?;
        } else {
            temporary.persist(path).map_err(|error| error.error)?;
        }
        fs::File::open(parent)?.sync_all()?;
        Self::load(&self.request).map_err(io::Error::other)
    }
}

fn validate_actions(actions: &[CustomAction]) -> io::Result<()> {
    let mut ids = BTreeSet::new();
    let mut shortcuts = BTreeSet::new();
    if actions.len() > 256 {
        return Err(io::Error::other("at most 256 custom actions are supported"));
    }
    for action in actions {
        action.validate()?;
        if !ids.insert(&action.id)
            || action
                .shortcut
                .as_ref()
                .is_some_and(|shortcut| !shortcuts.insert(shortcut.trim().to_lowercase()))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "duplicate action ID or shortcut",
            ));
        }
    }
    Ok(())
}

fn parse_actions(document: &KdlDocument) -> Result<Vec<CustomAction>, Diagnostic> {
    let mut actions = Vec::new();
    for node in document.nodes() {
        if matches!(
            node.name().value(),
            "view" | "toolbar" | "sidebar" | "places" | "shortcut"
        ) {
            continue;
        }
        let fail = || Diagnostic::new(Code::Schema, Some(node.span().offset()));
        if node.name().value() != "action" || node.ty().is_some() {
            return Err(fail());
        }
        let mut action = CustomAction::new("", "", "");
        let mut seen = BTreeSet::new();
        for entry in node.entries() {
            if entry.ty().is_some() {
                return Err(fail());
            }
            let key = entry.name().map_or("id", |name| name.value());
            if !seen.insert(key) {
                return Err(fail());
            }
            match key {
                "id" | "name" | "command" | "icon" | "shortcut" => {
                    let text = entry.value().as_string().ok_or_else(fail)?.to_owned();
                    match key {
                        "id" => action.id = text,
                        "name" => action.name = text,
                        "command" => action.command = text,
                        "icon" => action.icon = Some(text),
                        _ => action.shortcut = Some(text),
                    }
                }
                "toolbar" => action.toolbar = entry.value().as_bool().ok_or_else(fail)?,
                _ => return Err(fail()),
            }
        }
        let mut seen = BTreeSet::new();
        if let Some(children) = node.children() {
            for child in children.nodes() {
                if child.ty().is_some()
                    || child.children().is_some()
                    || !seen.insert(child.name().value())
                {
                    return Err(fail());
                }
                match child.name().value() {
                    "selection" => {
                        let mut seen = BTreeSet::new();
                        for entry in child.entries() {
                            let key = entry.name().ok_or_else(fail)?.value();
                            if entry.ty().is_some() || !seen.insert(key) {
                                return Err(fail());
                            }
                            match key {
                                "min" | "max" => {
                                    let count: usize = entry
                                        .value()
                                        .as_integer()
                                        .and_then(|n| n.try_into().ok())
                                        .ok_or_else(fail)?;
                                    if key == "min" {
                                        action.conditions.minimum_selection = count;
                                    } else {
                                        action.conditions.maximum_selection = Some(count);
                                    }
                                }
                                "files" => {
                                    action.conditions.files =
                                        entry.value().as_bool().ok_or_else(fail)?
                                }
                                "directories" => {
                                    action.conditions.directories =
                                        entry.value().as_bool().ok_or_else(fail)?
                                }
                                _ => return Err(fail()),
                            }
                        }
                    }
                    "patterns" => {
                        for entry in child.entries() {
                            if entry.name().is_some() || entry.ty().is_some() {
                                return Err(fail());
                            }
                            action
                                .conditions
                                .patterns
                                .push(entry.value().as_string().ok_or_else(fail)?.to_owned());
                        }
                    }
                    _ => return Err(fail()),
                }
            }
        }
        action.validate().map_err(|_| fail())?;
        actions.push(action);
    }
    validate_actions(&actions).map_err(|_| Diagnostic::new(Code::Schema, None))?;
    Ok(actions)
}

fn action_node(action: &CustomAction) -> KdlNode {
    let mut node = KdlNode::new("action");
    node.push(action.id.as_str());
    node.push(("name", action.name.as_str()));
    node.push(("command", action.command.as_str()));
    if let Some(icon) = &action.icon {
        node.push(("icon", icon.as_str()));
    }
    if let Some(shortcut) = &action.shortcut {
        node.push(("shortcut", shortcut.as_str()));
    }
    node.push(("toolbar", action.toolbar));
    let mut children = KdlDocument::new();
    let mut selection = KdlNode::new("selection");
    selection.push(("min", action.conditions.minimum_selection as i128));
    if let Some(max) = action.conditions.maximum_selection {
        selection.push(("max", max as i128));
    }
    selection.push(("files", action.conditions.files));
    selection.push(("directories", action.conditions.directories));
    children.nodes_mut().push(selection);
    if !action.conditions.patterns.is_empty() {
        let mut patterns = KdlNode::new("patterns");
        for pattern in &action.conditions.patterns {
            patterns.push(pattern.as_str());
        }
        children.nodes_mut().push(patterns);
    }
    node.set_children(children);
    node
}
