mod actions;
mod bars;
mod error;
mod fields;
mod launcher;
mod modules;
mod style;

use ::kdl::{KdlDocument, KdlNode};

use super::{
    BarConfig, CommandPolicy, Config, MenuConfig, MenuItemConfig, MenuSourceConfig, MenuSourceKind,
    ModuleConfig, PanelProviderConfig,
};
#[cfg(test)]
use crate::config::ConfigError;
#[cfg(test)]
pub(crate) use crate::config::ConfigSourceName;
use crate::config::{ConfigResult, ConfigSource};
use crate::modules::ModuleKind;
use error::ParseCtx;
use fields::field;

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct ConfigInput<'a> {
    pub(crate) name: ConfigSourceName,
    pub(crate) text: &'a str,
}

/// Parse a KDL string into a fully-typed `Config`.
///
/// Missing sections fall back to defaults; constraint validation happens in `validate()`.
#[cfg(test)]
pub(super) fn parse(input: &str) -> ConfigResult<Config> {
    parse_input(ConfigInput {
        name: ConfigSourceName::test("<test config>"),
        text: input,
    })
}

#[cfg(test)]
pub(crate) fn parse_input(input: ConfigInput<'_>) -> ConfigResult<Config> {
    let source = ConfigSource::new(input.name, input.text);
    let doc: KdlDocument = input
        .text
        .parse()
        .map_err(|e| ConfigError::from_kdl(source.clone(), e))?;
    parse_document(&doc, source)
}

pub(crate) fn parse_document(doc: &KdlDocument, source: ConfigSource) -> ConfigResult<Config> {
    let ctx = ParseCtx::root(source);
    ctx.validate_child_schema(
        doc,
        &[
            "bar",
            "command-policy",
            "modules",
            "menus",
            "panel-providers",
            "osd",
            "windows",
            "templates",
        ],
        &[
            "command-policy",
            "modules",
            "menus",
            "panel-providers",
            "osd",
            "windows",
            "templates",
        ],
    )?;
    let command_policy = parse_command_policy(doc, &ctx.child("command-policy"))?;
    let bars = bars::parse(doc, &ctx, &command_policy)?;
    let bars = if bars.is_empty() {
        vec![BarConfig::default()]
    } else {
        bars
    };
    let modules = parse_modules_section(doc, &ctx, &command_policy)?;
    let menus = parse_menus_section(doc, &ctx, &command_policy)?;
    let panel_providers = parse_panel_providers_section(doc, &ctx, &command_policy)?;
    let osd = parse_osd(doc, &ctx.child("osd"))?;
    let windows = parse_windows(doc, &ctx.child("windows"))?;
    let templates = parse_templates(doc, &ctx.child("templates"))?;
    Ok(Config {
        appearance: Default::default(),
        element_ui: parse_element_ui(doc, &ctx)?,
        bars,
        modules,
        menus,
        panel_providers,
        osd,
        windows,
        templates,
    })
}

fn parse_templates(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<Vec<super::TemplateConfig>> {
    let Some(children) = doc.get("templates").and_then(KdlNode::children) else {
        return Ok(Vec::new());
    };
    let mut templates: Vec<super::TemplateConfig> = Vec::new();
    for (index, node) in children.nodes().iter().enumerate() {
        let ctx = ctx.index(index);
        if node.name().value() != "template" {
            return Err(ctx.node_err(node, "expected `template \"NAME\" { app ... }`", None));
        }
        let name = node
            .get(0)
            .and_then(|entry| entry.as_string())
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| ctx.node_err(node, "a template needs a name", None))?
            .to_owned();
        if templates.iter().any(|other| other.name == name) {
            return Err(ctx.node_err(node, format!("duplicate template {name:?}"), None));
        }
        let mut apps = Vec::new();
        for app in node.children().map(|apps| apps.nodes()).unwrap_or_default() {
            if app.name().value() != "app" {
                return Err(ctx.node_err(
                    app,
                    "expected `app \"org.example.App.desktop\" width=50`",
                    None,
                ));
            }
            let Some(id) = app.get(0).and_then(|entry| entry.as_string()) else {
                return Err(ctx.node_err(app, "an app needs its desktop ID", None));
            };
            let desktop_id = if id.ends_with(".desktop") {
                id.to_owned()
            } else {
                format!("{id}.desktop")
            };
            let width_percent = match app.get("width") {
                None => None,
                Some(value) => match value
                    .as_float()
                    .or_else(|| value.as_integer().map(|v| v as f64))
                {
                    Some(width) if (5.0..=100.0).contains(&width) => Some(width),
                    _ => {
                        return Err(ctx.node_err(
                            app,
                            "width is a percentage of the screen, 5-100",
                            None,
                        ));
                    }
                },
            };
            apps.push(super::TemplateApp {
                desktop_id,
                width_percent,
            });
        }
        if apps.is_empty() {
            return Err(ctx.node_err(node, format!("template {name:?} has no apps"), None));
        }
        templates.push(super::TemplateConfig { name, apps });
    }
    Ok(templates)
}

