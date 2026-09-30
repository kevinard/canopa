pub trait DynamicModuleInstance: Send + Sync {
    fn update_workspace(&mut self, _workspace: &niri_ipc::Workspace) {}

    fn config_name(&self) -> &str;
}
