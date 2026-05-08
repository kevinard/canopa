use std::collections::HashMap;

use super::App;
use crate::config::BarConfig;
use crate::modules::{ModuleId, Zone};

impl App {
    /// Build the ordered module layout for one bar after expanding any dynamic module instances.
    pub(super) fn effective_module_layout_for_bar(&self, bar: &BarConfig) -> Vec<(Zone, ModuleId)> {
        let mut layout = Vec::new();

        for (zone, names) in [
            (Zone::Start, &bar.modules.start),
            (Zone::Center, &bar.modules.center),
            (Zone::End, &bar.modules.end),
        ] {
            for name in names {
                let expanded_ids: Vec<ModuleId> = self
                    .modules
                    .iter()
                    .filter(|(_, module)| module.config_name() == Some(name.as_str()))
                    .map(|(_, module)| module.id())
                    .collect();

                if expanded_ids.is_empty() {
                    if let Some((_, module)) = self
                        .modules
                        .iter()
                        .find(|(_, module)| module.id().0 == *name)
                    {
                        layout.push((zone.clone(), module.id()));
                    }
                } else {
                    layout.extend(expanded_ids.into_iter().map(|id| (zone.clone(), id)));
                }
            }
        }

        layout
    }

    pub(super) fn effective_module_zones_for_bar(
        &self,
        bar: &BarConfig,
    ) -> HashMap<ModuleId, Zone> {
        self.effective_module_layout_for_bar(bar)
            .into_iter()
            .map(|(zone, id)| (id, zone))
            .collect()
    }
}