fn parse_windows(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<super::WindowsConfig> {
    let Some(children) = doc.get("windows").and_then(KdlNode::children) else {
        return Ok(super::WindowsConfig::default());
    };
    ctx.validate_child_schema(children, &["restore"], &["restore"])?;
    Ok(super::WindowsConfig {
        restore: field(children, ctx, "restore")
            .optional()
            .help("use `restore #true` or `restore #false`")
            .bool()
            .unwrap_or(false)?,
    })
}

fn parse_osd(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<super::OsdConfig> {
    let defaults = super::OsdConfig::default();
    let Some(children) = doc.get("osd").and_then(KdlNode::children) else {
        return Ok(defaults);
    };
    let fields = ["enabled", "volume-step", "brightness-step", "timeout-ms"];
    ctx.validate_child_schema(children, &fields, &fields)?;
    let timeout_ms = field(children, ctx, "timeout-ms")
        .optional()
        .help("use `timeout-ms 1500` for one and a half seconds")
        .u64_min(500)
        .unwrap_or(defaults.timeout_ms)?;
    if timeout_ms > 10_000 {
        let node = children.get("timeout-ms").expect("parsed above");
        return Err(ctx.node_err(
            node,
            format!("value {timeout_ms} out of range. Expected: integer 500-10000"),
            None,
        ));
    }
    Ok(super::OsdConfig {
        enabled: field(children, ctx, "enabled")
            .optional()
            .help("use `enabled #true` or `enabled #false`")
            .bool()
            .unwrap_or(defaults.enabled)?,
        volume_step: field(children, ctx, "volume-step")
            .optional()
            .u8_range(1, 100)
            .unwrap_or(defaults.volume_step)?,
        brightness_step: field(children, ctx, "brightness-step")
            .optional()
            .u8_range(1, 50)
            .unwrap_or(defaults.brightness_step)?,
        timeout_ms,
    })
}

fn parse_ui(
    doc: &KdlDocument,
    ctx: &ParseCtx,
    kind: canopa_theme::UiKind,
) -> ConfigResult<canopa_theme::UiStyle> {
    let nodes = doc
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "ui")
        .collect::<Vec<_>>();
    if nodes.len() > 1 {
        return Err(ctx.node_err(nodes[1], "duplicate ui block", None));
    }
    match nodes.first() {
        None => Ok(Default::default()),
        Some(node) => {
            canopa_config::ui_style(node, kind).map_err(|e| ctx.node_err(node, e.to_string(), None))
        }
    }
}
fn parse_element_ui(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<super::ElementUi> {
    let mut result = super::ElementUi::default();
    for (section, kind, map) in [
        ("modules", canopa_theme::UiKind::Module, &mut result.modules),
        ("menus", canopa_theme::UiKind::Menu, &mut result.menus),
    ] {
        if let Some(children) = doc.get(section).and_then(|n| n.children()) {
            for node in children.nodes() {
                if let (Some(name), Some(children)) =
                    (node.get(0).and_then(|e| e.as_string()), node.children())
                {
                    let ui = parse_ui(children, &ctx.child(section), kind)?;
                    if ui != Default::default() {
                        map.insert(name.to_owned(), ui);
                    }
                }
            }
        }
    }
    Ok(result)
}

fn parse_command_policy(doc: &KdlDocument, ctx: &ParseCtx) -> ConfigResult<CommandPolicy> {
    let Some(node) = doc.get("command-policy") else {
        return Ok(CommandPolicy::default());
    };
    let Some(children) = node.children() else {
        return Ok(CommandPolicy::default());
    };

    ctx.validate_child_schema(
        children,
        &["allow-shell", "action-timeout-ms"],
        &["allow-shell", "action-timeout-ms"],
    )?;
    let defaults = CommandPolicy::default();
    let allow_shell = field(children, ctx, "allow-shell")
        .optional()
        .help("use `allow-shell #true` or `allow-shell #false`")
        .bool()
        .unwrap_or(defaults.allow_shell)?;
    let action_timeout_ms = field(children, ctx, "action-timeout-ms")
        .optional()
        .help("use `action-timeout-ms 3000` for a three-second timeout")
        .u64_min(1)
        .unwrap_or(defaults.action_timeout_ms)?;

    Ok(CommandPolicy {
        allow_shell,
        action_timeout_ms,
    })
}

fn parse_modules_section(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<ModuleConfig>> {
    let Some(modules_node) = doc.get("modules") else {
        return Ok(vec![]);
    };
    let Some(children) = modules_node.children() else {
        return Ok(vec![]);
    };

    let mut result = Vec::new();
    for (i, node) in children.nodes().iter().enumerate() {
        let node_name = node.name().value();
        let module_index_ctx = root_ctx.child("modules").index(i);
        if node_name != "module" {
            return Err(module_index_ctx.node_err(
                node,
                format!("expected node name \"module\", got {node_name:?}"),
                None,
            ));
        }
        let name = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .ok_or_else(|| {
                module_index_ctx.node_err(
                    node,
                    "missing module name",
                    Some("expected a string argument, e.g. `module \"clock\"`".into()),
                )
            })?
            .to_string();

        let explicit_type = match node.children() {
            Some(module_children) => field(module_children, &module_index_ctx, "type")
                .optional()
                .string()
                .transpose()?,
            None => None,
        };
        let module_type = if super::is_known_config_module_type(&name) {
            if let Some(explicit_type) = explicit_type.as_deref()
                && explicit_type != name.as_str()
            {
                return Err(module_index_ctx.node_err(
                    node,
                    format!(
                        "reserved built-in Module Name must keep Module Kind {name:?}, got {explicit_type:?}"
                    ),
                    None,
                ));
            }
            name.clone()
        } else {
            explicit_type
                .ok_or_else(|| {
                    module_index_ctx.node_err(
                        node,
                        format!("module name {name:?} is not a known module type"),
                        Some(format!(
                            "add a `type \"<module type>\"` child; expected one of: {}",
                            crate::modules::config_module_type_list()
                        )),
                    )
                })?
                .to_string()
        };

        let kind = ModuleKind::from_config_name(&module_type).ok_or_else(|| {
            module_index_ctx.node_err(
                node,
                format!("unknown module type {module_type:?}"),
                Some(format!(
                    "expected one of: {}",
                    crate::modules::config_module_type_list()
                )),
            )
        })?;
        let module_ctx = module_index_ctx.child(&module_type);
        let actions = actions::parse(node, i, &module_type, &module_ctx, command_policy)?;
        let mut content_node = node.clone();
        if let Some(children) = content_node.children_mut() {
            children.nodes_mut().retain(|n| n.name().value() != "ui");
        }
        let params = modules::dispatch(
            kind,
            &content_node,
            i,
            &actions,
            &module_ctx,
            command_policy,
        )?;
        result.push(ModuleConfig {
            name,
            module_type,
            params,
            actions,
        });
    }
    Ok(result)
}

fn parse_menus_section(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<MenuConfig>> {
    let Some(menus_node) = doc.get("menus") else {
        return Ok(vec![]);
    };
    let Some(children) = menus_node.children() else {
        return Ok(vec![]);
    };

    let mut menus = Vec::new();
    for (menu_idx, node) in children.nodes().iter().enumerate() {
        let menu_ctx = root_ctx.child("menus").index(menu_idx);
        if node.name().value() != "menu" {
            return Err(menu_ctx.node_err(
                node,
                format!("expected node name \"menu\", got {:?}", node.name().value()),
                None,
            ));
        }

        let name = required_menu_name(node, &menu_ctx)?;
        let show_icons = parse_menu_show_icons(node, &menu_ctx)?;
        let prompt = parse_menu_prompt(node, &menu_ctx)?;
        let title = fields::property_text_or_message(node, &menu_ctx, "title", "title-id")?
            .filter(|title| !title.is_empty());
        let mut source = None;
        let mut item_nodes = Vec::new();
        if let Some(menu_children) = node.children() {
            for child in menu_children.nodes() {
                match child.name().value() {
                    "ui" => {}
                    "source" => {
                        if source.is_some() {
                            return Err(menu_ctx.node_err(
                                node,
                                "menu can declare only one source",
                                None,
                            ));
                        }
                        source = Some(parse_menu_source(child, &menu_ctx.child("source"))?);
                    }
                    "item" => item_nodes.push(child),
                    other => {
                        return Err(menu_ctx.node_err(
                            child,
                            format!("expected node name \"item\" or \"source\", got {other:?}"),
                            None,
                        ));
                    }
                }
            }
        }

        if source.is_some() && !item_nodes.is_empty() {
            return Err(menu_ctx.node_err(node, "menu cannot mix source and item nodes", None));
        }

        let items = parse_menu_items_from_nodes(item_nodes, &menu_ctx, command_policy)?;
        menus.push(MenuConfig {
            name,
            title,
            show_icons,
            prompt,
            source,
            items,
        });
    }
    Ok(menus)
}

fn parse_panel_providers_section(
    doc: &KdlDocument,
    root_ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<PanelProviderConfig>> {
    let Some(providers_node) = doc.get("panel-providers") else {
        return Ok(Vec::new());
    };
    let Some(children) = providers_node.children() else {
        return Ok(Vec::new());
    };

    let mut providers = Vec::new();
    for (provider_idx, node) in children.nodes().iter().enumerate() {
        let provider_ctx = root_ctx.child("panel-providers").index(provider_idx);
        if node.name().value() != "provider" {
            return Err(provider_ctx.node_err(
                node,
                format!(
                    "expected node name \"provider\", got {:?}",
                    node.name().value()
                ),
                None,
            ));
        }

        let name = required_string_argument(node, &provider_ctx, "panel provider name")?;
        let provider_children = node.children().ok_or_else(|| {
            provider_ctx.node_err(
                node,
                "missing provider command",
                Some("expected `command \"...\"` or `command { argv \"program\" }`".into()),
            )
        })?;
        provider_ctx.validate_child_schema(
            provider_children,
            &["command", "protocol"],
            &["command", "protocol"],
        )?;
        let transport = field(provider_children, &provider_ctx, "protocol")
            .optional()
            .string()
            .transpose()?;
        let protocol = match transport.as_deref() {
            None | Some("niribar-v1") => crate::external_panel::ProviderProtocol::NiribarV1,
            Some("canopa-v4") => crate::external_panel::ProviderProtocol::CanopaV4,
            Some("canopa-v5") => crate::external_panel::ProviderProtocol::CanopaV5,
            Some(_) => {
                return Err(provider_ctx.node_err(
                    node,
                    "unsupported provider protocol",
                    Some("expected niribar-v1, canopa-v4 or canopa-v5".into()),
                ));
            }
        };
        let command_node = field(provider_children, &provider_ctx, "command").required_node(
            "missing provider command",
            Some("expected `command \"...\"` or `command { argv \"program\" }`".into()),
        )?;
        let command = actions::parse_command_node_ctx(
            command_node,
            &provider_ctx.child("command"),
            command_policy,
        )?;

        providers.push(PanelProviderConfig {
            name,
            command,
            protocol,
        });
    }

    Ok(providers)
}

fn parse_menu_prompt(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Option<String>> {
    let prompt = fields::property_text_or_message(node, ctx, "prompt", "prompt-id")?;
    Ok(prompt.filter(|value| !value.is_empty()))
}

fn parse_menu_source(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<MenuSourceConfig> {
    if node
        .entries()
        .first()
        .and_then(|entry| entry.value().as_string())
        == Some("launcher")
    {
        return launcher::parse(node, ctx).map(|config| MenuSourceConfig {
            kind: MenuSourceKind::Launcher(config),
        });
    }
    if node.children().is_some() || node.entries().iter().any(|entry| entry.name().is_some()) {
        return Err(ctx.node_err(node, "source options are not supported yet", None));
    }
    if node.entries().len() != 1 {
        return Err(ctx.node_err(node, "expected one menu source string", None));
    }
    let Some(kind) = node.entries()[0].value().as_string() else {
        return Err(ctx.entry_err(&node.entries()[0], "expected menu source string", None));
    };
    match kind {
        "installed-apps" => Ok(MenuSourceConfig {
            kind: MenuSourceKind::InstalledApps,
        }),
        other => Err(ctx.entry_err(
            &node.entries()[0],
            format!("unknown menu source {other:?}"),
            Some("expected `installed-apps` or `launcher`".into()),
        )),
    }
}

fn parse_menu_items_from_nodes(
    nodes: Vec<&KdlNode>,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<Vec<MenuItemConfig>> {
    let mut items = Vec::new();
    for (item_idx, node) in nodes.into_iter().enumerate() {
        let item_ctx = ctx.child("item").index(item_idx);
        if node.name().value() != "item" {
            return Err(item_ctx.node_err(
                node,
                format!("expected node name \"item\", got {:?}", node.name().value()),
                None,
            ));
        }
        items.push(parse_menu_item(node, &item_ctx, command_policy)?);
    }
    Ok(items)
}

fn parse_menu_item(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<MenuItemConfig> {
    let label = match fields::property_text_or_message(node, ctx, "label", "label-id")? {
        Some(_) if node.entries().iter().any(|entry| entry.name().is_none()) => {
            return Err(ctx.node_err(
                node,
                "an item has either a label string or `label-id`",
                Some("keep the label string for your own text".into()),
            ));
        }
        Some(label) => label,
        None => required_string_argument(node, ctx, "item label")?,
    };
    let Some(children) = node.children() else {
        return Ok(MenuItemConfig {
            label,
            icon: None,
            keywords: Vec::new(),
            action: None,
            children: Vec::new(),
        });
    };

    let mut icon = None;
    let mut keywords = Vec::new();
    let mut action = None;
    let mut item_nodes = Vec::new();
    for child in children.nodes() {
        match child.name().value() {
            "icon" => {
                if icon.is_some() {
                    return Err(ctx.node_err(child, "menu item can declare only one icon", None));
                }
                let icon_name = required_string_argument(child, &ctx.child("icon"), "icon name")?;
                if !crate::icon_theme::is_valid_icon_name(&icon_name) {
                    return Err(ctx.node_err(
                        child,
                        "icon name must be an XDG icon name, not a path",
                        None,
                    ));
                }
                icon = Some(icon_name);
            }
            "keywords" => keywords = parse_keywords(child, &ctx.child("keywords"))?,
            "item" => item_nodes.push(child),
            operation => {
                if action.is_some() {
                    return Err(ctx.node_err(node, "expected exactly one action operation", None));
                }
                action = Some(parse_menu_action_operation(
                    child,
                    &ctx.child(operation),
                    command_policy,
                )?);
            }
        }
    }

    if action.is_some() && !item_nodes.is_empty() {
        return Err(ctx.node_err(
            node,
            "menu item cannot mix an action operation and child item nodes",
            None,
        ));
    }

    let mut children_items = Vec::new();
    for (idx, child) in item_nodes.into_iter().enumerate() {
        children_items.push(parse_menu_item(
            child,
            &ctx.child("item").index(idx),
            command_policy,
        )?);
    }

    Ok(MenuItemConfig {
        label,
        icon,
        keywords,
        action,
        children: children_items,
    })
}

fn parse_menu_action_operation(
    node: &KdlNode,
    ctx: &ParseCtx,
    command_policy: &CommandPolicy,
) -> ConfigResult<super::PanelAction> {
    match node.name().value() {
        "command" => actions::parse_command_node_ctx(node, ctx, command_policy)
            .map(super::PanelAction::Command),
        "launch" => actions::parse_launch(node, ctx).map(super::PanelAction::Launch),
        "session" => actions::parse_session(node, ctx).map(super::PanelAction::Session),
        "open-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::OpenPanel)
        }
        "toggle-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::TogglePanel)
        }
        "open-bar-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::OpenBarPanel)
        }
        "toggle-bar-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::ToggleBarPanel)
        }
        "close-panel" => {
            require_no_arguments(node, ctx)?;
            Ok(super::PanelAction::ClosePanel)
        }
        "open-menu" => required_menu_name(node, ctx).map(super::PanelAction::OpenMenu),
        "toggle-menu" => required_menu_name(node, ctx).map(super::PanelAction::ToggleMenu),
        "open-provider" => required_provider_name(node, ctx).map(super::PanelAction::OpenProvider),
        other => Err(ctx.node_err(
            node,
            format!("unknown menu item operation {other:?}"),
            Some(
                "expected `command`, `launch`, `session`, `open-panel`, `toggle-panel`, `open-bar-panel`, `toggle-bar-panel`, `close-panel`, `open-menu`, `toggle-menu`, or `open-provider`"
                    .into(),
            ),
        )),
    }
}

fn require_no_arguments(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<()> {
    if !node.entries().is_empty() || node.children().is_some() {
        return Err(ctx.node_err(node, "expected no arguments", None));
    }
    Ok(())
}

fn required_menu_name(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let value = required_string_argument(node, ctx, "menu name")?;
    Ok(value)
}

fn required_provider_name(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let value = required_string_argument(node, ctx, "panel provider name")?;
    Ok(value)
}

fn parse_menu_show_icons(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<bool> {
    let mut show_icons = false;
    for entry in node.entries().iter().filter(|entry| entry.name().is_some()) {
        match entry.name().map(|name| name.value()) {
            Some("show-icons") => {
                show_icons = entry
                    .value()
                    .as_bool()
                    .ok_or_else(|| ctx.entry_err(entry, "expected #true or #false", None))?;
            }
            Some("prompt" | "prompt-id" | "title" | "title-id") => {}
            Some(name) => {
                return Err(ctx.entry_err(entry, format!("unknown menu property {name:?}"), None));
            }
            None => {}
        }
    }
    Ok(show_icons)
}

fn parse_keywords(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<Vec<String>> {
    let mut values = Vec::new();
    for entry in node.entries() {
        let Some(value) = entry.value().as_string() else {
            return Err(ctx.entry_err(entry, "expected string keyword", None));
        };
        values.push(value.to_string());
    }
    Ok(values)
}

fn required_string_argument(node: &KdlNode, ctx: &ParseCtx, label: &str) -> ConfigResult<String> {
    let positional = node
        .entries()
        .iter()
        .filter(|entry| entry.name().is_none())
        .collect::<Vec<_>>();
    if positional.len() != 1 {
        return Err(ctx.node_err(node, format!("expected one {label} string"), None));
    }
    match positional[0].value().as_string() {
        Some("") => Err(ctx.entry_err(
            &node.entries()[0],
            format!("{label} must not be empty"),
            None,
        )),
        Some(value) => Ok(value.to_string()),
        None => Err(ctx.entry_err(&node.entries()[0], "expected string", None)),
    }
}

#[cfg(test)]
mod tests;
