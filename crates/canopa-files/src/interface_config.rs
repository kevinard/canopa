use crate::{
    interface::{Command, InterfaceSettings, Place, PlaceKind, SidebarMode},
    preferences::{Detail, SortField, ViewMode},
};
use kdl::{KdlDocument, KdlNode};
use std::{
    collections::BTreeSet,
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid Files interface settings",
    )
}
fn strings(node: &KdlNode) -> io::Result<Vec<&str>> {
    if node.ty().is_some() || node.children().is_some() {
        return Err(invalid());
    }
    node.entries()
        .iter()
        .map(|entry| {
            if entry.name().is_some() || entry.ty().is_some() {
                return Err(invalid());
            }
            entry.value().as_string().ok_or_else(invalid)
        })
        .collect()
}
pub(crate) fn parse(document: &KdlDocument) -> io::Result<InterfaceSettings> {
    let mut settings = InterfaceSettings::default();
    let mut seen = BTreeSet::new();
    for node in document.nodes() {
        let name = node.name().value();
        if name == "action" {
            continue;
        }
        if node.ty().is_some() || (name != "shortcut" && !seen.insert(name)) {
            return Err(invalid());
        }
        match name {
            "view" => {
                let mut props = BTreeSet::new();
                for entry in node.entries() {
                    let name = entry.name().ok_or_else(invalid)?.value();
                    if entry.ty().is_some() || !props.insert(name) {
                        return Err(invalid());
                    }
                    match name {
                        "mode" => {
                            settings.view.mode = match entry.value().as_string() {
                                Some("columns") => ViewMode::Columns,
                                Some("list") => ViewMode::List,
                                Some("icons") => ViewMode::Icons,
                                _ => return Err(invalid()),
                            }
                        }
                        "sort" => {
                            settings.view.sort = match entry.value().as_string() {
                                Some("name") => SortField::Name,
                                Some("size") => SortField::Size,
                                Some("modified") => SortField::Modified,
                                Some("kind") => SortField::Kind,
                                _ => return Err(invalid()),
                            }
                        }
                        "descending" => {
                            settings.view.descending =
                                entry.value().as_bool().ok_or_else(invalid)?
                        }
                        "column-width" => {
                            settings.view.column_width = entry
                                .value()
                                .as_integer()
                                .and_then(|n| n.try_into().ok())
                                .ok_or_else(invalid)?
                        }
                        _ => return Err(invalid()),
                    }
                }
                if let Some(children) = node.children() {
                    if children.nodes().len() != 1
                        || children.nodes()[0].name().value() != "details"
                    {
                        return Err(invalid());
                    }
                    settings.view.details = strings(&children.nodes()[0])?
                        .into_iter()
                        .map(|detail| match detail {
                            "size" => Ok(Detail::Size),
                            "modified" => Ok(Detail::Modified),
                            "kind" => Ok(Detail::Kind),
                            _ => Err(invalid()),
                        })
                        .collect::<io::Result<_>>()?;
                }
            }
            "toolbar" => settings.toolbar = strings(node)?.into_iter().map(str::to_owned).collect(),
            "sidebar" => {
                let values = strings(node)?;
                if values.len() != 1 {
                    return Err(invalid());
                }
                settings.sidebar = SidebarMode::from_id(values[0]).ok_or_else(invalid)?;
            }
            "places" => {
                if !node.entries().is_empty() {
                    return Err(invalid());
                }
                settings.places.clear();
                if let Some(children) = node.children() {
                    for place in children.nodes() {
                        if place.ty().is_some() || place.children().is_some() {
                            return Err(invalid());
                        }
                        match place.name().value() {
                            "place" => {
                                let values = strings(place)?;
                                if values.len() != 1 {
                                    return Err(invalid());
                                }
                                settings.places.push(Place::Builtin(
                                    PlaceKind::from_id(values[0]).ok_or_else(invalid)?,
                                ));
                            }
                            "bookmark" => {
                                let mut name = None;
                                let mut path = None;
                                for entry in place.entries() {
                                    if entry.ty().is_some() {
                                        return Err(invalid());
                                    }
                                    let value = entry.value().as_string().ok_or_else(invalid)?;
                                    match entry.name().map(|n| n.value()) {
                                        None if name.is_none() => name = Some(value.to_owned()),
                                        Some("path") if path.is_none() => {
                                            path = Some(PathBuf::from(value))
                                        }
                                        Some("path-bytes") if path.is_none() => {
                                            if value.len() > 32768 || value.len() % 2 != 0 {
                                                return Err(invalid());
                                            }
                                            let bytes = value
                                                .as_bytes()
                                                .chunks_exact(2)
                                                .map(|pair| {
                                                    let digit = |b: u8| {
                                                        (b as char)
                                                            .to_digit(16)
                                                            .map(|n| n as u8)
                                                            .ok_or_else(invalid)
                                                    };
                                                    Ok(digit(pair[0])? * 16 + digit(pair[1])?)
                                                })
                                                .collect::<io::Result<Vec<u8>>>()?;
                                            path = Some(PathBuf::from(
                                                std::ffi::OsString::from_vec(bytes),
                                            ));
                                        }
                                        _ => return Err(invalid()),
                                    }
                                }
                                settings.places.push(Place::Bookmark {
                                    name: name.ok_or_else(invalid)?,
                                    path: path.ok_or_else(invalid)?,
                                });
                            }
                            _ => return Err(invalid()),
                        }
                    }
                }
            }
            "shortcut" => {
                let values = strings(node)?;
                let command = values
                    .first()
                    .and_then(|id| Command::from_id(id))
                    .ok_or_else(invalid)?;
                if settings
                    .shortcuts
                    .insert(
                        command,
                        values[1..].iter().map(|s| (*s).to_owned()).collect(),
                    )
                    .is_some()
                {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
    }
    settings.view.validate()?;
    Ok(settings)
}

pub(crate) fn nodes(settings: &InterfaceSettings) -> Vec<KdlNode> {
    let mut view = KdlNode::new("view");
    view.push((
        "mode",
        match settings.view.mode {
            ViewMode::Columns => "columns",
            ViewMode::List => "list",
            ViewMode::Icons => "icons",
        },
    ));
    view.push((
        "sort",
        match settings.view.sort {
            SortField::Name => "name",
            SortField::Size => "size",
            SortField::Modified => "modified",
            SortField::Kind => "kind",
        },
    ));
    view.push(("descending", settings.view.descending));
    view.push(("column-width", i128::from(settings.view.column_width)));
    let mut details = KdlNode::new("details");
    for detail in &settings.view.details {
        details.push(match detail {
            Detail::Size => "size",
            Detail::Modified => "modified",
            Detail::Kind => "kind",
        });
    }
    let mut children = KdlDocument::new();
    children.nodes_mut().push(details);
    view.set_children(children);
    let mut toolbar = KdlNode::new("toolbar");
    for id in &settings.toolbar {
        toolbar.push(id.as_str());
    }
    let mut sidebar = KdlNode::new("sidebar");
    sidebar.push(settings.sidebar.id());
    let mut places = KdlNode::new("places");
    let mut children = KdlDocument::new();
    for place in &settings.places {
        let mut node = match place {
            Place::Builtin(place) => {
                let mut node = KdlNode::new("place");
                node.push(place.id());
                node
            }
            Place::Bookmark { name, path } => {
                let mut node = KdlNode::new("bookmark");
                node.push(name.as_str());
                if let Some(path) = path.to_str() {
                    node.push(("path", path));
                } else {
                    let hex = path
                        .as_os_str()
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();
                    node.push(("path-bytes", hex.as_str()));
                }
                node
            }
        };
        node.autoformat();
        children.nodes_mut().push(node);
    }
    places.set_children(children);
    let mut nodes = vec![view, toolbar, sidebar, places];
    for (command, bindings) in &settings.shortcuts {
        let mut node = KdlNode::new("shortcut");
        node.push(command.id());
        for binding in bindings {
            node.push(binding.as_str());
        }
        nodes.push(node);
    }
    nodes
}
