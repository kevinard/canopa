pub mod layout;

/// Per-output render state. Tracks surface-specific data (expand/collapse, focus).
/// Shared module data lives in SharedState (established in Story 2.1), never here.
// Stub for Story 1.2 — fields and usage added in Stories 2.1, 3.1, 6.1
#[allow(dead_code)]
pub struct SurfaceState {
    pub output_name: String,
    // expand: ExpandState — added in Story 3.1
    // focused_module: Option<ModuleId> — added in Story 6.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_state_constructs() {
        let state = SurfaceState {
            output_name: String::from("test"),
        };
        assert_eq!(state.output_name, "test");
    }
}
