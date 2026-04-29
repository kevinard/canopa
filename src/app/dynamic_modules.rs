use super::App;
use crate::modules::ModuleId;

impl App {
    pub(super) fn clear_surface_module_refs(&mut self, ids: &[ModuleId]) {
        for surface in self.surfaces.values_mut() {
            if surface
                .focused_module
                .as_ref()
                .is_some_and(|id| ids.contains(id))
            {
                surface.focused_module = None;
            }
            if surface
                .hovered_module
                .as_ref()
                .is_some_and(|id| ids.contains(id))
            {
                surface.hovered_module = None;
            }
        }
    }
}
