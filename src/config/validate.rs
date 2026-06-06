use super::{
    Config, ConfigError, ConfigResult, ConfigSource, bar_module_placements,
    is_known_config_module_type, module_descriptor_for_config_name,
};

/// Validate a fully-parsed config for constraint violations.
///
/// Returns an error with a spec-formatted message if any constraint is violated.
#[cfg(test)]
pub(crate) fn validate(config: &Config) -> ConfigResult<()> {
    validate_with_source(
        config,
        ConfigSource::new(super::ConfigSourceName::EmbeddedDefault, ""),
    )
}

/// Validate a parsed config and attach diagnostics to the original source.
///
/// Validation currently works on the typed config, so diagnostics do not yet
/// have source spans. They still flow through `ConfigError` so callers render
/// them consistently with parse errors.
pub(crate) fn validate_with_source(config: &Config, source: ConfigSource) -> ConfigResult<()> {
    let mut seen_bar_names = std::collections::HashSet::new();
    for bar in &config.bars {
        if !seen_bar_names.insert(bar.name.as_str()) {
            return Err(validation_error(
                source.clone(),
                format!(
                    "bar {:?}: duplicate name. Each bar section must have a unique name.",
                    bar.name
                ),
            ));
        }
    }

    for bar in &config.bars {
        if bar.collapsed_width < 2 {
            return Err(validation_error(
                source.clone(),
                format!(
                    "bar {:?}: collapsed-width: value {} is too small. Expected: 2 or greater \
                 (minimum 1px content + 1px border).",
                    bar.name, bar.collapsed_width
                ),
            ));
        }
        if let (Some(width), Some(max_width)) = (bar.panel.width, bar.panel.max_width)
            && width > max_width
        {
            return Err(validation_error(
                source.clone(),
                format!(
                    "bar {:?}: panel.width: value {} exceeds max-width {}.",
                    bar.name, width, max_width
                ),
            ));
        }
        if let (Some(height), Some(max_height)) = (bar.panel.height, bar.panel.max_height)
            && height > max_height
        {
            return Err(validation_error(
                source.clone(),
                format!(
                    "bar {:?}: panel.height: value {} exceeds max-height {}.",
                    bar.name, height, max_height
                ),
            ));
        }
    }

    let mut declared_modules = std::collections::HashMap::new();
    for (i, mc) in config.modules.iter().enumerate() {
        if !is_known_config_module_type(&mc.module_type) {
            return Err(validation_error(
                source.clone(),
                format!(
                    "modules[{i}].type: unknown module type {:?}. \
                 Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray.",
                    mc.module_type
                ),
            ));
        }
        if let Some(previous) = declared_modules.insert(mc.name.as_str(), mc) {
            return Err(validation_error(
                source.clone(),
                format!(
                    "modules[{i}] {:?}: duplicate name. **Module Names** must be globally unique; {:?} was already declared as type {:?}.",
                    mc.name, previous.name, previous.module_type
                ),
            ));
        }
        if let Some(reserved) = module_descriptor_for_config_name(&mc.name)
            && reserved.config_name != mc.module_type
        {
            return Err(validation_error(
                source.clone(),
                format!(
                    "modules[{i}] {:?}: reserved built-in Module Name must keep Module Kind {:?}, got {:?}.",
                    mc.name, reserved.config_name, mc.module_type
                ),
            ));
        }
    }

    let mut declared_menus = std::collections::HashSet::new();
    for menu in &config.menus {
        if !declared_menus.insert(menu.name.as_str()) {
            return Err(validation_error(
                source.clone(),
                format!("menu {:?}: duplicate menu name.", menu.name),
            ));
        }
    }

    for bar in &config.bars {
        validate_module_layout(
            source.clone(),
            &format!("bar.{}.modules", bar.name),
            bar_module_placements(bar),
            &declared_modules,
        )?;
        validate_module_layout(
            source.clone(),
            &format!("bar.{}.panel", bar.name),
            [
                (
                    crate::modules::Zone::Start,
                    bar.panel.modules.start.as_slice(),
                ),
                (
                    crate::modules::Zone::Center,
                    bar.panel.modules.center.as_slice(),
                ),
                (crate::modules::Zone::End, bar.panel.modules.end.as_slice()),
            ],
            &declared_modules,
        )?;
        validate_actions_reference_declared_menus(
            source.clone(),
            &format!("bar.{}.actions", bar.name),
            &bar.actions,
            &declared_menus,
        )?;
    }
    for module in &config.modules {
        validate_actions_reference_declared_menus(
            source.clone(),
            &format!("module.{}.actions", module.name),
            &module.actions,
            &declared_menus,
        )?;
    }
    for menu in &config.menus {
        validate_menu_items_reference_declared_menus(
            source.clone(),
            &format!("menu.{}", menu.name),
            &menu.items,
            &declared_menus,
        )?;
    }
    Ok(())
}

