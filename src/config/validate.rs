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
        if bar.expanded_width < bar.collapsed_width {
            return Err(validation_error(
                source.clone(),
                format!(
                    "bar {:?}: expanded-width: value {} must be >= collapsed-width ({}). \
                 Expected: a width greater than or equal to the collapsed width.",
                    bar.name, bar.expanded_width, bar.collapsed_width
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

    for bar in &config.bars {
        let mut seen_in_bar = std::collections::HashSet::new();
        for (zone, names) in bar_module_placements(bar) {
            for name in names {
                let resolved_name = if declared_modules.contains_key(name.as_str()) {
                    name.as_str()
                } else if let Some(descriptor) = module_descriptor_for_config_name(name) {
                    if descriptor.synthesized_params_factory.is_none() {
                        return Err(validation_error(
                            source.clone(),
                            format!(
                                "bar {:?}.modules.{zone:?}: module {:?} must be declared before it can be placed.",
                                bar.name, name
                            ),
                        ));
                    }
                    descriptor.config_name
                } else {
                    return Err(validation_error(
                        source.clone(),
                        format!(
                            "bar {:?}.modules.{zone:?}: unknown module {:?}. Expected a declared Module Name or a synthesizable built-in Module Kind.",
                            bar.name, name
                        ),
                    ));
                };

                if !seen_in_bar.insert(resolved_name) {
                    return Err(validation_error(
                        source.clone(),
                        format!(
                            "bar {:?}: module {:?} is placed more than once in this Bar.",
                            bar.name, resolved_name
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validation_error(source: ConfigSource, message: String) -> ConfigError {
    ConfigError::validation_message(source, message)
}
