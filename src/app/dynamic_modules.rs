use super::App;

impl App {
    pub(super) fn clear_surface_module_refs_with_prefix(&mut self, prefix: &str) {
        for surface in self.surfaces.values_mut() {
            if surface
                .focused_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.focused_module = None;
            }
            if surface
                .hovered_module
                .as_ref()
                .map(|id| id.0.starts_with(prefix))
                .unwrap_or(false)
            {
                surface.hovered_module = None;
            }
        }
    }
}
