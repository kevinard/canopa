use super::App;
use crate::config::{BarConfig, BarModules, PanelModules};
use crate::modules::{ModuleId, Zone};

impl App {
    /// Build the ordered module layout for one bar after expanding any dynamic module instances.
    pub(super) fn effective_module_layout_for_bar(&self, bar: &BarConfig) -> Vec<(Zone, ModuleId)> {
        self.effective_layout_for_module_groups(&bar.modules)
    }

    /// Build the ordered panel module layout for one bar after expanding dynamic placeholders.
    pub(super) fn effective_panel_module_layout_for_bar(
        &self,
        bar: &BarConfig,
    ) -> Vec<(Zone, ModuleId)> {
        self.effective_layout_for_module_groups(&bar.panel.modules)
    }

    fn effective_layout_for_module_groups(
        &self,
        groups: &impl ModuleGroups,
    ) -> Vec<(Zone, ModuleId)> {
        let mut layout = Vec::new();

        for (zone, names) in groups.placements() {
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
}

trait ModuleGroups {
    fn placements(&self) -> [(Zone, &[String]); 3];
}

impl ModuleGroups for BarModules {
    fn placements(&self) -> [(Zone, &[String]); 3] {
        [
            (Zone::Start, &self.start),
            (Zone::Center, &self.center),
            (Zone::End, &self.end),
        ]
    }
}

impl ModuleGroups for PanelModules {
    fn placements(&self) -> [(Zone, &[String]); 3] {
        [
            (Zone::Start, &self.start),
            (Zone::Center, &self.center),
            (Zone::End, &self.end),
        ]
    }
}
