use super::App;
use crate::config::BarConfig;
use crate::modules::{ModuleId, Zone};

impl App {
    /// Build the ordered module layout for one bar after expanding any dynamic module instances.
    pub(super) fn effective_module_layout_for_bar(&self, bar: &BarConfig) -> Vec<(Zone, ModuleId)> {
        self.modules.expand_dynamic_module_layout(
            super::dynamic_modules::placements_for_bar_modules(&bar.modules),
        )
    }

    /// Build the ordered panel module layout for one bar after expanding dynamic placeholders.
    pub(super) fn effective_panel_module_layout_for_bar(
        &self,
        bar: &BarConfig,
    ) -> Vec<(Zone, ModuleId)> {
        self.modules.expand_dynamic_module_layout(
            super::dynamic_modules::placements_for_panel_modules(&bar.panel.modules),
        )
    }
}