fn validate_module_layout<'a>(
    source: ConfigSource,
    path: &str,
    placements: [(crate::modules::Zone, &'a [String]); 3],
    declared_modules: &std::collections::HashMap<&str, &'a super::ModuleConfig>,
) -> ConfigResult<()> {
    let mut seen_in_layout = std::collections::HashSet::new();
    for (zone, names) in placements {
        for name in names {
            let resolved_name = if declared_modules.contains_key(name.as_str()) {
                name.as_str()
            } else if let Some(descriptor) = module_descriptor_for_config_name(name) {
                if descriptor.synthesized_params_factory.is_none() {
                    return Err(validation_error(
                        source.clone(),
                        format!(
                            "{path}.{zone:?}: module {:?} must be declared before it can be placed.",
                            name
                        ),
                    ));
                }
                descriptor.config_name
            } else {
                return Err(validation_error(
                    source.clone(),
                    format!(
                        "{path}.{zone:?}: unknown module {:?}. Expected a declared Module Name or a synthesizable built-in Module Kind.",
                        name
                    ),
                ));
            };

            if !seen_in_layout.insert(resolved_name) {
                return Err(validation_error(
                    source.clone(),
                    format!(
                        "{path}: module {:?} is placed more than once in this layout.",
                        resolved_name
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_actions_reference_declared_menus(
    source: ConfigSource,
    path: &str,
    actions: &super::ActionsConfig,
    declared_menus: &std::collections::HashSet<&str>,
) -> ConfigResult<()> {
    for binding in &actions.bindings {
        validate_action_references_declared_menu(
            source.clone(),
            path,
            &binding.action,
            declared_menus,
        )?;
    }
    Ok(())
}

fn validate_menu_items_reference_declared_menus(
    source: ConfigSource,
    path: &str,
    items: &[super::MenuItemConfig],
    declared_menus: &std::collections::HashSet<&str>,
) -> ConfigResult<()> {
    for item in items {
        let item_path = format!("{path}.item({:?})", item.label);
        if let Some(action) = &item.action {
            validate_action_references_declared_menu(
                source.clone(),
                &item_path,
                action,
                declared_menus,
            )?;
        }
        validate_menu_items_reference_declared_menus(
            source.clone(),
            &item_path,
            &item.children,
            declared_menus,
        )?;
    }
    Ok(())
}

fn validate_action_references_declared_menu(
    source: ConfigSource,
    path: &str,
    action: &super::PanelAction,
    declared_menus: &std::collections::HashSet<&str>,
) -> ConfigResult<()> {
    let menu_name = match action {
        super::PanelAction::OpenMenu(name) | super::PanelAction::ToggleMenu(name) => name,
        _ => return Ok(()),
    };
    if !declared_menus.contains(menu_name.as_str()) {
        return Err(validation_error(
            source,
            format!("{path}: menu {:?} is not declared.", menu_name),
        ));
    }
    Ok(())
}

fn validation_error(source: ConfigSource, message: String) -> ConfigError {
    ConfigError::validation_message(source, message)
}
