use super::error::ParseCtx;
use crate::{
    config::ConfigResult,
    launcher::{Config, Extension},
};
use kdl::KdlNode;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

fn strings(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Vec<String>> {
    if node.children().is_some() || node.entries().iter().any(|entry| entry.name().is_some()) {
        return Err(ctx.node_err(node, "expected only literal string arguments", None));
    }
    node.entries()
        .iter()
        .map(|entry| {
            entry
                .value()
                .as_string()
                .filter(|value| !value.contains('\0'))
                .map(String::from)
                .ok_or_else(|| ctx.entry_err(entry, "expected a string without NUL bytes", None))
        })
        .collect()
}

pub(super) fn parse(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Config> {
    if node.entries().len() != 1 || node.entries()[0].name().is_some() {
        return Err(ctx.node_err(
            node,
            "expected source \"launcher\" with optional child settings",
            None,
        ));
    }
    let mut config = Config::default();
    let Some(children) = node.children() else {
        return Ok(config);
    };
    ctx.validate_child_schema(
        children,
        &["root", "exclude", "max-paths", "extension", "history"],
        &["max-paths", "history"],
    )?;
    let mut roots = Vec::new();
    let mut names = HashSet::new();
    let mut keywords = HashSet::from(["apps".to_string(), "files".to_string(), "=".to_string()]);
    for child in children.nodes() {
        match child.name().value() {
            "history" => {
                if child.children().is_some()
                    || child.entries().len() != 1
                    || child.entries()[0].name().is_some()
                {
                    return Err(ctx.node_err(child, "history requires one boolean", None));
                }
                config.history = child.entries()[0]
                    .value()
                    .as_bool()
                    .ok_or_else(|| ctx.node_err(child, "history requires one boolean", None))?;
            }
            "root" => {
                let values = strings(child, ctx)?;
                if values.len() != 1 || (values[0] != "~" && !Path::new(&values[0]).is_absolute()) {
                    return Err(ctx.node_err(
                        child,
                        "root requires one absolute directory or ~",
                        None,
                    ));
                }
                roots.push(PathBuf::from(&values[0]));
            }
            "exclude" => {
                let values = strings(child, ctx)?;
                if values.is_empty()
                    || values
                        .iter()
                        .any(|value| value.is_empty() || value.contains('/'))
                {
                    return Err(ctx.node_err(
                        child,
                        "exclude requires directory or file basenames",
                        None,
                    ));
                }
                config.excludes.extend(values);
            }
            "max-paths" => {
                if child.entries().len() != 1
                    || child.entries()[0].name().is_some()
                    || child.children().is_some()
                {
                    return Err(ctx.node_err(child, "max-paths requires one integer", None));
                }
                let Some(value) = child.entries()[0]
                    .value()
                    .as_integer()
                    .filter(|value| (1..=1_000_000).contains(value))
                else {
                    return Err(ctx.node_err(
                        child,
                        "max-paths must be between 1 and 1000000",
                        None,
                    ));
                };
                config.max_paths = value as usize;
            }
            "extension" => {
                let positional: Vec<_> = child
                    .entries()
                    .iter()
                    .filter(|entry| entry.name().is_none())
                    .collect();
                let Some(name) = positional
                    .first()
                    .and_then(|entry| entry.value().as_string())
                    .filter(|value| {
                        !value.trim().is_empty() && value.len() <= 128 && !value.contains('\0')
                    })
                else {
                    return Err(ctx.node_err(child, "extension requires one nonempty name", None));
                };
                if positional.len() != 1 || !names.insert(name.to_string()) {
                    return Err(ctx.node_err(child, "extension names must be unique", None));
                }
                let mut keyword = name.to_string();
                let mut global = false;
                for entry in child
                    .entries()
                    .iter()
                    .filter(|entry| entry.name().is_some())
                {
                    match entry.name().unwrap().value() {
                        "keyword" => {
                            keyword = entry
                                .value()
                                .as_string()
                                .ok_or_else(|| {
                                    ctx.entry_err(entry, "keyword must be a string", None)
                                })?
                                .into()
                        }
                        "global" => {
                            global = entry.value().as_bool().ok_or_else(|| {
                                ctx.entry_err(entry, "global must be a boolean", None)
                            })?
                        }
                        _ => return Err(ctx.entry_err(entry, "unknown extension property", None)),
                    }
                }
                if keyword.is_empty()
                    || keyword.starts_with('=')
                    || keyword.len() > 64
                    || keyword
                        .chars()
                        .any(|ch| ch.is_whitespace() || ch.is_control())
                    || !keywords.insert(keyword.clone())
                {
                    return Err(ctx.node_err(child, "extension keywords must be unique single words other than apps, files, or =", None));
                }
                let Some(body) = child.children() else {
                    return Err(ctx.node_err(child, "extension requires an argv child", None));
                };
                if body.nodes().len() != 1 || body.nodes()[0].name().value() != "argv" {
                    return Err(ctx.node_err(
                        child,
                        "extension requires exactly one argv child",
                        None,
                    ));
                }
                let argv = strings(&body.nodes()[0], ctx)?;
                if argv.is_empty()
                    || argv[0].trim().is_empty()
                    || argv.len() > 64
                    || argv.iter().any(|arg| arg.len() > 4096)
                {
                    return Err(ctx.node_err(
                        child,
                        "extension argv requires a program and at most 64 bounded arguments",
                        None,
                    ));
                }
                config.extensions.push(Extension {
                    id: format!("custom:{name}"),
                    name: name.into(),
                    keyword,
                    global,
                    argv,
                });
            }
            _ => unreachable!(),
        }
    }
    if config.extensions.len() > 8 {
        return Err(ctx.node_err(
            node,
            "at most eight launcher extensions are supported",
            None,
        ));
    }
    if !roots.is_empty() {
        config.roots = roots;
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    #[test]
    fn launcher_settings_and_extension_references_are_validated() {
        let parse = |source: &str| {
            crate::config::parse::parse(&format!("menus {{ menu \"Search\" {{ {source}; }}; }}"))
        };
        let good = parse("source \"launcher\" { root \"/tmp\"; exclude \"cache\"; max-paths 50; history #false; extension \"Projects\" keyword=\"p\" { argv \"python3\" \"/tmp/projects.py\"; }; }").unwrap();
        let crate::config::MenuSourceKind::Launcher(config) =
            &good.menus[0].source.as_ref().unwrap().kind
        else {
            panic!()
        };
        assert_eq!(config.max_paths, 50);
        assert!(!config.history);
        assert_eq!(config.extensions[0].keyword, "p");
        for source in [
            "source \"launcher\" { history \"false\"; }",
            "source \"launcher\" { history #true; history #false; }",
            "source \"launcher\" { root \"relative\"; }",
            "source \"launcher\" { max-paths 0; }",
            "source \"launcher\" { extension \"x\" keyword=\"files\" { argv \"x\"; }; }",
            "source \"launcher\" { extension \"x\" { argv \"\"; }; }",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}
